# 02 Archetype Runtime detailed design

| Field | Value |
| --- | --- |
| Status | In progress |
| PRD | [02-Archetype-Runtime-PRD.md](../prd/02-Archetype-Runtime-PRD.md) |

## Process boundaries

```text
HTTPS page -> page preload -> Electron main RuntimeClient -> Gateway
Gateway -> verified deployment -> Function Host child process -> PostgreSQL/MySQL
Business transaction -> _archetype_outbox -> Worker -> platform queue -> deployed worker
                                                     -> Gateway SSE -> browser main -> page listener
```

The page preload exposes a frozen facade only in a top-level trustworthy context. Every IPC request is rebound to `senderFrame.url`; navigation invalidates outstanding requests. The main process performs discovery and holds the opaque session plus origin-scoped Ed25519 key material encrypted with Electron safe storage.

## Discovery and invocation

`/.well-known/archetype-runtime.json` contains version, project ID and HTTPS gateway URL. The gateway manifest confirms the origin and advertised operations. Before a call, the main process obtains a 60-second HMAC-signed capability bound to project, origin, operation, subject and device public key. Calls then carry that capability plus a body digest, timestamp, nonce, device public key and Ed25519 signature. The gateway verifies capability scope, replay state, session, operation schema and idempotency key before invoking the active deployment.

Subscriptions use the same boundary with a topic-bound capability. The Electron main process owns the authenticated SSE connection, refreshes it before capability expiry, and cancels it on unsubscribe or top-level navigation. Gateway event reads require a matching session and return only payloads whose `userId` or `subject` equals the session subject.

## Runtime services

- Gateway exposes public discovery/auth/invoke/event endpoints and token-protected admin endpoints.
- Function Host verifies the deployment SHA-256, imports the bundled ESM module, and executes it in a bounded child process lifecycle.
- DB adapters build Kysely PostgreSQL/MySQL connections, inject transactions and append events to the transaction-local outbox.
- Worker leases outbox rows with `SKIP LOCKED`, copies events to the platform queue by unique event ID, then executes matching deployed workers with retries and dead-lettering.

## Storage

Platform PostgreSQL owns projects, origins, encrypted connections, deployments, sessions, nonces, idempotency records, audit events, queue jobs and dead letters. Connection ciphertext uses a random data key and AES-256-GCM; the installation master key wraps the data key. Application databases contain application tables and `_archetype_outbox` only.

## Release

VitePress is deployed from the repository to GitHub Pages. Pushes and pull requests run type checking, tests, coverage and builds. A semantic `vX.Y.Z` tag must match root and browser versions before Windows x64 NSIS plus macOS x64/arm64 DMG and ZIP artifacts are released with SHA-256 checksums.
