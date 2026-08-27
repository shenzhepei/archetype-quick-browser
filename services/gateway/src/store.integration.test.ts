import { randomUUID } from 'node:crypto'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'
import { PlatformStore } from './store.js'

const enabled = process.env.RUN_INTEGRATION_TESTS === '1'
const store = new PlatformStore(
  process.env.TEST_PLATFORM_POSTGRES_URL ?? 'postgres://archetype:archetype@localhost:5432/archetype',
  'integration-master-key-with-sufficient-entropy'
)

describe.skipIf(!enabled)('platform persistence boundaries', () => {
  const projectId = `test-${randomUUID()}`

  beforeAll(async () => {
    await store.initialize()
    await store.seedProject(projectId, 'Integration', ['https://example.test'])
  })

  afterAll(async () => store.pool.end())

  test('stores database credentials only as an authenticated envelope', async () => {
    const secret = `postgres://runtime:${randomUUID()}@database.internal/app`
    await store.setConnection(projectId, 'postgres', secret)
    expect(await store.connection(projectId)).toEqual({ dialect: 'postgres', url: secret })
    const raw = await store.pool.query('SELECT envelope::text AS envelope FROM connections WHERE project_id = $1', [projectId])
    expect(raw.rows[0].envelope).not.toContain(secret)
  })

  test('rejects a nonce replay', async () => {
    const nonce = randomUUID()
    expect(await store.useNonce(projectId, 'https://example.test', nonce)).toBe(true)
    expect(await store.useNonce(projectId, 'https://example.test', nonce)).toBe(false)
  })

  test('returns cached responses and rejects an idempotency hash conflict', async () => {
    const key = randomUUID()
    await store.storeResponse(projectId, 'user-1', 'order.claim', key, 'hash-a', { claimed: true })
    expect(await store.cachedResponse(projectId, 'user-1', 'order.claim', key, 'hash-a')).toEqual({ claimed: true })
    await expect(store.cachedResponse(projectId, 'user-1', 'order.claim', key, 'hash-b')).rejects.toThrow('IDEMPOTENCY_CONFLICT')
  })

  test('hashes opaque session tokens and revokes them', async () => {
    const session = await store.createSession(projectId, 'https://example.test', 'user-1', 'Example User', { role: 'buyer' })
    const raw = await store.pool.query('SELECT token_hash FROM sessions WHERE project_id = $1', [projectId])
    expect(raw.rows[0].token_hash).not.toBe(session.token)
    expect((await store.session(session.token))?.subject).toBe('user-1')
    await store.revokeSession(session.token)
    expect(await store.session(session.token)).toBeNull()
  })

  test('persists one-time control login transactions and hashed control sessions', async () => {
    const state = randomUUID()
    await store.createControlTransaction(state, 'pkce-verifier', 'oidc-nonce', '/console/')
    expect(await store.consumeControlTransaction(state)).toEqual({ verifier: 'pkce-verifier', nonce: 'oidc-nonce', returnTo: '/console/' })
    expect(await store.consumeControlTransaction(state)).toBeNull()

    const session = await store.createControlSession('control-user', 'Control User', { email: 'control@example.test' })
    const raw = await store.pool.query('SELECT token_hash FROM control_sessions WHERE subject = $1', ['control-user'])
    expect(raw.rows[0].token_hash).not.toBe(session.token)
    expect((await store.controlSession(session.token))?.subject).toBe('control-user')
    await store.revokeControlSession(session.token)
    expect(await store.controlSession(session.token)).toBeNull()
  })

  test('persists organization roles and lists members without claims', async () => {
    const organizationId = `organization-${randomUUID()}`
    await store.ensureBootstrapOrganization(organizationId, 'Integration organization', ['owner-subject'])
    await store.addControlMember(organizationId, 'auditor-subject', 'Audit User', 'auditor')
    expect(await store.controlRole('auditor-subject', organizationId)).toBe('auditor')
    expect(await store.controlMembers(organizationId)).toEqual([
      { subject: 'auditor-subject', displayName: 'Audit User', role: 'auditor' },
      { subject: 'owner-subject', role: 'owner' }
    ])
  })

  test('round-trips deployment operation descriptors through JSONB', async () => {
    const deployment = {
      projectId,
      sha256: 'a'.repeat(64),
      path: '/var/lib/archetype/deployments/test.mjs',
      operations: [{ name: 'order.list', auth: 'optional' as const, timeoutMs: 30_000 }]
    }
    await store.setDeployment(deployment)
    expect(await store.deployment(projectId)).toEqual(deployment)
  })

  test('delivers queue events only to the matching authenticated subject', async () => {
    const firstId = randomUUID()
    const secondId = randomUUID()
    const since = new Date(Date.now() - 1_000)
    await store.pool.query(
      `INSERT INTO runtime_queue (event_id, project_id, topic, payload, created_at)
       VALUES ($1, $3, 'order.claimed', $4, NOW()), ($2, $3, 'order.claimed', $5, NOW())`,
      [firstId, secondId, projectId, { orderId: 'one', userId: 'user-1' }, { orderId: 'two', userId: 'user-2' }]
    )
    const events = await store.eventsSince(projectId, 'order.claimed', 'user-1', since)
    expect(events).toHaveLength(1)
    expect(events[0]?.id).toBe(firstId)
    expect(events[0]?.payload).toEqual({ orderId: 'one', userId: 'user-1' })
  })
})
