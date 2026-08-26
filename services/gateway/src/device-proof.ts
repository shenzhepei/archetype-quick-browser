import { createHash, createPublicKey, verify, type JsonWebKey as NodeJsonWebKey } from 'node:crypto'
import { canonicalJson, signingMessage, type SignedInvokeRequest } from '@archetype/protocol'

export function invocationDigest(operation: string, input: unknown, idempotencyKey?: string, timeoutMs?: number): string {
  return createHash('sha256').update(canonicalJson({ operation, input, idempotencyKey: idempotencyKey ?? null, timeoutMs: timeoutMs ?? null })).digest('hex')
}

export function verifyDeviceProof(request: SignedInvokeRequest): boolean {
  const { signature, publicKey, capabilityTicket: _capabilityTicket, ...message } = request
  try {
    const key = createPublicKey({ key: publicKey as NodeJsonWebKey, format: 'jwk' })
    return verify(null, Buffer.from(signingMessage(message)), key, Buffer.from(signature, 'base64url'))
  } catch {
    return false
  }
}
