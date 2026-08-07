use std::error::Error;
use std::fs;
use std::path::Path;

use clap::CommandFactory;

#[path = "src/artifacts.rs"]
mod artifacts;
#[path = "src/command.rs"]
mod command;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/artifacts.rs");
    println!("cargo:rerun-if-changed=src/command.rs");

    let manifest_directory = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace_root) = find_workspace_root(manifest_directory)? else {
        return Ok(());
    };
    println!(
        "cargo:rerun-if-changed={}",
        workspace_root.join("Cargo.toml").display()
    );

    let assets_directory = workspace_root.join("assets");
    let man_directory = assets_directory.join("man");
    let completions_directory = assets_directory.join("completions");
    fs::create_dir_all(&man_directory)?;
    fs::create_dir_all(&completions_directory)?;

    let (man_pages, completions) = artifacts::render(command::Cli::command())?;
    write_directory(&man_directory, &man_pages)?;
    write_directory(&completions_directory, &completions)?;

    Ok(())
}

fn find_workspace_root(
    manifest_directory: &Path,
) -> Result<Option<std::path::PathBuf>, Box<dyn Error>> {
    for ancestor in manifest_directory.ancestors() {
        let cargo_manifest = ancestor.join("Cargo.toml");
        if cargo_manifest.is_file() && fs::read_to_string(&cargo_manifest)?.contains("[workspace]")
        {
            return Ok(Some(ancestor.to_path_buf()));
        }
    }
    Ok(None)
}

fn write_directory(
    directory: &Path,
    files: &std::collections::BTreeMap<String, Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && !files.contains_key(&entry.file_name().to_string_lossy().into_owned())
        {
            fs::remove_file(entry.path())?;
        }
    }
    for (filename, contents) in files {
        fs::write(directory.join(filename), contents)?;
    }
    Ok(())
}
