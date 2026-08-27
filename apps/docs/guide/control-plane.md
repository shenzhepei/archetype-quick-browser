# Enterprise control plane

The Gateway serves the administrator console at `/console/`. This is an enterprise workspace, separate from the consumer browser. Human administrators sign in through OIDC Authorization Code with PKCE; the resulting eight-hour session is an opaque, same-origin, HttpOnly cookie. The OIDC state, PKCE verifier, and nonce are persisted as a one-time PostgreSQL transaction.

## Roles

| Role | Access |
| --- | --- |
| Owner / Admin | Manage members, create and configure projects, deploy, and read audit logs |
| Developer | Read and configure projects, deploy, and read audit logs |
| Operator | Read and configure projects and read audit logs; cannot deploy |
| Auditor | Read projects and audit logs only |

Every project belongs to an organization. Authorization is recalculated from the authenticated OIDC subject and current organization membership on each request. A role shown in the UI is not an authorization source.

## Administrator OIDC

Configure production identity on Gateway:

```dotenv
ARCHETYPE_PUBLIC_URL=https://runtime.example.com
ARCHETYPE_CONTROL_OIDC_ISSUER=https://identity.example.com
ARCHETYPE_CONTROL_OIDC_CLIENT_ID=archetype-control
ARCHETYPE_CONTROL_OIDC_CLIENT_SECRET=replace-me
ARCHETYPE_CONTROL_BOOTSTRAP_SUBJECTS=first-owner-subject
```

The callback URL is `https://runtime.example.com/v1/control/auth/callback`. `ARCHETYPE_CONTROL_DEV_LOGIN=true` enables the explicit local `development-admin` identity and must not be used in production.

## Human and automation access

The console uses the human OIDC session. The CLI uses the separately configured `ARCHETYPE_ADMIN_TOKEN` for deployment automation and never receives the console cookie. No default automation token exists in Gateway source. Database URLs are write-only inputs: Gateway envelope-encrypts them and neither the console nor the CLI can read them back.
