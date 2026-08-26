# Data, transactions, and durable jobs

The Runtime supports PostgreSQL 16+ and MySQL 8+. Functions receive Kysely through `context.db`; page code never creates a database connection.

## Order claim tutorial

Create the example schema with `examples/order-claim/migrations/postgres.sql` or `mysql.sql`, bind that database with `archetype db add`, then deploy `examples/order-claim/src/index.ts`. The actual function used by the example and integration tests is imported below:

<<< ../../../examples/order-claim/src/index.ts#claim-order{ts}

The conditional `UPDATE ... WHERE status = 'available'` is the concurrency gate. The database locks the row while updating it; exactly one of 100 concurrent requests changes one row, and every later request observes zero changed rows. A uniqueness constraint protects downstream inventory reservation as a second invariant.

## Transactional events

```ts
return db.transaction(async (tx) => {
  const result = await tx.db.updateTable('orders')
    .set({ status: 'claimed', claimed_by: user.id })
    .where('id', '=', input.orderId)
    .where('status', '=', 'available')
    .executeTakeFirst()

  if (Number(result.numUpdatedRows) === 1) {
    await tx.events.publish('order.claimed', { orderId: input.orderId })
  }
})
```

The event is inserted into `_archetype_outbox` in the same transaction. A dispatcher leases rows with `SKIP LOCKED`, deduplicates by event ID, and inserts jobs into platform PostgreSQL.

If the handler throws, both the order update and outbox insert roll back. If the Runtime stops after commit but before dispatch, the unprocessed outbox row remains and is recovered after restart.

## Delivery contract

Workers receive at-least-once delivery. Jobs use leases, fencing tokens, exponential retry delays and a final dead-letter record. Worker handlers must enforce idempotency with a unique business key, such as `inventory_reservations.order_id`.

<<< ../../../examples/order-claim/src/index.ts#order-worker{ts}

A worker claims a lease and receives a monotonically increasing fencing token. Stale workers cannot acknowledge a newer lease. A failed job is rescheduled with exponential backoff until `maxAttempts`; the final failure is copied to the dead-letter table for inspection and manual replay.

From the website, subscribe after sign-in. Browser events must include `userId` or `subject`; Gateway filters them to the active session before delivery.

```ts
const unsubscribe = navigator.archetype.subscribe('order.claimed', (event) => {
  console.log(event.orderId)
})
```
