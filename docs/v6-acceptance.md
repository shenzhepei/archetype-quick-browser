# Archetype V6 Static Responsive CSS Acceptance

V6 delivers workspace and Runtime `0.6.0`, Protocol v4.1 and the existing `archetype-sdk 0.1`
developer preview. Scope and exclusions are defined by the paired
[V6 PRD](./prd/07-Archetype-V6-静态响应式CSS-PRD.md),
[detailed design](./detailed-design/07-Archetype-V6-静态响应式CSS详设.md), and
[machine-readable support matrix](./html-css-support.json). The checked-in
[resource-trend report](./v6-acceptance-report.json) records the local one-minute baseline.

## Acceptance matrix

| Requirement | Evidence | Status |
| --- | --- | --- |
| Custom properties | Token-level bounded `var()` expansion, inheritance, overrides, fallback and cycle tests | Pass |
| Width media queries | Structured `screen`/`all` min/max-width AST and 320/1280 style tests | Pass |
| Runtime responsiveness | Real SDK pages render red/blue RGBA branches from the same document at different widths | Pass |
| Flex item sizing | Row/column `flex-basis`, integer `order`, reverse and stable document-order tests | Pass |
| Positioning | Relative flow preservation, absolute containing blocks, start/end and percentage offsets | Pass |
| Layering | Stable integer z-index paint order with positioned descendant propagation | Pass |
| Corpus | 62 deterministic pages, including 12 V6 pages, match macOS PNG references within 0.5% | Pass |
| Compatibility | Protocol v4.1 defaults missing viewport height to 900 px and current matrices match `0.6.0` | Pass |
| Regression | Workspace tests, strict Clippy, rustdoc, coverage, sandbox, entitlements and SDK recovery remain passing | Pass |

| Resource baseline | Result |
| --- | --- |
| Fresh-profile startup | P95 1,643.06 ms across 20 launches |
| Static page pipeline | P95 0.219 ms across 3,720 loads |
| Reference raster | P95 0.808 ms across 3,720 frames |
| CPU trend | 2.77% average; second/first-half cost ratio 1.049x |
| Memory trend | 30.23 MiB initial; 22.25 MiB final; -7.98 MiB growth |

## Reproduce

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/verify_support_matrix.sh
scripts/verify_sdk_compatibility.sh
cargo run -p arch-browser --example update_snapshots
cargo test -p arch-browser snapshot::tests::fixture_snapshots_match_references
./target/release/arch-v4-acceptance \
  --duration-seconds 60 \
  --cycle-delay-milliseconds 1000 \
  --startup-samples 20 \
  --expected-fixtures 62 \
  --output docs/v6-acceptance-report.json
```

The V6 acceptance workflow also runs the 62-page one-minute CPU/RSS trend probe and Runtime
sandbox/entitlement verification. The resource probe compares the second half of the minute with
the first half; it does not wait for a two-hour crash test.

## Limits

- V6 supports static custom properties, width media queries, Flex item sizing/order,
  relative/absolute positioning and a basic stable z-index model.
- Grid, fixed/sticky positioning, general media queries, transitions, animation, filters, 3D
  transforms and GPU compositing remain unsupported.
- Process isolation, the development sandbox probe and a production signed App Sandbox remain
  distinct claims. The release artifact is unsigned and not notarized.
