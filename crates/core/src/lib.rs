use std::env;
use std::error::Error as StdError;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io {
        operation: &'static str,
        source: io::Error,
    },
    InvalidWorkingDirectory,
    MissingHomeDirectory,
    InvalidInput(String),
}

impl Error {
    fn io(operation: &'static str, source: io::Error) -> Self {
        Self::Io { operation, source }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { operation, source } => write!(formatter, "could not {operation}: {source}"),
            Self::InvalidWorkingDirectory => {
                formatter.write_str("the current working directory is not available")
            }
            Self::MissingHomeDirectory => formatter.write_str(
                "could not determine the user data directory; set HOME or the platform equivalent",
            ),
            Self::InvalidInput(message) => formatter.write_str(message),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidWorkingDirectory | Self::MissingHomeDirectory | Self::InvalidInput(_) => {
                None
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreScope {
    Global,
    Project,
}

impl StoreScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project => "project",
        }
    }
}

impl fmt::Display for StoreScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformDirs {
    data_root: PathBuf,
    cache_root: PathBuf,
}

impl PlatformDirs {
    pub fn from_environment() -> Result<Self> {
        let home = home_directory().ok_or(Error::MissingHomeDirectory)?;
        let (data_root, cache_root) = match env::consts::OS {
            "macos" => (
                env_path("XDG_DATA_HOME")
                    .unwrap_or_else(|| home.join("Library").join("Application Support")),
                env_path("XDG_CACHE_HOME").unwrap_or_else(|| home.join("Library").join("Caches")),
            ),
            "windows" => {
                let data = env_path("LOCALAPPDATA")
                    .or_else(|| env_path("APPDATA"))
                    .unwrap_or_else(|| home.join("AppData").join("Local"));
                (data.clone(), data)
            }
            _ => (
                env_path("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local").join("share")),
                env_path("XDG_CACHE_HOME").unwrap_or_else(|| home.join(".cache")),
            ),
        };

