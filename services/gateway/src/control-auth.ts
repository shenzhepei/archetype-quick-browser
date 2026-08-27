import { createHash, randomBytes } from 'node:crypto'
import { createRemoteJWKSet, jwtVerify } from 'jose'
import type { FastifyReply, FastifyRequest } from 'fastify'
import type { ControlRole, ControlSessionRecord, PlatformStore } from './store.js'

export type ControlPermission = 'organization:read' | 'member:write' | 'project:create' | 'project:read' | 'project:configure' | 'deployment:write' | 'audit:read'

export const controlPermissions: Record<ControlRole, ReadonlySet<ControlPermission>> = {
  owner: new Set(['organization:read', 'member:write', 'project:create', 'project:read', 'project:configure', 'deployment:write', 'audit:read']),
  admin: new Set(['organization:read', 'member:write', 'project:create', 'project:read', 'project:configure', 'deployment:write', 'audit:read']),
  developer: new Set(['organization:read', 'project:read', 'project:configure', 'deployment:write', 'audit:read']),
  operator: new Set(['organization:read', 'project:read', 'project:configure', 'audit:read']),
  auditor: new Set(['organization:read', 'project:read', 'audit:read'])
}

interface ControlOidcConfig {
  issuer: string
  clientId: string
  clientSecret?: string
}

function parseCookies(header: string | undefined): Record<string, string> {
  return Object.fromEntries((header ?? '').split(';').map((part) => part.trim()).filter(Boolean).map((part) => {
    const separator = part.indexOf('=')
    return separator < 0 ? [part, ''] : [part.slice(0, separator), decodeURIComponent(part.slice(separator + 1))]
  }))
}

function safeReturnTo(value: string | undefined): string {
  return value?.startsWith('/console') && !value.startsWith('//') ? value : '/console/'
}

export class ControlAuthService {
  constructor(
    private readonly store: PlatformStore,
    private readonly publicUrl: string,
    private readonly oidc: ControlOidcConfig | null,
    private readonly developmentLogin: boolean
  ) {}

  async login(returnTo?: string): Promise<string> {
    const state = randomBytes(24).toString('base64url')
    const verifier = randomBytes(32).toString('base64url')
    const nonce = randomBytes(24).toString('base64url')
    await this.store.createControlTransaction(state, verifier, nonce, safeReturnTo(returnTo))
    if (!this.oidc) {
      if (!this.developmentLogin) throw Object.assign(new Error('Administrator OIDC is not configured.'), { statusCode: 503 })
      return `${this.publicUrl}/v1/control/auth/dev?state=${encodeURIComponent(state)}`
    }
    const discovery = await this.discovery(this.oidc.issuer)
    const challenge = createHash('sha256').update(verifier).digest('base64url')
    const url = new URL(discovery.authorization_endpoint)
    url.search = new URLSearchParams({
      response_type: 'code',
      client_id: this.oidc.clientId,
      redirect_uri: `${this.publicUrl}/v1/control/auth/callback`,
      scope: 'openid profile email',
      state,
      nonce,
      code_challenge: challenge,
      code_challenge_method: 'S256'
    }).toString()
    return url.toString()
  }

  async completeDevelopment(state: string): Promise<{ session: ControlSessionRecord; returnTo: string }> {
    if (!this.developmentLogin) throw Object.assign(new Error('Development administrator login is disabled.'), { statusCode: 404 })
    const transaction = await this.store.consumeControlTransaction(state)
    if (!transaction) throw Object.assign(new Error('Administrator login transaction was not found or expired.'), { statusCode: 400 })
    const session = await this.store.createControlSession('development-admin', 'Development Admin', { sub: 'development-admin', name: 'Development Admin' })
    return { session, returnTo: transaction.returnTo }
  }

