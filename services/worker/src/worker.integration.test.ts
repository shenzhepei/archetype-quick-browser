import { createHash, randomUUID } from 'node:crypto'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import pg from 'pg'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'
import { createRuntimeDatabase } from '@archetype/db-adapters'
import { DurableWorker } from './worker.js'

const enabled = process.env.RUN_INTEGRATION_TESTS === '1'
const platformUrl = process.env.TEST_PLATFORM_POSTGRES_URL ?? 'postgres://archetype:archetype@localhost:5432/archetype'
const applicationAdminUrl = process.env.TEST_POSTGRES_URL ?? 'postgres://demo:demo@localhost:5433/demo'

describe.skipIf(!enabled)('durable worker delivery lifecycle', () => {
  const suffix = randomUUID().replaceAll('-', '')
  const projectId = `worker-${suffix}`
  const databaseName = `worker_${suffix}`
  const topic = `test.${suffix}.completed`
  const failingTopic = `test.${suffix}.failed`
  const admin = new pg.Pool({ connectionString: applicationAdminUrl })
  const platform = new pg.Pool({ connectionString: platformUrl })
  let directory = ''
  let databaseUrl = ''
  let app: Awaited<ReturnType<typeof createRuntimeDatabase>>
  let worker: DurableWorker

  beforeAll(async () => {
    await admin.query(`CREATE DATABASE ${databaseName}`)
    const parsed = new URL(applicationAdminUrl)
    parsed.pathname = `/${databaseName}`
    databaseUrl = parsed.toString()
    await platform.query('INSERT INTO projects (id, name, allowed_origins) VALUES ($1, $2, $3)', [projectId, 'Worker Integration', ['https://worker.example']])
    directory = await mkdtemp(join(tmpdir(), 'archetype-worker-'))
    const source = `export default {
      functions: [],
      workers: [
        { event: ${JSON.stringify(topic)}, input: { parse: value => value }, handler: async () => undefined },
        { event: ${JSON.stringify(failingTopic)}, input: { parse: value => value }, handler: async () => { throw new Error('planned worker failure') } }
      ]
    }`
    const path = join(directory, 'deployment.mjs')
    await writeFile(path, source)
    await platform.query(
      'INSERT INTO deployments (project_id, path, sha256, operations) VALUES ($1, $2, $3, $4)',
      [projectId, path, createHash('sha256').update(source).digest('hex'), JSON.stringify([])]
    )
    app = await createRuntimeDatabase({ dialect: 'postgres', url: databaseUrl })
    worker = new DurableWorker(projectId, platformUrl, { dialect: 'postgres', url: databaseUrl })
  })

  afterAll(async () => {
    await worker?.close()
    await app?.destroy()
    await platform.query('DELETE FROM dead_letters WHERE project_id = $1', [projectId])
    await platform.query('DELETE FROM runtime_queue WHERE project_id = $1', [projectId])
    await platform.query('DELETE FROM projects WHERE id = $1', [projectId])
    await platform.end()
    await admin.query(`DROP DATABASE ${databaseName} WITH (FORCE)`)
    await admin.end()
    if (directory) await rm(directory, { recursive: true, force: true })
  })

  async function publish(event: string, payload: unknown): Promise<string> {
    return app.transaction(async (transaction) => (await transaction.events.publish(event, payload)).eventId)
  }

  test('moves Outbox events, completes jobs and returns false when the queue is empty', async () => {
    const eventId = await publish(topic, { value: 'ok' })
    expect(await worker.transferOutbox()).toBe(1)
    expect(await worker.consumeOne()).toBe(true)
    const job = await platform.query('SELECT status, attempts, fencing_token FROM runtime_queue WHERE event_id = $1', [eventId])
    expect(job.rows[0]).toMatchObject({ status: 'completed', attempts: 1, fencing_token: '1' })
    expect(await worker.consumeOne()).toBe(false)
  })

  test('retries with a new fencing token and moves exhausted jobs to dead letters', async () => {
    const eventId = await publish(failingTopic, { value: 'fail' })
    expect(await worker.transferOutbox()).toBe(1)
    await platform.query('UPDATE runtime_queue SET max_attempts = 2 WHERE event_id = $1', [eventId])
    expect(await worker.consumeOne()).toBe(true)
    let job = await platform.query('SELECT status, attempts, fencing_token, last_error FROM runtime_queue WHERE event_id = $1', [eventId])
    expect(job.rows[0]).toMatchObject({ status: 'pending', attempts: 1, fencing_token: '1', last_error: 'planned worker failure' })
    await platform.query('UPDATE runtime_queue SET available_at = NOW() WHERE event_id = $1', [eventId])
    expect(await worker.consumeOne()).toBe(true)
    job = await platform.query('SELECT status, attempts, fencing_token FROM runtime_queue WHERE event_id = $1', [eventId])
    expect(job.rows[0]).toMatchObject({ status: 'dead', attempts: 2, fencing_token: '2' })
    const dead = await platform.query('SELECT attempts, error FROM dead_letters WHERE event_id = $1', [eventId])
    expect(dead.rows[0]).toEqual({ attempts: 2, error: 'planned worker failure' })
  })
})
