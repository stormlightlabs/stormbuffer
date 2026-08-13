use std::env;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod backup;
mod codec;
mod embedder;
mod evaluation;
mod feedback;
mod index;
mod invoke;
mod record;
mod repository;
mod vector;

pub use backup::*;
pub use embedder::*;
pub use evaluation::*;
pub use feedback::*;
pub use index::*;
pub use invoke::*;
pub use record::*;

pub use codec::{parse_markdown, render_markdown};
pub use repository::{ProposalResult, RecordRepository, RepositoryError, StoredRecord};
pub use vector::{SqliteVectorIndex, VectorFilter, VectorHit, VectorIndex, VectorMetadata};

const STORE_FORMAT_VERSION: u32 = 1;
const PRIVATE_PROJECT_GITIGNORE: &[u8] = b"*\n!.gitignore\n";
const SHARED_PROJECT_GITIGNORE: &str = "# Track only configuration, ignore rules, and canonical Markdown records.\n*\n!.gitignore\n!store.toml\n!records/\n!records/**/\n!records/**/*.md\n";

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DestructionAcknowledgement {
    private: (),
}

impl DestructionAcknowledgement {
    pub const fn deliberate() -> Self {
        Self { private: () }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    #[error("field `{field}` is invalid: {message}")]
    Validation { field: String, message: String },
    #[error("Markdown is invalid: {message}")]
    Markdown { message: String },
    #[error("frontmatter TOML is invalid: {source}")]
    TomlParse {
        #[source]
        source: toml::de::Error,
    },
    #[error("frontmatter TOML could not be rendered: {source}")]
    TomlRender {
        #[source]
        source: toml::ser::Error,
    },
    #[error("frontmatter format version {found} is unsupported; expected {expected}")]
    UnsupportedFormatVersion { found: u32, expected: u32 },
}

#[derive(Debug, thiserror::Error)]
pub enum StoreConfigError {
    #[error("store metadata TOML is invalid: {source}")]
    TomlParse {
        #[source]
        source: toml::de::Error,
    },
    #[error("store metadata could not be rendered: {source}")]
    TomlRender {
        #[source]
        source: toml::ser::Error,
    },
    #[error("store metadata format version {found} is unsupported; expected {expected}")]
    UnsupportedFormatVersion { found: u32, expected: u32 },
    #[error("scope is `{actual}`, expected `{expected}`")]
    WrongScope { actual: String, expected: String },
    #[error("visibility must be `private` or `shared`, got `{value}`")]
    InvalidVisibility { value: String },
    #[error("project stores require a valid project_id")]
    InvalidProjectId,
    #[error("project stores require a non-empty project_name without control characters")]
    InvalidProjectName,
    #[error("global stores must not contain project identity fields")]
    UnexpectedProjectIdentity,
    #[error("store is already {existing}; explicit shared initialization requires a shared store")]
    VisibilityConflict { existing: StoreVisibility },
    #[error("shared project ignore rules are incomplete")]
    IncompleteIgnoreRules,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not {operation}: {source}")]
    Io {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("the current working directory is not available")]
    InvalidWorkingDirectory,
    #[error("could not determine the user data directory; set HOME or the platform equivalent")]
    MissingHomeDirectory,
    #[error("invalid input: {message}")]
    InvalidInput { message: String },
    #[error("shared stores require project scope")]
    SharedStoreRequiresProject,
    #[error("invalid record {context}: {source}")]
    InvalidRecord {
        context: String,
        #[source]
        source: RecordError,
    },
    #[error("invalid store configuration {context}: {source}")]
    InvalidStoreConfiguration {
        context: String,
        #[source]
        source: StoreConfigError,
    },
    #[error("repository operation failed: {source}")]
    Repository {
        #[source]
        source: RepositoryError,
    },
    #[error("index operation failed: {operation}: {source}")]
    Index {
        operation: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    #[error("SQLite projection unavailable: {source}")]
    IndexUnavailable {
        #[source]
        source: Box<Error>,
    },
    #[error("SQLite projection is busy; retry the operation")]
    IndexBusy,
    #[error("embedding operation failed: {operation}: {message}")]
    Embedding {
        operation: &'static str,
        message: String,
    },
    #[error("backup operation failed: {source}")]
    Backup {
        #[source]
        source: BackupError,
    },
}

impl Error {
    fn io(operation: &'static str, source: io::Error) -> Self {
        Self::Io { operation, source }
    }

