use std::{env, io::Write as _, path::PathBuf, thread, time::Duration};

use anyhow::{Context, Result};
use arch_browser::BrowserCore;
use url::Url;

fn main() -> Result<()> {
    let profile = PathBuf::from(env::args().nth(1).context("missing profile path")?);
    let first = fixture_url(env::args().nth(2).context("missing first fixture")?)?;
    let second = fixture_url(env::args().nth(3).context("missing second fixture")?)?;

    let mut core = BrowserCore::open_with_cookie_key_for_testing(profile, [0x5a; 32])?;
    let work = core.create_space("Work")?;
    let personal = core.create_space("Personal")?;
    let folder = core.create_bookmark_folder(&work.id, None, "References")?;
    let bookmark = core.create_bookmark(&work.id, Some(&folder.id), "Fixture", &first)?;
    let first_page = core.create_page(&first)?;
    core.navigate(&first_page, &first, 1280.0)?;
    let second_page = core.create_page(&second)?;
    core.navigate(&second_page, &second, 1280.0)?;
    core.save_selection(Some(&personal.id), Some(&second_page.id))?;

    println!(
        "READY\t{}\t{}\t{}\t{}\t{}\t{}",
        work.id, personal.id, folder.id, bookmark.id, first_page.id, second_page.id
    );
    std::io::stdout().flush()?;
    thread::sleep(Duration::from_secs(300));
    Ok(())
}

fn fixture_url(path: String) -> Result<Url> {
    Url::from_file_path(
        PathBuf::from(path)
            .canonicalize()
            .context("could not resolve fixture path")?,
    )
    .map_err(|()| anyhow::anyhow!("fixture path cannot be represented as URL"))
}
