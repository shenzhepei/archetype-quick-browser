# Third-Party Notices

Dependency versions are locked in `Cargo.lock`. Before distribution, generate and review the
complete license inventory. The V3 implementation currently uses crates under permissive or
file-level copyleft licenses.

The SVG files in `assets/icons/system/` were extracted from the generated
`@e-cloud/eslink-icons` sources and are used without their Vue/TypeScript runtime. The source
package declares the ISC license; the copied SVGs retain their original geometry and
`currentColor` behavior.

`cssparser` 0.37.0 is used as an unmodified Cargo dependency for standards-aware CSS tokenization
and rule parsing. Its source is maintained at <https://github.com/servo/rust-cssparser> and is
licensed under MPL-2.0. Quick Browser converts its parser output into project-owned value types and
does not copy Servo engine modules into the product.
