use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use stormbuffer_core::StoreScope;

use crate::command::SkillInstallArgs;
use crate::echo::Echo;
use crate::report_error;

const PROJECT_SKILL: &str = include_str!("../assets/stormbuffer-memory.md");
const SKILL_FILE: &str = "SKILL.md";

pub(crate) fn run_install(scope: StoreScope, arguments: SkillInstallArgs, output: &Echo) -> i32 {
    match install(scope, &arguments.directory, arguments.force) {
        Ok(path) => {
            output.line(&format!(
                "{} {}",
                output.success("Installed"),
                output.path(path.display())
            ));
            0
        }
        Err(error) => report_error(error, output),
    }
}

fn install(scope: StoreScope, directory: &Path, force: bool) -> Result<PathBuf> {
    let (skill_directory, contents) = match scope {
        StoreScope::Global => ("stormbuffer-global-memory", global_skill()),
        StoreScope::Project => ("stormbuffer-memory", PROJECT_SKILL.to_owned()),
    };
    let destination = directory.join(skill_directory).join(SKILL_FILE);

    let replace = match fs::read(&destination) {
        Ok(existing) if existing == contents.as_bytes() => return Ok(destination),
        Ok(_) if !force => {
            anyhow::bail!(
                "{} already contains different content; rerun with --force to replace it",
                destination.display()
            );
        }
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => force,
        Err(error) => {
            return Err(error).with_context(|| format!("could not read {}", destination.display()));
        }
    };

    if let Err(error) = write_atomic(&destination, contents.as_bytes(), replace) {
        if !replace && error.kind() == io::ErrorKind::AlreadyExists {
            anyhow::bail!(
                "{} already contains different content; rerun with --force to replace it",
                destination.display()
            );
        }
        return Err(error).with_context(|| format!("could not install {}", destination.display()));
    }
    Ok(destination)
}

fn global_skill() -> String {
    PROJECT_SKILL
        .replacen("name: stormbuffer-memory", "name: stormbuffer-global-memory", 1)
        .replacen(
            "Use Stormbuffer's public CLI JSON or MCP interfaces when work depends on prior project decisions, conventions, commands, architecture, or unfinished work; retrieve and cite evidence, then propose only small durable memories.",
            "Use Stormbuffer's global store when work depends on cross-project preferences, decisions, conventions, procedures, or unfinished context; retrieve and cite evidence, then propose only small durable memories.",
            1,
        )
        .replace("--project", "--global")
        .replacen(
            "Keep the project store as the default boundary",
            "Keep the global store as the boundary",
            1,
        )
        .replacen(
            "Project retrieval can also return global records. Ignore them unless the task asks for global\ncontext or a record directly constrains this project. Never widen scope merely because a result\nis available.",
            "Global retrieval stays within the global store. Never widen beyond that boundary merely because\nother memory is available.",
            1,
        )
}

fn write_atomic(path: &Path, contents: &[u8], replace: bool) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("skill path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let (temporary_path, mut temporary_file) = create_temporary_file(path)?;
    let mut cleanup = TemporaryCleanup(Some(temporary_path.clone()));
    temporary_file.write_all(contents)?;
    temporary_file.sync_all()?;
    drop(temporary_file);
    if replace {
        replace_file(&temporary_path, path)?;
    } else {
        install_new_file(&temporary_path, path)?;
    }
    sync_directory(parent)?;
    cleanup.0 = None;
    Ok(())
}

fn install_new_file(from: &Path, to: &Path) -> io::Result<()> {
    fs::hard_link(from, to)?;
    fs::remove_file(from)
}

fn create_temporary_file(path: &Path) -> io::Result<(PathBuf, File)> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("skill path has no parent directory"))?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    for attempt in 0..1000u32 {
        let candidate = parent.join(format!(
            ".{name}.{}.{}.tmp",
            std::process::id(),
            stamp + u128::from(attempt)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a temporary skill file",
    ))
}

struct TemporaryCleanup(Option<PathBuf>);

impl Drop for TemporaryCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(not(windows))]
fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)
}

#[cfg(windows)]
fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        // Both vectors are NUL-terminated and remain alive for this call.
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "stormbuffer-skill-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary directory");
        path
    }

    #[test]
    fn global_variant_uses_the_global_store_explicitly() {
        let skill = global_skill();
        assert!(skill.contains("name: stormbuffer-global-memory"));
        assert!(skill.contains("cross-project preferences"));
        assert!(skill.contains("sbuf --global invoke search"));
        assert!(!skill.contains("--project"));
        assert!(skill.contains("stormbuffer-mcp --stdio --global"));
        assert!(!skill.contains("Project retrieval can also return global records"));
    }

    #[test]
    fn no_replace_commit_preserves_a_racing_destination() {
        let directory = temporary_directory("race");
        let temporary = directory.join("temporary");
        let destination = directory.join("SKILL.md");
        fs::write(&temporary, "shipped").expect("write temporary file");
        fs::write(&destination, "racing writer").expect("write destination");

        let error = install_new_file(&temporary, &destination).expect_err("destination conflicts");

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read_to_string(&destination).expect("read destination"),
            "racing writer"
        );
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }
}
