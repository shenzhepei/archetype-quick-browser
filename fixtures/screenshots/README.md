# V3 Reference Snapshots

This directory contains the 72 fixed `1280x800 @1x` PNG references used by the V3-V7 corpus.
Tests render each fixture through the production document pipeline and a deterministic DisplayList
rasterizer, then fail when more than 0.5% of pixels differ from its reference.

The rasterizer uses the repository-owned `NotoSansSC-Regular.otf` test asset for repeatability
across macOS releases. Application builds continue to use GPUI and the user's system fonts.

Regenerate references only after reviewing an intentional rendering change:

```bash
cargo run -p arch-browser --example update_snapshots
cargo test -p arch-browser snapshot::tests::fixture_snapshots_match_references
```
