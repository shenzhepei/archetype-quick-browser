# Archetype V3 Acceptance Evidence

Archetype V3 is accepted against commit `57f837973554e4428d4889c8b1756ac9897063ca`.
The machine-readable measurements are checked in as
[`v3-acceptance-report.json`](./v3-acceptance-report.json).

## Performance and stability

| Check | Result | Evidence |
| --- | --- | --- |
| Fresh-profile startup | P95 1,493.65 ms; mean 175.69 ms | 20 desktop process launches measured from spawn to window-created readiness |
| Fixture stability | 60.03 seconds; 60 cycles; 1,800 page loads | Continuous complete cycles through all 30 fixed pages |
| Page pipeline | P95 0.239 ms; mean 0.115 ms | Load, parse, style, layout, and paint timing for every completed page load |
| Reference raster | P95 1.080 ms; mean 0.506 ms | Fixed `1280x800` frame raster timing for every completed page load |
| CPU trend | 1.95% average; second/first-half cost ratio 1.053x | 0.633 ms/page in the first half and 0.667 ms/page in the second half; limit 1.5x |
| Memory trend | 30.02 MiB initial; 22.44 MiB final; 30.22 MiB peak | RSS growth -7.58 MiB after the first warm cycle; limit +16 MiB |

The acceptance executable is reproducible with:

```bash
cargo build --release --bins
./target/release/arch-v3-acceptance \
  --duration-seconds 60 \
  --cycle-delay-milliseconds 1000 \
  --startup-samples 20 \
  --output docs/v3-acceptance-report.json
```

The report records the repository commit, UTC generation time, machine model, processor, memory,
sample distributions, cycle count, page-load count, CPU cost trend, RSS trend, and pass/fail
outcomes for the V3 startup, corpus, duration, and resource thresholds.

## Acceptance matrix

| V3 requirement | Evidence | Status |
| --- | --- | --- |
| Startup P95 below 2 seconds | 20 fresh-profile process probes; measured P95 1,493.65 ms | Pass |
| 30-page resource trend remains bounded for 60 seconds | 60 complete cycles; CPU cost ratio 1.053x and RSS growth -7.58 MiB | Pass |
| All fixtures are readable | Corpus-wide title, text, link, image, and diagnostic assertions | Pass |
| Snapshot difference at or below 0.5% | 30 checked-in `1280x800` references and `fixture_snapshots_match_references` | Pass |
| Links, back, forward, and reload | Link resolution plus browser-core and session navigation tests | Pass |
| Stop and navigation cancellation | Navigation identity, stop invalidation, and stale-result rejection tests | Pass |
| Force-exit recovery | Cross-process `forced_exit_restores_spaces_bookmarks_tabs_and_selection` integration test | Pass |
| Space and tab independence | Store deletion test and global tab schema/migration tests | Pass |
| Recoverable error states | Invalid address, HTTP status, connection refusal, timeout, TLS-chain, parse, and viewport tests | Pass |
| Parser robustness | HTML and CSS fuzz targets in [GitHub Actions run 32691617342](https://github.com/shenzhepei/archetype-quick-browser/actions/runs/32691617342) | Pass |
| Storage migration and recovery | Empty database through migrations 1 and 2, legacy migration, restart recovery, and corrupt database preservation tests | Pass |
| Dependency licenses | Generated [`THIRD_PARTY_LICENSES.md`](../THIRD_PARTY_LICENSES.md), synchronized with `Cargo.lock` in CI | Pass |

The desktop shell also keeps each tab's rendered page independently, restores and reloads the
selected tab, scrolls the active tab into view, and performs cancellable page work outside the UI
thread. Startup probes repeatedly created the native window. Click-level Computer Use QA could not
be completed because the test Mac was locked; navigation, persistence, tab selection, close
selection, and cancellation behavior are covered by automated tests.

## Known limits

- V3 renders static HTML. It does not execute JavaScript or DOM mutation and does not implement
  forms, media, downloads, or printing.
- CSS excludes Flexbox, Grid, `position`, `float`, media queries, pseudo-classes, and
  pseudo-elements. Percentage heights resolve only when the containing height is definite.
- Documents use a single-process developer-preview architecture without an operating-system
  renderer sandbox.
- Stylesheets and images are restricted to the document's same origin.
- The native application uses macOS system-font fallback. Fixed Noto Sans SC is used only for
  deterministic snapshot tests.
- V3 targets Apple Silicon macOS and is not a signed public distribution.

The V4 ADR topics are recorded in section 11 of the
[`V3 detailed design`](./detailed-design/03-Archetype-V3-详设.md): renderer sandboxing, IPC encoding
and versioning, encrypted snapshots, web security policy, cross-origin loading, and JavaScript
engine selection.
