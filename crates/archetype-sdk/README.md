# archetype-sdk

`archetype-sdk 0.1` is the UI-framework-independent Rust client for the Archetype V5 developer
preview. It starts a matching `archetype-runtime`, renders caller-provided static HTML and bounded
same-origin resources, and returns owned RGBA8 frames without exposing GPUI, DOM, layout, paint, or
protocol types.

```rust
let engine = Engine::builder().runtime_path(runtime).build().await?;
let page = engine.create_page(PageOptions::new(1280, 800)).await?;
let document = StaticDocument::new("https://example.test", "<h1>Hello</h1>")?;
let navigation = page.render(document).await?;
navigation.frame().save_png("frame.png")?;
engine.shutdown().await?;
```

The SDK requires Rust 1.85 or newer and currently supports Protocol v4.1 with Runtime `0.6.x` on
macOS. This is an unsigned static-rendering developer preview, not an SDK 1.0 compatibility or
production sandbox/signing commitment.
