import { createHash, randomBytes, randomUUID } from 'node:crypto'
import pg from 'pg'
import { initializePlatformSchema } from '@archetype/db-adapters'
import type { OperationDescriptor, SessionSummary } from '@archetype/protocol'
import { decryptSecret, encryptSecret, type SecretEnvelope } from './secret-envelope.js'

export interface ProjectRecord {
  id: string
  name: string
  organizationId: string | null
  allowedOrigins: string[]
  oidc: Record<string, unknown> | null
}

export type ControlRole = 'owner' | 'admin' | 'developer' | 'operator' | 'auditor'

export interface OrganizationRecord {
  id: string
  name: string
  role: ControlRole
}

export interface ControlMemberRecord {
  subject: string
  displayName?: string
  role: ControlRole
}

export interface ControlSessionRecord {
  token: string
  subject: string
  displayName?: string
  expiresAt: number
  claims: Record<string, unknown>
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

  async seedProject(id: string, name: string, origins: string[], organizationId: string | null = null): Promise<void> {
    await this.pool.query(
      `INSERT INTO projects (id, name, allowed_origins, organization_id) VALUES ($1, $2, $3, $4)
       ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, allowed_origins = EXCLUDED.allowed_origins,
         organization_id = COALESCE(projects.organization_id, EXCLUDED.organization_id)`,
      [id, name, origins, organizationId]
    )
  }

  async ensureBootstrapOrganization(id: string, name: string, ownerSubjects: string[]): Promise<void> {
    await this.pool.query(
      `INSERT INTO organizations (id, name) VALUES ($1, $2)
       ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name`,
      [id, name]
    )
    await this.pool.query('UPDATE projects SET organization_id = $1 WHERE organization_id IS NULL', [id])
    for (const subject of ownerSubjects) {
      await this.pool.query(
        `INSERT INTO control_memberships (organization_id, subject, role) VALUES ($1, $2, 'owner')
         ON CONFLICT (organization_id, subject) DO NOTHING`,
        [id, subject]
      )
    }
  }

  async createProject(name: string, organizationId: string | null = null): Promise<ProjectRecord> {
    const id = `${name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')}-${randomBytes(3).toString('hex')}`
    await this.pool.query('INSERT INTO projects (id, name, organization_id) VALUES ($1, $2, $3)', [id, name, organizationId])
    return { id, name, organizationId, allowedOrigins: [], oidc: null }
  }

  async project(id: string): Promise<ProjectRecord | null> {
    const result = await this.pool.query('SELECT id, name, organization_id, allowed_origins, oidc FROM projects WHERE id = $1', [id])
    const row = result.rows[0]
    return row ? { id: row.id, name: row.name, organizationId: row.organization_id, allowedOrigins: row.allowed_origins, oidc: row.oidc } : null
  }

  async organizationsForSubject(subject: string): Promise<OrganizationRecord[]> {
    const result = await this.pool.query(
      `SELECT o.id, o.name, m.role FROM organizations o
       JOIN control_memberships m ON m.organization_id = o.id
       WHERE m.subject = $1 ORDER BY o.name`,
      [subject]
    )
    return result.rows.map((row) => ({ id: row.id, name: row.name, role: row.role }))
  }

  async projectsForSubject(subject: string): Promise<Array<ProjectRecord & { role: ControlRole; hasDatabase: boolean; deployedAt: string | null }>> {
    const result = await this.pool.query(
      `SELECT p.id, p.name, p.organization_id, p.allowed_origins, p.oidc, m.role,
              (c.project_id IS NOT NULL) AS has_database, d.deployed_at
       FROM projects p
       JOIN control_memberships m ON m.organization_id = p.organization_id AND m.subject = $1
       LEFT JOIN connections c ON c.project_id = p.id
       LEFT JOIN deployments d ON d.project_id = p.id
       ORDER BY p.name`,
      [subject]
    )
    return result.rows.map((row) => ({
      id: row.id,
      name: row.name,
      organizationId: row.organization_id,
      allowedOrigins: row.allowed_origins,
      oidc: row.oidc,
      role: row.role,
      hasDatabase: row.has_database,
      deployedAt: row.deployed_at ? new Date(row.deployed_at).toISOString() : null
    }))
  }

