import { baseSiteInfo } from '../src/shared/browser'

it('classifies network and internal URLs without claiming unverified HTTPS is secure', () => {
  expect(baseSiteInfo('https://example.com').connection).toBe('verifying')
  expect(baseSiteInfo('http://example.com').connection).toBe('insecure')
  expect(baseSiteInfo('archetype://settings/appearance').connection).toBe('internal')
  expect(baseSiteInfo('file:///tmp/page.html').connection).toBe('local')
  expect(baseSiteInfo('about:blank').connection).toBe('none')
})
