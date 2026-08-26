# `navigator.archetype` reference

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

`invoke` supports `idempotencyKey`, `timeoutMs`, and `AbortSignal`. Abort stops the browser-side request but does not undo a transaction already committed by the function. Business operations must still be idempotent.

`subscribe` opens an authenticated event stream owned by the Electron main process. A subscription returns an unsubscribe function and is invalidated by navigation. The Gateway delivers only events whose payload `userId` or `subject` matches the signed-in subject; publish one of those fields when an event is intended for a browser subscriber. Worker delivery remains independent and at least once.

Before each invocation or event-stream connection, the browser obtains a 60-second capability ticket bound to the project, trusted origin, operation or topic, signed-in subject, and origin-scoped device public key. The ticket, session token, and device private key never enter page JavaScript. Long-lived subscriptions reconnect with a fresh ticket in the main process.

Errors use DOM-compatible names such as `SecurityError`, `NotSupportedError`, `AbortError`, and `OperationError`; Runtime error messages never include connection strings or tokens.
