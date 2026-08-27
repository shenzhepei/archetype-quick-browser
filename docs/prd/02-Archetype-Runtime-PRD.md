# 02 Archetype Runtime PRD

| Field | Value |
| --- | --- |
| Status | Released |
| Product | Archetype Runtime |
| Detailed design | [02-Archetype-Runtime-Design.md](../detailed-design/02-Archetype-Runtime-Design.md) |

## Product goal

Archetype Runtime is an Electron Chromium application browser that gives every top-level secure website a typed, origin-bound cloud runtime API. Developers deploy trusted Node.js functions and workers to their own Docker environment; consumer users only browse, authenticate, and invoke declared capabilities.

## Requirements

| ID | Requirement | Acceptance |
| --- | --- | --- |
| R02-01 | Core browser | Multi-tab HTTPS browsing, navigation, theme, English-default bilingual UI, and a Runtime status view work in Electron |
| R02-02 | Runtime Web API | Top-level HTTPS and localhost pages receive `navigator.archetype`; unsupported contexts do not |
| R02-03 | Origin discovery | The browser resolves `/.well-known/archetype-runtime.json` and rejects unregistered or mismatched origins |
| R02-04 | Trusted invocation | Pages call only deployed operations; sender origin, 60-second capability, identity, device proof, schema and timeout are validated outside the page |
| R02-05 | Identity | OIDC Authorization Code with PKCE creates an opaque session without exposing tokens to page JavaScript |
| R02-06 | Self-hosting | Docker Compose starts the gateway, function host, worker, platform PostgreSQL, example PostgreSQL/MySQL and optional Caddy |
| R02-07 | Functions | Node.js functions use typed schemas, database bindings, transactions, idempotency and structured logs |
| R02-08 | Durable events | PostgreSQL/MySQL transactional outboxes feed leased, retryable jobs with dead-letter handling and subject-filtered browser subscriptions |
| R02-09 | Developer workflow | CLI commands create projects, register origins/databases, build deployments, produce discovery files and read logs |
| R02-10 | Documentation | A bilingual VitePress site documents browser, functions, data, queues, security, CLI and self-hosting |
| R02-11 | Delivery | CI, Codecov, Pages and tag-triggered Windows/macOS GitHub Releases are automated |
| R02-12 | Product separation | The consumer browser opens New Tab; enterprise administration is isolated in the Gateway console with OIDC and organization RBAC |

## Security invariants

- Database credentials, OIDC tokens and capability tokens never enter website JavaScript.
- The main process derives origin from the Electron frame and never trusts an origin supplied by a page.
- Device keys are scoped to an origin and project; stable identifiers are not shared between sites.
- Capabilities expire after 60 seconds and bind project, origin, operation or topic, subject and device key.
- Arbitrary SQL and arbitrary executable code cannot be submitted through the page API.
- The self-hosted v1 trusts deployed function authors; it is not a hostile multi-tenant sandbox.

## Non-goals

- Chrome extension compatibility, bookmarks, browsing history sync and local SQLite.
- Hardware-backed remote attestation or a claim that a modified local client cannot call an authorized service.
- Exactly-once job delivery. Workers receive at-least-once delivery and must be idempotent.
