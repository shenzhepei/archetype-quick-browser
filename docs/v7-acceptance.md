# Archetype V7 Grid and Visual CSS Acceptance

V7 delivers workspace and Runtime `0.7.0`, keeps Protocol v4 compatibility and preserves the
`archetype-sdk 0.1` public boundary. Scope and implementation decisions are defined by the
[V7 PRD](./prd/08-Archetype-V7-Grid与视觉CSS-PRD.md) and
[detailed design](./detailed-design/08-Archetype-V7-Grid与视觉CSS详设.md).

## Acceptance Matrix

| Area | Evidence | Status |
|------|----------|--------|
| Grid style | Fixed, percentage, `fr` and bounded `repeat()` tracks compute into typed style | Pass |
| Grid layout | Mixed tracks, gaps and row-major multi-row placement have layout tests | Pass |
| Visual CSS | Radius, opacity, single outer shadow, color background shorthand and text decoration cross DisplayList | Pass |
| Raster | Rounded alpha blending and bounded shadow/text paths use deterministic CPU rendering | Pass |
| Desktop | GPUI renders radius, alpha, shadow and text decoration from the same commands | Pass |
| Corpus | 72 deterministic pages include 10 V7 fixtures; the original 62 references remain unchanged | Pass |
| Compatibility | Support matrix reports `0.7.0`; advanced Grid and animation remain unsupported | Pass |
| Resource trend | 59 cycles and 4,248 page loads in 60.04 seconds; CPU cost ratio `0.9947`; RSS growth 384 KiB | Pass |

## Local Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc -p archetype-sdk --no-deps
scripts/verify_support_matrix.sh
scripts/verify_sdk_compatibility.sh
```

The recorded [one-minute acceptance report](./v7-acceptance-report.json) measured 3.40% average
CPU. The second-half CPU cost per page was 99.47% of the first half, so CPU cost did not grow;
RSS increased from 31.23 MiB to 31.61 MiB.

The `V7 Acceptance` workflow additionally runs the 72-page one-minute CPU/RSS trend probe,
Runtime sandbox and entitlement probes, license inventory verification, real Runtime subprocess
tests and SDK restart coverage.

## Known Limits

- Explicit Grid placement, spans, named lines, implicit track controls, `minmax()`, auto-fit,
  auto-fill, subgrid and masonry are not implemented.
- Multiple/inset/spread shadows, gradients, background images, per-corner elliptical radii and
  full compositing groups are not implemented.
- JavaScript, transition, animation, filter, transform and GPU compositing remain unsupported.
