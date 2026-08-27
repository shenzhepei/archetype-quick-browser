import { createHash, randomUUID } from 'node:crypto'
import { existsSync } from 'node:fs'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import Fastify, { type FastifyInstance } from 'fastify'
import cors from '@fastify/cors'
import fastifyStatic from '@fastify/static'
import { capabilityRequestSchema, operationDescriptorSchema, projectDescriptorSchema, runtimeDiscoverySchema, sessionSummarySchema, signedInvokeRequestSchema, topicSchema } from '@archetype/protocol'
import { z } from 'zod'
import { AuthService } from './auth.js'
import { ControlAuthService, controlOidcFromEnvironment } from './control-auth.js'
import { deviceKeyDigest, issueCapability, verifyCapability } from './capability.js'
import { invocationDigest, verifyDeviceProof } from './device-proof.js'
import { PlatformStore } from './store.js'

const serviceToken = process.env.ARCHETYPE_SERVICE_TOKEN ?? 'development-service-token'
const publicUrl = process.env.ARCHETYPE_PUBLIC_URL ?? 'http://localhost:8787'
const deploymentsDirectory = process.env.ARCHETYPE_DEPLOYMENTS_DIR ?? join(process.cwd(), '.archetype', 'deployments')
const functionHostUrl = process.env.ARCHETYPE_FUNCTION_HOST_URL ?? 'http://localhost:8790'
const capabilityKey = process.env.ARCHETYPE_CAPABILITY_KEY ?? process.env.ARCHETYPE_MASTER_KEY ?? 'development-capability-key-change-me'

function bearer(header: string | undefined): string | undefined {
  return header?.startsWith('Bearer ') ? header.slice(7) : undefined
}

function requireAdmin(header: string | undefined): void {
  const adminToken = process.env.ARCHETYPE_ADMIN_TOKEN
  if (!adminToken || bearer(header) !== adminToken) throw Object.assign(new Error('Administrative automation token is invalid or disabled.'), { statusCode: 401 })
}

function safeOrigin(value: string): string {
  const parsed = new URL(value)
  if (parsed.protocol !== 'https:' && !(parsed.protocol === 'http:' && ['localhost', '127.0.0.1'].includes(parsed.hostname))) {
    throw Object.assign(new Error('Origin must be HTTPS outside localhost.'), { statusCode: 400 })
  }
  return parsed.origin
}

