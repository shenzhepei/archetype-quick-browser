# Archetype Runtime Browser

<!-- repo-languages:start -->
English | [简体中文](README-zh-CN.md)
<!-- repo-languages:end -->

<!-- repo-badges:start -->
[![Node.js 24](https://img.shields.io/badge/Node.js-24-339933?style=flat-square&logo=nodedotjs&logoColor=white)](https://nodejs.org)
[![pnpm 10.33.2](https://img.shields.io/badge/pnpm-10.33.2-F69220?style=flat-square&logo=pnpm&logoColor=white)](https://pnpm.io)
[![Vite 7.3.6](https://img.shields.io/badge/Vite-7.3.6-646CFF?style=flat-square&logo=vite&logoColor=white)](https://vite.dev)
[![TypeScript 6.0.3](https://img.shields.io/badge/TypeScript-6.0.3-3178C6?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org)
[![Sass 1.103.1](https://img.shields.io/badge/Sass-1.103.1-CC6699?style=flat-square&logo=sass&logoColor=white)](https://sass-lang.com)
[![Test Coverage](https://img.shields.io/codecov/c/github/shenzhepei/archetype-runtime-browser?style=flat-square&logo=codecov)](https://codecov.io/gh/shenzhepei/archetype-runtime-browser)
[![License](https://img.shields.io/github/license/shenzhepei/archetype-runtime-browser?style=flat-square)](https://github.com/shenzhepei/archetype-runtime-browser/blob/HEAD/LICENSE)
[![Sponsor](https://img.shields.io/github/sponsors/shenzhepei?style=flat-square&logo=githubsponsors&label=Sponsor)](https://github.com/sponsors/shenzhepei)
<!-- repo-badges:end -->

Archetype Runtime Browser lets a top-level HTTPS application call trusted, self-hosted Node.js functions through `navigator.archetype`. Database credentials, OIDC tokens, device proofs, queues, and transaction logic stay outside website JavaScript.

[Read the documentation](https://shenzhepei.github.io/archetype-runtime-browser/) · [Download releases](https://github.com/shenzhepei/archetype-runtime-browser/releases)

![Archetype Runtime Browser](docs/preview.webp)

## What It Includes

- An Electron/Chromium browser with tabs, navigation, site permissions, runtime status, themes, and persistent English/Simplified Chinese localization.
- A frozen `navigator.archetype` API for eligible top-level HTTPS and localhost pages. HTTP, files, internal pages, iframes, and Service Workers are excluded.
- A Docker self-hosted gateway, isolated function host, durable worker, platform PostgreSQL, example PostgreSQL/MySQL, and optional Caddy edge.
- Origin-bound device keys, OIDC Authorization Code with PKCE, 60-second resource capabilities, signed invocation proofs, replay detection, auditing, and AES-256-GCM envelope-encrypted database secrets.
- PostgreSQL/MySQL adapters, transactional outbox, leased jobs, fencing tokens, retries, dead letters, and an atomic order-claim example.
- A deployment CLI and typed function, worker, protocol, and generated-client packages.

The browser never accepts arbitrary SQL from a website and never exposes a database URL to page JavaScript. Authorization remains business logic enforced by trusted functions, identity, transactions, and database constraints.

## Quick Start

Requires Node.js 24, pnpm 10.33.2, Docker, and Docker Compose.

```bash
corepack enable
pnpm install --frozen-lockfile
cp infra/docker/.env.example infra/docker/.env
pnpm docker:up
```

In another terminal:

```bash
pnpm typecheck
pnpm test
pnpm dev
```

See the [self-hosting guide](https://shenzhepei.github.io/archetype-runtime-browser/guide/self-hosting) and [order claim tutorial](https://shenzhepei.github.io/archetype-runtime-browser/guide/data-jobs) for project, origin, database, and deployment setup.

## Workspace

| Path | Responsibility |
| --- | --- |
| `apps/browser` | Electron browser shell and page runtime bridge |
| `apps/docs` | English and Simplified Chinese VitePress documentation |
| `services/gateway` | Discovery, identity, policy, secrets, idempotency, audit, and routing |
| `services/function-host` | Digest-verified Node.js function subprocesses |
| `services/worker` | Outbox dispatch, queue leases, retries, and dead letters |
| `packages/*` | Protocol, SDKs, adapters, typed client, and CLI |
| `examples/order-claim` | PostgreSQL/MySQL concurrency and worker example |
| `infra/docker` | Self-hosted runtime images and Compose stack |

The [product requirements](docs/prd/02-Archetype-Runtime-PRD.md) and [detailed design](docs/detailed-design/02-Archetype-Runtime-Design.md) define the security and execution boundaries.

## Build And Release

```bash
pnpm test:coverage
pnpm build
pnpm package:mac
```

Semantic tags such as `v1.0.0` create a Windows x64 NSIS installer and macOS arm64/x64 DMG and ZIP artifacts. Initial packages are unsigned and not notarized, so Windows SmartScreen or macOS Gatekeeper may report an unknown publisher. This project does not provide scripts that bypass operating-system security controls.

## License

Licensed under the [Apache License 2.0](LICENSE).
