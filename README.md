# Archetype Quick Browser

<!-- repo-languages:start -->
English | [简体中文](README-zh-CN.md)
<!-- repo-languages:end -->

[![Rust](https://img.shields.io/badge/Rust-1.85%2B-000000?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![GPUI](https://img.shields.io/badge/GPUI-0.2.2-2F80ED?style=flat-square)](https://www.gpui.rs/)
[![CI](https://img.shields.io/github/actions/workflow/status/shenzhepei/archetype-quick-browser/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/shenzhepei/archetype-quick-browser/actions/workflows/ci.yml)

<!-- repo-badges:start -->
[![Test Coverage](https://img.shields.io/codecov/c/github/shenzhepei/archetype-quick-browser?style=flat-square&logo=codecov)](https://codecov.io/gh/shenzhepei/archetype-quick-browser)
[![License](https://img.shields.io/github/license/shenzhepei/archetype-quick-browser?style=flat-square)](https://github.com/shenzhepei/archetype-quick-browser/blob/HEAD/LICENSE)
[![Sponsor](https://img.shields.io/github/sponsors/shenzhepei?style=flat-square&logo=githubsponsors&label=Sponsor)](https://github.com/sponsors/shenzhepei)
<!-- repo-badges:end -->

Archetype Quick Browser is a developer-preview desktop browser for static HTML documents. Its
Rust workspace provides a GPUI/gpui-component shell, a brokered Renderer Runtime, encrypted Cookie
profiles, basic forms, Flexbox, hibernating tabs, constrained local and HTTP(S) loading, and
PNG/JPEG display. V5 also provides an executor-neutral Rust SDK with owned RGBA frames.

> [!NOTE]
> This project is an engine prototype for development and experimentation, not a production web
> browser. The current desktop application and CI target macOS.

![Deterministic Archetype rendering preview](fixtures/screenshots/07-box-paint.png)

_Deterministic Archetype rendering preview from the checked-in V3 fixture corpus._

## Features

- Desktop tabs, Spaces, nested bookmarks, and persisted navigation history.
- HTML5 parsing and an initial CSS parser, cascade, style, layout, and paint pipeline.
- Constrained local and HTTP(S) resource loading with classified TLS, parsing, and rendering error pages.
- PNG and JPEG decoding, recursive block layout, inline text, borders, and backgrounds.
- SQLite-backed browser state with corrupt-profile recovery.
- Cancellable background navigation, independent per-tab rendered pages, and active-tab overflow scrolling.
- Browser-brokered GET/POST sessions with encrypted persistent Cookies and basic interactive forms.
- A supervised Renderer Runtime with bounded IPC, crash recovery, macOS sandbox probes, and metadata-only tab hibernation.
- Flexbox direction, wrapping, alignment, growth, shrinkage, and gaps.
- V6 custom properties, width media queries, Flex item basis/order, relative/absolute positioning, and basic z-index.
- `archetype-sdk 0.1` Engine/Page APIs, Runtime lifecycle, structured events, integrity checks, and RGBA8 frames.

## Run

Install the Rust toolchain declared in `rust-toolchain.toml`, then run:

```bash
cargo run -p arch-browser
```

The desktop app stores its profile in the platform application-support directory. New pages open
the first deterministic fixture by default. The desktop shell follows the operating system locale:
Chinese locales (`zh*`) use Chinese, while every other locale falls back to English. To inspect a
page through the rendering pipeline without opening a window:

```bash
cargo run -p arch-browser -- --inspect fixtures/pages/05-image/index.html
```

The application writes newline-delimited JSON diagnostics to
`<application-support>/Archetype/logs/archetype.jsonl`. Logs remain local and the application does
not collect or upload telemetry. Set `ARCHETYPE_DATA_DIR` to isolate both the profile and logs in a
custom directory during development or testing. The build messages printed by `cargo run` come
from Cargo and are absent when launching a packaged binary; the local application diagnostics are
intentional in both development and release builds.

## Verify

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The workspace test suite compares all 50 fixture renders with the checked-in `1280x800` PNG
references. After an intentional rendering change, regenerate and review them with:

```bash
cargo run -p arch-browser --example update_snapshots
```

The restart-recovery integration test force-terminates a separate browser-profile process before
checking that Spaces, nested bookmarks, tabs, titles, order, and selection survive. HTML, CSS, and
versioned protocol fuzzing runs in CI. To reproduce the fuzz targets locally, install nightly Rust and
[`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz), then run:

```bash
cargo +nightly fuzz run html -- -max_total_time=20 -timeout=5
cargo +nightly fuzz run css -- -max_total_time=20 -timeout=5
cargo +nightly fuzz run protocol -- -max_total_time=20 -timeout=5
```

To reproduce the V3 startup, performance, memory, and one-minute resource-trend evidence, build
all release binaries and run:

```bash
cargo build --release --bins
./target/release/arch-v3-acceptance \
  --duration-seconds 60 \
  --cycle-delay-milliseconds 1000 \
  --startup-samples 20 \
  --output docs/v3-acceptance-report.json
```

To generate the same LCOV report uploaded by CI, install
[`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) and run:

```bash
mkdir -p coverage
cargo llvm-cov --workspace --lcov --output-path coverage/lcov.info
```

The `V4 Acceptance` workflow now runs the current 72-page variant of the one-minute trend probe together with
the Runtime recovery, sandbox, entitlement, support-matrix, and workspace quality gates.

The `V6 Acceptance` workflow extends the deterministic corpus to 62 pages and verifies responsive
style selection at multiple viewport widths, positioning, the Runtime/SDK path, and the same
one-minute CPU/RSS trend gate.

The `V7 Acceptance` workflow extends the corpus to 72 pages and verifies bounded Grid tracks,
row-major placement, rounded corners, opacity, single outer shadows, text decoration, and the
same Runtime/SDK and one-minute resource gates.

To run the V5 UI-neutral partner example:

```bash
cargo build -p archetype-runtime --bin archetype-runtime
cargo run -p archetype-sdk --example partner_render -- \
  target/debug/archetype-runtime artifacts/sdk-partner.png
```

## Architecture

The workspace separates browser concerns into focused crates:

| Crate | Responsibility |
| --- | --- |
| `archetype-types`, `archetype-protocol` | Stable values, framed IPC, negotiation, routing, and bounded transports |
| `archetype-sdk`, `archetype-raster` | UI-neutral async client, Runtime lifecycle, owned RGBA frames, and deterministic rasterization |
| `archetype-runtime` | Static document renderer subprocess and framed command loop |
| `arch-browser` | Desktop shell, orchestration, localization, and rendering integration |
| `arch-html`, `arch-dom` | HTML parsing and document representation |
| `arch-css`, `arch-style` | CSS parsing, cascade, and computed style |
| `arch-layout`, `arch-paint` | Layout and display-list generation |
| `arch-net` | Constrained document and resource loading |
| `arch-session`, `arch-store` | Navigation state and SQLite persistence |

The scoped product requirements and detailed implementation plans are in:

- [`docs/prd/03-Archetype-V3-PRD.md`](docs/prd/03-Archetype-V3-PRD.md)
- [`docs/detailed-design/03-Archetype-V3-详设.md`](docs/detailed-design/03-Archetype-V3-详设.md)
- [`docs/prd/05-Archetype-V4-安全运行时-PRD.md`](docs/prd/05-Archetype-V4-安全运行时-PRD.md)
- [`docs/detailed-design/05-Archetype-V4-安全运行时详设.md`](docs/detailed-design/05-Archetype-V4-安全运行时详设.md)
- [`docs/v3-acceptance.md`](docs/v3-acceptance.md) and its
  [machine-readable report](docs/v3-acceptance-report.json)
- [`docs/v4-acceptance.md`](docs/v4-acceptance.md), its [machine-readable resource report](docs/v4-acceptance-report.json),
  and the machine-readable [HTML/CSS support matrix](docs/html-css-support.json)
- [`docs/prd/06-Archetype-V5-Rust-SDK预览-PRD.md`](docs/prd/06-Archetype-V5-Rust-SDK预览-PRD.md),
  its [detailed design](docs/detailed-design/06-Archetype-V5-Rust-SDK预览详设.md),
  [acceptance evidence](docs/v5-acceptance.md), and [compatibility matrix](docs/sdk-compatibility.json)
- [`docs/prd/07-Archetype-V6-静态响应式CSS-PRD.md`](docs/prd/07-Archetype-V6-静态响应式CSS-PRD.md),
  its [detailed design](docs/detailed-design/07-Archetype-V6-静态响应式CSS详设.md), and
  [acceptance evidence](docs/v6-acceptance.md) with a
  [machine-readable resource report](docs/v6-acceptance-report.json)
- [`docs/prd/08-Archetype-V7-Grid与视觉CSS-PRD.md`](docs/prd/08-Archetype-V7-Grid与视觉CSS-PRD.md),
  its [detailed design](docs/detailed-design/08-Archetype-V7-Grid与视觉CSS详设.md), and
  [acceptance evidence](docs/v7-acceptance.md)

## V3 Status

- V3 complete: workspace and CI; desktop shell with title-bar tabs, compact Space switching, and
  per-Space root bookmarks; DOM and HTML parsing; initial CSS parser/cascade;
  recursive block boxes and inline text runs; serializable display lists with positioned text,
  images, backgrounds, and borders; text color, font family, weight, style, alignment, line height,
  white-space, and overflow clipping; constrained document, stylesheet, PNG, and JPEG loading with
  image fallbacks; classified error pages; SQLite Space/Page persistence with corrupt-profile recovery;
  local structured JSONL diagnostics without telemetry;
  global tab persistence, hierarchical Space bookmark storage and root bookmark bar, navigation identity,
  redirects, links, and history; all 30 deterministic fixtures with corpus-wide assertions for
  titles, painted text, resolved links, loaded images, expected diagnostics, and fixed PNG
  screenshot regression at the V3 0.5% pixel-difference threshold; force-exit profile recovery;
  CI fuzzing of the HTML and CSS parser entry points; cancellable background navigation; independent
  per-tab render state; and recorded startup, page-pipeline, frame, CPU, and RSS trend evidence.
- Known limits and the evidence for every acceptance requirement are maintained in
  [`docs/v3-acceptance.md`](docs/v3-acceptance.md). JavaScript, forms, media, Flexbox, Grid,
  multi-process rendering, sandboxing, and public signed distribution remain outside V3 scope.

## V4 Status

- V4 complete: stable IDs and versioned bounded IPC; authenticated Renderer Runtime supervision;
  Browser-owned file, network, Cookie, GET and POST brokers; 100-cycle child termination recovery;
  macOS development sandbox and signed-entitlement probes; encrypted persistent Cookie profiles;
  interactive basic forms; Flexbox; metadata-only clean-tab hibernation; and 50 deterministic
  screenshot fixtures.
- The Release workflow packages both required binaries, SHA-256 checksums, acceptance evidence,
  licenses, and the machine-readable support matrix. Its artifacts are unsigned, not notarized,
  and are developer previews rather than public production distributions.
- JavaScript, Grid, media, complete forms, public SDK compatibility, production signing,
  notarization, and automatic updates remain outside V4 scope.

## V5 Status

- V5 complete: `archetype-sdk 0.1.0` starts and authenticates Runtime `0.5.x`, creates independent
  pages, validates bounded same-origin inputs, rejects stale navigation results, publishes
  structured events, and returns tightly packed owned RGBA8 frames without exposing GPUI, DOM,
  layout, DisplayList, or protocol types.
- The partner example renders English, Chinese, and Flexbox content to PNG. SDK-level failure tests
  cover correct and incorrect Runtime SHA-256, graceful shutdown, disconnect events, event pressure,
  and 100 terminate/restart/render cycles.
- V5 remains an Apple Silicon macOS developer preview. SDK 1.0, JavaScript, Runtime-owned network,
  production signing, notarization, Windows, Linux, shared-memory frames, and GPU handles are not implemented.

## V6 Status

- V6 is complete: inherited custom properties with bounded `var()` fallback resolution, `screen`/`all`
  min/max-width media queries, Flex `basis`/`order`, relative/absolute positioning, percentage
  offsets, and stable basic z-index painting.
- The deterministic corpus contains 62 pages. Browser, Runtime, and SDK use the caller's actual
  viewport width; Protocol v4.1 also carries viewport height for positioned initial containing blocks.
- Grid, fixed/sticky positioning, general media queries, transitions, animation and GPU compositing
  remain unsupported and are reported separately in the machine-readable support matrix.

## V7 Status

- V7 is complete: bounded fixed, percentage, `fr`, and `repeat()` Grid columns with row-major
  placement; independent row/column gaps; color `background` shorthand; rounded corners; element
  opacity; one bounded outer shadow; and underline/line-through text decoration.
- The deterministic corpus contains 72 pages, including 10 V7 fixtures. The original 62 reference
  images remain byte-for-byte unchanged after regenerating the corpus.
- The [one-minute V7 acceptance report](docs/v7-acceptance-report.json) completed 4,248 page loads;
  second-half CPU cost per page was 99.47% of the first half and RSS grew by 384 KiB.
- Advanced Grid placement, spans, `minmax()`, subgrid, multiple/inset shadows, gradients,
  transitions, animation, JavaScript and GPU compositing remain unsupported.

## License

Licensed under the [Apache License 2.0](LICENSE). Third-party attribution details are documented in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
