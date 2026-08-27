import { wellKnownPath } from '@archetype/protocol'

export function eligibleRuntimeOrigin(url: string): string | null {
  try {
    const parsed = new URL(url)
    if (parsed.protocol === 'https:') return parsed.origin
    if (parsed.protocol === 'http:' && ['localhost', '127.0.0.1'].includes(parsed.hostname)) return parsed.origin
    return null
  } catch {
    return null
  }
}

export function discoveryUrl(origin: string): string {
  const parsed = new URL(origin)
  return new URL(wellKnownPath, parsed.origin).toString()
}

export function normalizeAddress(input: string): string {
  const value = input.trim()
  if (!value) return 'archetype://newtab'
  if (/^(https?|archetype):/i.test(value)) return value
  if (/^(localhost|127\.0\.0\.1)(:\d+)?(\/|$)/i.test(value)) return `http://${value}`
  if (/^[\w-]+(\.[\w-]+)+(\/.*)?$/i.test(value)) return `https://${value}`
  return `https://www.google.com/search?q=${encodeURIComponent(value)}`
}
