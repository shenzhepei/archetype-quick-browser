import { createHash } from 'node:crypto'
import { SignJWT, jwtVerify, type JWTPayload } from 'jose'
import { canonicalJson } from '@archetype/protocol'

export interface CapabilityClaims {
  projectId: string
  origin: string
  resource: string
  deviceKey: string
  subject: string | null
}

const issuer = 'archetype-gateway'
const audience = 'archetype-runtime-browser'

function signingKey(secret: string): Uint8Array {
  return createHash('sha256').update(secret).digest()
}

export function deviceKeyDigest(publicKey: JsonWebKey | Record<string, unknown>): string {
  return createHash('sha256').update(canonicalJson(publicKey)).digest('hex')
}

export async function issueCapability(claims: CapabilityClaims, secret: string, now = new Date()): Promise<{ ticket: string; expiresAt: number }> {
  const issuedAt = Math.floor(now.getTime() / 1000)
  const expiresAt = issuedAt + 60
  const ticket = await new SignJWT({ ...claims })
    .setProtectedHeader({ alg: 'HS256', typ: 'JWT' })
    .setIssuer(issuer)
    .setAudience(audience)
    .setIssuedAt(issuedAt)
    .setExpirationTime(expiresAt)
    .setJti(crypto.randomUUID())
    .sign(signingKey(secret))
  return { ticket, expiresAt: expiresAt * 1000 }
}

export async function verifyCapability(ticket: string, expected: CapabilityClaims, secret: string, now = new Date()): Promise<boolean> {
  try {
    const result = await jwtVerify(ticket, signingKey(secret), { issuer, audience, currentDate: now })
    const payload = result.payload as JWTPayload & Partial<CapabilityClaims>
    return payload.projectId === expected.projectId
      && payload.origin === expected.origin
      && payload.resource === expected.resource
      && payload.deviceKey === expected.deviceKey
      && payload.subject === expected.subject
  } catch {
    return false
  }
}
