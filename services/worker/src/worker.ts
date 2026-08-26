import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { pathToFileURL } from 'node:url'
import pg from 'pg'
import { createRuntimeDatabase } from '@archetype/db-adapters'
import type { RuntimeDeployment, RuntimeWorker } from '@archetype/function-sdk'

interface QueueJob {
  event_id: string
  project_id: string
  topic: string
  payload: unknown
  attempts: number
  max_attempts: number
  fencing_token: string
}

export function retryDelayMs(attempt: number): number {
  return Math.min(60_000, 500 * 2 ** Math.max(0, attempt - 1))
}

export class DurableWorker {
  private readonly platform: pg.Pool
  private readonly appPromise
  private stopped = false

  constructor(
    private readonly projectId: string,
    platformUrl: string,
    private readonly connection: { dialect: 'postgres' | 'mysql'; url: string }
  ) {
    this.platform = new pg.Pool({ connectionString: platformUrl, max: 6 })
    this.appPromise = createRuntimeDatabase(connection)
  }

  async run(): Promise<void> {
    while (!this.stopped) {
      try {
        await this.transferOutbox()
        await this.consumeOne()
      } catch (error) {
        console.error('Worker cycle failed', error)
      }
      await new Promise((resolve) => setTimeout(resolve, 500))
    }
  }

  stop(): void {
    this.stopped = true
  }

  async close(): Promise<void> {
    this.stop()
    await (await this.appPromise).destroy()
    await this.platform.end()
  }

  async transferOutbox(): Promise<number> {
    const app = await this.appPromise
    return app.db.transaction().execute(async (transaction) => {
      const events = await transaction.selectFrom('_archetype_outbox')
        .select(['event_id', 'topic', 'payload'])
        .where('published_at', 'is', null)
        .orderBy('created_at')
        .limit(20)
        .forUpdate()
        .skipLocked()
        .execute() as Array<{ event_id: string; topic: string; payload: unknown }>
      for (const event of events) {
        const payload = typeof event.payload === 'string' ? JSON.parse(event.payload) : event.payload
        await this.platform.query(
          `INSERT INTO runtime_queue (event_id, project_id, topic, payload)
           VALUES ($1, $2, $3, $4) ON CONFLICT (event_id) DO NOTHING`,
          [event.event_id, this.projectId, event.topic, payload]
        )
        await transaction.updateTable('_archetype_outbox').set({ published_at: new Date() }).where('event_id', '=', event.event_id).execute()
      }
      return events.length
    })
  }

  async consumeOne(): Promise<boolean> {
    const client = await this.platform.connect()
    let job: QueueJob | undefined
    try {
      await client.query('BEGIN')
      const result = await client.query<QueueJob>(`
        WITH candidate AS (
          SELECT event_id FROM runtime_queue
          WHERE project_id = $1 AND status = 'pending' AND available_at <= NOW()
            AND (lease_until IS NULL OR lease_until < NOW())
          ORDER BY created_at
          FOR UPDATE SKIP LOCKED LIMIT 1
        )
        UPDATE runtime_queue q SET
          status = 'running', lease_until = NOW() + INTERVAL '30 seconds',
          fencing_token = fencing_token + 1, attempts = attempts + 1
        FROM candidate WHERE q.event_id = candidate.event_id
        RETURNING q.*
      `, [this.projectId])
      job = result.rows[0]
      await client.query('COMMIT')
    } catch (error) {
      await client.query('ROLLBACK')
      throw error
    } finally {
      client.release()
    }
    if (!job) return false

    try {
      const worker = await this.loadWorker(job.topic)
      if (!worker) throw new Error(`No deployed worker handles ${job.topic}.`)
      const input = worker.input.parse(job.payload)
      const database = await this.appPromise
      await worker.handler({ eventId: job.event_id, projectId: this.projectId, attempt: job.attempts, db: database, log: console }, input)
      await this.platform.query("UPDATE runtime_queue SET status = 'completed', lease_until = NULL WHERE event_id = $1 AND fencing_token = $2", [job.event_id, job.fencing_token])
      return true
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Worker failed.'
      if (job.attempts >= job.max_attempts) {
        const deadLetterClient = await this.platform.connect()
        try {
          await deadLetterClient.query('BEGIN')
          await deadLetterClient.query(
            `INSERT INTO dead_letters (event_id, project_id, topic, payload, attempts, error)
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (event_id) DO NOTHING`,
            [job.event_id, job.project_id, job.topic, job.payload, job.attempts, message]
          )
          await deadLetterClient.query("UPDATE runtime_queue SET status = 'dead', lease_until = NULL, last_error = $2 WHERE event_id = $1 AND fencing_token = $3", [job.event_id, message, job.fencing_token])
          await deadLetterClient.query('COMMIT')
        } catch (transactionError) {
          await deadLetterClient.query('ROLLBACK')
          throw transactionError
        } finally {
          deadLetterClient.release()
        }
      } else {
        await this.platform.query(
          "UPDATE runtime_queue SET status = 'pending', lease_until = NULL, last_error = $2, available_at = NOW() + ($3 * INTERVAL '1 millisecond') WHERE event_id = $1 AND fencing_token = $4",
          [job.event_id, message, retryDelayMs(job.attempts), job.fencing_token]
        )
      }
      return true
    }
  }

  private async loadWorker(topic: string): Promise<RuntimeWorker | undefined> {
    const result = await this.platform.query<{ path: string; sha256: string }>('SELECT path, sha256 FROM deployments WHERE project_id = $1', [this.projectId])
    const deployment = result.rows[0]
    if (!deployment) return undefined
    const bytes = await readFile(deployment.path)
    if (createHash('sha256').update(bytes).digest('hex') !== deployment.sha256) throw new Error('Deployment artifact failed integrity verification.')
    const imported = await import(`${pathToFileURL(deployment.path).href}?sha=${deployment.sha256}`) as { default?: RuntimeDeployment; deployment?: RuntimeDeployment }
    const runtime = imported.default ?? imported.deployment
    return runtime?.workers.find((candidate) => candidate.event === topic)
  }
}

if (process.env.NODE_ENV !== 'test') {
  const projectId = process.env.ARCHETYPE_PROJECT_ID ?? 'order-claim-demo'
  const platformUrl = process.env.PLATFORM_DATABASE_URL ?? 'postgres://archetype:archetype@localhost:5432/archetype'
  const dialect = process.env.APP_DATABASE_DIALECT === 'mysql' ? 'mysql' : 'postgres'
  const url = process.env.APP_DATABASE_URL ?? 'postgres://demo:demo@localhost:5433/demo'
  const worker = new DurableWorker(projectId, platformUrl, { dialect, url })
  process.once('SIGTERM', () => worker.stop())
  process.once('SIGINT', () => worker.stop())
  void worker.run()
}
