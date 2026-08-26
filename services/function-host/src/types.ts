import type { DatabaseConnection } from '@archetype/db-adapters'
import type { OperationDescriptor } from '@archetype/protocol'

export interface DeploymentRecord {
  projectId: string
  sha256: string
  path: string
  operations: OperationDescriptor[]
}

export interface FunctionInvocation {
  requestId: string
  projectId: string
  origin: string
  operation: string
  input: unknown
  user: { id: string; displayName?: string; claims: Record<string, unknown> } | null
  deployment: DeploymentRecord
  connection: DatabaseConnection
  timeoutMs: number
}

export interface RunnerResult {
  ok: boolean
  result?: unknown
  error?: { code: string; message: string }
}
