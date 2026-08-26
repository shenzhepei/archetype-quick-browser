import { randomUUID } from 'node:crypto'
import { Kysely, MysqlDialect, PostgresDialect, sql, type Transaction } from 'kysely'
import { createPool as createMysqlPool } from 'mysql2'
import pg from 'pg'
import type { DatabaseDialect, RuntimeDatabase, RuntimeTransaction } from '@archetype/function-sdk'

type DatabaseShape = any

export interface DatabaseConnection {
  dialect: DatabaseDialect
  url: string
}

export async function ensureRuntimeTables(database: Kysely<DatabaseShape>, dialect: DatabaseDialect): Promise<void> {
  if (dialect === 'postgres') {
    await sql`CREATE TABLE IF NOT EXISTS _archetype_outbox (
      event_id UUID PRIMARY KEY,
      topic TEXT NOT NULL,
      payload JSONB NOT NULL,
      created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      published_at TIMESTAMPTZ,
      attempts INTEGER NOT NULL DEFAULT 0
    )`.execute(database)
  } else {
    await sql`CREATE TABLE IF NOT EXISTS _archetype_outbox (
      event_id CHAR(36) PRIMARY KEY,
      topic VARCHAR(190) NOT NULL,
      payload JSON NOT NULL,
      created_at TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
      published_at TIMESTAMP(3) NULL,
      attempts INTEGER NOT NULL DEFAULT 0
    )`.execute(database)
  }
}

function transactionFacade(dialect: DatabaseDialect, transaction: Transaction<DatabaseShape>): RuntimeTransaction {
  return {
    db: transaction,
    events: {
      async publish(topic, payload) {
        const eventId = randomUUID()
        await transaction.insertInto('_archetype_outbox').values({
          event_id: eventId,
          topic,
          payload: dialect === 'postgres' ? JSON.stringify(payload) : JSON.stringify(payload)
        }).execute()
        return { eventId }
      }
    }
  }
}

export async function createRuntimeDatabase(connection: DatabaseConnection): Promise<RuntimeDatabase & { destroy(): Promise<void> }> {
  const dialect = connection.dialect === 'postgres'
    ? new PostgresDialect({ pool: new pg.Pool({ connectionString: connection.url, max: 10 }) })
    : new MysqlDialect({ pool: createMysqlPool({ uri: connection.url, connectionLimit: 10 }) })
  const db = new Kysely<DatabaseShape>({ dialect })
  await ensureRuntimeTables(db, connection.dialect)
  return {
    dialect: connection.dialect,
    db,
    transaction: (callback) => db.transaction().execute((transaction) => callback(transactionFacade(connection.dialect, transaction))),
    destroy: () => db.destroy()
  }
}
