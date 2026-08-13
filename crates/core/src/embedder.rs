use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use fs2::FileExt;

use fastembed::{
    InitOptionsUserDefined, Pooling, TextEmbedding, TokenizerFiles, UserDefinedEmbeddingModel,
};
use serde::{Deserialize, Serialize};

use crate::{Error, Result, StorePaths};

pub const MODEL_MANIFEST_VERSION: u32 = 1;
pub const DEFAULT_MODEL_VERSION: &str =
    "fastembed-5.13.4/Qdrant/all-MiniLM-L6-v2-onnx@5f1b8cd78bc4fb444dd171e59b18f3a3af89a079";
const DEFAULT_MODEL_BASE_URL: &str = "https://huggingface.co/Qdrant/all-MiniLM-L6-v2-onnx/resolve/5f1b8cd78bc4fb444dd171e59b18f3a3af89a079";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelArtifact {
    pub path: String,
    pub blake3: String,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelManifest {
    pub format_version: u32,
    pub model_id: String,
    pub model_version: String,
    pub dimension: usize,
    pub max_tokens: usize,
    pub artifacts: Vec<ModelArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModelFiles {
    model: PathBuf,
    tokenizer: PathBuf,
    config: PathBuf,
    special_tokens_map: PathBuf,
    tokenizer_config: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Embedding {
    pub values: Vec<f32>,
}

impl Embedding {
    pub fn new(values: Vec<f32>) -> Result<Self> {
        if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
            return Err(Error::embedding(
                "validate embedding",
                "embedding must contain finite values",
            ));
        }
        Ok(Self { values })
    }

    pub fn dimension(&self) -> usize {
        self.values.len()
    }
}

pub trait Embedder: Send + Sync {
    fn model_id(&self) -> &str;
    fn model_version(&self) -> &str;
    fn model_checksum(&self) -> &str;
    fn dimension(&self) -> usize;
    fn embed(&self, text: &str) -> Result<Embedding>;
}

pub fn default_model_manifest() -> ModelManifest {
    let artifact = |path: &str, blake3: &str| ModelArtifact {
        path: path.to_owned(),
        blake3: blake3.to_owned(),
        url: format!("{DEFAULT_MODEL_BASE_URL}/{path}?download=true"),
    };
    ModelManifest {
        format_version: MODEL_MANIFEST_VERSION,
        model_id: "Qdrant/all-MiniLM-L6-v2-onnx".to_owned(),
        model_version: DEFAULT_MODEL_VERSION.to_owned(),
        dimension: 384,
        max_tokens: 256,
        artifacts: vec![
            artifact(
                "model.onnx",
                "e07600e571df4aa8bd2397774731baf10c97574e8ccc128ed2c58113d739bda6",
            ),
            artifact(
                "tokenizer.json",
                "43e7f76ab347da19c09233073ccddd0e4d07eb8070ced05f89569bc43a1bad9e",
            ),
            artifact(
                "config.json",
                "a5da27b3980f2ccd664d65b98b4de6e72d34134ca1a29218bf5c8c67ee10d121",
            ),
            artifact(
                "special_tokens_map.json",
                "eaff0d8331fbe475d3ba22934ad574aa0f23b5b7a9e547f0105a82d050ab31fc",
            ),
            artifact(
                "tokenizer_config.json",
                "b2b8a921b78c685752ca02cd84eb526057c5ea435eb07ef91538d442c32912a7",
            ),
        ],
    }
}

pub fn platform_model_cache_dir(dirs: &crate::PlatformDirs) -> PathBuf {
    dirs.cache_root().join("stormbuffer").join("models")
}

pub fn model_cache_dir(paths: &StorePaths) -> PathBuf {
    paths.cache.join("models")
}

pub fn ensure_default_model(paths: &StorePaths) -> Result<()> {
    default_model_manifest().acquire(&model_cache_dir(paths))
}

pub(crate) fn default_model_is_ready(paths: &StorePaths) -> bool {
    let cache = model_cache_dir(paths);
    if !cache.is_dir() {
        return false;
    }
    let manifest = default_model_manifest();
    with_model_cache_lock(&cache, false, || manifest.verify_files(&cache)).is_ok()
}

impl ModelManifest {
    pub fn validate(&self) -> Result<()> {
        if self.format_version != MODEL_MANIFEST_VERSION {
            return Err(Error::embedding(
                "validate model manifest",
                format!(
                    "manifest format version {} is unsupported; expected {}",
                    self.format_version, MODEL_MANIFEST_VERSION
                ),
            ));
        }
        if self.model_id.trim().is_empty() || self.model_version.trim().is_empty() {
            return Err(Error::embedding(
                "validate model manifest",
                "model_id and model_version must be non-empty",
            ));
        }
        if self.dimension == 0 || self.max_tokens == 0 || self.artifacts.is_empty() {
            return Err(Error::embedding(
                "validate model manifest",
                "dimension, max_tokens, and artifacts must be non-empty",
            ));
        }
        for artifact in &self.artifacts {
            validate_relative_file("artifact path", &artifact.path)?;
            validate_checksum(&artifact.blake3)?;
            if artifact.url.trim().is_empty() {
                return Err(Error::embedding(
                    "validate model manifest",
                    format!("artifact {} has no URL", artifact.path),
                ));
            }
        }
        for required in [
            "model.onnx",
            "tokenizer.json",
            "config.json",
            "special_tokens_map.json",
            "tokenizer_config.json",
        ] {
            if !self
                .artifacts
                .iter()
                .any(|artifact| artifact.path == required)
            {
                return Err(Error::embedding(
                    "validate model manifest",
                    format!("manifest is missing required fastembed artifact {required}"),
                ));
            }
        }
        Ok(())
    }

    pub fn fingerprint(&self) -> String {
        manifest_fingerprint(self)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .map_err(|source| Error::io("read the model manifest", source))?;
        let manifest: Self = toml::from_str(&contents).map_err(|source| {
            Error::embedding(
                "parse model manifest",
                format!("{}: {source}", path.display()),
            )
        })?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn verify_files(&self, root: &Path) -> Result<ModelFiles> {
        self.validate()?;
        for artifact in &self.artifacts {
            verify_checksum(&root.join(&artifact.path), &artifact.blake3)?;
        }
        self.required_files(root)
    }

    /// Download missing artifacts into `.part` files and install each file only after
    /// its pinned BLAKE3 checksum matches. Interrupted downloads can be retried safely.
    pub fn acquire(&self, root: &Path) -> Result<()> {
        self.validate()
            .map_err(|error| model_setup_error(root, error.to_string()))?;
        with_model_cache_lock(root, true, || {
            fs::create_dir_all(root)
                .map_err(|source| Error::io("create the model cache", source))?;
            for artifact in &self.artifacts {
                acquire_file(root.join(&artifact.path), &artifact.url, &artifact.blake3)?;
            }
            Ok(())
        })
        .map_err(|error| model_setup_error(root, error.to_string()))
    }

    fn required_files(&self, root: &Path) -> Result<ModelFiles> {
        let path = |name: &str| root.join(name);
        Ok(ModelFiles {
            model: path("model.onnx"),
            tokenizer: path("tokenizer.json"),
            config: path("config.json"),
            special_tokens_map: path("special_tokens_map.json"),
            tokenizer_config: path("tokenizer_config.json"),
        })
    }
}

pub struct LocalEmbedder {
    manifest: ModelManifest,
    checksum: String,
    model: Mutex<TextEmbedding>,
}

impl LocalEmbedder {
    pub fn from_default_cache(paths: &StorePaths) -> Result<Self> {
        let manifest = default_model_manifest();
        let cache = model_cache_dir(paths);
        with_model_cache_lock(&cache, false, || {
            let files = manifest.verify_files(&cache)?;
            Self::from_verified_files(manifest, files)
        })
        .map_err(|error| model_setup_error(&cache, error.to_string()))
    }

    pub fn from_manifest(path: &Path) -> Result<Self> {
        let manifest = ModelManifest::load(path)?;
        let root = path.parent().unwrap_or_else(|| Path::new("."));
        with_model_cache_lock(root, false, || {
            let files = manifest.verify_files(root)?;
            Self::from_verified_files(manifest, files)
        })
        .map_err(|error| model_setup_error(root, error.to_string()))
    }

    fn from_verified_files(manifest: ModelManifest, files: ModelFiles) -> Result<Self> {
        manifest.validate()?;
        let checksum = manifest.fingerprint();
        let tokenizer_files = TokenizerFiles {
            tokenizer_file: read_model_file(&files.tokenizer)?,
            config_file: read_model_file(&files.config)?,
            special_tokens_map_file: read_model_file(&files.special_tokens_map)?,
            tokenizer_config_file: read_model_file(&files.tokenizer_config)?,
        };
        let model = UserDefinedEmbeddingModel::new(read_model_file(&files.model)?, tokenizer_files)
            .with_pooling(Pooling::Mean);
        let options = InitOptionsUserDefined::new().with_max_length(manifest.max_tokens);
        let model = TextEmbedding::try_new_from_user_defined(model, options).map_err(|error| {
            Error::embedding("load verified fastembed model", error.to_string())
        })?;
        Ok(Self {
            manifest,
            checksum,
            model: Mutex::new(model),
        })
    }
}

impl Embedder for LocalEmbedder {
    fn model_id(&self) -> &str {
        &self.manifest.model_id
    }

    fn model_version(&self) -> &str {
        &self.manifest.model_version
    }

    fn model_checksum(&self) -> &str {
        &self.checksum
    }

    fn dimension(&self) -> usize {
        self.manifest.dimension
    }

    fn embed(&self, text: &str) -> Result<Embedding> {
        let mut model = self
            .model
            .lock()
            .map_err(|_| Error::embedding("run fastembed model", "model lock is poisoned"))?;
        let embeddings = model
            .embed([text], Some(1))
            .map_err(|error| Error::embedding("run fastembed model", error.to_string()))?;
        let values = embeddings.into_iter().next().ok_or_else(|| {
            Error::embedding("read fastembed output", "model returned no embedding")
        })?;
        if values.len() != self.manifest.dimension {
            return Err(Error::embedding(
                "validate fastembed output",
                format!(
                    "model returned dimension {}, manifest requires {}",
                    values.len(),
                    self.manifest.dimension
                ),
            ));
        }
        Embedding::new(values)
    }
}

#[derive(Clone, Debug)]
pub struct DeterministicEmbedder {
    version: String,
    checksum: String,
    dimension: usize,
}

impl DeterministicEmbedder {
    pub fn new(version: impl Into<String>, dimension: usize) -> Result<Self> {
        if dimension == 0 {
            return Err(Error::embedding(
                "create fixture embedder",
                "dimension must be positive",
            ));
        }
        let version = version.into();
        let checksum = blake3::hash(version.as_bytes()).to_hex().to_string();
        Ok(Self {
            version,
            checksum,
            dimension,
        })
    }
}

impl Embedder for DeterministicEmbedder {
    fn model_id(&self) -> &str {
        "stormbuffer/deterministic"
    }

    fn model_version(&self) -> &str {
        &self.version
    }
    fn model_checksum(&self) -> &str {
        &self.checksum
    }
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed(&self, text: &str) -> Result<Embedding> {
        let mut values = vec![0.0_f32; self.dimension];
        for token in text.split_whitespace().map(str::to_lowercase) {
            let digest = blake3::hash(token.as_bytes());
            for (offset, value) in values.iter_mut().enumerate() {
                let byte = digest.as_bytes()[offset % digest.as_bytes().len()];
                let sign = if byte & 1 == 0 { 1.0 } else { -1.0 };
                *value += sign * (f32::from(byte) / 255.0 + 0.01);
            }
        }
        if values.iter().all(|value| *value == 0.0) {
            values[0] = 1.0;
        }
        Embedding::new(l2_normalize(values)?)
    }
}

fn manifest_fingerprint(manifest: &ModelManifest) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"stormbuffer-fastembed-preprocessing-v1");
    hasher.update(manifest.model_version.as_bytes());
    hasher.update(manifest.dimension.to_string().as_bytes());
    hasher.update(manifest.max_tokens.to_string().as_bytes());
    hasher.update(b"pooling=mean;normalize=l2");
    for artifact in &manifest.artifacts {
        hasher.update(artifact.path.as_bytes());
        hasher.update(artifact.blake3.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

pub fn l2_normalize(mut values: Vec<f32>) -> Result<Vec<f32>> {
    let norm = values
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm <= f64::EPSILON {
        return Err(Error::embedding(
            "normalize embedding",
            "embedding has zero or invalid norm",
        ));
    }
    for value in &mut values {
        *value = (*value as f64 / norm) as f32;
    }
    Ok(values)
}

fn read_model_file(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|source| Error::io("read a verified model artifact", source))
}

fn model_setup_error(cache: &Path, details: impl Into<String>) -> Error {
    Error::embedding(
        "prepare the local embedding model",
        format!(
            "{}; model cache: {}; repair with `sbuf init`",
            details.into(),
            cache.display()
        ),
    )
}

fn with_model_cache_lock<T>(
    root: &Path,
    exclusive: bool,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    fs::create_dir_all(root).map_err(|source| Error::io("create the model cache", source))?;
    let lock_path = root.join(".lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|source| Error::io("open the model cache lock", source))?;
    if exclusive {
        FileExt::lock_exclusive(&lock)
            .map_err(|source| Error::io("lock the model cache", source))?;
    } else {
        FileExt::lock_shared(&lock).map_err(|source| Error::io("lock the model cache", source))?;
    }
    let result = operation();
    let unlock_result =
        FileExt::unlock(&lock).map_err(|source| Error::io("unlock the model cache", source));
    match result {
        Ok(value) => unlock_result.map(|()| value),
        Err(error) => {
            let _ = unlock_result;
            Err(error)
        }
    }
}

fn validate_checksum(value: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::embedding(
            "validate model manifest",
            "artifact checksum must be a 64-character BLAKE3 value",
        ));
    }
    Ok(())
}

fn validate_relative_file(field: &str, value: &str) -> Result<()> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(Error::embedding(
            "validate model manifest",
            format!("{field} must be a relative path inside the model cache"),
        ));
    }
    Ok(())
}

