use std::{collections::BTreeSet, fmt::Write as _, process::Command};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    version: String,
    source: Option<String>,
    license: Option<String>,
    repository: Option<String>,
    homepage: Option<String>,
}

fn main() -> Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("cargo")
        .args(["metadata", "--locked", "--format-version", "1"])
        .current_dir(&root)
        .output()
        .context("could not run cargo metadata")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let metadata: Metadata = serde_json::from_slice(&output.stdout)?;
    let mut dependencies = BTreeSet::new();
    for package in metadata
        .packages
        .into_iter()
        .filter(|package| package.source.is_some())
    {
        let license = package
            .license
            .context("dependency is missing license metadata")?;
        let source = package.repository.or(package.homepage).unwrap_or_else(|| {
            format!(
                "https://crates.io/crates/{}/{}",
                package.name, package.version
            )
        });
        dependencies.insert((package.name, package.version, license, source));
    }

    let mut document = String::from(
        "# Third-Party License Inventory\n\n\
         Generated from the locked Cargo dependency graph. Regenerate with \
         `cargo run -p arch-browser --example update_license_inventory`.\n\n\
         | Package | Version | License | Source |\n\
         | --- | --- | --- | --- |\n",
    );
    for (name, version, license, source) in dependencies {
        writeln!(
            document,
            "| `{name}` | `{version}` | `{}` | <{}> |",
            license.replace('|', "\\|"),
            source.replace('>', "%3E")
        )?;
    }
    std::fs::write(root.join("THIRD_PARTY_LICENSES.md"), document)?;
    Ok(())
}
