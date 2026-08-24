use std::{fs, path::Path};

use anyhow::{Context, Result};
use arch_browser::{
    render_url,
    snapshot::{SnapshotRenderer, VIEWPORT_WIDTH, save_snapshot},
};
use arch_net::Loader;
use url::Url;

fn main() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pages = root.join("fixtures/pages");
    let screenshots = root.join("fixtures/screenshots");
    let mut directories = fs::read_dir(&pages)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    directories.retain(|path| path.is_dir());
    directories.sort();

    let loader = Loader::default();
    let mut renderer = SnapshotRenderer::default();
    for directory in directories {
        let name = directory
            .file_name()
            .context("fixture has no directory name")?;
        let source = directory.join("index.html");
        let page = render_url(
            &loader,
            &Url::from_file_path(&source).map_err(|()| anyhow::anyhow!("invalid fixture path"))?,
            VIEWPORT_WIDTH,
        )?;
        let target = screenshots.join(name).with_extension("png");
        save_snapshot(&renderer.render(&page), &target)?;
        println!("updated {}", target.display());
    }
    Ok(())
}
