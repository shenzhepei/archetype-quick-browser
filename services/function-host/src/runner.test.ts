import { createHash, randomUUID } from 'node:crypto'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, beforeAll, describe, expect, test, vi } from 'vitest'
import type { FunctionInvocation } from './types.js'

const runtimeDatabase = vi.hoisted(() => ({ dialect: 'postgres', db: {}, transaction: vi.fn(), destroy: vi.fn(async () => undefined) }))
vi.mock('@archetype/db-adapters', () => ({ createRuntimeDatabase: vi.fn(async () => runtimeDatabase) }))

import { executeInvocation } from './runner.js'

describe('isolated function runner', () => {
  let directory = ''

  beforeAll(async () => { directory = await mkdtemp(join(tmpdir(), 'archetype-runner-')) })
  afterAll(async () => rm(directory, { recursive: true, force: true }))

  async function invocation(definition: string, overrides: Partial<FunctionInvocation> = {}): Promise<FunctionInvocation> {
    const path = join(directory, `${randomUUID()}.mjs`)
    const source = `export default { workers: [], functions: [${definition}] }`
    await writeFile(path, source)
    return {
      requestId: 'request-1', projectId: 'project-1', origin: 'https://example.test', operation: 'demo.run', input: { value: 2 },
      user: { id: 'user-1', claims: {} }, connection: { dialect: 'postgres', url: 'postgres://unused' }, timeoutMs: 1_000,
      deployment: { projectId: 'project-1', path, sha256: createHash('sha256').update(source).digest('hex'), operations: [] },
      ...overrides
    }
  }

  test('rejects a deployment whose bytes no longer match its digest', async () => {
    const call = await invocation(`{ name:'demo.run', auth:'anonymous' }`)
    call.deployment.sha256 = '0'.repeat(64)
    expect(await executeInvocation(call)).toMatchObject({ ok: false, error: { code: 'DEPLOYMENT_TAMPERED' } })
  })

  test('rejects missing operations, missing identity and invalid input', async () => {
    const missing = await invocation(`{ name:'other.run', auth:'anonymous' }`)
    expect((await executeInvocation(missing)).error?.code).toBe('OPERATION_NOT_FOUND')
    const secured = await invocation(`{ name:'demo.run', auth:'required', input:{safeParse:v=>({success:true,data:v})}, handler:async()=>null }`, { user: null })
    expect((await executeInvocation(secured)).error?.code).toBe('AUTH_REQUIRED')
    const invalid = await invocation(`{ name:'demo.run', auth:'anonymous', input:{safeParse:()=>({success:false,error:{issues:[{path:['value'],message:'must be positive'}]}})}, handler:async()=>null }`)
    expect(await executeInvocation(invalid)).toMatchObject({ ok: false, error: { code: 'INVALID_INPUT', message: 'value: must be positive' } })
  })

  test('validates output, returns successful output and closes the database', async () => {
    runtimeDatabase.destroy.mockClear()
    const invalidOutput = await invocation(`{ name:'demo.run', auth:'anonymous', input:{safeParse:v=>({success:true,data:v})}, output:{safeParse:()=>({success:false})}, handler:async()=>({bad:true}) }`)
    expect((await executeInvocation(invalidOutput)).error?.code).toBe('INVALID_OUTPUT')
    const success = await invocation(`{ name:'demo.run', auth:'anonymous', input:{safeParse:v=>({success:true,data:v})}, output:{safeParse:v=>({success:true,data:v})}, handler:async(ctx,input)=>({value:input.value+1,projectId:ctx.projectId}) }`)
    expect(await executeInvocation(success)).toEqual({ ok: true, result: { value: 3, projectId: 'project-1' } })
    expect(runtimeDatabase.destroy).toHaveBeenCalledTimes(2)
  })

  test('normalizes thrown and non-Error function failures', async () => {
    const thrown = await invocation(`{ name:'demo.run', auth:'anonymous', input:{safeParse:v=>({success:true,data:v})}, handler:async()=>{throw new Error('planned failure')} }`)
    expect(await executeInvocation(thrown)).toMatchObject({ ok: false, error: { code: 'FUNCTION_FAILED', message: 'planned failure' } })
    const nonError = await invocation(`{ name:'demo.run', auth:'anonymous', input:{safeParse:v=>({success:true,data:v})}, handler:async()=>{throw 'failed'} }`)
    expect((await executeInvocation(nonError)).error?.message).toBe('Function execution failed.')
  })
})
