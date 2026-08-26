import type { Kysely, Transaction } from 'kysely'
import type { ZodType } from 'zod'

export type FunctionAuth = 'required' | 'optional' | 'anonymous'
export type DatabaseDialect = 'postgres' | 'mysql'

export interface TrustedUser {
  id: string
  displayName?: string
  claims: Record<string, unknown>
}

export interface EventPublisher {
  publish(topic: string, payload: unknown): Promise<{ eventId: string }>
}

export interface RuntimeTransaction {
  db: Transaction<any>
  events: EventPublisher
}

export interface RuntimeDatabase {
  dialect: DatabaseDialect
  db: Kysely<any>
  transaction<T>(callback: (transaction: RuntimeTransaction) => Promise<T>): Promise<T>
}

export interface FunctionContext {
  requestId: string
  projectId: string
  origin: string
  user: TrustedUser | null
  db: RuntimeDatabase
  log: Pick<Console, 'info' | 'warn' | 'error'>
}

export interface WorkerContext {
  eventId: string
  projectId: string
  attempt: number
  db: RuntimeDatabase
  log: Pick<Console, 'info' | 'warn' | 'error'>
}

export interface RuntimeFunction<Input = unknown, Output = unknown> {
  kind: 'function'
  name: string
  auth: FunctionAuth
  timeoutMs: number
  input: ZodType<Input>
  output?: ZodType<Output>
  handler(context: FunctionContext, input: Input): Promise<Output>
}

export interface RuntimeWorker<Event = unknown> {
  kind: 'worker'
  event: string
  maxAttempts: number
  input: ZodType<Event>
  handler(context: WorkerContext, event: Event): Promise<void>
}

export function defineFunction<Input, Output>(definition: Omit<RuntimeFunction<Input, Output>, 'kind' | 'timeoutMs'> & { timeoutMs?: number }): RuntimeFunction<Input, Output> {
  return Object.freeze({ ...definition, kind: 'function' as const, timeoutMs: definition.timeoutMs ?? 30_000 })
}

export function defineWorker<Event>(definition: Omit<RuntimeWorker<Event>, 'kind' | 'maxAttempts'> & { maxAttempts?: number }): RuntimeWorker<Event> {
  return Object.freeze({ ...definition, kind: 'worker' as const, maxAttempts: definition.maxAttempts ?? 8 })
}

export interface RuntimeDeployment {
  functions: ReadonlyArray<RuntimeFunction<any, any>>
  workers: ReadonlyArray<RuntimeWorker<any>>
}
