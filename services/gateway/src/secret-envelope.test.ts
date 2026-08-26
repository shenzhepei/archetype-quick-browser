import { expect, it } from 'vitest'
import { decryptSecret, encryptSecret } from './secret-envelope.js'

it('uses randomized authenticated envelope encryption', () => {
  const first = encryptSecret('postgres://private', 'master-key')
  const second = encryptSecret('postgres://private', 'master-key')
  expect(first.ciphertext).not.toBe(second.ciphertext)
  expect(decryptSecret(first, 'master-key')).toBe('postgres://private')
  expect(() => decryptSecret(first, 'wrong-key')).toThrow()
})
