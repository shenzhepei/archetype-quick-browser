# Security model

## Protected

- Database addresses and credentials stay in the self-hosted Runtime.
- OIDC and Runtime session tokens stay outside website JavaScript.
- The Electron main process derives the caller origin from its frame.
- Each origin and project gets a separate encrypted device key.
- A 60-second capability ticket binds each invocation or subscription to its project, origin, resource, subject and device public key.
- Timestamp, nonce, body digest and signature prevent ordinary token copying and replay.
- Browser event streams require a matching session and filter event payloads to the current subject.
- Function schemas, database constraints and transactions remain server-side.

OIDC uses Authorization Code with PKCE. The Gateway owns the callback and token exchange, while Electron keeps only an opaque Runtime session. Calls include a body digest, timestamp, unique nonce and Ed25519 signature; the Gateway validates clock skew and stores used nonces before routing the operation.

Database credentials use envelope encryption: AES-256-GCM encrypts each credential with a random data key, and `ARCHETYPE_MASTER_KEY` wraps that key. Function responses, audit entries, and browser errors are designed not to include connection strings or tokens.

## Not guaranteed

The user controls their computer. Electron safe storage and process boundaries raise the cost of extracting secrets, but v1 does not provide hardware remote attestation and cannot prove an unmodified client. A malicious authorized user can automate access to data they are permitted to see.

Function deployments are trusted organization code. The child process provides timeout and crash isolation, not a hostile multi-tenant sandbox. Public multi-tenant hosting requires a stronger isolation layer such as dedicated containers or microVMs.

Authorization is ultimately server-side business logic. A valid browser capability proves the origin, device key, operation, and current session, but does not replace row-level ownership checks, atomic updates, uniqueness constraints, idempotency, or transaction boundaries.
