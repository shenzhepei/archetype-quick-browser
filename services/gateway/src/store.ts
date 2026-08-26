import { createHash, randomBytes, randomUUID } from 'node:crypto'
import pg from 'pg'
import { initializePlatformSchema } from '@archetype/db-adapters'
import type { OperationDescriptor, SessionSummary } from '@archetype/protocol'
import { decryptSecret, encryptSecret, type SecretEnvelope } from './secret-envelope.js'

export interface ProjectRecord {
  id: string
  name: string
  allowedOrigins: string[]
  oidc: Record<string, unknown> | null
}

export interface DeploymentRecord {
  projectId: string
  sha256: string
  path: string
  operations: OperationDescriptor[]
}

export interface SessionRecord extends SessionSummary {
  token: string
  projectId: string
  origin: string
  claims: Record<string, unknown>
}

export interface RuntimeEventRecord {
  id: string
  topic: string
  payload: unknown
  createdAt: string
}

export class PlatformStore {
  readonly pool: pg.Pool

  constructor(databaseUrl: string, private readonly masterKey: string) {
    this.pool = new pg.Pool({ connectionString: databaseUrl, max: 12 })
  }

  async initialize(): Promise<void> {
    await initializePlatformSchema(this.pool)
  }

  async seedProject(id: string, name: string, origins: string[]): Promise<void> {
    await this.pool.query(
      `INSERT INTO projects (id, name, allowed_origins) VALUES ($1, $2, $3)
       ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, allowed_origins = EXCLUDED.allowed_origins`,
      [id, name, origins]
    )
  }

  async createProject(name: string): Promise<ProjectRecord> {
    const id = `${name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')}-${randomBytes(3).toString('hex')}`
    await this.pool.query('INSERT INTO projects (id, name) VALUES ($1, $2)', [id, name])
    return { id, name, allowedOrigins: [], oidc: null }
  }

  async project(id: string): Promise<ProjectRecord | null> {
    const result = await this.pool.query('SELECT id, name, allowed_origins, oidc FROM projects WHERE id = $1', [id])
    const row = result.rows[0]
    return row ? { id: row.id, name: row.name, allowedOrigins: row.allowed_origins, oidc: row.oidc } : null
  }

  async addOrigin(projectId: string, origin: string): Promise<void> {
    await this.pool.query('UPDATE projects SET allowed_origins = ARRAY(SELECT DISTINCT unnest(allowed_origins || $2::text)) WHERE id = $1', [projectId, origin])
  }

  async setOidc(projectId: string, oidc: Record<string, unknown>): Promise<void> {
    await this.pool.query('UPDATE projects SET oidc = $2 WHERE id = $1', [projectId, oidc])
  }

  async setConnection(projectId: string, dialect: 'postgres' | 'mysql', url: string): Promise<void> {
    const envelope = encryptSecret(url, this.masterKey)
    await this.pool.query(
      `INSERT INTO connections (project_id, dialect, envelope) VALUES ($1, $2, $3)
       ON CONFLICT (project_id) DO UPDATE SET dialect = EXCLUDED.dialect, envelope = EXCLUDED.envelope, updated_at = NOW()`,
      [projectId, dialect, envelope]
    )
  }

  async connection(projectId: string): Promise<{ dialect: 'postgres' | 'mysql'; url: string } | null> {
    const result = await this.pool.query('SELECT dialect, envelope FROM connections WHERE project_id = $1', [projectId])
    const row = result.rows[0]
    return row ? { dialect: row.dialect, url: decryptSecret(row.envelope as SecretEnvelope, this.masterKey) } : null
  }

  async setDeployment(deployment: DeploymentRecord): Promise<void> {
    await this.pool.query(
      `INSERT INTO deployments (project_id, sha256, path, operations) VALUES ($1, $2, $3, $4)
       ON CONFLICT (project_id) DO UPDATE SET sha256 = EXCLUDED.sha256, path = EXCLUDED.path, operations = EXCLUDED.operations, deployed_at = NOW()`,
      [deployment.projectId, deployment.sha256, deployment.path, JSON.stringify(deployment.operations)]
    )
  }

  async deployment(projectId: string): Promise<DeploymentRecord | null> {
    const result = await this.pool.query('SELECT project_id, sha256, path, operations FROM deployments WHERE project_id = $1', [projectId])
    const row = result.rows[0]
    return row ? { projectId: row.project_id, sha256: row.sha256, path: row.path, operations: row.operations } : null
  }

