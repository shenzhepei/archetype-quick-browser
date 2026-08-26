import { createHash, createPrivateKey, createPublicKey, generateKeyPairSync, randomBytes, sign } from 'node:crypto'
import { readFile, rename, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { app, safeStorage, shell } from 'electron'
import { canonicalJson, projectDescriptorSchema, runtimeDiscoverySchema, sessionSummarySchema, signingMessage, topicSchema, type ProjectDescriptor, type RuntimeDiscovery, type SessionSummary, type SignedInvokeRequest } from '@archetype/protocol'
import { discoveryUrl, eligibleRuntimeOrigin } from '@archetype/browser-runtime'

interface StoredSecrets {
  keys: Record<string, string>
  sessions: Record<string, string>
}

interface DeviceIdentity {
  privateKey: ReturnType<typeof createPrivateKey>
  publicKey: JsonWebKey
}

class SecureStore {
  private data: StoredSecrets = { keys: {}, sessions: {} }
  private readonly file = join(app.getPath('userData'), 'runtime-secrets.json')

  async initialize(): Promise<void> {
    try {
      this.data = JSON.parse(await readFile(this.file, 'utf8')) as StoredSecrets
    } catch {
      this.data = { keys: {}, sessions: {} }
    }
  }

  identity(scope: string): DeviceIdentity {
    const stored = this.data.keys[scope]
    if (stored && safeStorage.isEncryptionAvailable()) {
      const pem = safeStorage.decryptString(Buffer.from(stored, 'base64'))
      const privateKey = createPrivateKey(pem)
      return { privateKey, publicKey: createPublicKey(privateKey).export({ format: 'jwk' }) }
    }
    const pair = generateKeyPairSync('ed25519')
    if (safeStorage.isEncryptionAvailable()) {
      this.data.keys[scope] = safeStorage.encryptString(pair.privateKey.export({ format: 'pem', type: 'pkcs8' }).toString()).toString('base64')
      void this.persist()
    }
    return { privateKey: pair.privateKey, publicKey: pair.publicKey.export({ format: 'jwk' }) }
  }

  session(scope: string): string | undefined {
    const stored = this.data.sessions[scope]
    if (!stored || !safeStorage.isEncryptionAvailable()) return undefined
    return safeStorage.decryptString(Buffer.from(stored, 'base64'))
  }

  setSession(scope: string, token: string | null): void {
    if (token && safeStorage.isEncryptionAvailable()) this.data.sessions[scope] = safeStorage.encryptString(token).toString('base64')
    else delete this.data.sessions[scope]
    void this.persist()
  }

  private async persist(): Promise<void> {
    const temporary = `${this.file}.tmp`
    await writeFile(temporary, JSON.stringify(this.data), { mode: 0o600 })
    await rename(temporary, this.file)
  }
}

export class RuntimeClient {
  private readonly secrets = new SecureStore()
  private readonly controllers = new Map<string, AbortController>()

  async initialize(): Promise<void> {
    await this.secrets.initialize()
  }

  async discover(pageUrl: string): Promise<ProjectDescriptor> {
    const origin = this.requiredOrigin(pageUrl)
    const discoveryResponse = await fetch(discoveryUrl(origin), { headers: { accept: 'application/json' }, signal: AbortSignal.timeout(5_000) })
    if (!discoveryResponse.ok) throw this.error('NotSupportedError', 'This website has not configured Archetype Runtime.')
    const discovery = runtimeDiscoverySchema.parse(await discoveryResponse.json())
    const manifest = await fetch(`${discovery.gatewayUrl}/v1/projects/${encodeURIComponent(discovery.projectId)}/manifest?origin=${encodeURIComponent(origin)}`, { signal: AbortSignal.timeout(5_000) })
    if (!manifest.ok) throw this.error('NotSupportedError', 'The Runtime project does not allow this website origin.')
    return projectDescriptorSchema.parse(await manifest.json())
  }

  async signIn(pageUrl: string): Promise<SessionSummary> {
    const origin = this.requiredOrigin(pageUrl)
    const project = await this.discover(pageUrl)
    const response = await fetch(`${await this.gateway(pageUrl)}/v1/auth/start`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ projectId: project.projectId, origin }) })
    if (!response.ok) throw this.error('OperationError', 'Unable to start the sign-in flow.')
    const { authUrl, pollToken } = await response.json() as { authUrl: string; pollToken: string }
    await shell.openExternal(authUrl)
    const gateway = await this.gateway(pageUrl)
    for (let attempt = 0; attempt < 600; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 1_000))
      const poll = await fetch(`${gateway}/v1/auth/poll?token=${encodeURIComponent(pollToken)}`)
      if (!poll.ok) continue
      const result = await poll.json() as { pending: boolean; token?: string }
      if (!result.pending && result.token) {
        this.secrets.setSession(this.scope(origin, project.projectId), result.token)
        const session = await this.session(pageUrl)
        if (session) return session
      }
    }
    throw this.error('TimeoutError', 'Sign-in was not completed within ten minutes.')
  }

  async signOut(pageUrl: string): Promise<void> {
    const origin = this.requiredOrigin(pageUrl)
    const project = await this.discover(pageUrl)
    const scope = this.scope(origin, project.projectId)
    const token = this.secrets.session(scope)
    if (token) await fetch(`${await this.gateway(pageUrl)}/v1/session`, { method: 'DELETE', headers: { authorization: `Bearer ${token}` } })
    this.secrets.setSession(scope, null)
  }

  async session(pageUrl: string): Promise<SessionSummary | null> {
    const origin = this.requiredOrigin(pageUrl)
    const project = await this.discover(pageUrl)
    const token = this.secrets.session(this.scope(origin, project.projectId))
    if (!token) return null
    const response = await fetch(`${await this.gateway(pageUrl)}/v1/session`, { headers: { authorization: `Bearer ${token}` } })
    if (!response.ok) return null
    return sessionSummarySchema.parse(await response.json())
  }

  async invoke(pageUrl: string, requestId: string, operation: string, input: unknown, options: { idempotencyKey?: string; timeoutMs?: number }): Promise<unknown> {
    const origin = this.requiredOrigin(pageUrl)
    const discovery = runtimeDiscoverySchema.parse(await fetch(discoveryUrl(origin), { signal: AbortSignal.timeout(5_000) }).then((response) => {
      if (!response.ok) throw this.error('NotSupportedError', 'This website has not configured Archetype Runtime.')
      return response.json()
    }))
    const scope = this.scope(origin, discovery.projectId)
    const identity = this.secrets.identity(scope)
    const normalized = { operation, input, idempotencyKey: options.idempotencyKey ?? null, timeoutMs: options.timeoutMs ?? null }
    const bodyDigest = createHash('sha256').update(canonicalJson(normalized)).digest('hex')
    const unsigned: Omit<SignedInvokeRequest, 'signature' | 'publicKey' | 'capabilityTicket'> = {
      projectId: discovery.projectId,
      origin,
      operation,
      input,
      timestamp: Date.now(),
      nonce: randomBytes(24).toString('base64url'),
      bodyDigest,
      ...(options.idempotencyKey ? { idempotencyKey: options.idempotencyKey } : {}),
      ...(options.timeoutMs ? { timeoutMs: options.timeoutMs } : {})
    }
    const signature = sign(null, Buffer.from(signingMessage(unsigned)), identity.privateKey).toString('base64url')
    const controller = new AbortController()
    this.controllers.set(requestId, controller)
    const token = this.secrets.session(scope)
    try {
      const capabilityTicket = await this.capability(discovery, origin, identity.publicKey, token, { kind: 'invoke', operation })
      const response = await fetch(`${discovery.gatewayUrl}/v1/invoke`, {
        method: 'POST',
        headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}) },
        body: JSON.stringify({ ...unsigned, publicKey: identity.publicKey, capabilityTicket, signature }),
        signal: options.timeoutMs ? AbortSignal.any([controller.signal, AbortSignal.timeout(options.timeoutMs)]) : controller.signal
      })
      const payload = await response.json() as { result?: unknown; error?: { code: string; message: string } }
      if (!response.ok) throw this.error(payload.error?.code ?? 'OperationError', payload.error?.message ?? 'Runtime invocation failed.')
      return payload.result
    } finally {
      this.controllers.delete(requestId)
    }
  }

  cancel(requestId: string): void {
    this.controllers.get(requestId)?.abort()
  }

  subscribe(pageUrl: string, topic: string, listener: (payload: unknown) => void): () => void {
    const normalizedTopic = topicSchema.parse(topic)
    const controller = new AbortController()
    void this.streamEvents(pageUrl, normalizedTopic, listener, controller.signal).catch((error) => {
      if (!controller.signal.aborted) console.warn('Runtime event subscription ended.', error instanceof Error ? error.message : error)
    })
    return () => controller.abort()
  }

  private async streamEvents(pageUrl: string, topic: string, listener: (payload: unknown) => void, signal: AbortSignal): Promise<void> {
    while (!signal.aborted) {
      const origin = this.requiredOrigin(pageUrl)
      const discovery = await this.discovery(origin)
      const scope = this.scope(origin, discovery.projectId)
      const identity = this.secrets.identity(scope)
      const token = this.secrets.session(scope)
      if (!token) throw this.error('NotAllowedError', 'Sign in before subscribing to Runtime events.')
      const capabilityTicket = await this.capability(discovery, origin, identity.publicKey, token, { kind: 'subscribe', topic })
      const response = await fetch(`${discovery.gatewayUrl}/v1/events?projectId=${encodeURIComponent(discovery.projectId)}&origin=${encodeURIComponent(origin)}&topic=${encodeURIComponent(topic)}`, {
        headers: {
          accept: 'text/event-stream',
          authorization: `Bearer ${token}`,
          'x-archetype-capability': capabilityTicket,
          'x-archetype-device-key': Buffer.from(JSON.stringify(identity.publicKey)).toString('base64url')
        },
        signal
      })
      if (!response.ok || !response.body) throw this.error('OperationError', `Runtime event stream returned ${response.status}.`)
      await this.readEventStream(response.body, listener, signal)
    }
  }

  private async readEventStream(stream: ReadableStream<Uint8Array>, listener: (payload: unknown) => void, signal: AbortSignal): Promise<void> {
    const reader = stream.getReader()
    const decoder = new TextDecoder()
    let buffer = ''
    try {
      while (!signal.aborted) {
        const chunk = await reader.read()
        if (chunk.done) break
        buffer += decoder.decode(chunk.value, { stream: true }).replaceAll('\r\n', '\n')
        let boundary = buffer.indexOf('\n\n')
        while (boundary >= 0) {
          const block = buffer.slice(0, boundary)
          buffer = buffer.slice(boundary + 2)
          const data = block.split('\n').filter((line) => line.startsWith('data:')).map((line) => line.slice(5).trimStart()).join('\n')
          if (data) listener(JSON.parse(data))
          boundary = buffer.indexOf('\n\n')
        }
      }
    } finally {
      reader.releaseLock()
    }
  }

  private async capability(
    discovery: RuntimeDiscovery,
    origin: string,
    publicKey: JsonWebKey,
    token: string | undefined,
    resource: { kind: 'invoke'; operation: string } | { kind: 'subscribe'; topic: string }
  ): Promise<string> {
    const response = await fetch(`${discovery.gatewayUrl}/v1/capabilities`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}) },
      body: JSON.stringify({ ...resource, projectId: discovery.projectId, origin, publicKey }),
      signal: AbortSignal.timeout(5_000)
    })
    const payload = await response.json() as { ticket?: string; error?: { code: string; message: string } }
    if (!response.ok || !payload.ticket) throw this.error(payload.error?.code ?? 'OperationError', payload.error?.message ?? 'Unable to obtain a Runtime capability.')
    return payload.ticket
  }

  private async discovery(origin: string): Promise<RuntimeDiscovery> {
    const response = await fetch(discoveryUrl(origin), { signal: AbortSignal.timeout(5_000) })
    if (!response.ok) throw this.error('NotSupportedError', 'This website has not configured Archetype Runtime.')
    return runtimeDiscoverySchema.parse(await response.json())
  }

  private async gateway(pageUrl: string): Promise<string> {
    const origin = this.requiredOrigin(pageUrl)
    return runtimeDiscoverySchema.parse(await fetch(discoveryUrl(origin), { signal: AbortSignal.timeout(5_000) }).then((response) => response.json())).gatewayUrl
  }

  private requiredOrigin(pageUrl: string): string {
    const origin = eligibleRuntimeOrigin(pageUrl)
    if (!origin) throw this.error('SecurityError', 'Archetype Runtime requires a top-level HTTPS page or localhost.')
    return origin
  }

  private scope(origin: string, projectId: string): string {
    return createHash('sha256').update(`${origin}\0${projectId}`).digest('hex')
  }

  private error(name: string, message: string): Error {
    return Object.assign(new Error(message), { name })
  }
}
