#!/usr/bin/env node
import { createHash } from 'node:crypto'
import { spawn } from 'node:child_process'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { build } from 'esbuild'
import { Command } from 'commander'
import type { RuntimeDeployment } from '@archetype/function-sdk'
import type { OperationDescriptor } from '@archetype/protocol'

const program = new Command()
  .name('archetype')
  .description('Develop and deploy Archetype Runtime applications.')
  .version('1.0.0')

function settings() {
  return {
    gateway: process.env.ARCHETYPE_GATEWAY_URL ?? 'http://localhost:8787',
    token: process.env.ARCHETYPE_ADMIN_TOKEN ?? 'development-admin-token'
  }
}

async function admin(path: string, init: RequestInit = {}): Promise<any> {
  const { gateway, token } = settings()
  const response = await fetch(`${gateway}${path}`, {
    ...init,
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}`, ...init.headers }
  })
  const body = response.status === 204 ? null : await response.json()
  if (!response.ok) throw new Error(body?.error?.message ?? `Gateway returned ${response.status}.`)
  return body
}

function run(command: string, args: string[]): Promise<void> {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, { stdio: 'inherit' })
    child.once('exit', (code) => code === 0 ? resolvePromise() : reject(new Error(`${command} exited with ${code}.`)))
    child.once('error', reject)
  })
}

program.command('doctor').description('Check the local runtime toolchain.').action(async () => {
  console.log(`Node ${process.version}`)
  if (Number(process.versions.node.split('.')[0]) < 24) throw new Error('Node.js 24 or newer is required.')
  await run('docker', ['version'])
  await run('docker-compose', ['version'])
  const response = await fetch(`${settings().gateway}/health`).catch(() => null)
  console.log(response?.ok ? 'Gateway is healthy.' : 'Gateway is not running yet.')
})

program.command('init').argument('[directory]', 'Project directory', '.').description('Create a Runtime application skeleton.').action(async (directory: string) => {
  const root = resolve(directory)
  await mkdir(join(root, 'src', 'functions'), { recursive: true })
  await mkdir(join(root, 'public', '.well-known'), { recursive: true })
  await writeFile(join(root, 'archetype.config.json'), JSON.stringify({ projectId: 'replace-after-project-create', functionEntry: 'src/functions/index.ts' }, null, 2) + '\n', { flag: 'wx' })
  console.log(`Created ${root}`)
})

const runtime = program.command('runtime')
runtime.command('up').description('Start the self-hosted Docker runtime.').action(() => run('docker-compose', ['-f', 'infra/docker/compose.yml', 'up', '--build']))

const project = program.command('project')
project.command('create').requiredOption('--name <name>').action(async ({ name }) => console.log(JSON.stringify(await admin('/v1/admin/projects', { method: 'POST', body: JSON.stringify({ name }) }), null, 2)))

const origin = program.command('origin')
origin.command('add').requiredOption('--project <id>').requiredOption('--origin <origin>').action(async (options) => {
  console.log(JSON.stringify(await admin(`/v1/admin/projects/${options.project}/origins`, { method: 'POST', body: JSON.stringify({ origin: options.origin }) }), null, 2))
})

const db = program.command('db')
db.command('add').requiredOption('--project <id>').requiredOption('--dialect <dialect>', 'postgres or mysql').description('Bind the URL from ARCHETYPE_DATABASE_URL without placing it in shell history.').action(async (options) => {
  const databaseUrl = process.env.ARCHETYPE_DATABASE_URL
  if (!databaseUrl) throw new Error('Set ARCHETYPE_DATABASE_URL before running db add.')
  await admin(`/v1/admin/projects/${options.project}/database`, { method: 'PUT', body: JSON.stringify({ dialect: options.dialect, databaseUrl }) })
  console.log(`Configured ${options.dialect} for ${options.project}.`)
})

program.command('deploy').requiredOption('--project <id>').requiredOption('--entry <file>').description('Bundle and activate a signed-by-digest function deployment.').action(async (options) => {
  const entry = resolve(options.entry)
  const output = join(tmpdir(), `archetype-${basename(entry).replace(/\W+/g, '-')}-${Date.now()}.mjs`)
  await build({ entryPoints: [entry], outfile: output, platform: 'node', target: 'node24', format: 'esm', bundle: true, sourcemap: false, minify: false })
  const imported = await import(`${pathToFileURL(output).href}?t=${Date.now()}`) as { default?: RuntimeDeployment; deployment?: RuntimeDeployment }
  const deployment = imported.default ?? imported.deployment
  if (!deployment?.functions || !deployment.workers) throw new Error('Entry must export a RuntimeDeployment as default or deployment.')
  const operations: OperationDescriptor[] = deployment.functions.map((fn) => ({ name: fn.name, auth: fn.auth, timeoutMs: fn.timeoutMs }))
  const bytes = await readFile(output)
  const sha256 = createHash('sha256').update(bytes).digest('hex')
  const response = await admin(`/v1/admin/projects/${options.project}/deployments`, { method: 'POST', body: JSON.stringify({ sha256, artifact: bytes.toString('base64'), operations }) })
  console.log(JSON.stringify(response, null, 2))
})

program.command('logs').requiredOption('--project <id>').action(async ({ project }) => console.log(JSON.stringify(await admin(`/v1/admin/projects/${project}/logs`), null, 2)))

const wellKnown = program.command('well-known')
wellKnown.command('generate').requiredOption('--project <id>').requiredOption('--gateway <url>').option('--output <file>', 'Output path', 'public/.well-known/archetype-runtime.json').action(async (options) => {
  const output = resolve(options.output)
  await mkdir(resolve(output, '..'), { recursive: true })
  await writeFile(output, JSON.stringify({ version: 1, projectId: options.project, gatewayUrl: options.gateway }, null, 2) + '\n')
  console.log(`Wrote ${output}`)
})

program.command('dev').description('Run Gateway, Function Host and Worker source processes.').action(async () => {
  await run('pnpm', ['dev:runtime'])
})

program.parseAsync().catch((error) => {
  console.error(error instanceof Error ? error.message : error)
  process.exitCode = 1
})
