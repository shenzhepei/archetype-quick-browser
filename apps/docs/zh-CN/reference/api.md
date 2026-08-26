# `navigator.archetype` 参考

```ts
interface ArchetypeRuntime {
  discover(): Promise<ProjectDescriptor>
  signIn(): Promise<SessionSummary>
  signOut(): Promise<void>
  session(): Promise<SessionSummary | null>
  invoke<T>(operation: string, input: unknown, options?: InvokeOptions): Promise<T>
  subscribe<T>(topic: string, listener: (event: T) => void): () => void
}
```

`invoke`支持 `idempotencyKey`、`timeoutMs` 和 `AbortSignal`。Abort会停止浏览器侧请求，但不能撤销函数已经提交的事务，业务操作仍须保证幂等。

`subscribe`通过Electron主进程建立已认证事件流，返回取消订阅函数，并在导航时失效。Gateway只投递Payload中 `userId` 或 `subject` 与当前登录用户一致的事件；需要浏览器订阅的事件必须发布其中一个字段。Worker投递与此独立，语义仍为至少一次。

每次调用或建立事件流之前，浏览器都会获取60秒能力票据。票据绑定项目、可信Origin、操作或Topic、登录用户和Origin级设备公钥。票据、会话Token和设备私钥均不进入页面JavaScript；长连接由主进程使用新票据自动重连。

错误使用 `SecurityError`、`NotSupportedError`、`AbortError` 和 `OperationError` 等DOM兼容名称，不在错误文本中包含连接串或令牌。
