export interface Organization { id: string; name: string; role: Role }
export type Role = 'owner' | 'admin' | 'developer' | 'operator' | 'auditor'
export interface Session { subject: string; displayName?: string; expiresAt: number; organizations: Organization[] }
export interface Member { subject: string; displayName?: string; role: Role }
export interface Project {
  id: string; name: string; organizationId: string; allowedOrigins: string[]; oidc: { issuer?: string; clientId?: string } | null
  role: Role; hasDatabase: boolean; deployedAt: string | null
}
export interface AuditEntry { event: string; detail: Record<string, unknown>; created_at: string }

export async function api<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    credentials: 'same-origin',
    headers: { ...(init?.body ? { 'content-type': 'application/json' } : {}), ...init?.headers }
  })
  if (!response.ok) {
    const body = await response.json().catch(() => null) as { error?: { message?: string } } | null
    throw Object.assign(new Error(body?.error?.message ?? `Request failed (${response.status}).`), { status: response.status })
  }
  return response.status === 204 ? undefined as T : response.json() as Promise<T>
}
