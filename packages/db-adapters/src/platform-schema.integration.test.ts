import pg from 'pg'
import { describe, expect, test } from 'vitest'
import { initializePlatformSchema } from './platform-schema.js'

const enabled = process.env.RUN_INTEGRATION_TESTS === '1'
const platformUrl = process.env.TEST_PLATFORM_POSTGRES_URL ?? 'postgres://archetype:archetype@localhost:5432/archetype'

describe.skipIf(!enabled)('platform schema initialization', () => {
  test('serializes concurrent initializers and creates every platform table', async () => {
    const pools = Array.from({ length: 8 }, () => new pg.Pool({ connectionString: platformUrl, max: 1 }))
    try {
      await Promise.all(pools.map((pool) => initializePlatformSchema(pool)))
      const result = await pools[0]!.query<{ name: string | null }>(`
        SELECT to_regclass(name)::text AS name
        FROM unnest(ARRAY[
          'projects', 'connections', 'deployments', 'sessions', 'replay_nonces',
          'idempotency', 'audit_log', 'runtime_queue', 'dead_letters'
        ]) AS name
      `)
      expect(result.rows.map((row) => row.name)).not.toContain(null)
    } finally {
      await Promise.all(pools.map((pool) => pool.end()))
    }
  })
})
