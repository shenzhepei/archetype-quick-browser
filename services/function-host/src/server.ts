import { fork } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import Fastify, { type FastifyInstance } from 'fastify'
import type { FunctionInvocation, RunnerResult } from './types.js'
import { TaskPool } from './pool.js'

const serviceToken = process.env.ARCHETYPE_SERVICE_TOKEN ?? 'development-service-token'
const invocationPool = new TaskPool(Number(process.env.ARCHETYPE_FUNCTION_CONCURRENCY ?? 16))

function runChild(invocation: FunctionInvocation): Promise<RunnerResult> {
  const runner = process.env.ARCHETYPE_RUNNER_PATH ?? fileURLToPath(new URL('./runner.js', import.meta.url))
  return new Promise((resolve) => {
    const child = fork(runner, [], {
      stdio: ['ignore', 'inherit', 'inherit', 'ipc'],
      execArgv: [`--max-old-space-size=${Number(process.env.ARCHETYPE_FUNCTION_MEMORY_MB ?? 256)}`]
    })
    const timeout = setTimeout(() => {
      child.kill('SIGKILL')
      resolve({ ok: false, error: { code: 'FUNCTION_TIMEOUT', message: `Function exceeded ${invocation.timeoutMs}ms.` } })
    }, invocation.timeoutMs)
    child.once('message', (message: RunnerResult) => {
      clearTimeout(timeout)
      child.kill()
      resolve(message)
    })
    child.once('error', (error) => {
      clearTimeout(timeout)
      resolve({ ok: false, error: { code: 'FUNCTION_HOST_ERROR', message: error.message } })
    })
    child.send(invocation)
  })
}

export function runIsolated(invocation: FunctionInvocation): Promise<RunnerResult> {
  return invocationPool.run(() => runChild(invocation))
}

export function buildServer(invoke: (invocation: FunctionInvocation) => Promise<RunnerResult> = runIsolated): FastifyInstance {
  const server = Fastify({ logger: { redact: ['req.headers.authorization', 'body.connection.url'] }, bodyLimit: 2 * 1024 * 1024 })
  server.get('/health', async () => ({ ok: true, service: 'archetype-function-host' }))
  server.post('/internal/invoke', async (request, reply) => {
    if (request.headers.authorization !== `Bearer ${serviceToken}`) return reply.status(401).send({ error: { code: 'UNAUTHORIZED', message: 'Invalid service token.' } })
    const result = await invoke(request.body as FunctionInvocation)
    return result.ok ? { result: result.result } : reply.status(result.error?.code === 'INVALID_INPUT' ? 400 : 500).send({ error: result.error })
  })
  return server
}

if (process.env.NODE_ENV !== 'test') {
  void buildServer().listen({ host: process.env.HOST ?? '0.0.0.0', port: Number(process.env.PORT ?? 8790) })
}
