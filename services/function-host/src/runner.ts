import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { pathToFileURL } from 'node:url'
import { createRuntimeDatabase } from '@archetype/db-adapters'
import type { RuntimeDeployment } from '@archetype/function-sdk'
import type { FunctionInvocation, RunnerResult } from './types.js'

export async function executeInvocation(invocation: FunctionInvocation): Promise<RunnerResult> {
  const bytes = await readFile(invocation.deployment.path)
  if (createHash('sha256').update(bytes).digest('hex') !== invocation.deployment.sha256) {
    return { ok: false, error: { code: 'DEPLOYMENT_TAMPERED', message: 'The deployed function artifact failed integrity verification.' } }
  }
  const imported = await import(`${pathToFileURL(invocation.deployment.path).href}?sha=${invocation.deployment.sha256}`) as { default?: RuntimeDeployment; deployment?: RuntimeDeployment }
  const deployment = imported.default ?? imported.deployment
  const fn = deployment?.functions?.find((candidate) => candidate.name === invocation.operation)
  if (!fn) return { ok: false, error: { code: 'OPERATION_NOT_FOUND', message: 'The requested function is not exported by this deployment.' } }
  if (fn.auth === 'required' && !invocation.user) return { ok: false, error: { code: 'AUTH_REQUIRED', message: 'This function requires an authenticated user.' } }

  const parsed = fn.input.safeParse(invocation.input)
  if (!parsed.success) return { ok: false, error: { code: 'INVALID_INPUT', message: parsed.error.issues.map((issue) => `${issue.path.join('.') || 'input'}: ${issue.message}`).join('; ') } }
  const database = await createRuntimeDatabase(invocation.connection)
  try {
    const result = await fn.handler({
      requestId: invocation.requestId,
      projectId: invocation.projectId,
      origin: invocation.origin,
      user: invocation.user,
      db: database,
      log: console
    }, parsed.data)
    if (fn.output) {
      const output = fn.output.safeParse(result)
      if (!output.success) return { ok: false, error: { code: 'INVALID_OUTPUT', message: 'Function output did not match its declared schema.' } }
      return { ok: true, result: output.data }
    }
    return { ok: true, result }
  } catch (error) {
    return { ok: false, error: { code: 'FUNCTION_FAILED', message: error instanceof Error ? error.message : 'Function execution failed.' } }
  } finally {
    await database.destroy()
  }
}

if (process.env.NODE_ENV !== 'test') {
  process.on('message', (invocation: FunctionInvocation) => {
    void executeInvocation(invocation).then((result) => {
      if (process.send) process.send(result)
    })
  })
}
