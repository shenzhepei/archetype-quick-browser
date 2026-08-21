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

## Features

- Desktop tabs, Spaces, nested bookmarks, and persisted navigation history.
- HTML5 parsing and an initial CSS parser, cascade, style, layout, and paint pipeline.
- Constrained local and HTTP(S) resource loading with classified error pages.
- PNG and JPEG decoding, recursive block layout, inline text, borders, and backgrounds.
- SQLite-backed browser state with corrupt-profile recovery.

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

## Verify

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
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
| `arch-browser` | Desktop shell, orchestration, localization, and rendering integration |
| `arch-html`, `arch-dom` | HTML parsing and document representation |
| `arch-css`, `arch-style` | CSS parsing, cascade, and computed style |
| `arch-layout`, `arch-paint` | Layout and display-list generation |
| `arch-net` | Constrained document and resource loading |
| `arch-session`, `arch-store` | Navigation state and SQLite persistence |

The scoped product requirements and detailed implementation plans are in:

- [`docs/prd/03-Archetype-V3-PRD.md`](docs/prd/03-Archetype-V3-PRD.md)
- [`docs/detailed-design/03-Archetype-V3-详设.md`](docs/detailed-design/03-Archetype-V3-详设.md)

## Current Coverage

- Implemented: workspace and CI; desktop shell with title-bar tabs, compact Space switching, and
  per-Space root bookmarks; DOM and HTML parsing; initial CSS parser/cascade;
  recursive block boxes and inline text runs; serializable display lists with positioned text,
  images, backgrounds, and borders; text color, font family, weight, style, alignment, line height,
  and white-space rendering; constrained document, stylesheet, PNG, and JPEG loading with image
  fallbacks; classified error pages; SQLite Space/Page persistence with corrupt-profile recovery;
  global tab persistence, hierarchical Space bookmark storage and root bookmark bar, navigation identity,
  redirects, links, and history; deterministic fixtures with a corpus-wide
  render test.
- Remaining: complete the CSS/layout support matrix, grow the corpus to 30 fixtures, improve font
  shaping and link interaction, add screenshot regression and fuzzing, record performance
  baselines, and gather release acceptance evidence.

## License

Licensed under the [Apache License 2.0](LICENSE). Third-party attribution details are documented in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
