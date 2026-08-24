# Archetype V4 Acceptance Evidence

Archetype V4 is the `0.4.0` unsigned developer preview. This document records the reproducible
acceptance gates for the paired [V4 PRD](./prd/05-Archetype-V4-安全运行时-PRD.md) and
[detailed design](./detailed-design/05-Archetype-V4-安全运行时详设.md). The checked-in
[machine-readable report](./v4-acceptance-report.json) records the resource baseline for commit
`5728271576021492531c5891660fcacb23f54bb5`.

## Automated gates

| Requirement | Reproducible evidence | Status |
| --- | --- | --- |
| Stable boundary and protocol | `archetype-types`, `archetype-protocol`, codec, transport, compatibility and fuzz tests | Pass |
| Renderer isolation and recovery | Real child-process suite, including 100 forced terminate/restart/render cycles | Pass |
| macOS sandbox | Development sandbox probe plus ad-hoc signed entitlement allowlist verification | Pass |
| Session and forms | Cookie policy/profile migration tests and GET/POST form tests through the Browser broker | Pass |
| Flexbox | 20 V4 pages, layout assertions and all 50 fixed `1280x800` snapshots at the 0.5% threshold | Pass |
| Hibernation | Versioned metadata-only snapshots, dirty-form rejection, migration and wake tests | Pass |
| Quality | Formatting, strict Clippy, workspace tests, LCOV, license inventory and three fuzz targets | Pass |
| Support disclosure | [`html-css-support.json`](./html-css-support.json), validated during acceptance and packaging | Pass |

The `V4 Acceptance` workflow reruns these gates on demand and uploads the one-minute resource-trend
JSON for the exact commit. The trend compares normalized CPU cost in the first and second halves
and final versus initial RSS; it does not substitute a long-duration crash test. Runtime request
timeout (5 seconds), RSS (512 MiB), queued bytes (64 MiB), frame (16 MiB), and pending request (64)
limits are separately enforced by code and tests.

| Baseline | Result |
| --- | --- |
| Fresh-profile startup | P95 232.68 ms across 20 launches |
| Static page pipeline | P95 0.225 ms across 3,000 loads |
| Reference raster | P95 0.912 ms across 3,000 frames |
| CPU trend | 2.40% average; second/first-half cost ratio 1.028x |
| Memory trend | 30.11 MiB initial; 22.41 MiB final; -7.70 MiB growth |

## Release artifact

The `Release` workflow packages `arch-browser` and `archetype-runtime` together with binary and
archive SHA-256 checksums, both acceptance reports, the support matrix, license, and dependency attribution.
The artifact is intentionally unsigned and not notarized. It is a macOS developer build, not a
public production distribution; process isolation and the development sandbox probes must not be
described as equivalent to a notarized production App Sandbox bundle.

## Known limits

- Static HTML/CSS only: no JavaScript, DOM mutation, WebAssembly, Service Worker, Grid, media,
  downloads, printing, extension execution, or public SDK compatibility commitment.
- Forms cover text, password, checkbox, radio, select, submit, GET, and URL-encoded POST only.
- The Browser owns network, file and Cookie policy and brokers bounded bytes to Runtime. Runtime
  receives no Cookie headers, Cookie values, source file paths, or network access.
- macOS is the only supported desktop and sandbox target for V4. The release archive is unsigned
  and may require explicit local approval to run.
