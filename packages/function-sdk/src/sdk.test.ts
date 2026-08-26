import { expect, it } from 'vitest'
import { z } from 'zod'
import { defineFunction, defineWorker } from './index.js'

it('applies bounded runtime defaults', () => {
  const fn = defineFunction({ name: 'order.read', auth: 'required', input: z.object({ id: z.string() }), handler: async () => ({ ok: true }) })
  const worker = defineWorker({ event: 'order.created', input: z.object({ id: z.string() }), handler: async () => undefined })
  expect(fn.timeoutMs).toBe(30_000)
  expect(worker.maxAttempts).toBe(8)
  expect(Object.isFrozen(fn)).toBe(true)
})