  async completeOidc(state: string, code: string): Promise<{ session: ControlSessionRecord; returnTo: string }> {
    if (!this.oidc) throw Object.assign(new Error('Administrator OIDC is not configured.'), { statusCode: 503 })
    const transaction = await this.store.consumeControlTransaction(state)
    if (!transaction) throw Object.assign(new Error('Administrator login transaction was not found or expired.'), { statusCode: 400 })
    const discovery = await this.discovery(this.oidc.issuer)
    const body = new URLSearchParams({
      grant_type: 'authorization_code',
      code,
      redirect_uri: `${this.publicUrl}/v1/control/auth/callback`,
      client_id: this.oidc.clientId,
      code_verifier: transaction.verifier
    })
    if (this.oidc.clientSecret) body.set('client_secret', this.oidc.clientSecret)
    const response = await fetch(discovery.token_endpoint, { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded' }, body })
    if (!response.ok) throw Object.assign(new Error('OIDC token exchange failed.'), { statusCode: 401 })
    const token = await response.json() as { id_token?: string }
    if (!token.id_token) throw Object.assign(new Error('OIDC provider did not return an ID token.'), { statusCode: 401 })
    const verified = await jwtVerify(token.id_token, createRemoteJWKSet(new URL(discovery.jwks_uri)), { issuer: this.oidc.issuer, audience: this.oidc.clientId })
    if (verified.payload.nonce !== transaction.nonce) throw Object.assign(new Error('OIDC ID token nonce does not match the login transaction.'), { statusCode: 401 })
    const subject = String(verified.payload.sub)
    const displayName = typeof verified.payload.name === 'string' ? verified.payload.name : typeof verified.payload.email === 'string' ? verified.payload.email : undefined
    const session = await this.store.createControlSession(subject, displayName, verified.payload as Record<string, unknown>)
    return { session, returnTo: transaction.returnTo }
  }

  setSessionCookie(reply: FastifyReply, token: string): void {
    const secure = this.publicUrl.startsWith('https://') ? '; Secure' : ''
    reply.header('set-cookie', `${this.cookieName()}=${encodeURIComponent(token)}; Path=/; HttpOnly; SameSite=Lax; Max-Age=28800${secure}`)
  }

  clearSessionCookie(reply: FastifyReply): void {
    const secure = this.publicUrl.startsWith('https://') ? '; Secure' : ''
    reply.header('set-cookie', `${this.cookieName()}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0${secure}`)
  }

  token(request: FastifyRequest): string | undefined {
    return parseCookies(request.headers.cookie)[this.cookieName()]
  }

  async session(request: FastifyRequest): Promise<ControlSessionRecord | null> {
    const token = this.token(request)
    return token ? this.store.controlSession(token) : null
  }

  async requireSession(request: FastifyRequest): Promise<ControlSessionRecord> {
    const session = await this.session(request)
    if (!session) throw Object.assign(new Error('Administrator sign-in is required.'), { statusCode: 401 })
    return session
  }

  async requireOrganizationPermission(subject: string, organizationId: string, permission: ControlPermission): Promise<ControlRole> {
    const role = await this.store.controlRole(subject, organizationId)
    if (!role || !controlPermissions[role].has(permission)) throw Object.assign(new Error('Your organization role does not permit this action.'), { statusCode: 403 })
    return role
  }

  async requireProjectPermission(subject: string, projectId: string, permission: ControlPermission): Promise<{ organizationId: string; role: ControlRole }> {
    const project = await this.store.project(projectId)
    if (!project?.organizationId) throw Object.assign(new Error('Project not found.'), { statusCode: 404 })
    const role = await this.requireOrganizationPermission(subject, project.organizationId, permission)
    return { organizationId: project.organizationId, role }
  }

  private async discovery(issuer: string): Promise<{ authorization_endpoint: string; token_endpoint: string; jwks_uri: string }> {
    const response = await fetch(`${issuer.replace(/\/$/, '')}/.well-known/openid-configuration`)
    if (!response.ok) throw Object.assign(new Error('Unable to discover the administrator OIDC provider.'), { statusCode: 502 })
    return response.json() as Promise<{ authorization_endpoint: string; token_endpoint: string; jwks_uri: string }>
  }

  private cookieName(): string {
    return this.publicUrl.startsWith('https://') ? '__Host-archetype-control' : 'archetype-control'
  }
}

export function controlOidcFromEnvironment(): ControlOidcConfig | null {
  const issuer = process.env.ARCHETYPE_CONTROL_OIDC_ISSUER
  const clientId = process.env.ARCHETYPE_CONTROL_OIDC_CLIENT_ID
  if (!issuer || !clientId) return null
  const clientSecret = process.env.ARCHETYPE_CONTROL_OIDC_CLIENT_SECRET
  return { issuer, clientId, ...(clientSecret ? { clientSecret } : {}) }
}

export function hasControlPermission(role: ControlRole, permission: ControlPermission): boolean {
  return controlPermissions[role].has(permission)
}
