# 数据、事务与可靠任务

Runtime支持PostgreSQL 16+和MySQL 8+。函数通过 `context.db` 使用Kysely，页面不会创建数据库连接。

## 抢单完整教程

先执行 `examples/order-claim/migrations/postgres.sql` 或 `mysql.sql` 创建示例Schema，用 `archetype db add` 绑定数据库，再部署 `examples/order-claim/src/index.ts`。下面直接导入示例和集成测试实际使用的函数：

<<< ../../../../examples/order-claim/src/index.ts#claim-order{ts}

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

事件和业务修改在同一事务写入 `_archetype_outbox`。Dispatcher使用 `SKIP LOCKED` 获取事件，以事件ID去重并写入平台PostgreSQL任务表。

Worker采用至少一次交付，支持租约、fencing token、指数退避和死信。处理器必须用业务唯一键实现幂等，例如 `inventory_reservations.order_id`。

条件更新 `UPDATE ... WHERE status = 'available'` 是并发闸门。数据库在更新时锁定记录；100个并发请求中只会有一个修改一行，其余请求看到修改行数为零。下游库存预约的唯一约束提供第二层不变量。

如果Handler抛错，订单更新和Outbox写入会一起回滚。如果Runtime在事务提交后、分发前停止，未处理的Outbox记录仍然存在，重启后会自动恢复。

<<< ../../../../examples/order-claim/src/index.ts#order-worker{ts}

Worker领取租约并获得单调递增的fencing token，过期Worker不能确认更新后的租约。失败任务按指数退避重新调度，达到 `maxAttempts` 后进入死信表，供检查和人工重放。

网站登录后可以订阅事件。浏览器事件必须包含 `userId` 或 `subject`，Gateway会在投递前按当前会话过滤。

```ts
const unsubscribe = navigator.archetype.subscribe('order.claimed', (event) => {
  console.log(event.orderId)
})
```
