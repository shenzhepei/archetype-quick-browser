import { describe, expect, test, vi } from 'vitest'
import { buildServer } from './server.js'
import type { FunctionInvocation } from './types.js'

const invocation = { requestId: 'request-1' } as FunctionInvocation

describe('Function Host HTTP boundary', () => {
  test('reports health and rejects calls without the service credential', async () => {
    const invoke = vi.fn()
    const server = buildServer(invoke)
    expect((await server.inject({ method: 'GET', url: '/health' })).json()).toEqual({ ok: true, service: 'archetype-function-host' })
    const response = await server.inject({ method: 'POST', url: '/internal/invoke', payload: invocation })
    expect(response.statusCode).toBe(401)
    expect(invoke).not.toHaveBeenCalled()
  })

  test.each([
    [{ ok: true, result: { accepted: true } }, 200],
    [{ ok: false, error: { code: 'INVALID_INPUT', message: 'bad input' } }, 400],
    [{ ok: false, error: { code: 'FUNCTION_FAILED', message: 'failed' } }, 500]
  ] as const)('maps isolated runner result %# to HTTP', async (result, status) => {
    const invoke = vi.fn(async () => result)
    const response = await buildServer(invoke).inject({
      method: 'POST',
      url: '/internal/invoke',
      headers: { authorization: 'Bearer development-service-token' },
      payload: invocation
    })
    expect(response.statusCode).toBe(status)
    expect(invoke).toHaveBeenCalledWith(invocation)
  })
})
