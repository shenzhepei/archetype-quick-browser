import { randomUUID } from 'node:crypto'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'
import { sql } from 'kysely'
import { createRuntimeDatabase, type DatabaseConnection } from '@archetype/db-adapters'
import type { RuntimeDatabase } from '@archetype/function-sdk'
import { claimOrder } from './index.js'

const enabled = process.env.RUN_INTEGRATION_TESTS === '1'
const connections: DatabaseConnection[] = [
  { dialect: 'postgres', url: process.env.TEST_POSTGRES_URL ?? 'postgres://demo:demo@localhost:5433/demo' },
  { dialect: 'mysql', url: process.env.TEST_MYSQL_URL ?? 'mysql://demo:demo@localhost:3307/demo' }
]

describe.skipIf(!enabled)('atomic order claims', () => {
  const databases: Array<RuntimeDatabase & { destroy(): Promise<void> }> = []

  beforeAll(async () => {
    for (const connection of connections) {
      const database = await createRuntimeDatabase(connection)
      databases.push(database)
      if (connection.dialect === 'postgres') {
        await sql`CREATE TABLE IF NOT EXISTS orders (
          id UUID PRIMARY KEY, status TEXT NOT NULL, claimed_by TEXT,
          claimed_at TIMESTAMPTZ, created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )`.execute(database.db)
      } else {
        await sql`CREATE TABLE IF NOT EXISTS orders (
          id CHAR(36) PRIMARY KEY, status VARCHAR(20) NOT NULL, claimed_by VARCHAR(190),
          claimed_at TIMESTAMP(3) NULL, created_at TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3)
        )`.execute(database.db)
      }
    }
  }, 30_000)

  afterAll(async () => Promise.all(databases.map((database) => database.destroy())))

  test.each(connections.map((connection, index) => [connection.dialect, index] as const))('%s allows exactly one of 100 claimants', async (_dialect, index) => {
    const database = databases[index]!
    const orderId = randomUUID()
    await database.db.insertInto('orders').values({ id: orderId, status: 'available' }).execute()

    const results = await Promise.all(Array.from({ length: 100 }, (_, claimant) => claimOrder.handler({
      requestId: randomUUID(),
      projectId: 'integration-test',
      origin: 'https://example.test',
      user: { id: `user-${claimant}`, claims: {} },
      db: database,
      log: console
    }, { orderId })))

    expect(results.filter((result) => result.success)).toHaveLength(1)
    const row = await database.db.selectFrom('orders').select(['status', 'claimed_by']).where('id', '=', orderId).executeTakeFirstOrThrow() as { status: string; claimed_by: string | null }
    expect(row.status).toBe('claimed')
    expect(row.claimed_by).toBe(results.find((result) => result.success)?.claimedBy)
    const outbox = await database.db.selectFrom('_archetype_outbox').select(({ fn }) => fn.countAll<number>().as('count')).where('topic', '=', 'order.claimed').executeTakeFirstOrThrow() as { count: number | string }
    expect(Number(outbox.count)).toBeGreaterThanOrEqual(1)
  }, 30_000)

  test.each(connections.map((connection, index) => [connection.dialect, index] as const))('%s rolls outbox events back with business data', async (_dialect, index) => {
    const database = databases[index]!
    const eventId = randomUUID()
    await expect(database.transaction(async (transaction) => {
      await transaction.events.publish(`rollback.${eventId}`, { eventId })
      throw new Error('rollback requested')
    })).rejects.toThrow('rollback requested')
    const rows = await database.db.selectFrom('_archetype_outbox').select('event_id').where('topic', '=', `rollback.${eventId}`).execute()
    expect(rows).toHaveLength(0)
  })
})
