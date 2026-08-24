# Third-Party Notices

Dependency versions are locked in `Cargo.lock`. The complete generated crate inventory, including
versions, declared licenses, and upstream sources, is in [`THIRD_PARTY_LICENSES.md`](THIRD_PARTY_LICENSES.md).
CI verifies that this inventory remains synchronized with the locked dependency graph.

The SVG files in `assets/icons/system/` were extracted from the generated
`@e-cloud/eslink-icons` sources and are used without their Vue/TypeScript runtime. The source
package declares the ISC license; the copied SVGs retain their original geometry and
`currentColor` behavior.

`cssparser` 0.37.0 is used as an unmodified Cargo dependency for standards-aware CSS tokenization
and rule parsing. Its source is maintained at <https://github.com/servo/rust-cssparser> and is
licensed under MPL-2.0. Quick Browser converts its parser output into project-owned value types and
does not copy Servo engine modules into the product.

`cosmic-text` 0.14.2 is used as an unmodified Cargo dependency for deterministic V3 reference
snapshot text shaping, system-font fallback, and glyph rasterization. It is maintained at
<https://github.com/pop-os/cosmic-text> and is dual-licensed under MIT or Apache-2.0.

`NotoSansSC-Regular.otf` is used only by the deterministic snapshot renderer so reference images do
not vary with the host macOS font version. The font is maintained by the Noto CJK project at
<https://github.com/notofonts/noto-cjk> and is licensed under the SIL Open Font License 1.1; the
license text is stored beside the font in `assets/fonts/NotoSansSC/OFL.txt`.
