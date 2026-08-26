import { createHash, generateKeyPairSync, randomBytes, randomUUID, sign } from 'node:crypto'
import { afterAll, beforeAll, describe, expect, test, vi } from 'vitest'
import { canonicalJson, signingMessage, type SignedInvokeRequest } from '@archetype/protocol'
import { invocationDigest } from './device-proof.js'
import { buildServer } from './server.js'
import { PlatformStore } from './store.js'

const enabled = process.env.RUN_INTEGRATION_TESTS === '1'
const store = new PlatformStore(
  process.env.TEST_PLATFORM_POSTGRES_URL ?? 'postgres://archetype:archetype@localhost:5432/archetype',
  'gateway-integration-master-key'
)

describe.skipIf(!enabled)('gateway capability and event boundaries', () => {
  const projectId = `gateway-${randomUUID()}`
  const origin = 'https://gateway.example.test'
  const subject = 'gateway-user'
  const pair = generateKeyPairSync('ed25519')
  const publicKey = pair.publicKey.export({ format: 'jwk' })
  let sessionToken = ''
  let server: Awaited<ReturnType<typeof buildServer>>
  let baseUrl = ''

  beforeAll(async () => {
    await store.initialize()
    await store.seedProject(projectId, 'Gateway Integration', [origin])
    await store.setConnection(projectId, 'postgres', 'postgres://unused:unused@localhost/unused')
    await store.setDeployment({
      projectId,
      sha256: 'b'.repeat(64),
      path: '/tmp/gateway-integration.mjs',
      operations: [{ name: 'order.claim', auth: 'required', timeoutMs: 30_000 }]
    })
    sessionToken = (await store.createSession(projectId, origin, subject, 'Gateway User', {})).token
    server = await buildServer(store)
    await server.listen({ host: '127.0.0.1', port: 0 })
    const address = server.addresses()[0]
    if (!address) throw new Error('Gateway integration server did not bind a port.')
    baseUrl = `http://127.0.0.1:${address.port}`
  })

  afterAll(async () => {
    vi.unstubAllGlobals()
    await server.close()
    await store.pool.end()
  })

  async function capability(kind: 'invoke' | 'subscribe', resource: string): Promise<string> {
    const body = kind === 'invoke'
      ? { kind, projectId, origin, operation: resource, publicKey }
      : { kind, projectId, origin, topic: resource, publicKey }
    const response = await fetch(`${baseUrl}/v1/capabilities`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', authorization: `Bearer ${sessionToken}` },
      body: JSON.stringify(body)
    })
    expect(response.status).toBe(200)
    return ((await response.json()) as { ticket: string }).ticket
  }

  function signedCall(capabilityTicket: string): SignedInvokeRequest {
    const input = { orderId: randomUUID() }
    const unsigned: Omit<SignedInvokeRequest, 'signature' | 'publicKey' | 'capabilityTicket'> = {
      projectId,
      origin,
      operation: 'order.claim',
      input,
      timestamp: Date.now(),
      nonce: randomBytes(24).toString('base64url'),
      bodyDigest: invocationDigest('order.claim', input)
    }
    return {
      ...unsigned,
      publicKey,
      capabilityTicket,
      signature: sign(null, Buffer.from(signingMessage(unsigned)), pair.privateKey).toString('base64url')
    }
  }

  test('requires an untampered 60-second capability before invoking the function host', async () => {
    const ticket = await capability('invoke', 'order.claim')
    const hostFetch = vi.fn(async () => new Response(JSON.stringify({ result: { claimed: true } }), { status: 200, headers: { 'content-type': 'application/json' } }))
    vi.stubGlobal('fetch', hostFetch)
    try {
      const accepted = await server.inject({
        method: 'POST', url: '/v1/invoke', headers: { authorization: `Bearer ${sessionToken}` }, payload: signedCall(ticket)
      })
      expect(accepted.statusCode).toBe(200)
      expect(accepted.json().result).toEqual({ claimed: true })

      const parts = ticket.split('.')
      const signature = parts[2]!
      const offset = Math.floor(signature.length / 2)
      parts[2] = `${signature.slice(0, offset)}${signature[offset] === 'x' ? 'y' : 'x'}${signature.slice(offset + 1)}`
      const rejected = await server.inject({
        method: 'POST', url: '/v1/invoke', headers: { authorization: `Bearer ${sessionToken}` }, payload: signedCall(parts.join('.'))
      })
      expect(rejected.statusCode).toBe(401)
      expect(rejected.json().error.code).toBe('INVALID_CAPABILITY')
    } finally {
      vi.unstubAllGlobals()
    }
  })

  test('streams only subject-addressed events over authenticated SSE', async () => {
    const ticket = await capability('subscribe', 'order.claimed')
    const controller = new AbortController()
    const response = await fetch(`${baseUrl}/v1/events?projectId=${projectId}&origin=${encodeURIComponent(origin)}&topic=order.claimed`, {
      headers: {
        authorization: `Bearer ${sessionToken}`,
        'x-archetype-capability': ticket,
        'x-archetype-device-key': Buffer.from(JSON.stringify(publicKey)).toString('base64url')
      },
      signal: controller.signal
    })
    expect(response.status).toBe(200)
    const reader = response.body?.getReader()
    if (!reader) throw new Error('SSE response has no body.')
    await reader.read()

    const eventId = randomUUID()
    await store.pool.query(
      `INSERT INTO runtime_queue (event_id, project_id, topic, payload)
       VALUES ($1, $2, 'order.claimed', $3)`,
      [eventId, projectId, { orderId: 'visible', userId: subject }]
    )

    const decoder = new TextDecoder()
    let received = ''
    for (let attempt = 0; attempt < 10 && !received.includes(eventId); attempt += 1) {
      const chunk = await reader.read()
      if (chunk.done) break
      received += decoder.decode(chunk.value)
    }
    expect(received).toContain(`id: ${eventId}`)
    expect(received).toContain('"orderId":"visible"')
    controller.abort()
  }, 10_000)

  test('serves manifests and completes the development identity lifecycle', async () => {
    const manifest = await server.inject({ method: 'GET', url: `/v1/projects/${projectId}/manifest?origin=${encodeURIComponent(origin)}` })
    expect(manifest.statusCode).toBe(200)
    expect(manifest.json()).toMatchObject({ projectId, origin, operations: [{ name: 'order.claim' }] })
    const rejected = await server.inject({ method: 'GET', url: `/v1/projects/${projectId}/manifest?origin=${encodeURIComponent('https://other.example')}` })
    expect(rejected.statusCode).toBe(404)
    expect((await server.inject({ method: 'GET', url: '/v1/session' })).json()).toBeNull()

    const started = await server.inject({ method: 'POST', url: '/v1/auth/start', payload: { projectId, origin } })
    expect(started.statusCode).toBe(200)
    const auth = started.json() as { authUrl: string; pollToken: string }
    expect((await server.inject({ method: 'GET', url: `/v1/auth/poll?token=${auth.pollToken}` })).json()).toEqual({ pending: true })
    const authPath = new URL(auth.authUrl).pathname + new URL(auth.authUrl).search
    expect((await server.inject({ method: 'GET', url: authPath })).statusCode).toBe(200)
    const completed = (await server.inject({ method: 'GET', url: `/v1/auth/poll?token=${auth.pollToken}` })).json() as { token: string }
    const summary = await server.inject({ method: 'GET', url: '/v1/session', headers: { authorization: `Bearer ${completed.token}` } })
    expect(summary.json()).toMatchObject({ authenticated: true, subject: 'demo-user', displayName: 'Demo User' })
    expect((await server.inject({ method: 'DELETE', url: '/v1/session', headers: { authorization: `Bearer ${completed.token}` } })).statusCode).toBe(204)
    expect((await server.inject({ method: 'GET', url: '/v1/session', headers: { authorization: `Bearer ${completed.token}` } })).statusCode).toBe(401)
  })

  test('protects and persists the administrative control plane', async () => {
    const unauthorized = await server.inject({ method: 'POST', url: '/v1/admin/projects', payload: { name: 'Managed Project' } })
    expect(unauthorized.statusCode).toBe(401)
    const adminHeaders = { authorization: 'Bearer development-admin-token' }
    const created = await server.inject({ method: 'POST', url: '/v1/admin/projects', headers: adminHeaders, payload: { name: 'Managed Project' } })
    expect(created.statusCode).toBe(200)
    const managedProjectId = created.json().id as string
    expect((await server.inject({ method: 'POST', url: `/v1/admin/projects/${managedProjectId}/origins`, headers: adminHeaders, payload: { origin: 'https://managed.example' } })).statusCode).toBe(200)
    expect((await server.inject({ method: 'PUT', url: `/v1/admin/projects/${managedProjectId}/oidc`, headers: adminHeaders, payload: { issuer: 'https://identity.example', clientId: 'managed-client' } })).statusCode).toBe(200)
    expect((await server.inject({ method: 'PUT', url: `/v1/admin/projects/${managedProjectId}/database`, headers: adminHeaders, payload: { dialect: 'postgres', databaseUrl: 'postgres://managed:secret@database.internal/app' } })).statusCode).toBe(200)

    const artifact = Buffer.from('export default { functions: [], workers: [] }')
    const digest = createHash('sha256').update(artifact).digest('hex')
    const deployed = await server.inject({
      method: 'POST', url: `/v1/admin/projects/${managedProjectId}/deployments`, headers: adminHeaders,
      payload: { sha256: digest, artifact: artifact.toString('base64'), operations: [{ name: 'managed.read', auth: 'optional', timeoutMs: 2_000 }] }
    })
    expect(deployed.statusCode).toBe(200)
    const logs = await server.inject({ method: 'GET', url: `/v1/admin/projects/${managedProjectId}/logs`, headers: adminHeaders })
    expect(logs.json().map((entry: { event: string }) => entry.event)).toEqual(expect.arrayContaining(['database.configured', 'deployment.activated']))
    expect((await server.inject({ method: 'GET', url: `/v1/internal/projects/${managedProjectId}/deployment` })).statusCode).toBe(401)
    const internal = await server.inject({ method: 'GET', url: `/v1/internal/projects/${managedProjectId}/deployment`, headers: { authorization: 'Bearer development-service-token' } })
    expect(internal.json()).toMatchObject({ projectId: managedProjectId, sha256: digest, operations: [{ name: 'managed.read' }] })
  })
})
