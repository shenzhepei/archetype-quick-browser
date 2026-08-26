import { z } from 'zod'

export const runtimeDiscoverySchema = z.object({
  version: z.literal(1),
  projectId: z.string().min(3).max(128),
  gatewayUrl: z.url().refine((value) => value.startsWith('https://') || /^http:\/\/(localhost|127\.0\.0\.1)(:\d+)?$/.test(value), 'Gateway must use HTTPS outside localhost.')
})

export const operationDescriptorSchema = z.object({
  name: z.string().regex(/^[a-z][a-z0-9.-]{1,127}$/),
  auth: z.enum(['required', 'optional', 'anonymous']),
  timeoutMs: z.number().int().min(100).max(120_000)
})

export const projectDescriptorSchema = z.object({
  version: z.literal(1),
  projectId: z.string(),
  name: z.string(),
  origin: z.url(),
  operations: z.array(operationDescriptorSchema)
})

export const sessionSummarySchema = z.object({
  authenticated: z.boolean(),
  subject: z.string().optional(),
  displayName: z.string().optional(),
  expiresAt: z.number().int().optional()
})

export const invokeRequestSchema = z.object({
  operation: operationDescriptorSchema.shape.name,
  input: z.unknown(),
  idempotencyKey: z.string().min(8).max(200).optional(),
  timeoutMs: z.number().int().min(100).max(120_000).optional()
})

export const topicSchema = z.string().regex(/^[a-z][a-z0-9.-]{1,127}$/)

export const capabilityRequestSchema = z.discriminatedUnion('kind', [
  z.object({
    kind: z.literal('invoke'),
    projectId: z.string().min(3).max(128),
    origin: z.url(),
    operation: operationDescriptorSchema.shape.name,
    publicKey: z.record(z.string(), z.unknown())
  }),
  z.object({
    kind: z.literal('subscribe'),
    projectId: z.string().min(3).max(128),
    origin: z.url(),
    topic: topicSchema,
    publicKey: z.record(z.string(), z.unknown())
  })
])

export const signedInvokeRequestSchema = invokeRequestSchema.extend({
  projectId: z.string(),
  origin: z.url(),
  timestamp: z.number().int(),
  nonce: z.string().min(16).max(200),
  publicKey: z.record(z.string(), z.unknown()),
  capabilityTicket: z.string().min(32),
  signature: z.string(),
  bodyDigest: z.string().regex(/^[a-f0-9]{64}$/)
})

export const wellKnownPath = '/.well-known/archetype-runtime.json'

export function canonicalJson(value: unknown): string {
  if (value === null || typeof value !== 'object') return JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  const record = value as Record<string, unknown>
  return `{${Object.keys(record).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`).join(',')}}`
}

export function signingMessage(request: Omit<SignedInvokeRequest, 'signature' | 'publicKey' | 'capabilityTicket'>): string {
  return canonicalJson(request)
}

export type RuntimeDiscovery = z.infer<typeof runtimeDiscoverySchema>
export type OperationDescriptor = z.infer<typeof operationDescriptorSchema>
export type ProjectDescriptor = z.infer<typeof projectDescriptorSchema>
export type SessionSummary = z.infer<typeof sessionSummarySchema>
export type InvokeRequest = z.infer<typeof invokeRequestSchema>
export type CapabilityRequest = z.infer<typeof capabilityRequestSchema>
export type SignedInvokeRequest = z.infer<typeof signedInvokeRequestSchema>

export interface RuntimeErrorShape {
  code: string
  message: string
  requestId?: string
  retryable?: boolean
}

export interface InvokeOptions {
  idempotencyKey?: string
  timeoutMs?: number
  signal?: AbortSignal
}

export interface ArchetypeRuntime {
  discover(): Promise<ProjectDescriptor>
  signIn(): Promise<SessionSummary>
  signOut(): Promise<void>
  session(): Promise<SessionSummary | null>
  invoke<T>(operation: string, input: unknown, options?: InvokeOptions): Promise<T>
  subscribe<T>(topic: string, listener: (event: T) => void): () => void
}
