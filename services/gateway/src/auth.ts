import { createHash, randomBytes } from 'node:crypto'
import { createRemoteJWKSet, decodeJwt, jwtVerify } from 'jose'
import type { PlatformStore, ProjectRecord } from './store.js'

interface AuthTransaction {
  projectId: string
  origin: string
  pollToken: string
  verifier: string
  redirectUri: string
  oidc?: { issuer: string; clientId: string; clientSecret?: string }
  sessionToken?: string
  createdAt: number
}

export class AuthService {
  private readonly transactions = new Map<string, AuthTransaction>()

  constructor(private readonly store: PlatformStore, private readonly publicUrl: string) {}

  async start(project: ProjectRecord, origin: string): Promise<{ authUrl: string; pollToken: string }> {
    const state = randomBytes(24).toString('base64url')
    const pollToken = randomBytes(24).toString('base64url')
    const verifier = randomBytes(32).toString('base64url')
    const redirectUri = `${this.publicUrl}/v1/auth/callback`
    const configured = project.oidc as { issuer?: string; clientId?: string; clientSecret?: string } | null
    const transaction: AuthTransaction = { projectId: project.id, origin, pollToken, verifier, redirectUri, createdAt: Date.now() }
    let authUrl = `${this.publicUrl}/v1/auth/dev?state=${encodeURIComponent(state)}`
    if (configured?.issuer && configured.clientId) {
      const oidc = { issuer: configured.issuer, clientId: configured.clientId, ...(configured.clientSecret ? { clientSecret: configured.clientSecret } : {}) }
      transaction.oidc = oidc
      const discovery = await fetch(`${oidc.issuer.replace(/\/$/, '')}/.well-known/openid-configuration`).then((response) => response.json()) as { authorization_endpoint: string }
      const challenge = createHash('sha256').update(verifier).digest('base64url')
      const url = new URL(discovery.authorization_endpoint)
      url.search = new URLSearchParams({ response_type: 'code', client_id: oidc.clientId, redirect_uri: redirectUri, scope: 'openid profile email', state, code_challenge: challenge, code_challenge_method: 'S256' }).toString()
      authUrl = url.toString()
    }
    this.transactions.set(state, transaction)
    return { authUrl, pollToken }
  }

  async completeDev(state: string): Promise<void> {
    const transaction = this.required(state)
    const session = await this.store.createSession(transaction.projectId, transaction.origin, 'demo-user', 'Demo User', { sub: 'demo-user', name: 'Demo User' })
    transaction.sessionToken = session.token
  }

  async completeOidc(state: string, code: string): Promise<void> {
    const transaction = this.required(state)
    if (!transaction.oidc) throw new Error('OIDC is not configured for this transaction.')
    const discovery = await fetch(`${transaction.oidc.issuer.replace(/\/$/, '')}/.well-known/openid-configuration`).then((response) => response.json()) as { token_endpoint: string; jwks_uri: string }
    const body = new URLSearchParams({ grant_type: 'authorization_code', code, redirect_uri: transaction.redirectUri, client_id: transaction.oidc.clientId, code_verifier: transaction.verifier })
    if (transaction.oidc.clientSecret) body.set('client_secret', transaction.oidc.clientSecret)
    const token = await fetch(discovery.token_endpoint, { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded' }, body }).then((response) => response.json()) as { id_token?: string }
    if (!token.id_token) throw new Error('OIDC provider did not return an ID token.')
    const unverified = decodeJwt(token.id_token)
    const verified = await jwtVerify(token.id_token, createRemoteJWKSet(new URL(discovery.jwks_uri)), { issuer: transaction.oidc.issuer, audience: transaction.oidc.clientId })
    const subject = String(verified.payload.sub)
    const displayName = typeof unverified.name === 'string' ? unverified.name : undefined
    const session = await this.store.createSession(transaction.projectId, transaction.origin, subject, displayName, verified.payload as Record<string, unknown>)
    transaction.sessionToken = session.token
  }

  poll(pollToken: string): { pending: true } | { pending: false; token: string } {
    for (const [state, transaction] of this.transactions) {
      if (transaction.createdAt < Date.now() - 10 * 60_000) {
        this.transactions.delete(state)
        continue
      }
      if (transaction.pollToken !== pollToken) continue
      return transaction.sessionToken ? { pending: false, token: transaction.sessionToken } : { pending: true }
    }
    throw new Error('Authentication transaction was not found or expired.')
  }

  private required(state: string): AuthTransaction {
    const transaction = this.transactions.get(state)
    if (!transaction || transaction.createdAt < Date.now() - 10 * 60_000) throw new Error('Authentication transaction was not found or expired.')
    return transaction
  }
}