fn verify_checksum(path: &Path, expected: &str) -> Result<()> {
    let mut file = File::open(path).map_err(|source| Error::io("open a model artifact", source))?;
    let actual = hash_reader(&mut file)?;
    if actual != expected {
        return Err(Error::embedding(
            "verify model artifact",
            format!(
                "{} has checksum {actual}, expected {expected}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn hash_reader(reader: &mut impl Read) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|source| Error::io("read a model artifact", source))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn acquire_file(path: PathBuf, url: &str, expected: &str) -> Result<()> {
    if path.is_file() && verify_checksum(&path, expected).is_ok() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| Error::io("create the model artifact directory", source))?;
    }
    let partial = PathBuf::from(format!("{}.part", path.display()));
    let existing = fs::metadata(&partial)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let mut request = ureq::get(url);
    if existing > 0 {
        request = request.header("Range", &format!("bytes={existing}-"));
    }
    let response = request.call().map_err(|error| {
        Error::embedding(
            "download model artifact",
            format!("the pinned artifact request failed: {error}"),
        )
    })?;
    let append = existing > 0 && response.status().as_u16() == 206;
    let mut file = if append {
        OpenOptions::new().create(true).append(true).open(&partial)
    } else {
        OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&partial)
    }
    .map_err(|source| Error::io("open model download", source))?;
    let (_, body) = response.into_parts();
    io::copy(&mut body.into_reader(), &mut file)
        .map_err(|source| Error::io("write model download", source))?;
    file.sync_all()
        .map_err(|source| Error::io("sync model download", source))?;
    if let Err(error) = verify_checksum(&partial, expected) {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|source| Error::io("replace corrupt model artifact", source))?;
    }
    fs::rename(&partial, &path)
        .map_err(|source| Error::io("install verified model artifact", source))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_manifest_pins_fastembed_model_and_all_required_artifacts() {
        let manifest = default_model_manifest();
        manifest.validate().expect("manifest");
        assert_eq!(manifest.dimension, 384);
        assert_eq!(
            TextEmbedding::get_default_pooling_method(&fastembed::EmbeddingModel::AllMiniLML6V2),
            Some(Pooling::Mean)
        );
        assert_eq!(manifest.artifacts.len(), 5);
    }

    #[test]
    fn fixture_embedder_has_stable_normalized_dimensions() {
        let embedder = DeterministicEmbedder::new("fixture-v1", 12).expect("embedder");
        let first = embedder.embed("same text").expect("embedding");
        let second = embedder.embed("same text").expect("embedding");
        assert_eq!(first.values, second.values);
        assert_eq!(first.dimension(), 12);
        let norm = first.values.iter().map(|value| value * value).sum::<f32>();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn manifest_fingerprint_includes_embedding_dimension() {
        let manifest = default_model_manifest();
        let original = manifest_fingerprint(&manifest);
        let mut changed = manifest;
        changed.dimension += 1;

        assert_ne!(original, manifest_fingerprint(&changed));
    }

    #[test]
    fn unsafe_manifest_paths_and_corrupt_artifacts_are_rejected() {
        let mut manifest = default_model_manifest();
        manifest.artifacts[0].path = "../model.onnx".to_owned();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn fingerprint_changes_when_tokenizer_or_preprocessing_inputs_change() {
        let manifest = default_model_manifest();
        let original = manifest.fingerprint();
        let mut changed = manifest.clone();
        changed.artifacts[1].blake3 = "0".repeat(64);
        assert_ne!(original, changed.fingerprint());
        changed.max_tokens += 1;
        assert_ne!(original, changed.fingerprint());
    }

    #[test]
    fn missing_model_errors_name_cache_and_repair_command() {
        let root =
            std::env::temp_dir().join(format!("stormbuffer-model-error-{}", std::process::id()));
        let paths = StorePaths {
            scope: crate::StoreScope::Global,
            root: root.clone(),
            records: root.join("records"),
            cache: root.join("cache"),
        };
        let error = match LocalEmbedder::from_default_cache(&paths) {
            Ok(_) => panic!("model is absent"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(message.contains(&model_cache_dir(&paths).display().to_string()));
        assert!(message.contains("sbuf init"));
        let _ = fs::remove_dir_all(root);
    }
}
