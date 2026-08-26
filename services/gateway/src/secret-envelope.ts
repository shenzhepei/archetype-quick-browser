import { createCipheriv, createDecipheriv, createHash, randomBytes } from 'node:crypto'

export interface SecretEnvelope {
  version: 1
  wrappedKey: string
  wrappedKeyIv: string
  wrappedKeyTag: string
  ciphertext: string
  iv: string
  tag: string
}

function keyFromMaster(master: string): Buffer {
  return createHash('sha256').update(master).digest()
}

function encryptWithKey(plaintext: Buffer, key: Buffer) {
  const iv = randomBytes(12)
  const cipher = createCipheriv('aes-256-gcm', key, iv)
  const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final()])
  return { ciphertext, iv, tag: cipher.getAuthTag() }
}

function decryptWithKey(ciphertext: Buffer, key: Buffer, iv: Buffer, tag: Buffer): Buffer {
  const decipher = createDecipheriv('aes-256-gcm', key, iv)
  decipher.setAuthTag(tag)
  return Buffer.concat([decipher.update(ciphertext), decipher.final()])
}

export function encryptSecret(plaintext: string, master: string): SecretEnvelope {
  const dataKey = randomBytes(32)
  const secret = encryptWithKey(Buffer.from(plaintext, 'utf8'), dataKey)
  const wrapped = encryptWithKey(dataKey, keyFromMaster(master))
  return {
    version: 1,
    wrappedKey: wrapped.ciphertext.toString('base64'),
    wrappedKeyIv: wrapped.iv.toString('base64'),
    wrappedKeyTag: wrapped.tag.toString('base64'),
    ciphertext: secret.ciphertext.toString('base64'),
    iv: secret.iv.toString('base64'),
    tag: secret.tag.toString('base64')
  }
}

export function decryptSecret(envelope: SecretEnvelope, master: string): string {
  if (envelope.version !== 1) throw new Error('Unsupported secret envelope version.')
  const dataKey = decryptWithKey(
    Buffer.from(envelope.wrappedKey, 'base64'),
    keyFromMaster(master),
    Buffer.from(envelope.wrappedKeyIv, 'base64'),
    Buffer.from(envelope.wrappedKeyTag, 'base64')
  )
  return decryptWithKey(
    Buffer.from(envelope.ciphertext, 'base64'),
    dataKey,
    Buffer.from(envelope.iv, 'base64'),
    Buffer.from(envelope.tag, 'base64')
  ).toString('utf8')
}
