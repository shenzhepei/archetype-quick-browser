import { normalizeExtensionPaths } from '../src/main/extension-paths'

it('normalizes persisted extension directories', () => {
  expect(normalizeExtensionPaths([' /tmp/one ', '', '/tmp/two', '/tmp/one'])).toEqual([
    '/tmp/one',
    '/tmp/two'
  ])
})
