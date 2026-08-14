mod i18n;
mod ui;

use std::{env, path::PathBuf};

use anyhow::{Context, Result};
use arch_browser::render_url;
use arch_net::Loader;
use url::Url;

fn main() -> Result<()> {
    if env::args().nth(1).as_deref() == Some("--inspect") {
        inspect(env::args().nth(2))
    } else {
        ui::run();
        Ok(())
    }
}

fn inspect(input: Option<String>) -> Result<()> {
    let input = input.unwrap_or_else(|| "fixtures/pages/01-document/index.html".to_owned());
    let url = parse_input(&input)?;
    let page = render_url(&Loader::new()?, &url, 1280.0)?;
    println!("Archetype V3");
    println!("title: {}", page.title);
    println!("url: {}", page.final_url);
    println!("display commands: {}", page.display_list.commands.len());
    println!("content height: {:.1}px", page.display_list.content_height);
    for diagnostic in page.diagnostics {
        eprintln!("diagnostic: {diagnostic}");
    }
    Ok(())
}

fn parse_input(input: &str) -> Result<Url> {
    if let Ok(url) = Url::parse(input) {
        return Ok(url);
    }
    let path = PathBuf::from(input)
        .canonicalize()
        .with_context(|| format!("invalid path: {input}"))?;
    Url::from_file_path(path)
        .map_err(|()| anyhow::anyhow!("path cannot be represented as file URL"))
}