export async function buildServer(store: PlatformStore): Promise<FastifyInstance> {
  const server = Fastify({ logger: { redact: ['req.headers.authorization', 'body.databaseUrl', 'body.artifact'] }, bodyLimit: 12 * 1024 * 1024 })
  await server.register(cors, { origin: false })
  const auth = new AuthService(store, publicUrl)
  const control = new ControlAuthService(store, publicUrl, controlOidcFromEnvironment(), process.env.ARCHETYPE_CONTROL_DEV_LOGIN === 'true')
  const controlOrigin = new URL(publicUrl).origin

  const requireControlOrigin = (request: Parameters<typeof control.requireSession>[0]): void => {
    if (request.headers.origin !== controlOrigin) {
      throw Object.assign(new Error('Control-plane writes require a same-origin request.'), { statusCode: 403 })
    }
  }

  const activateDeployment = async (projectId: string, body: { sha256: string; artifact: string; operations: z.infer<typeof operationDescriptorSchema>[] }, actor?: string) => {
    const bytes = Buffer.from(body.artifact, 'base64')
    if (createHash('sha256').update(bytes).digest('hex') !== body.sha256) throw Object.assign(new Error('Deployment digest does not match its artifact.'), { statusCode: 400 })
    const projectDirectory = join(deploymentsDirectory, projectId)
    await mkdir(projectDirectory, { recursive: true })
    const path = join(projectDirectory, `${body.sha256}.mjs`)
    await writeFile(path, bytes, { mode: 0o600 })
    await store.setDeployment({ projectId, sha256: body.sha256, path, operations: body.operations })
    await store.audit(projectId, 'deployment.activated', { sha256: body.sha256, operations: body.operations.map((operation) => operation.name), ...(actor ? { actor } : {}) })
    return { projectId, sha256: body.sha256, operations: body.operations }
  }

  server.setErrorHandler((error, _request, reply) => {
    const normalized = error instanceof Error ? error : new Error('Unexpected gateway error.')
    const statusCode = typeof (normalized as { statusCode?: unknown }).statusCode === 'number' ? (normalized as Error & { statusCode: number }).statusCode : 500
    reply.status(statusCode).send({ error: { code: statusCode === 500 ? 'INTERNAL_ERROR' : 'REQUEST_REJECTED', message: normalized.message } })
  })

  server.get('/health', async () => ({ ok: true, service: 'archetype-gateway' }))

  const consoleDirectory = process.env.ARCHETYPE_CONSOLE_DIR ?? join(process.cwd(), 'apps', 'console', 'dist')
  if (existsSync(consoleDirectory)) {
    await server.register(fastifyStatic, { root: consoleDirectory, prefix: '/console/', decorateReply: true })
    server.get('/console', async (_request, reply) => reply.redirect('/console/'))
  }

  server.get('/v1/projects/:projectId/manifest', async (request, reply) => {
    const { projectId } = request.params as { projectId: string }
    const origin = safeOrigin(String((request.query as { origin?: string }).origin ?? ''))
    const project = await store.project(projectId)
    if (!project || !project.allowedOrigins.includes(origin)) return reply.status(404).send({ error: { code: 'PROJECT_NOT_FOUND', message: 'No Runtime project is registered for this origin.' } })
    const deployment = await store.deployment(projectId)
    return projectDescriptorSchema.parse({ version: 1, projectId, name: project.name, origin, operations: deployment?.operations ?? [] })
  })

  server.post('/v1/auth/start', async (request, reply) => {
    const body = z.object({ projectId: z.string(), origin: z.string() }).parse(request.body)
    const origin = safeOrigin(body.origin)
    const project = await store.project(body.projectId)
    if (!project || !project.allowedOrigins.includes(origin)) return reply.status(404).send({ error: { code: 'PROJECT_NOT_FOUND', message: 'No Runtime project is registered for this origin.' } })
    return auth.start(project, origin)
  })

  server.get('/v1/auth/dev', async (request, reply) => {
    const state = z.string().parse((request.query as { state?: string }).state)
    await auth.completeDev(state)
    return reply.type('text/html').send('<!doctype html><html><body style="font:16px system-ui;padding:40px"><h1>Signed in</h1><p>You can return to Archetype Runtime.</p></body></html>')
  })

  server.get('/v1/auth/callback', async (request, reply) => {
    const query = z.object({ state: z.string(), code: z.string() }).parse(request.query)
    await auth.completeOidc(query.state, query.code)
    return reply.type('text/html').send('<!doctype html><html><body style="font:16px system-ui;padding:40px"><h1>Signed in</h1><p>You can return to Archetype Runtime.</p></body></html>')
  })

  server.get('/v1/auth/poll', async (request) => auth.poll(z.string().parse((request.query as { token?: string }).token)))

  server.get('/v1/session', async (request, reply) => {
    const token = bearer(request.headers.authorization)
    if (!token) return null
    const session = await store.session(token)
    return session ? sessionSummarySchema.parse(session) : reply.status(401).send({ error: { code: 'SESSION_EXPIRED', message: 'The Runtime session expired.' } })
  })

  server.delete('/v1/session', async (request, reply) => {
    const token = bearer(request.headers.authorization)
    if (token) await store.revokeSession(token)
    return reply.status(204).send()
  })

  server.get('/v1/control/auth/login', async (request, reply) => {
    const returnTo = z.string().optional().parse((request.query as { returnTo?: string }).returnTo)
    return reply.redirect(await control.login(returnTo))
  })

  server.get('/v1/control/auth/dev', async (request, reply) => {
    const state = z.string().parse((request.query as { state?: string }).state)
    const result = await control.completeDevelopment(state)
    control.setSessionCookie(reply, result.session.token)
    return reply.redirect(result.returnTo)
  })

  server.get('/v1/control/auth/callback', async (request, reply) => {
    const query = z.object({ state: z.string(), code: z.string() }).parse(request.query)
    const result = await control.completeOidc(query.state, query.code)
    control.setSessionCookie(reply, result.session.token)
    return reply.redirect(result.returnTo)
  })

  server.get('/v1/control/session', async (request, reply) => {
    const session = await control.session(request)
    if (!session) return reply.status(401).send({ error: { code: 'CONTROL_AUTH_REQUIRED', message: 'Administrator sign-in is required.' } })
    const organizations = await store.organizationsForSubject(session.subject)
    return { subject: session.subject, displayName: session.displayName, expiresAt: session.expiresAt, organizations }
  })

  server.delete('/v1/control/session', async (request, reply) => {
    requireControlOrigin(request)
    const token = control.token(request)
    if (token) await store.revokeControlSession(token)
    control.clearSessionCookie(reply)
    return reply.status(204).send()
  })

  server.get('/v1/control/organizations', async (request) => {
    const session = await control.requireSession(request)
    return store.organizationsForSubject(session.subject)
  })

  server.post('/v1/control/organizations/:organizationId/members', async (request, reply) => {
    requireControlOrigin(request)
    const session = await control.requireSession(request)
    const { organizationId } = request.params as { organizationId: string }
    await control.requireOrganizationPermission(session.subject, organizationId, 'member:write')
    const body = z.object({ subject: z.string().min(1).max(255), displayName: z.string().max(255).optional(), role: z.enum(['owner', 'admin', 'developer', 'operator', 'auditor']) }).parse(request.body)
    await store.addControlMember(organizationId, body.subject, body.displayName, body.role)
    await store.audit(null, 'control.member.updated', { organizationId, subject: body.subject, role: body.role, actor: session.subject })
    return reply.status(201).send({ organizationId, ...body })
  })

  server.get('/v1/control/organizations/:organizationId/members', async (request) => {
    const session = await control.requireSession(request)
    const { organizationId } = request.params as { organizationId: string }
    await control.requireOrganizationPermission(session.subject, organizationId, 'organization:read')
    return store.controlMembers(organizationId)
  })

  server.get('/v1/control/projects', async (request) => {
    const session = await control.requireSession(request)
    return store.projectsForSubject(session.subject)
  })

  server.post('/v1/control/projects', async (request, reply) => {
    requireControlOrigin(request)
    const session = await control.requireSession(request)
    const body = z.object({ organizationId: z.string().min(1), name: z.string().min(2).max(100) }).parse(request.body)
    await control.requireOrganizationPermission(session.subject, body.organizationId, 'project:create')
    const project = await store.createProject(body.name, body.organizationId)
    await store.audit(project.id, 'project.created', { organizationId: body.organizationId, actor: session.subject })
    return reply.status(201).send(project)
  })

  server.post('/v1/control/projects/:projectId/origins', async (request, reply) => {
    requireControlOrigin(request)
    const session = await control.requireSession(request)
    const { projectId } = request.params as { projectId: string }
    await control.requireProjectPermission(session.subject, projectId, 'project:configure')
    const origin = safeOrigin(z.object({ origin: z.string() }).parse(request.body).origin)
    await store.addOrigin(projectId, origin)
    await store.audit(projectId, 'origin.added', { origin, actor: session.subject })
    return reply.status(201).send({ projectId, origin })
  })

  server.put('/v1/control/projects/:projectId/oidc', async (request) => {
    requireControlOrigin(request)
    const session = await control.requireSession(request)
    const { projectId } = request.params as { projectId: string }
    await control.requireProjectPermission(session.subject, projectId, 'project:configure')
    const oidc = z.object({ issuer: z.url(), clientId: z.string(), clientSecret: z.string().optional() }).parse(request.body)
    await store.setOidc(projectId, oidc)
    await store.audit(projectId, 'oidc.configured', { issuer: oidc.issuer, actor: session.subject })
    return { ok: true }
  })

  server.put('/v1/control/projects/:projectId/database', async (request) => {
    requireControlOrigin(request)
    const session = await control.requireSession(request)
    const { projectId } = request.params as { projectId: string }
    await control.requireProjectPermission(session.subject, projectId, 'project:configure')
    const body = z.object({ dialect: z.enum(['postgres', 'mysql']), databaseUrl: z.string().min(10) }).parse(request.body)
    await store.setConnection(projectId, body.dialect, body.databaseUrl)
    await store.audit(projectId, 'database.configured', { dialect: body.dialect, actor: session.subject })
    return { ok: true }
  })

  server.get('/v1/control/projects/:projectId/logs', async (request) => {
    const session = await control.requireSession(request)
    const { projectId } = request.params as { projectId: string }
    await control.requireProjectPermission(session.subject, projectId, 'audit:read')
    return store.logs(projectId)
  })

  server.post('/v1/control/projects/:projectId/deployments', async (request) => {
    requireControlOrigin(request)
    const session = await control.requireSession(request)
    const { projectId } = request.params as { projectId: string }
    await control.requireProjectPermission(session.subject, projectId, 'deployment:write')
    const body = z.object({ sha256: z.string().regex(/^[a-f0-9]{64}$/), artifact: z.string(), operations: z.array(operationDescriptorSchema) }).parse(request.body)
    return activateDeployment(projectId, body, session.subject)
  })

  server.post('/v1/capabilities', async (request, reply) => {
    const body = capabilityRequestSchema.parse(request.body)
    const origin = safeOrigin(body.origin)
    const project = await store.project(body.projectId)
    if (!project || !project.allowedOrigins.includes(origin)) return reply.status(404).send({ error: { code: 'PROJECT_NOT_FOUND', message: 'No Runtime project is registered for this origin.' } })

    const token = bearer(request.headers.authorization)
    const session = token ? await store.session(token) : null
    if (session && (session.projectId !== body.projectId || session.origin !== origin)) return reply.status(403).send({ error: { code: 'SESSION_SCOPE_MISMATCH', message: 'The session belongs to another project or origin.' } })

    let resource: string
    if (body.kind === 'invoke') {
      const deployment = await store.deployment(body.projectId)
      const operation = deployment?.operations.find((candidate) => candidate.name === body.operation)
      if (!operation) return reply.status(404).send({ error: { code: 'OPERATION_NOT_FOUND', message: 'The operation is not deployed.' } })
      if (operation.auth === 'required' && !session) return reply.status(401).send({ error: { code: 'AUTH_REQUIRED', message: 'Sign in before invoking this operation.' } })
      resource = `invoke:${body.operation}`
    } else {
      if (!session) return reply.status(401).send({ error: { code: 'AUTH_REQUIRED', message: 'Sign in before subscribing to project events.' } })
      resource = `subscribe:${body.topic}`
    }

    const issued = await issueCapability({ projectId: body.projectId, origin, resource, deviceKey: deviceKeyDigest(body.publicKey), subject: session?.subject ?? null }, capabilityKey)
    await store.audit(body.projectId, 'capability.issued', { origin, resource, subject: session?.subject ?? 'anonymous', expiresAt: issued.expiresAt })
    return issued
  })

  server.post('/v1/invoke', async (request, reply) => {
    const body = signedInvokeRequestSchema.parse(request.body)
    const origin = safeOrigin(body.origin)
    if (Math.abs(Date.now() - body.timestamp) > 120_000) return reply.status(401).send({ error: { code: 'STALE_PROOF', message: 'The signed request is outside the allowed time window.' } })
    if (body.bodyDigest !== invocationDigest(body.operation, body.input, body.idempotencyKey, body.timeoutMs)) return reply.status(400).send({ error: { code: 'DIGEST_MISMATCH', message: 'The request digest does not match its input.' } })
    if (!verifyDeviceProof(body)) return reply.status(401).send({ error: { code: 'INVALID_DEVICE_PROOF', message: 'The device signature is invalid.' } })
    if (!await store.useNonce(body.projectId, origin, body.nonce)) return reply.status(409).send({ error: { code: 'REPLAYED_REQUEST', message: 'This signed request has already been used.' } })

    const project = await store.project(body.projectId)
    if (!project || !project.allowedOrigins.includes(origin)) return reply.status(404).send({ error: { code: 'PROJECT_NOT_FOUND', message: 'No Runtime project is registered for this origin.' } })
    const deployment = await store.deployment(body.projectId)
    const operation = deployment?.operations.find((candidate) => candidate.name === body.operation)
    if (!deployment || !operation) return reply.status(404).send({ error: { code: 'OPERATION_NOT_FOUND', message: 'The operation is not deployed.' } })

    const token = bearer(request.headers.authorization)
    const session = token ? await store.session(token) : null
    if (session && (session.projectId !== body.projectId || session.origin !== origin)) return reply.status(403).send({ error: { code: 'SESSION_SCOPE_MISMATCH', message: 'The session belongs to another project or origin.' } })
    if (operation.auth === 'required' && !session) return reply.status(401).send({ error: { code: 'AUTH_REQUIRED', message: 'Sign in before invoking this operation.' } })
    const permitted = await verifyCapability(body.capabilityTicket, {
      projectId: body.projectId,
      origin,
      resource: `invoke:${body.operation}`,
      deviceKey: deviceKeyDigest(body.publicKey),
      subject: session?.subject ?? null
    }, capabilityKey)
    if (!permitted) return reply.status(401).send({ error: { code: 'INVALID_CAPABILITY', message: 'The capability ticket is expired or does not match this request.' } })

    const subject = session?.subject ?? `anonymous:${createHash('sha256').update(JSON.stringify(body.publicKey)).digest('hex')}`
    if (body.idempotencyKey) {
      try {
        const cached = await store.cachedResponse(body.projectId, subject, body.operation, body.idempotencyKey, body.bodyDigest)
        if (cached !== undefined) return { requestId: 'cached', result: cached, cached: true }
      } catch {
        return reply.status(409).send({ error: { code: 'IDEMPOTENCY_CONFLICT', message: 'The idempotency key was already used for different input.' } })
      }
    }

    const connection = await store.connection(body.projectId)
    if (!connection) return reply.status(503).send({ error: { code: 'DATABASE_NOT_CONFIGURED', message: 'The project has no database binding.' } })
    const requestId = randomUUID()
    const response = await fetch(`${functionHostUrl}/internal/invoke`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', authorization: `Bearer ${serviceToken}` },
      body: JSON.stringify({ requestId, projectId: body.projectId, origin, operation: body.operation, input: body.input, user: session ? { id: session.subject, displayName: session.displayName, claims: session.claims } : null, deployment, connection, timeoutMs: Math.min(body.timeoutMs ?? operation.timeoutMs, operation.timeoutMs) }),
      signal: AbortSignal.timeout(Math.min(body.timeoutMs ?? operation.timeoutMs, operation.timeoutMs) + 2_000)
    })
    const payload = await response.json() as { result?: unknown; error?: { code: string; message: string } }
    await store.audit(body.projectId, 'function.invoked', { requestId, operation: body.operation, origin, subject, ok: response.ok })
    if (!response.ok) return reply.status(response.status >= 400 && response.status < 600 ? response.status : 500).send(payload)
    if (body.idempotencyKey) await store.storeResponse(body.projectId, subject, body.operation, body.idempotencyKey, body.bodyDigest, payload.result)
    return { requestId, result: payload.result }
  })

  server.get('/v1/events', async (request, reply) => {
    const query = z.object({ projectId: z.string(), origin: z.string(), topic: topicSchema }).parse(request.query)
    const origin = safeOrigin(query.origin)
    const sessionToken = bearer(request.headers.authorization)
    const session = sessionToken ? await store.session(sessionToken) : null
    const subject = session?.subject
    if (!session || !subject || session.projectId !== query.projectId || session.origin !== origin) return reply.status(401).send({ error: { code: 'AUTH_REQUIRED', message: 'A matching Runtime session is required.' } })
    const ticket = z.string().min(32).parse(request.headers['x-archetype-capability'])
    const encodedKey = z.string().min(16).parse(request.headers['x-archetype-device-key'])
    const publicKey = z.record(z.string(), z.unknown()).parse(JSON.parse(Buffer.from(encodedKey, 'base64url').toString('utf8')))
    const permitted = await verifyCapability(ticket, {
      projectId: query.projectId,
      origin,
      resource: `subscribe:${query.topic}`,
      deviceKey: deviceKeyDigest(publicKey),
      subject
    }, capabilityKey)
    if (!permitted) return reply.status(401).send({ error: { code: 'INVALID_CAPABILITY', message: 'The subscription capability is expired or invalid.' } })

    reply.hijack()
    reply.raw.writeHead(200, {
      'content-type': 'text/event-stream; charset=utf-8',
      'cache-control': 'no-cache, no-transform',
      connection: 'keep-alive',
      'x-accel-buffering': 'no'
    })
    let closed = false
    const seen = new Set<string>()
    let since = new Date()
    let afterId = ''
    const close = (): void => { closed = true }
    request.raw.once('close', close)
    const deadline = Date.now() + 55_000
    try {
      while (!closed && Date.now() < deadline) {
        const events = await store.eventsSince(query.projectId, query.topic, subject, since, afterId)
        for (const event of events) {
          if (seen.has(event.id)) continue
          seen.add(event.id)
          reply.raw.write(`id: ${event.id}\nevent: ${event.topic}\ndata: ${JSON.stringify(event.payload)}\n\n`)
          since = new Date(event.createdAt)
          afterId = event.id
        }
        reply.raw.write(': heartbeat\n\n')
        await new Promise((resolve) => setTimeout(resolve, 500))
      }
    } finally {
      request.raw.off('close', close)
      if (!closed) reply.raw.end()
    }
  })

  server.post('/v1/admin/projects', async (request) => {
    requireAdmin(request.headers.authorization)
    const body = z.object({ name: z.string().min(2).max(100), organizationId: z.string().optional() }).parse(request.body)
    return store.createProject(body.name, body.organizationId ?? null)
  })

  server.post('/v1/admin/projects/:projectId/origins', async (request, reply) => {
    requireAdmin(request.headers.authorization)
    const { projectId } = request.params as { projectId: string }
    const origin = safeOrigin(z.object({ origin: z.string() }).parse(request.body).origin)
    if (!await store.project(projectId)) return reply.status(404).send({ error: { code: 'PROJECT_NOT_FOUND', message: 'Project not found.' } })
    await store.addOrigin(projectId, origin)
    return { projectId, origin }
  })

  server.put('/v1/admin/projects/:projectId/oidc', async (request) => {
    requireAdmin(request.headers.authorization)
    const { projectId } = request.params as { projectId: string }
    const oidc = z.object({ issuer: z.url(), clientId: z.string(), clientSecret: z.string().optional() }).parse(request.body)
    await store.setOidc(projectId, oidc)
    return { ok: true }
  })

  server.put('/v1/admin/projects/:projectId/database', async (request) => {
    requireAdmin(request.headers.authorization)
    const { projectId } = request.params as { projectId: string }
    const body = z.object({ dialect: z.enum(['postgres', 'mysql']), databaseUrl: z.string().min(10) }).parse(request.body)
    await store.setConnection(projectId, body.dialect, body.databaseUrl)
    await store.audit(projectId, 'database.configured', { dialect: body.dialect })
    return { ok: true }
  })

  server.post('/v1/admin/projects/:projectId/deployments', async (request) => {
    requireAdmin(request.headers.authorization)
    const { projectId } = request.params as { projectId: string }
    const body = z.object({ sha256: z.string().regex(/^[a-f0-9]{64}$/), artifact: z.string(), operations: z.array(operationDescriptorSchema) }).parse(request.body)
    return activateDeployment(projectId, body)
  })

  server.get('/v1/admin/projects/:projectId/logs', async (request) => {
    requireAdmin(request.headers.authorization)
    return store.logs((request.params as { projectId: string }).projectId)
  })

  server.get('/v1/internal/projects/:projectId/deployment', async (request, reply) => {
    if (bearer(request.headers.authorization) !== serviceToken) return reply.status(401).send({ error: { code: 'UNAUTHORIZED', message: 'Invalid service token.' } })
    const deployment = await store.deployment((request.params as { projectId: string }).projectId)
    return deployment ?? reply.status(404).send({ error: { code: 'DEPLOYMENT_NOT_FOUND', message: 'Deployment not found.' } })
  })

  return server
}

