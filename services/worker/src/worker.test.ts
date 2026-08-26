import { expect, it } from 'vitest'
import { retryDelayMs } from './worker.js'

it('backs off retries with a fixed upper bound', () => {
  expect(retryDelayMs(1)).toBe(500)
  expect(retryDelayMs(4)).toBe(4_000)
  expect(retryDelayMs(20)).toBe(60_000)
})
