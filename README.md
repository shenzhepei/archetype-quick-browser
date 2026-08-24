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
Rust workspace provides a GPUI/gpui-component shell, title-bar tabs, compact Spaces, per-Space
bookmarks, navigation history, constrained local and HTTP(S) loading, HTML/CSS processing, basic
layout, and PNG/JPEG display.

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
custom directory during development or testing.

## Verify

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The workspace test suite compares all 30 fixture renders with the checked-in `1280x800` PNG
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

## Architecture

The workspace separates browser concerns into focused crates:

| Crate | Responsibility |
| --- | --- |
| `archetype-types`, `archetype-protocol` | Stable values, framed IPC, negotiation, routing, and bounded transports |
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

## V4 Development

- The stable type, protocol, C1 subprocess, and C2 resource-broker slices are complete: UUID V7 page IDs, monotonic
  navigation IDs, validated URL values, a versioned length-prefixed JSON codec, capability
  negotiation, bounded in-memory transport, request routing, cancellation, timeout handling,
  backpressure, protocol fuzzing, a real Renderer Runtime handshake, static document rendering,
  display-list return, asynchronous Browser supervision, structured disconnects, and a 20-cycle
  subprocess termination test. The Browser now transfers size-limited same-origin stylesheet and
  image bytes; Runtime parses and decodes those bytes without receiving paths or using `arch-net`.
- Local document subresources are restricted to the document directory tree, cross-origin and
  cross-origin redirect resources are rejected, and the final encoded request is checked against
  the 16 MiB protocol frame limit before it reaches the child process.
- The subprocess path is not yet the desktop application's default navigation path and is not a
  security sandbox. The next slice is D1 macOS sandboxing and resource supervision. V4 remains
  incomplete until desktop integration, signed sandbox evidence, session features, Flexbox,
  hibernation, and release acceptance all pass the paired specification.

## License

Licensed under the [Apache License 2.0](LICENSE). Third-party attribution details are documented in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
