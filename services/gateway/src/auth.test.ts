import { describe, expect, test, vi } from 'vitest'
import type { PlatformStore, ProjectRecord } from './store.js'
import { AuthService } from './auth.js'

function store() {
  return {
    createSession: vi.fn(async (projectId: string, origin: string, subject: string, displayName: string) => ({ token: 'session-token', projectId, origin, subject, displayName }))
  } as unknown as PlatformStore
}

describe('OIDC transaction service', () => {
  test('completes and polls the development sign-in flow', async () => {
    const persistence = store()
    const auth = new AuthService(persistence, 'https://runtime.example')
    const project = { id: 'project-1', oidc: null } as ProjectRecord
    const started = await auth.start(project, 'https://app.example')
    expect(started.authUrl).toContain('/v1/auth/dev?state=')
    expect(auth.poll(started.pollToken)).toEqual({ pending: true })
    const state = new URL(started.authUrl).searchParams.get('state')!
    await auth.completeDev(state)
    expect(auth.poll(started.pollToken)).toEqual({ pending: false, token: 'session-token' })
  })

  test('builds Authorization Code with PKCE from provider discovery', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(JSON.stringify({ authorization_endpoint: 'https://identity.example/authorize' })))
    const auth = new AuthService(store(), 'https://runtime.example')
    const project = { id: 'project-1', oidc: { issuer: 'https://identity.example/', clientId: 'client-1', clientSecret: 'secret' } } as unknown as ProjectRecord
    const started = await auth.start(project, 'https://app.example')
    const url = new URL(started.authUrl)
    expect(url.origin + url.pathname).toBe('https://identity.example/authorize')
    expect(url.searchParams.get('response_type')).toBe('code')
    expect(url.searchParams.get('code_challenge_method')).toBe('S256')
    expect(url.searchParams.get('code_challenge')).toBeTruthy()
    fetchMock.mockRestore()
  })

  test('rejects unknown, expired and non-OIDC transactions', async () => {
    const auth = new AuthService(store(), 'https://runtime.example')
    expect(() => auth.poll('missing')).toThrow('not found or expired')
    await expect(auth.completeDev('missing')).rejects.toThrow('not found or expired')
    const started = await auth.start({ id: 'project-1', oidc: null } as ProjectRecord, 'https://app.example')
    const state = new URL(started.authUrl).searchParams.get('state')!
    await expect(auth.completeOidc(state, 'code')).rejects.toThrow('OIDC is not configured')
    vi.useFakeTimers()
    vi.advanceTimersByTime(10 * 60_000 + 1)
    expect(() => auth.poll(started.pollToken)).toThrow('not found or expired')
    vi.useRealTimers()
  })
})
