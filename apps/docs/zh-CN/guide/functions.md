# Node.js云函数

函数以ESM部署包发布，并从自托管Runtime获得可信用户身份和数据库绑定。

## 快速开始

用 `archetype init` 创建项目，导出一个 `RuntimeDeployment`，开发时运行 `archetype dev`。`archetype deploy` 面向Node.js 24打包入口、计算SHA-256并通过控制面上传；Function Host确认摘要一致后才会激活。

```ts
import { z } from 'zod'
import { defineFunction } from '@archetype/function-sdk'

export const readOrder = defineFunction({
  name: 'order.read',
  auth: 'required',
  input: z.object({ orderId: z.string().uuid() }),
  async handler({ user, db }, input) {
    return db.db.selectFrom('orders')
      .select(['id', 'status'])
      .where('id', '=', input.orderId)
      .where('user_id', '=', user!.id)
      .executeTakeFirst()
  }
})
```

浏览器只提交操作名和输入：

```ts
const order = await navigator.archetype.invoke(
  'order.read',
  { orderId },
  { idempotencyKey: crypto.randomUUID() }
)
```

Function Host内部验证输入和输出Schema。上传包的SHA-256与声明一致后才会激活。

`required`要求登录；`optional`允许登录或匿名调用；`anonymous`不要求登录，但仍绑定项目、Origin和设备证明。

## Schema与错误

所有输入和跨越信任边界的输出都应使用Zod。非法输入在Handler运行前被拒绝。抛出的公开错误信息不得包含连接串、Token、堆栈或Secret；这些信息只进入服务端结构化日志。浏览器把运行时失败映射为 `OperationError` 等DOM兼容错误。

## 执行边界

Function Host只执行通过认证CLI部署的产物。每次调用进入有界子进程池，支持超时、取消信号、内存限制、日志捕获和崩溃进程替换。这能隔离函数的意外失败，但不是面向恶意多租户代码的沙箱。
