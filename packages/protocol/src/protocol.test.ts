import { describe, expect, it } from 'vitest'
import { capabilityRequestSchema, runtimeDiscoverySchema, signedInvokeRequestSchema } from './index.js'

describe('runtime discovery', () => {
  it('accepts HTTPS and localhost development gateways', () => {
    expect(runtimeDiscoverySchema.parse({ version: 1, projectId: 'shop', gatewayUrl: 'https://runtime.example.com' }).projectId).toBe('shop')
    expect(runtimeDiscoverySchema.parse({ version: 1, projectId: 'shop', gatewayUrl: 'http://localhost:8787' }).projectId).toBe('shop')
  })

  it('rejects an insecure remote gateway', () => {
    expect(() => runtimeDiscoverySchema.parse({ version: 1, projectId: 'shop', gatewayUrl: 'http://runtime.example.com' })).toThrow()
  })
})

describe('capability-bound invocation protocol', () => {
  it('accepts invoke and subscription capability requests', () => {
    const base = { projectId: 'shop', origin: 'https://shop.example', publicKey: { kty: 'OKP', crv: 'Ed25519', x: 'key' } }
    expect(capabilityRequestSchema.parse({ ...base, kind: 'invoke', operation: 'order.claim' }).kind).toBe('invoke')
    expect(capabilityRequestSchema.parse({ ...base, kind: 'subscribe', topic: 'order.claimed' }).kind).toBe('subscribe')
  })

  it('requires a capability ticket on signed calls', () => {
    const call = {
      projectId: 'shop', origin: 'https://shop.example', operation: 'order.claim', input: {},
      timestamp: Date.now(), nonce: 'a'.repeat(16), publicKey: {}, signature: 'signature', bodyDigest: 'a'.repeat(64)
    }
    expect(() => signedInvokeRequestSchema.parse(call)).toThrow()
    expect(signedInvokeRequestSchema.parse({ ...call, capabilityTicket: 't'.repeat(32) }).capabilityTicket).toHaveLength(32)
  })
})
