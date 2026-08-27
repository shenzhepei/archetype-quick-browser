import type pg from 'pg'

const schemaLock = 'archetype_platform_schema_v1'

const platformSchema = `
  CREATE TABLE IF NOT EXISTS organizations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  );
  CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    organization_id TEXT REFERENCES organizations(id) ON DELETE CASCADE,
    allowed_origins TEXT[] NOT NULL DEFAULT '{}',
    oidc JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  );
  ALTER TABLE projects ADD COLUMN IF NOT EXISTS organization_id TEXT REFERENCES organizations(id) ON DELETE CASCADE;
  CREATE TABLE IF NOT EXISTS control_memberships (
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    subject TEXT NOT NULL,
    display_name TEXT,
    role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'developer', 'operator', 'auditor')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (organization_id, subject)
  );
  CREATE TABLE IF NOT EXISTS control_sessions (
    token_hash TEXT PRIMARY KEY,
    subject TEXT NOT NULL,
    display_name TEXT,
    claims JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  );
  CREATE TABLE IF NOT EXISTS control_auth_transactions (
    state_hash TEXT PRIMARY KEY,
    verifier TEXT NOT NULL,
    nonce TEXT NOT NULL,
    return_to TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  );
  ALTER TABLE control_auth_transactions ADD COLUMN IF NOT EXISTS nonce TEXT;
  CREATE TABLE IF NOT EXISTS connections (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    dialect TEXT NOT NULL CHECK (dialect IN ('postgres', 'mysql')),
    envelope JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  );
  CREATE TABLE IF NOT EXISTS deployments (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    sha256 TEXT NOT NULL,
    path TEXT NOT NULL,
    operations JSONB NOT NULL,
    deployed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  );
  CREATE TABLE IF NOT EXISTS sessions (
    token_hash TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    origin TEXT NOT NULL,
    subject TEXT NOT NULL,
    display_name TEXT,
    claims JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
  );
  CREATE TABLE IF NOT EXISTS replay_nonces (
    project_id TEXT NOT NULL,
    origin TEXT NOT NULL,
    nonce TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, origin, nonce)
  );
  CREATE TABLE IF NOT EXISTS idempotency (
    project_id TEXT NOT NULL,
    subject TEXT NOT NULL,
    operation TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    response JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, subject, operation, idempotency_key)
  );
  CREATE TABLE IF NOT EXISTS audit_log (
    id UUID PRIMARY KEY,
    project_id TEXT,
    event TEXT NOT NULL,
    detail JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  );
  CREATE TABLE IF NOT EXISTS runtime_queue (
    event_id UUID PRIMARY KEY,
    project_id TEXT NOT NULL,
    topic TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 8,
    available_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    lease_until TIMESTAMPTZ,
    fencing_token BIGINT NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  );
  CREATE TABLE IF NOT EXISTS dead_letters (
    event_id UUID PRIMARY KEY,
    project_id TEXT NOT NULL,
    topic TEXT NOT NULL,
    payload JSONB NOT NULL,
    attempts INTEGER NOT NULL,
    error TEXT NOT NULL,
    failed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  );
  CREATE INDEX IF NOT EXISTS projects_organization_id_idx ON projects (organization_id);
  CREATE INDEX IF NOT EXISTS control_memberships_subject_idx ON control_memberships (subject);
  CREATE INDEX IF NOT EXISTS control_sessions_expires_at_idx ON control_sessions (expires_at)
`

export async function initializePlatformSchema(pool: pg.Pool): Promise<void> {
  const client = await pool.connect()
  let locked = false
  try {
    await client.query('SELECT pg_advisory_lock(hashtext($1))', [schemaLock])
    locked = true
    await client.query(platformSchema)
  } finally {
    try {
      if (locked) await client.query('SELECT pg_advisory_unlock(hashtext($1))', [schemaLock])
    } finally {
      client.release()
    }
  }
}