  async useNonce(projectId: string, origin: string, nonce: string): Promise<boolean> {
    try {
      await this.pool.query('INSERT INTO replay_nonces (project_id, origin, nonce) VALUES ($1, $2, $3)', [projectId, origin, nonce])
      await this.pool.query("DELETE FROM replay_nonces WHERE created_at < NOW() - INTERVAL '10 minutes'")
      return true
    } catch (error) {
      if ((error as { code?: string }).code === '23505') return false
      throw error
    }
  }

  async createSession(projectId: string, origin: string, subject: string, displayName: string | undefined, claims: Record<string, unknown>): Promise<SessionRecord> {
    const token = randomBytes(32).toString('base64url')
    const expiresAt = Date.now() + 8 * 60 * 60 * 1000
    await this.pool.query(
      'INSERT INTO sessions (token_hash, project_id, origin, subject, display_name, claims, expires_at) VALUES ($1, $2, $3, $4, $5, $6, to_timestamp($7 / 1000.0))',
      [hashToken(token), projectId, origin, subject, displayName ?? null, claims, expiresAt]
    )
    return { token, projectId, origin, authenticated: true, subject, ...(displayName ? { displayName } : {}), expiresAt, claims }
  }

  async session(token: string): Promise<SessionRecord | null> {
    const result = await this.pool.query(
      'SELECT project_id, origin, subject, display_name, claims, EXTRACT(EPOCH FROM expires_at) * 1000 AS expires_at FROM sessions WHERE token_hash = $1 AND expires_at > NOW()',
      [hashToken(token)]
    )
    const row = result.rows[0]
    return row ? {
      token,
      projectId: row.project_id,
      origin: row.origin,
      authenticated: true,
      subject: row.subject,
      ...(row.display_name ? { displayName: row.display_name } : {}),
      expiresAt: Number(row.expires_at),
      claims: row.claims
    } : null
  }

  async revokeSession(token: string): Promise<void> {
    await this.pool.query('DELETE FROM sessions WHERE token_hash = $1', [hashToken(token)])
  }

  async cachedResponse(projectId: string, subject: string, operation: string, key: string, requestHash: string): Promise<unknown | undefined> {
    const result = await this.pool.query(
      'SELECT request_hash, response FROM idempotency WHERE project_id = $1 AND subject = $2 AND operation = $3 AND idempotency_key = $4',
      [projectId, subject, operation, key]
    )
    if (!result.rows[0]) return undefined
    if (result.rows[0].request_hash !== requestHash) throw new Error('IDEMPOTENCY_CONFLICT')
    return result.rows[0].response
  }

  async storeResponse(projectId: string, subject: string, operation: string, key: string, requestHash: string, response: unknown): Promise<void> {
    await this.pool.query(
      `INSERT INTO idempotency (project_id, subject, operation, idempotency_key, request_hash, response)
       VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING`,
      [projectId, subject, operation, key, requestHash, JSON.stringify(response)]
    )
  }

  async audit(projectId: string | null, event: string, detail: Record<string, unknown>): Promise<void> {
    await this.pool.query('INSERT INTO audit_log (id, project_id, event, detail) VALUES ($1, $2, $3, $4)', [randomUUID(), projectId, event, detail])
  }

  async logs(projectId: string): Promise<unknown[]> {
    const result = await this.pool.query('SELECT event, detail, created_at FROM audit_log WHERE project_id = $1 ORDER BY created_at DESC LIMIT 100', [projectId])
    return result.rows
  }

  async eventsSince(projectId: string, topic: string, subject: string, since: Date, afterId = '', limit = 100): Promise<RuntimeEventRecord[]> {
    const result = await this.pool.query(
      `SELECT event_id, topic, payload, created_at
       FROM runtime_queue
       WHERE project_id = $1 AND topic = $2
         AND (created_at > $3 OR (created_at = $3 AND event_id::text > $4))
         AND (payload->>'userId' = $5 OR payload->>'subject' = $5)
       ORDER BY created_at, event_id
       LIMIT $6`,
      [projectId, topic, since, afterId, subject, limit]
    )
    return result.rows.map((row) => ({ id: row.event_id, topic: row.topic, payload: row.payload, createdAt: new Date(row.created_at).toISOString() }))
  }
}

function hashToken(token: string): string {
  return createHash('sha256').update(token).digest('hex')
}
