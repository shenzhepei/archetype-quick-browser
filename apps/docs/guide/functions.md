# Node.js cloud functions

Functions are bundled ESM deployments. They receive trusted identity and a database binding from the self-hosted Runtime.

## Quick start

Create a project with `archetype init`, export a `RuntimeDeployment`, and use `archetype dev` while building. `archetype deploy` bundles the entry for Node.js 24, calculates SHA-256, uploads it through the control plane, and activates it only after Function Host verifies the same digest.

```ts
import { z } from 'zod'
import { defineFunction } from '@archetype/function-sdk'

export const readOrder = defineFunction({
  name: 'order.read',
  auth: 'required',
  input: z.object({ orderId: z.string().uuid() }),
  async handler({ user, db }, input) {
    return db.db.selectFrom('orders')
      .select(['id', 'status'])
      .where('id', '=', input.orderId)
      .where('user_id', '=', user!.id)
      .executeTakeFirst()
  }
})
```

The browser calls only the operation name:

```ts
const order = await navigator.archetype.invoke(
  'order.read',
  { orderId },
  { idempotencyKey: crypto.randomUUID() }
)
```

Input and output schemas are enforced inside the Function Host. A deployment is activated only after its SHA-256 digest matches the uploaded bundle.

## Authentication modes

- `required` rejects calls without an origin-bound OIDC session.
- `optional` provides a user when signed in and otherwise runs anonymously.
- `anonymous` never requires login but remains bound to the project, origin and device proof.

## Schema and errors

Use Zod for every input and for outputs that cross the trust boundary. Invalid input is rejected before the handler runs. Throw errors with safe public messages only; connection strings, tokens, stack traces, and secret values stay in structured server logs. The browser maps runtime failures to DOM-compatible errors such as `OperationError`.

## Execution boundary

Function Host runs only artifacts deployed by the authenticated CLI. Each invocation uses a bounded child-process pool with a timeout, cancellation signal, memory limit, captured logs, and crash replacement. This isolates accidental failures between invocations; it is not a sandbox for hostile multi-tenant code.
