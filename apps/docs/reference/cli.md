# CLI reference

| Command | Purpose |
| --- | --- |
| `archetype init` | Create a function project and discovery-file skeleton |
| `archetype doctor` | Check Node, Docker, Compose and Gateway health |
| `archetype runtime up` | Start the self-hosted stack |
| `archetype project create` | Create a project |
| `archetype origin add` | Register an HTTPS application origin |
| `archetype db add` | Encrypt and bind a PostgreSQL/MySQL URL |
| `archetype dev` | Run Gateway, Function Host and Worker from source |
| `archetype deploy` | Bundle, hash and activate functions and workers |
| `archetype logs` | Read the latest project audit records |
| `archetype well-known generate` | Create the same-origin discovery file |

Administrative automation calls require an explicitly configured `ARCHETYPE_ADMIN_TOKEN`; there is no default token. `project create` accepts `--organization`. Human administrators should use the OIDC-backed enterprise console instead. Database binding reads `ARCHETYPE_DATABASE_URL` so the URL does not need to appear as a command argument.
