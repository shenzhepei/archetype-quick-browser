mod i18n;
mod logging;
mod ui;

use std::{env, path::PathBuf};

use anyhow::{Context, Result};
use arch_browser::render_url;
use arch_net::Loader;
use url::Url;

fn main() -> Result<()> {
    let inspect_mode = env::args().nth(1).as_deref() == Some("--inspect");
    if let Err(error) = logging::init() {
        eprintln!("could not initialize local logging: {error}");
    }
    logging::application_started(inspect_mode);
    let result = if inspect_mode {
        inspect(env::args().nth(2))
    } else {
        ui::run();
        Ok(())
    };
    if let Err(error) = &result {
        logging::application_failed(&format!("{error:#}"));
    }
    result
}

fn inspect(input: Option<String>) -> Result<()> {
    let input = input.unwrap_or_else(|| "fixtures/pages/01-document/index.html".to_owned());
    let url = parse_input(&input)?;
    let page = render_url(&Loader::new()?, &url, 1280.0)?;
    logging::inspection_completed(
        page.final_url.as_str(),
        &page.title,
        page.display_list.commands.len(),
        page.diagnostics.len(),
    );
    println!("Archetype V3");
    println!("title: {}", page.title);
    println!("url: {}", page.final_url);
    println!("display commands: {}", page.display_list.commands.len());
    println!("content height: {:.1}px", page.display_list.content_height);
    for diagnostic in page.diagnostics {
        logging::render_diagnostic(None, &diagnostic);
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
