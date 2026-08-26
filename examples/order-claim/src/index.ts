import { sql } from 'kysely'
import { z } from 'zod'
import { defineFunction, defineWorker, type RuntimeDeployment } from '@archetype/function-sdk'

const claimInput = z.object({ orderId: z.string().uuid() })
const claimOutput = z.object({ success: z.boolean(), orderId: z.string().uuid(), claimedBy: z.string().nullable() })

// #region claim-order
export const claimOrder = defineFunction({
  name: 'order.claim',
  auth: 'required',
  timeoutMs: 10_000,
  input: claimInput,
  output: claimOutput,
  async handler({ user, db }, input) {
    if (!user) throw new Error('Authenticated user is required.')
    return db.transaction(async (transaction) => {
      const result = await transaction.db.updateTable('orders')
        .set({ status: 'claimed', claimed_by: user.id, claimed_at: new Date() })
        .where('id', '=', input.orderId)
        .where('status', '=', 'available')
        .executeTakeFirst()
      const success = Number(result.numUpdatedRows) === 1
      if (success) await transaction.events.publish('order.claimed', { orderId: input.orderId, userId: user.id })
      return { success, orderId: input.orderId, claimedBy: success ? user.id : null }
    })
  }
})
// #endregion claim-order

export const listOrders = defineFunction({
  name: 'order.list',
  auth: 'optional',
  input: z.object({}),
  output: z.array(z.object({ id: z.string(), status: z.string(), claimedBy: z.string().nullable() })),
  async handler({ db }) {
    const rows = await db.db.selectFrom('orders').select(['id', 'status', 'claimed_by']).orderBy('created_at', 'desc').limit(50).execute() as Array<{ id: string; status: string; claimed_by: string | null }>
    return rows.map((row) => ({ id: row.id, status: row.status, claimedBy: row.claimed_by }))
  }
})

// #region order-worker
export const allocateInventory = defineWorker({
  event: 'order.claimed',
  maxAttempts: 8,
  input: z.object({ orderId: z.string().uuid(), userId: z.string() }),
  async handler({ db }, event) {
    if (db.dialect === 'postgres') {
      await sql`INSERT INTO inventory_reservations (order_id, user_id)
        VALUES (${event.orderId}, ${event.userId})
        ON CONFLICT (order_id) DO NOTHING`.execute(db.db)
    } else {
      await sql`INSERT IGNORE INTO inventory_reservations (order_id, user_id)
        VALUES (${event.orderId}, ${event.userId})`.execute(db.db)
    }
  }
})
// #endregion order-worker

const deployment: RuntimeDeployment = {
  functions: [claimOrder, listOrders],
  workers: [allocateInventory]
}

export default deployment