    pub(crate) fn invalid_record(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidRecord {
            context: "record".to_owned(),
            source: RecordError::Validation {
                field: field.into(),
                message: message.into(),
            },
        }
    }

    pub(crate) fn invalid_record_at(path: &Path, source: RecordError) -> Self {
        Self::InvalidRecord {
            context: path_context(path),
            source,
        }
    }

    fn invalid_store_at(path: &Path, source: StoreConfigError) -> Self {
        Self::InvalidStoreConfiguration {
            context: path_context(path),
            source,
        }
    }

    pub(crate) fn repository(source: RepositoryError) -> Self {
        Self::Repository { source }
    }

    pub(crate) fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    pub(crate) fn embedding(operation: &'static str, message: impl Into<String>) -> Self {
        Self::Embedding {
            operation,
            message: message.into(),
        }
    }

    pub(crate) fn backup(source: BackupError) -> Self {
        Self::Backup { source }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreScope {
    Global,
    Project,
    Local,
}

impl StoreScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project => "project",
            Self::Local => "local",
        }
    }

    const fn config_scope(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project | Self::Local => "project",
        }
    }

    pub const fn is_project_store(self) -> bool {
        matches!(self, Self::Project | Self::Local)
    }
}

impl fmt::Display for StoreScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreVisibility {
    Private,
    Shared,
}

impl StoreVisibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Shared => "shared",
        }
    }

    fn parse(value: &str) -> std::result::Result<Self, StoreConfigError> {
        match value {
            "private" => Ok(Self::Private),
            "shared" => Ok(Self::Shared),
            _ => Err(StoreConfigError::InvalidVisibility {
                value: value.to_owned(),
            }),
        }
    }
}