        Ok(Self {
            data_root,
            cache_root,
        })
    }

    pub fn new(data_root: PathBuf, cache_root: PathBuf) -> Self {
        Self {
            data_root,
            cache_root,
        }
    }

    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    pub fn cache_root(&self) -> &Path {
        &self.cache_root
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorePaths {
    pub scope: StoreScope,
    pub root: PathBuf,
    pub records: PathBuf,
    pub cache: PathBuf,
}

impl StorePaths {
    fn new(scope: StoreScope, root: PathBuf, cache: PathBuf) -> Self {
        Self {
            scope,
            records: root.join("records"),
            root,
            cache,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreStatus {
    pub scope: StoreScope,
    pub root: PathBuf,
    pub initialized: bool,
    pub record_count: usize,
}

pub fn resolve_store(scope: StoreScope, cwd: &Path) -> Result<StorePaths> {
    let dirs = PlatformDirs::from_environment()?;
    resolve_store_with_dirs(scope, cwd, &dirs)
}

pub fn resolve_store_with_dirs(
    scope: StoreScope,
    cwd: &Path,
    dirs: &PlatformDirs,
) -> Result<StorePaths> {
    if !cwd.is_dir() {
        return Err(Error::InvalidWorkingDirectory);
    }

    let root = match scope {
        StoreScope::Global => dirs.data_root().join("stormbuffer"),
        StoreScope::Project => project_store_root(cwd),
    };
    let cache = dirs.cache_root().join("stormbuffer");
    let paths = StorePaths::new(scope, root, cache);
    tracing::debug!(scope = %scope, root = ?paths.root, "resolved store");
    Ok(paths)
}

pub fn initialize_store(paths: &StorePaths) -> Result<bool> {
    fs::create_dir_all(&paths.root)
        .map_err(|source| Error::io("create the store directory", source))?;
    fs::create_dir_all(&paths.records)
        .map_err(|source| Error::io("create the records directory", source))?;
    fs::create_dir_all(&paths.cache)
        .map_err(|source| Error::io("create the cache directory", source))?;

    let marker = paths.root.join("store.toml");
    let created = create_marker(&marker, paths.scope)?;

    if paths.scope == StoreScope::Project {
        create_file_if_missing(&paths.root.join(".gitignore"), b"*\n!.gitignore\n")?;
    }

    tracing::info!(scope = %paths.scope, created, "initialized store");
    Ok(created)
}

pub fn inspect_store(paths: &StorePaths) -> Result<StoreStatus> {
    let initialized = paths.root.join("store.toml").is_file();
    let record_count = if initialized && paths.records.is_dir() {
        count_markdown_files(&paths.records)?
    } else {
        0
    };

    Ok(StoreStatus {
        scope: paths.scope,
        root: paths.root.clone(),
        initialized,
        record_count,
    })
}

fn project_store_root(cwd: &Path) -> PathBuf {
    let mut current = cwd;
    loop {
        let candidate = current.join(".stormbuffer");
        if candidate.is_dir() {
            return candidate;
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return cwd.join(".stormbuffer"),
        }
    }
}

fn count_markdown_files(directory: &Path) -> Result<usize> {
    let mut count = 0;
    let entries =
        fs::read_dir(directory).map_err(|source| Error::io("inspect the records", source))?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::io("inspect the records", source))?;
        let file_type = entry
            .file_type()
            .map_err(|source| Error::io("inspect the records", source))?;
        if file_type.is_dir() {
            count += count_markdown_files(&entry.path())?;
        } else if file_type.is_file() && entry.path().extension().is_some_and(|ext| ext == "md") {
            count += 1;
        }
    }
    Ok(count)
}

fn create_marker(path: &Path, scope: StoreScope) -> Result<bool> {
    let contents = format!(
        "format_version = 1\nscope = \"{}\"\nvisibility = \"private\"\n",
        scope.as_str()
    );
    create_file_if_missing(path, contents.as_bytes())
}

fn create_file_if_missing(path: &Path, contents: &[u8]) -> Result<bool> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(contents)
                .map_err(|source| Error::io("write store metadata", source))?;
            file.sync_all()
                .map_err(|source| Error::io("sync store metadata", source))?;
            Ok(true)
        }
        Err(source) if source.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(source) => Err(Error::io("create store metadata", source)),
    }
}

fn home_directory() -> Option<PathBuf> {
    env_path("HOME").or_else(|| env_path("USERPROFILE"))
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("stormbuffer-core-{name}-{suffix}"));
        fs::create_dir_all(&path).expect("create test directory");
        path
    }

    #[test]
    fn project_resolution_discovers_the_nearest_store() {
        let root = temporary_directory("resolution");
        let nested = root.join("one").join("two");
        fs::create_dir_all(&nested).expect("create nested directory");
        let dirs = PlatformDirs::new(root.join("data"), root.join("cache"));
        let project = root.join(".stormbuffer");
        fs::create_dir_all(&project).expect("create project store");

        let paths = resolve_store_with_dirs(StoreScope::Project, &nested, &dirs).expect("resolve");
        assert_eq!(paths.root, project);

        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn initialization_is_idempotent_and_status_counts_records() {
        let root = temporary_directory("initialization");
        let dirs = PlatformDirs::new(root.join("data"), root.join("cache"));
        let paths = resolve_store_with_dirs(StoreScope::Project, &root, &dirs).expect("resolve");

        assert!(initialize_store(&paths).expect("initialize store"));
        assert!(!initialize_store(&paths).expect("initialize store again"));
        fs::write(paths.records.join("example.md"), "not a parsed record yet")
            .expect("write record");

        let status = inspect_store(&paths).expect("inspect store");
        assert!(status.initialized);
        assert_eq!(status.record_count, 1);

        fs::remove_dir_all(root).expect("remove test directory");
    }
}
