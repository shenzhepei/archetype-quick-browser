use std::{env, path::PathBuf};

use archetype_sdk::{Engine, PageOptions, StaticDocument};
use futures_executor::block_on;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os().skip(1);
    let runtime = arguments
        .next()
        .map(PathBuf::from)
        .ok_or("usage: partner_render RUNTIME OUTPUT.png")?;
    let output = arguments
        .next()
        .map(PathBuf::from)
        .ok_or("usage: partner_render RUNTIME OUTPUT.png")?;
    let engine = block_on(Engine::builder().runtime_path(runtime).build())?;
    eprintln!("Runtime ready");
    let page = block_on(engine.create_page(PageOptions::new(640, 360)))?;
    let document = StaticDocument::new(
        "https://example.test/sdk-preview",
        "<!doctype html><title>SDK partner preview</title><style>body{background:#eef4ff;color:#173b72}main{display:flex;gap:24px}strong{color:#a52b3a}</style><main><h1>Archetype SDK</h1><strong>Rust UI neutral / 框架无关</strong></main>",
    )?;
    let navigation = block_on(page.render(document))?;
    eprintln!("Frame ready");
    navigation.frame().save_png(output)?;
    eprintln!("PNG saved");
    block_on(engine.shutdown())?;
    Ok(())
}
