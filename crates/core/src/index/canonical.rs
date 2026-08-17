use std::fs;
use std::path::{Path, PathBuf};

use crate::{Error, Record, StorePaths};

use super::content_hash;

pub(super) fn read_canonical(path: &Path) -> crate::Result<(Record, String)> {
    let bytes = fs::read(path).map_err(|source| Error::io("read a canonical record", source))?;
    let markdown =
        String::from_utf8(bytes).map_err(|_| Error::invalid_input(format!("{} is not valid UTF-8", path.display())))?;
    let record = crate::parse_markdown(path, &markdown)?;
    Ok((record, markdown))
}

pub(super) fn collect_markdown_paths(directory: &Path) -> crate::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_markdown_paths_inner(directory, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_markdown_paths_inner(directory: &Path, paths: &mut Vec<PathBuf>) -> crate::Result<()> {
    let entries = fs::read_dir(directory).map_err(|source| Error::io("scan canonical records", source))?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::io("scan canonical records", source))?;
        let file_type = entry
            .file_type()
            .map_err(|source| Error::io("inspect canonical records", source))?;
        if file_type.is_dir() {
            collect_markdown_paths_inner(&entry.path(), paths)?;
        } else if file_type.is_file() && entry.path().extension().is_some_and(|ext| ext == "md") {
            paths.push(entry.path());
        }
    }
    Ok(())
}

pub(super) fn canonical_fingerprint(paths: &StorePaths) -> crate::Result<String> {
    let mut hasher = blake3::Hasher::new();
    for path in collect_markdown_paths(&paths.records)? {
        let markdown =
            fs::read(&path).map_err(|source| Error::io("read canonical record for semantic freshness", source))?;
        fingerprint_value(&mut hasher, &path.display().to_string());
        fingerprint_value(
            &mut hasher,
            &content_hash(
                std::str::from_utf8(&markdown)
                    .map_err(|_| Error::invalid_input(format!("{} is not valid UTF-8", path.display())))?,
            ),
        );
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub(super) fn fingerprint_value(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}
