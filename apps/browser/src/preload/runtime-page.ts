import { contextBridge, ipcRenderer } from 'electron'

interface ResponseShape { ok: boolean; value?: unknown; error?: { name: string; message: string } }

if (process.isMainFrame && (location.protocol === 'https:' || (location.protocol === 'http:' && ['localhost', '127.0.0.1'].includes(location.hostname)))) {
  const transportKey = `__archetypeRuntime_${crypto.randomUUID().replaceAll('-', '')}`
  const listeners = new Map<string, Set<(payload: unknown) => void>>()
  ipcRenderer.on('runtime:event', (_event, message: { topic: string; payload: unknown }) => {
    for (const listener of listeners.get(message.topic) ?? []) listener(message.payload)
  })

  contextBridge.exposeInMainWorld(transportKey, {
    async request(payload: Record<string, unknown>): Promise<ResponseShape> {
      return ipcRenderer.invoke('runtime:request', payload) as Promise<ResponseShape>
    },
    cancel(requestId: string): void { ipcRenderer.send('runtime:cancel', requestId) },
    subscribe(topic: string, listener: (payload: unknown) => void): () => void {
      const topicListeners = listeners.get(topic) ?? new Set()
      if (topicListeners.size === 0) ipcRenderer.send('runtime:subscribe', topic)
      topicListeners.add(listener)
      listeners.set(topic, topicListeners)
      return () => {
        topicListeners.delete(listener)
        if (topicListeners.size === 0) {
          listeners.delete(topic)
          ipcRenderer.send('runtime:unsubscribe', topic)
        }
      }
    }
  })

  void contextBridge.executeInMainWorld({
    func: (key: string) => {
      const transport = (globalThis as any)[key]
      const request = async (payload: Record<string, unknown>) => {
        const response = await transport.request(payload)
        if (!response.ok) throw new DOMException(response.error?.message ?? 'Runtime request failed.', response.error?.name ?? 'OperationError')
        return response.value
      }
      const runtime = Object.freeze({
        discover: () => request({ action: 'discover' }),
        signIn: () => request({ action: 'signIn' }),
        signOut: () => request({ action: 'signOut' }),
        session: () => request({ action: 'session' }),
        invoke(operation: string, input: unknown, options: { idempotencyKey?: string; timeoutMs?: number; signal?: AbortSignal } = {}) {
          if (typeof operation !== 'string' || operation.length < 2) return Promise.reject(new TypeError('Runtime operation name is required.'))
          const requestId = crypto.randomUUID()
          if (options.signal?.aborted) return Promise.reject(new DOMException('The operation was aborted.', 'AbortError'))
          const abort = () => transport.cancel(requestId)
          options.signal?.addEventListener('abort', abort, { once: true })
          return request({ action: 'invoke', requestId, operation, input, options: { idempotencyKey: options.idempotencyKey, timeoutMs: options.timeoutMs } })
            .finally(() => options.signal?.removeEventListener('abort', abort))
        },
        subscribe(topic: string, listener: (event: unknown) => void) {
          if (typeof listener !== 'function') throw new TypeError('Runtime subscription listener must be a function.')
          return transport.subscribe(String(topic), listener)
        }
      })
      Object.defineProperty(Navigator.prototype, 'archetype', { configurable: false, enumerable: true, get: () => runtime })
      delete (globalThis as any)[key]
    },
    args: [transportKey]
  })
}