async function start(): Promise<void> {
  const store = new PlatformStore(process.env.PLATFORM_DATABASE_URL ?? 'postgres://archetype:archetype@localhost:5432/archetype', process.env.ARCHETYPE_MASTER_KEY ?? 'development-master-key-change-me')
  for (let attempt = 0; ; attempt += 1) {
    try {
      await store.initialize()
      break
    } catch (error) {
      if (attempt >= 20) throw error
      await new Promise((resolve) => setTimeout(resolve, 1_000))
    }
  }
  const bootstrapOrganizationId = process.env.ARCHETYPE_CONTROL_ORGANIZATION_ID ?? 'default'
  const bootstrapSubjects = (process.env.ARCHETYPE_CONTROL_BOOTSTRAP_SUBJECTS ?? (process.env.ARCHETYPE_CONTROL_DEV_LOGIN === 'true' ? 'development-admin' : '')).split(',').map((value) => value.trim()).filter(Boolean)
  await store.ensureBootstrapOrganization(bootstrapOrganizationId, process.env.ARCHETYPE_CONTROL_ORGANIZATION_NAME ?? 'Default organization', bootstrapSubjects)
  const devProject = process.env.ARCHETYPE_DEV_PROJECT_ID
  if (devProject) await store.seedProject(devProject, process.env.ARCHETYPE_DEV_PROJECT_NAME ?? 'Order Claim Demo', (process.env.ARCHETYPE_DEV_ORIGINS ?? 'http://localhost:4173').split(','), bootstrapOrganizationId)
  const server = await buildServer(store)
  await server.listen({ host: process.env.HOST ?? '0.0.0.0', port: Number(process.env.PORT ?? 8787) })
}

if (process.env.NODE_ENV !== 'test') void start()
