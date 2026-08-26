import { generateKeyPairSync } from 'node:crypto'
import { describe, expect, it } from 'vitest'
import { deviceKeyDigest, issueCapability, verifyCapability } from './capability.js'

describe('capability tickets', () => {
  const publicKey = generateKeyPairSync('ed25519').publicKey.export({ format: 'jwk' })
  const claims = {
    projectId: 'order-claim-demo',
    origin: 'https://orders.example',
    resource: 'invoke:order.claim',
    deviceKey: deviceKeyDigest(publicKey),
    subject: 'user-1'
  }

  it('binds a 60 second ticket to project, origin, resource, user and device', async () => {
    const now = new Date('2026-08-26T00:00:00Z')
    const issued = await issueCapability(claims, 'test-secret', now)
    expect(issued.expiresAt).toBe(now.getTime() + 60_000)
    await expect(verifyCapability(issued.ticket, claims, 'test-secret', new Date(now.getTime() + 59_000))).resolves.toBe(true)
    await expect(verifyCapability(issued.ticket, { ...claims, resource: 'invoke:order.list' }, 'test-secret', now)).resolves.toBe(false)
    await expect(verifyCapability(issued.ticket, claims, 'test-secret', new Date(now.getTime() + 61_000))).resolves.toBe(false)
  })
})
