# Browser guide

Archetype Runtime is a focused Electron Chromium browser. It keeps navigation and tabs familiar while adding one capability boundary for configured HTTPS applications.

## Install and use

Download the installer for your platform from GitHub Releases. The first release is unsigned and not notarized, so Windows SmartScreen or macOS Gatekeeper can identify it as an unknown publisher. Review the release checksum before opening it; Archetype does not ship a script that disables operating-system security checks.

Open an HTTPS application as you would in a normal browser. The toolbar shows site permission and Runtime state. `archetype://runtime` reports the discovered project, signed-in user, Gateway health, and operations granted to the current origin.

## Site discovery

Serve this file from the same origin as the application:

```json
{
  "version": 1,
  "projectId": "shop-production",
  "gatewayUrl": "https://runtime.example.com"
}
```

The exact path is `/.well-known/archetype-runtime.json`. Remote gateways must use HTTPS; `http://localhost` is accepted for development.

## Availability

`navigator.archetype` exists only in a top-level HTTPS or localhost document. It is not injected into remote HTTP pages, files, internal pages, iframes, or Service Workers. Discovery still verifies that the project allows the real frame origin.

```ts
const project = await navigator.archetype.discover()
console.log(project.operations)
```

The Electron main process performs discovery, identity and signed network requests. Page JavaScript never receives the database URL, OIDC token, capability token, or device private key.

## Sign-in and device binding

`signIn()` starts OIDC Authorization Code with PKCE in the browser, while the Gateway receives and exchanges the callback. OIDC access and refresh tokens remain at the Gateway. The website receives only a safe session summary.

Electron creates a separate Ed25519 device key for every project and origin. Its private key is encrypted with Electron `safeStorage`; the Gateway issues 60-second capabilities bound to its public key. This prevents ordinary token copying and replay, but it is not hardware remote attestation.

```ts
const session = await navigator.archetype.signIn()
console.log(session.displayName)
await navigator.archetype.signOut()
```
