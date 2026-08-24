# Archetype V5 Rust SDK Preview Acceptance

V5 delivers `archetype-sdk 0.1.0` with Runtime `0.5.x` and Protocol v4 as an unsigned macOS
developer preview. Scope and limits are defined by the paired
[V5 PRD](./prd/06-Archetype-V5-Rust-SDK预览-PRD.md),
[detailed design](./detailed-design/06-Archetype-V5-Rust-SDK预览详设.md), and
[machine-readable compatibility matrix](./sdk-compatibility.json).

## Acceptance matrix

| Requirement | Evidence | Status |
| --- | --- | --- |
| UI-neutral API | `Engine`, `Page`, `StaticDocument`, `PageEvent`, owned `Frame`, and `SdkError`; public source boundary scan | Pass |
| Non-blocking await | Worker-backed `SdkFuture`; controlled first-poll Pending and wake/completion test | Pass |
| Runtime lifecycle | Explicit/sibling discovery, authentication, graceful shutdown, disconnect event, and three-step finite recovery | Pass |
| Runtime integrity | Streaming SHA-256 success and pre-launch mismatch rejection tests | Pass |
| Static rendering | Real Runtime renders HTML, Chinese text and Flexbox into owned RGBA8 | Pass |
| Partner example | `partner_render` writes a decodable `640x360` PNG with non-white pixels | Pass |
| Navigation safety | Per-page monotonic IDs and concurrent stale-result rejection | Pass |
| Event pressure | 64-item queue test preserves disconnect while replacing superseded navigation/frame events | Pass |
| Crash recovery | Public SDK API completes 100 terminate/restart/render cycles while the caller remains alive | Pass |
| Compatibility | Matrix exactly matches SDK, Protocol, Runtime and Apple Silicon target versions | Pass |
| Regression | V4 Browser, 50 screenshots, sandbox, entitlements, forms, Flexbox and hibernation remain passing | Pass |

## Reproduce

```bash
cargo build -p archetype-runtime --bin archetype-runtime
cargo run -p archetype-sdk --example partner_render -- \
  target/debug/archetype-runtime artifacts/sdk-partner.png
RUSTDOCFLAGS="-D warnings" cargo doc -p archetype-sdk --no-deps
cargo test -p archetype-runtime --test sdk
scripts/verify_sdk_compatibility.sh
```

The V5 acceptance workflow also runs workspace formatting, strict Clippy, all tests, license
inventory, Runtime sandbox and entitlement probes, and uploads the example PNG for the exact commit.

## Limits

- SDK and Runtime are developer previews: no SDK 1.0, production signing, notarization or update promise.
- The caller supplies policy-approved HTTP(S) HTML and same-origin bytes. Runtime receives no file,
  network, Cookie or credential capability.
- The public frame is tightly packed RGBA8 rasterized in the SDK process. Shared memory and GPU
  handles are not implemented.
- JavaScript, DOM mutation, extensions, cloud sync, Windows and Linux remain outside V5.
