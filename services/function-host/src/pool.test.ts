import { describe, expect, test } from 'vitest'
import { TaskPool } from './pool.js'

describe('TaskPool', () => {
  test('never runs more than the configured concurrency', async () => {
    const pool = new TaskPool(3)
    let active = 0
    let maximum = 0
    await Promise.all(Array.from({ length: 20 }, () => pool.run(async () => {
      active += 1
      maximum = Math.max(maximum, active)
      await new Promise((resolve) => setTimeout(resolve, 2))
      active -= 1
    })))
    expect(maximum).toBe(3)
  })

  test('continues dispatching after a task rejects', async () => {
    const pool = new TaskPool(1)
    const failed = pool.run(async () => { throw new Error('expected failure') })
    const next = pool.run(async () => 'completed')
    await expect(failed).rejects.toThrow('expected failure')
    await expect(next).resolves.toBe('completed')
  })

  test('rejects invalid concurrency', () => {
    expect(() => new TaskPool(0)).toThrow(RangeError)
  })
})
