# Archetype Quick Browser

Archetype V3 is a developer-preview browser for static HTML documents. The current implementation
provides a GPUI/gpui-component desktop shell, title-bar tabs, compact Spaces, per-Space bookmarks,
navigation history, constrained
local and HTTP(S) loading, HTML/CSS processing, basic layout, and PNG/JPEG display.

## Run

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

The scoped product requirements and detailed implementation plan are in:

- [`docs/prd/03-Archetype-V3-PRD.md`](docs/prd/03-Archetype-V3-PRD.md)
- [`docs/detailed-design/03-Archetype-V3-详设.md`](docs/detailed-design/03-Archetype-V3-详设.md)

## Current Coverage

- Implemented: workspace and CI; desktop shell with title-bar tabs, compact Space switching, and
  per-Space root bookmarks; DOM and HTML parsing; initial CSS parser/cascade;
  recursive block boxes and inline text runs; serializable display lists with positioned text,
  images, backgrounds, and borders; text color, weight, style, alignment, line height, and
  white-space rendering; constrained document, stylesheet, PNG, and JPEG loading with image
  fallbacks; classified error pages; SQLite Space/Page persistence with corrupt-profile recovery;
  global tab persistence, hierarchical Space bookmark storage and root bookmark bar, navigation identity,
  redirects, links, and history; deterministic fixtures with a corpus-wide
  render test.
- Remaining: complete the CSS/layout support matrix, grow the corpus to 30 fixtures, improve font
  shaping and link interaction, add screenshot regression and fuzzing, record performance
  baselines, and gather release acceptance evidence.