  async controlRole(subject: string, organizationId: string): Promise<ControlRole | null> {
    const result = await this.pool.query('SELECT role FROM control_memberships WHERE organization_id = $1 AND subject = $2', [organizationId, subject])
    return result.rows[0]?.role ?? null
  }

  async controlMembers(organizationId: string): Promise<ControlMemberRecord[]> {
    const result = await this.pool.query(
      'SELECT subject, display_name, role FROM control_memberships WHERE organization_id = $1 ORDER BY display_name NULLS LAST, subject',
      [organizationId]
    )
    return result.rows.map((row) => ({ subject: row.subject, ...(row.display_name ? { displayName: row.display_name } : {}), role: row.role }))
  }

  async addControlMember(organizationId: string, subject: string, displayName: string | undefined, role: ControlRole): Promise<void> {
    await this.pool.query(
      `INSERT INTO control_memberships (organization_id, subject, display_name, role) VALUES ($1, $2, $3, $4)
       ON CONFLICT (organization_id, subject) DO UPDATE SET display_name = EXCLUDED.display_name, role = EXCLUDED.role`,
      [organizationId, subject, displayName ?? null, role]
    )
  }

  async createControlTransaction(state: string, verifier: string, nonce: string, returnTo: string): Promise<void> {
    await this.pool.query(
      `INSERT INTO control_auth_transactions (state_hash, verifier, nonce, return_to, expires_at)
       VALUES ($1, $2, $3, $4, NOW() + INTERVAL '10 minutes')`,
      [hashToken(state), verifier, nonce, returnTo]
    )
  }

  async consumeControlTransaction(state: string): Promise<{ verifier: string; nonce: string; returnTo: string } | null> {
    const client = await this.pool.connect()
    try {
      await client.query('BEGIN')
      const result = await client.query(
        `DELETE FROM control_auth_transactions
         WHERE state_hash = $1 AND expires_at > NOW()
         RETURNING verifier, nonce, return_to`,
        [hashToken(state)]
      )
      await client.query("DELETE FROM control_auth_transactions WHERE expires_at <= NOW()")
      await client.query('COMMIT')
      const row = result.rows[0]
      return row ? { verifier: row.verifier, nonce: row.nonce, returnTo: row.return_to } : null
    } catch (error) {
      await client.query('ROLLBACK')
      throw error
    } finally {
      client.release()
    }
  }

  async createControlSession(subject: string, displayName: string | undefined, claims: Record<string, unknown>): Promise<ControlSessionRecord> {
    const token = randomBytes(32).toString('base64url')
    const expiresAt = Date.now() + 8 * 60 * 60 * 1000
    await this.pool.query(
      `INSERT INTO control_sessions (token_hash, subject, display_name, claims, expires_at)
       VALUES ($1, $2, $3, $4, to_timestamp($5 / 1000.0))`,
      [hashToken(token), subject, displayName ?? null, claims, expiresAt]
    )
    return { token, subject, ...(displayName ? { displayName } : {}), expiresAt, claims }
  }

  async controlSession(token: string): Promise<ControlSessionRecord | null> {
    const result = await this.pool.query(
      `SELECT subject, display_name, claims, EXTRACT(EPOCH FROM expires_at) * 1000 AS expires_at
       FROM control_sessions WHERE token_hash = $1 AND expires_at > NOW()`,
      [hashToken(token)]
    )
    const row = result.rows[0]
    return row ? { token, subject: row.subject, ...(row.display_name ? { displayName: row.display_name } : {}), expiresAt: Number(row.expires_at), claims: row.claims } : null
  }

  async revokeControlSession(token: string): Promise<void> {
    await this.pool.query('DELETE FROM control_sessions WHERE token_hash = $1', [hashToken(token)])
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
