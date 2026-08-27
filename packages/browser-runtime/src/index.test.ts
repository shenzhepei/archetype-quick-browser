import { expect, it } from 'vitest'
import { discoveryUrl, eligibleRuntimeOrigin, normalizeAddress } from './index.js'

it('restricts Runtime injection to secure top-level origins and localhost', () => {
  expect(eligibleRuntimeOrigin('https://shop.example/path')).toBe('https://shop.example')
  expect(eligibleRuntimeOrigin('http://localhost:4173')).toBe('http://localhost:4173')
  expect(eligibleRuntimeOrigin('http://shop.example')).toBeNull()
  expect(eligibleRuntimeOrigin('file:///tmp/app.html')).toBeNull()
})

it('builds discovery and navigation URLs', () => {
  expect(discoveryUrl('https://shop.example')).toBe('https://shop.example/.well-known/archetype-runtime.json')
  expect(normalizeAddress('  ')).toBe('archetype://newtab')
  expect(normalizeAddress('shop.example/orders')).toBe('https://shop.example/orders')
  expect(normalizeAddress('runtime functions')).toContain('google.com/search')
})
