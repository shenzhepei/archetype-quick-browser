# Docker self-hosting

## Requirements

- Docker with Compose
- 4 CPU cores and 8 GB memory for the complete development stack
- A random `ARCHETYPE_MASTER_KEY` and service token; administrator OIDC for production

```bash
cp infra/docker/.env.example infra/docker/.env
docker-compose --env-file infra/docker/.env -f infra/docker/compose.yml up --build
```

The stack exposes Gateway on `http://localhost:8787` for development. Production deployments should enable the Caddy profile with a real hostname and HTTPS certificate.

Open `http://localhost:8787/console/` for local administration. The example environment explicitly enables development login. In production, disable it and configure the administrator OIDC variables described in [Enterprise control plane](./control-plane).

The Compose project contains Gateway, Function Host, Worker, platform PostgreSQL, example PostgreSQL, example MySQL, and Caddy. Only Gateway/Caddy is a public application boundary; databases and internal service ports should stay on the private Docker network.

## First deployment

```bash
ARCHETYPE_ADMIN_TOKEN='...' archetype project create --name "Shop" --organization default
archetype origin add --project PROJECT_ID --origin https://shop.example
ARCHETYPE_DATABASE_URL='postgres://...' archetype db add --project PROJECT_ID --dialect postgres
archetype deploy --project PROJECT_ID --entry src/functions/index.ts
archetype well-known generate --project PROJECT_ID --gateway https://runtime.example.com
```

Do not place database URLs in frontend environment variables or discovery files. `db add` reads `ARCHETYPE_DATABASE_URL` and sends it only to the authenticated control plane, where envelope encryption protects it at rest.

## Secrets

Generate `ARCHETYPE_MASTER_KEY`, optional CLI automation `ARCHETYPE_ADMIN_TOKEN`, and `ARCHETYPE_SERVICE_TOKEN` outside the repository. The Gateway creates a random data-encryption key for each database credential, encrypts the credential with AES-256-GCM, and wraps that data key with the installation master key. Back up the master key separately; losing it makes stored credentials unrecoverable.

Rotate database passwords by running `db add` again. Rotate service/admin tokens during a maintenance window. Master-key rotation requires decrypting and rewrapping every stored data key and should be treated as an explicit migration.

## Production HTTPS with Caddy

Set the public Runtime hostname in `infra/docker/Caddyfile`, point DNS to the host, and expose only ports 80/443. Caddy terminates TLS and proxies Gateway. The discovery document must use the final `https://` Gateway URL and the Gateway project must contain the website's exact origin.

## Logs and troubleshooting

Run `archetype doctor` first, then inspect `docker-compose -f infra/docker/compose.yml ps` and service logs. `archetype logs --project PROJECT_ID` returns project audit records without returning Secret values. Common failures are an unregistered origin, a non-HTTPS remote Gateway, expired OIDC configuration, a mismatched deployment digest, or an unavailable database.

Health checks are available on Gateway and internal services. Restarting Function Host or Worker is safe: in-flight functions fail and can be retried with an idempotency key, while committed outbox rows and leased jobs are recovered from PostgreSQL.