impl fmt::Display for StoreVisibility {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreInitMode {
    Default,
    Shared,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoreConfig {
    format_version: u32,
    scope: String,
    visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProjectId(Uuid);

impl ProjectId {
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7())
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ProjectId {
    type Err = StoreConfigError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let id = Uuid::parse_str(value).map_err(|_| StoreConfigError::InvalidProjectId)?;
        if id.is_nil() || id.to_string() != value {
            return Err(StoreConfigError::InvalidProjectId);
        }
        Ok(Self(id))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectIdentity {
    pub id: ProjectId,
    pub name: String,
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
    pub visibility: Option<StoreVisibility>,
    pub project: Option<ProjectIdentity>,
    pub record_count: usize,
}

struct StoreConfigWithVisibility {
    visibility: StoreVisibility,
    project: Option<ProjectIdentity>,
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
        StoreScope::Project | StoreScope::Local => project_store_root(cwd),
    };
    let cache = dirs.cache_root().join("stormbuffer");
    let paths = StorePaths::new(scope, root, cache);
    tracing::debug!(scope = %scope, root = ?paths.root, "resolved store");
    Ok(paths)
}

pub fn initialize_store(paths: &StorePaths, mode: StoreInitMode) -> Result<bool> {
    if !paths.scope.is_project_store() && mode == StoreInitMode::Shared {
        return Err(Error::SharedStoreRequiresProject);
    }

    fs::create_dir_all(&paths.root)
        .map_err(|source| Error::io("create the store directory", source))?;
    fs::create_dir_all(&paths.records)
        .map_err(|source| Error::io("create the records directory", source))?;
    fs::create_dir_all(&paths.cache)
        .map_err(|source| Error::io("create the cache directory", source))?;

    let (created, visibility) = create_marker(paths, mode)?;

    if paths.scope.is_project_store() {
        ensure_project_gitignore(&paths.root.join(".gitignore"), visibility)?;
    }

    tracing::info!(scope = %paths.scope, %visibility, created, "initialized store");
    Ok(created)
}

pub fn inspect_store(paths: &StorePaths) -> Result<StoreStatus> {
    let marker = paths.root.join("store.toml");
    let config = if marker.is_file() {
        Some(read_store_config(&marker, paths.scope)?)
    } else {
        None
    };
    let initialized = config.is_some();
    let record_count = if initialized && paths.records.is_dir() {
        count_markdown_files(&paths.records)?
    } else {
        0
    };

    Ok(StoreStatus {
        scope: paths.scope,
        root: paths.root.clone(),
        initialized,
        visibility: config.as_ref().map(|config| config.visibility),
        project: config.and_then(|config| config.project),
        record_count,
    })
}

fn project_store_root(cwd: &Path) -> PathBuf {
    let mut current = cwd;
    loop {
        let candidate = current.join(".sbuf");
        if candidate.is_dir() {
            return candidate;
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return cwd.join(".sbuf"),
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

fn create_marker(paths: &StorePaths, mode: StoreInitMode) -> Result<(bool, StoreVisibility)> {
    let path = paths.root.join("store.toml");
    let _lock = repository::acquire_store_initialization_lock(paths)?;
    let requested_visibility = match mode {
        StoreInitMode::Default => StoreVisibility::Private,
        StoreInitMode::Shared => StoreVisibility::Shared,
    };
    let project_name = path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|name| name.to_str());
    let contents = render_store_config(paths.scope, requested_visibility, project_name)?;

    match fs::metadata(&path) {
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            repository::write_atomic(&path, contents.as_bytes())?;
            Ok((true, requested_visibility))
        }
        Ok(_) => {
            let existing = read_store_config(&path, paths.scope)?;
            if mode == StoreInitMode::Shared && existing.visibility != StoreVisibility::Shared {
                return Err(Error::invalid_store_at(
                    &path,
                    StoreConfigError::VisibilityConflict {
                        existing: existing.visibility,
                    },
                ));
            }
            Ok((false, existing.visibility))
        }
        Err(source) => Err(Error::io("inspect store metadata", source)),
    }
}

fn render_store_config(
    scope: StoreScope,
    visibility: StoreVisibility,
    project_name: Option<&str>,
) -> Result<String> {
    let (project_id, project_name) = if scope.is_project_store() {
        let name = project_name
            .filter(|name| valid_project_name(name))
            .ok_or_else(|| {
                Error::invalid_store_at(
                    Path::new("store.toml"),
                    StoreConfigError::InvalidProjectName,
                )
            })?;
        (Some(ProjectId::new_v7().to_string()), Some(name.to_owned()))
    } else {
        (None, None)
    };
    toml::to_string(&StoreConfig {
        format_version: STORE_FORMAT_VERSION,
        scope: scope.config_scope().to_owned(),
        visibility: visibility.as_str().to_owned(),
        project_id,
        project_name,
    })
    .map_err(|source| {
        Error::invalid_store_at(
            Path::new("store.toml"),
            StoreConfigError::TomlRender { source },
        )
    })
}

fn read_store_config(path: &Path, expected_scope: StoreScope) -> Result<StoreConfigWithVisibility> {
    let contents =
        fs::read_to_string(path).map_err(|source| Error::io("read store metadata", source))?;
    let config: StoreConfig = toml::from_str(&contents)
        .map_err(|source| Error::invalid_store_at(path, StoreConfigError::TomlParse { source }))?;
    if config.format_version != STORE_FORMAT_VERSION {
        return Err(Error::invalid_store_at(
            path,
            StoreConfigError::UnsupportedFormatVersion {
                found: config.format_version,
                expected: STORE_FORMAT_VERSION,
            },
        ));
    }
    if config.scope != expected_scope.config_scope() {
        return Err(Error::invalid_store_at(
            path,
            StoreConfigError::WrongScope {
                actual: config.scope,
                expected: expected_scope.config_scope().to_owned(),
            },
        ));
    }
    let visibility = StoreVisibility::parse(&config.visibility)
        .map_err(|source| Error::invalid_store_at(path, source))?;
    let project = if expected_scope.is_project_store() {
        let id = config
            .project_id
            .as_deref()
            .ok_or(StoreConfigError::InvalidProjectId)
            .and_then(ProjectId::from_str)
            .map_err(|source| Error::invalid_store_at(path, source))?;
        let name = config
            .project_name
            .filter(|name| valid_project_name(name))
            .ok_or_else(|| Error::invalid_store_at(path, StoreConfigError::InvalidProjectName))?;
        Some(ProjectIdentity { id, name })
    } else {
        if config.project_id.is_some() || config.project_name.is_some() {
            return Err(Error::invalid_store_at(
                path,
                StoreConfigError::UnexpectedProjectIdentity,
            ));
        }
        None
    };
    Ok(StoreConfigWithVisibility {
        visibility,
        project,
    })
}

fn valid_project_name(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 128 && !name.chars().any(char::is_control)
}

pub fn record_scope(paths: &StorePaths) -> Result<Scope> {
    let marker = paths.root.join("store.toml");
    if !marker.is_file() {
        return Err(Error::repository(RepositoryError::StoreNotInitialized {
            root: paths.root.clone(),
        }));
    }
    let config = read_store_config(&marker, paths.scope)?;
    match paths.scope {
        StoreScope::Global => Scope::parse("global").map_err(Error::invalid_input),
        StoreScope::Project | StoreScope::Local => {
            let project = config.project.ok_or_else(|| {
                Error::invalid_store_at(&marker, StoreConfigError::InvalidProjectId)
            })?;
            Scope::parse(&format!("project:{}", project.id)).map_err(Error::invalid_input)
        }
    }
}

pub fn retrieval_stores(paths: &StorePaths, cwd: &Path) -> Result<Vec<StorePaths>> {
    let mut stores = vec![paths.clone()];
    if paths.scope == StoreScope::Project {
        let global = resolve_store(StoreScope::Global, cwd)?;
        if global.root.join("store.toml").is_file() {
            stores.push(global);
        }
    }
    Ok(stores)
}

fn ensure_project_gitignore(path: &Path, visibility: StoreVisibility) -> Result<()> {
    let expected = match visibility {
        StoreVisibility::Private => PRIVATE_PROJECT_GITIGNORE,
        StoreVisibility::Shared => SHARED_PROJECT_GITIGNORE.as_bytes(),
    };

    if path.is_file() {
        if visibility == StoreVisibility::Shared {
            let current =
                fs::read(path).map_err(|source| Error::io("read project ignore rules", source))?;
            let current = String::from_utf8_lossy(&current);
            let actual_rules = current
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'));
            let expected_rules = SHARED_PROJECT_GITIGNORE
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'));
            if !actual_rules.eq(expected_rules) {
                return Err(Error::invalid_store_at(
                    path,
                    StoreConfigError::IncompleteIgnoreRules,
                ));
            }
        }
        return Ok(());
    }

    create_file_if_missing(path, expected)?;
    Ok(())
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

fn path_context(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
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
        let project = root.join(".sbuf");
        fs::create_dir_all(&project).expect("create project store");

        let paths = resolve_store_with_dirs(StoreScope::Project, &nested, &dirs).expect("resolve");
        assert_eq!(paths.root, project);

        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn project_initialization_defaults_private_and_supports_explicit_shared_mode() {
        let root = temporary_directory("visibility");
        let dirs = PlatformDirs::new(root.join("data"), root.join("cache"));
        let private_paths =
            resolve_store_with_dirs(StoreScope::Project, &root, &dirs).expect("resolve");

        assert!(
            initialize_store(&private_paths, StoreInitMode::Default).expect("initialize store")
        );
        assert!(
            fs::read_to_string(private_paths.root.join("store.toml"))
                .expect("read private metadata")
                .contains("visibility = \"private\"")
        );
        assert_eq!(
            fs::read(private_paths.root.join(".gitignore")).expect("read private ignore rules"),
            b"*\n!.gitignore\n"
        );
        assert_eq!(
            inspect_store(&private_paths)
                .expect("inspect private store")
                .visibility,
            Some(StoreVisibility::Private)
        );

        let shared_root = temporary_directory("shared-store");
        let shared_paths =
            resolve_store_with_dirs(StoreScope::Project, &shared_root, &dirs).expect("resolve");
        assert!(
            initialize_store(&shared_paths, StoreInitMode::Shared)
                .expect("initialize shared store")
        );
        let ignore = fs::read_to_string(shared_paths.root.join(".gitignore"))
            .expect("read shared ignore rules");
        for pattern in [
            "*",
            "!.gitignore",
            "!store.toml",
            "!records/",
            "!records/**/",
            "!records/**/*.md",
        ] {
            assert!(
                ignore.lines().any(|line| line == pattern),
                "missing {pattern}"
            );
        }
        assert_eq!(
            inspect_store(&shared_paths)
                .expect("inspect shared store")
                .visibility,
            Some(StoreVisibility::Shared)
        );

        fs::write(
            shared_paths.root.join(".gitignore"),
            format!("{SHARED_PROJECT_GITIGNORE}\n!index.sqlite3\n"),
        )
        .expect("make generated artifact trackable");
        let error = initialize_store(&shared_paths, StoreInitMode::Default)
            .expect_err("unsafe shared ignore rules must fail");
        assert!(matches!(
            error,
            Error::InvalidStoreConfiguration {
                source: StoreConfigError::IncompleteIgnoreRules,
                ..
            }
        ));

        let global_paths =
            resolve_store_with_dirs(StoreScope::Global, &root, &dirs).expect("resolve");
        let error = initialize_store(&global_paths, StoreInitMode::Shared)
            .expect_err("global shared initialization should fail");
        assert!(matches!(error, Error::SharedStoreRequiresProject));
        assert!(!global_paths.root.exists());

        fs::remove_dir_all(shared_root).expect("remove shared test directory");
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn initialization_is_idempotent_and_status_counts_records() {
        let root = temporary_directory("initialization");
        let dirs = PlatformDirs::new(root.join("data"), root.join("cache"));
        let paths = resolve_store_with_dirs(StoreScope::Project, &root, &dirs).expect("resolve");

        assert!(initialize_store(&paths, StoreInitMode::Default).expect("initialize store"));
        assert!(!initialize_store(&paths, StoreInitMode::Default).expect("initialize store again"));
        fs::write(paths.records.join("example.md"), "not a parsed record yet")
            .expect("write record");

        let status = inspect_store(&paths).expect("inspect store");
        assert!(status.initialized);
        assert_eq!(status.record_count, 1);

        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn concurrent_initialization_creates_one_stable_identity() {
        let root = temporary_directory("concurrent-initialization");
        let dirs = PlatformDirs::new(root.join("data"), root.join("cache"));
        let paths = resolve_store_with_dirs(StoreScope::Project, &root, &dirs).expect("resolve");
        let barrier = Arc::new(Barrier::new(3));

        let handles = (0..2)
            .map(|_| {
                let paths = paths.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    initialize_store(&paths, StoreInitMode::Default)
                })
            })
            .collect::<Vec<_>>();

        barrier.wait();
        let results = handles
            .into_iter()
            .map(|handle| handle.join().expect("join initializer"))
            .collect::<Vec<_>>();
        let created = results
            .iter()
            .filter(|result| matches!(result, Ok(true)))
            .count();
        assert_eq!(created, 1);
        assert!(results.iter().all(|result| matches!(
            result,
            Ok(_)
                | Err(Error::Repository {
                    source: RepositoryError::MutationBusy { .. }
                })
        )));
        assert!(!initialize_store(&paths, StoreInitMode::Default).expect("retry initialization"));
        assert!(
            inspect_store(&paths)
                .expect("inspect project")
                .project
                .is_some()
        );

        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn project_identity_survives_rename_and_same_named_projects_are_distinct() {
        let root = temporary_directory("project-identity");
        let dirs = PlatformDirs::new(root.join("data"), root.join("cache"));
        let first_root = root.join("one").join("service");
        let second_root = root.join("two").join("service");
        fs::create_dir_all(&first_root).expect("create first project");
        fs::create_dir_all(&second_root).expect("create second project");
        let first = resolve_store_with_dirs(StoreScope::Project, &first_root, &dirs)
            .expect("resolve first project");
        let second = resolve_store_with_dirs(StoreScope::Project, &second_root, &dirs)
            .expect("resolve second project");
        initialize_store(&first, StoreInitMode::Default).expect("initialize first project");
        initialize_store(&second, StoreInitMode::Default).expect("initialize second project");

        let first_identity = inspect_store(&first)
            .expect("inspect first project")
            .project
            .expect("first project identity");
        let second_identity = inspect_store(&second)
            .expect("inspect second project")
            .project
            .expect("second project identity");
        assert_eq!(first_identity.name, "service");
        assert_eq!(second_identity.name, "service");
        assert_ne!(first_identity.id, second_identity.id);

        let renamed_root = root.join("one").join("renamed-service");
        fs::rename(&first_root, &renamed_root).expect("rename project directory");
        let renamed = resolve_store_with_dirs(StoreScope::Project, &renamed_root, &dirs)
            .expect("resolve renamed project");
        assert_eq!(
            inspect_store(&renamed)
                .expect("inspect renamed project")
                .project
                .expect("renamed project identity"),
            first_identity
        );
        assert_eq!(
            record_scope(&renamed).expect("read renamed project scope"),
            Scope::parse(&format!("project:{}", first_identity.id)).expect("expected scope")
        );

        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn project_name_is_editable_but_validated() {
        let root = temporary_directory("project-name");
        let dirs = PlatformDirs::new(root.join("data"), root.join("cache"));
        let paths =
            resolve_store_with_dirs(StoreScope::Project, &root, &dirs).expect("resolve project");
        initialize_store(&paths, StoreInitMode::Default).expect("initialize project");
        let marker = paths.root.join("store.toml");
        let metadata = fs::read_to_string(&marker).expect("read metadata");
        fs::write(
            &marker,
            metadata.replace(
                &format!(
                    "project_name = \"{}\"",
                    root.file_name().unwrap().to_string_lossy()
                ),
                "project_name = \"Friendly project\"",
            ),
        )
        .expect("edit project name");
        assert_eq!(
            inspect_store(&paths)
                .expect("inspect edited project")
                .project
                .expect("project identity")
                .name,
            "Friendly project"
        );

        let metadata = fs::read_to_string(&marker).expect("read edited metadata");
        fs::write(
            &marker,
            metadata.replace(
                "project_name = \"Friendly project\"",
                "project_name = \"   \"",
            ),
        )
        .expect("write invalid project name");
        assert!(matches!(
            inspect_store(&paths),
            Err(Error::InvalidStoreConfiguration {
                source: StoreConfigError::InvalidProjectName,
                ..
            })
        ));

        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn project_identity_rejects_the_nil_uuid() {
        let root = temporary_directory("nil-project-id");
        let dirs = PlatformDirs::new(root.join("data"), root.join("cache"));
        let paths =
            resolve_store_with_dirs(StoreScope::Project, &root, &dirs).expect("resolve project");
        initialize_store(&paths, StoreInitMode::Default).expect("initialize project");
        let marker = paths.root.join("store.toml");
        let metadata = fs::read_to_string(&marker).expect("read metadata");
        let project_id = inspect_store(&paths)
            .expect("inspect project")
            .project
            .expect("project identity")
            .id
            .to_string();
        fs::write(
            &marker,
            metadata.replace(&project_id, "00000000-0000-0000-0000-000000000000"),
        )
        .expect("write nil project id");

        assert!(matches!(
            record_scope(&paths),
            Err(Error::InvalidStoreConfiguration {
                source: StoreConfigError::InvalidProjectId,
                ..
            })
        ));

        fs::remove_dir_all(root).expect("remove test directory");
    }
}
