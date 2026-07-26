//! Authenticated Pangolin ONNX bundle inspection and the raw CPU model kernel.
//!
//! This crate deliberately stops at twelve raw replicate channels. Variant
//! construction, ensemble arithmetic, masking, extrema, and public score
//! rendering belong to later layers.

use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::{Tensor, TensorElementType, ValueType},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    num::NonZeroUsize,
    path::Path,
    str::FromStr,
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

pub const MODEL_BUNDLE_SCHEMA: &str = "pangopup-model-bundle-v1";
pub const PRODUCTION_PROFILE: &str = "pangolin-1.0.2-5cf94b8-onnx-cpu-v1";
pub const MINI_PROFILE: &str = "pangopup-model-kernel-mini-v1";
pub const PRODUCTION_UPSTREAM_COMMIT: &str = "5cf94b8db938c658391b4305cd7ce33297d44ff7";
pub const INPUT_NAME: &str = "sequence";
pub const OUTPUT_NAME: &str = "replicate_scores";
pub const CHANNELS: usize = 12;
pub const CONTEXT_FLANKS: usize = 10_000;
pub const MIN_CONTEXT_LENGTH: usize = 10_001;
pub const MAX_CONTEXT_LENGTH: usize = 10_200;
pub const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
pub const MAX_NOTICE_BYTES: u64 = 64 * 1024;
pub const MAX_MODEL_BYTES: u64 = 48 * 1024 * 1024;
pub const MAX_AGGREGATE_BYTES: u64 = 49 * 1024 * 1024;
pub const ORT_CRATE_VERSION: &str = "2.0.0-rc.12";
pub const ONNX_RUNTIME_VERSION: &str = "1.24.2";

const EXACT_MEMBERS: [&str; 3] = ["NOTICE", "manifest.json", "model.onnx"];
const PRODUCTION_MODEL_SOURCE_SHA256: &str =
    "sha256:4a1c5c2570aafe1452bb43332255321677e6c6c817adf84b9dd438e3ca4be6f8";

/// ONNX Runtime's graph execution mode for one model session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuExecutionMode {
    Sequential,
    Parallel,
}

/// ONNX Runtime's within-node thread policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntraOpThreads {
    /// Leave the thread count unset so ONNX Runtime can use its affinity-aware
    /// automatic policy.
    Auto,
    Fixed(NonZeroUsize),
}

/// Immutable CPU session policy for the low-level model kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuPolicy {
    execution: CpuExecutionMode,
    intra_op: IntraOpThreads,
    inter_op: NonZeroUsize,
}

impl CpuPolicy {
    pub const SEQUENTIAL_AUTO_1: Self = Self {
        execution: CpuExecutionMode::Sequential,
        intra_op: IntraOpThreads::Auto,
        inter_op: NonZeroUsize::MIN,
    };
    pub const SEQUENTIAL_1_1: Self = Self {
        execution: CpuExecutionMode::Sequential,
        intra_op: IntraOpThreads::Fixed(NonZeroUsize::MIN),
        inter_op: NonZeroUsize::MIN,
    };
    pub const SEQUENTIAL_2_1: Self = Self {
        execution: CpuExecutionMode::Sequential,
        intra_op: IntraOpThreads::Fixed(NonZeroUsize::new(2).expect("nonzero")),
        inter_op: NonZeroUsize::MIN,
    };
    pub const SEQUENTIAL_4_1: Self = Self {
        execution: CpuExecutionMode::Sequential,
        intra_op: IntraOpThreads::Fixed(NonZeroUsize::new(4).expect("nonzero")),
        inter_op: NonZeroUsize::MIN,
    };
    pub const SEQUENTIAL_8_1: Self = Self {
        execution: CpuExecutionMode::Sequential,
        intra_op: IntraOpThreads::Fixed(NonZeroUsize::new(8).expect("nonzero")),
        inter_op: NonZeroUsize::MIN,
    };
    pub const PARALLEL_1_2: Self = Self {
        execution: CpuExecutionMode::Parallel,
        intra_op: IntraOpThreads::Fixed(NonZeroUsize::MIN),
        inter_op: NonZeroUsize::new(2).expect("nonzero"),
    };
    pub const PARALLEL_1_4: Self = Self {
        execution: CpuExecutionMode::Parallel,
        intra_op: IntraOpThreads::Fixed(NonZeroUsize::MIN),
        inter_op: NonZeroUsize::new(4).expect("nonzero"),
    };
    pub const PARALLEL_1_8: Self = Self {
        execution: CpuExecutionMode::Parallel,
        intra_op: IntraOpThreads::Fixed(NonZeroUsize::MIN),
        inter_op: NonZeroUsize::new(8).expect("nonzero"),
    };

    /// The reviewed policy used by ordinary callers.
    ///
    /// This remains a compiled choice rather than runtime configuration.
    #[must_use]
    pub const fn production_default() -> Self {
        Self::SEQUENTIAL_1_1
    }

    pub fn new(
        execution: CpuExecutionMode,
        intra_op: IntraOpThreads,
        inter_op: NonZeroUsize,
    ) -> Result<Self, CpuPolicyError> {
        if let IntraOpThreads::Fixed(threads) = intra_op {
            i32::try_from(threads.get())
                .map_err(|_| CpuPolicyError::ThreadCountOutOfRange("intra-op"))?;
        }
        i32::try_from(inter_op.get())
            .map_err(|_| CpuPolicyError::ThreadCountOutOfRange("inter-op"))?;
        if execution == CpuExecutionMode::Sequential && inter_op != NonZeroUsize::MIN {
            return Err(CpuPolicyError::SequentialInterOp);
        }
        Ok(Self {
            execution,
            intra_op,
            inter_op,
        })
    }

    #[must_use]
    pub const fn execution(self) -> CpuExecutionMode {
        self.execution
    }

    #[must_use]
    pub const fn intra_op(self) -> IntraOpThreads {
        self.intra_op
    }

    #[must_use]
    pub const fn inter_op(self) -> NonZeroUsize {
        self.inter_op
    }
}

impl fmt::Display for CpuPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let execution = match self.execution {
            CpuExecutionMode::Sequential => "sequential",
            CpuExecutionMode::Parallel => "parallel",
        };
        match self.intra_op {
            IntraOpThreads::Auto => {
                write!(formatter, "{execution}:auto/{}", self.inter_op)
            }
            IntraOpThreads::Fixed(threads) => {
                write!(formatter, "{execution}:{threads}/{}", self.inter_op)
            }
        }
    }
}

impl FromStr for CpuPolicy {
    type Err = CpuPolicyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "sequential:auto/1" => Ok(Self::SEQUENTIAL_AUTO_1),
            "sequential:1/1" => Ok(Self::SEQUENTIAL_1_1),
            "sequential:2/1" => Ok(Self::SEQUENTIAL_2_1),
            "sequential:4/1" => Ok(Self::SEQUENTIAL_4_1),
            "sequential:8/1" => Ok(Self::SEQUENTIAL_8_1),
            "parallel:1/2" => Ok(Self::PARALLEL_1_2),
            "parallel:1/4" => Ok(Self::PARALLEL_1_4),
            "parallel:1/8" => Ok(Self::PARALLEL_1_8),
            _ => Err(CpuPolicyError::UnknownCandidate),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuPolicyError {
    SequentialInterOp,
    ThreadCountOutOfRange(&'static str),
    UnknownCandidate,
}

impl fmt::Display for CpuPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SequentialInterOp => {
                formatter.write_str("sequential CPU policy requires one inter-op thread")
            }
            Self::ThreadCountOutOfRange(field) => {
                write!(
                    formatter,
                    "{field} CPU thread count exceeds ONNX Runtime's signed 32-bit domain"
                )
            }
            Self::UnknownCandidate => formatter.write_str(
                "unknown CPU policy candidate; expected sequential:auto/1, sequential:1/1, \
                 sequential:2/1, sequential:4/1, sequential:8/1, parallel:1/2, \
                 parallel:1/4, or parallel:1/8",
            ),
        }
    }
}

impl std::error::Error for CpuPolicyError {}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BundleKind {
    Production,
    SyntheticTest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileIdentity {
    pub filename: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointIdentity {
    pub ordinal: u8,
    pub filename: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSource {
    pub identity: String,
    pub upstream_url: String,
    pub upstream_commit: String,
    pub model_source: FileIdentity,
    pub checkpoints: Vec<CheckpointIdentity>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversionEnvironment {
    pub python: String,
    pub pytorch: String,
    pub numpy: String,
    pub onnx: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TensorContract {
    pub name: String,
    pub element_type: String,
    pub shape: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelMapping {
    pub checkpoint_ordinal: u8,
    pub selected_channel: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExporterSettings {
    pub dynamo: bool,
    pub constant_folding: bool,
    pub dynamic_axis: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphContract {
    pub opset: u8,
    pub input: TensorContract,
    pub output: TensorContract,
    pub channels: Vec<ChannelMapping>,
    pub exporter: ExporterSettings,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversionManifest {
    pub converter: FileIdentity,
    pub checkpoint_inventory: FileIdentity,
    pub qualification_evidence: FileIdentity,
    pub environment: ConversionEnvironment,
    pub graph: GraphContract,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelManifest {
    pub schema: String,
    pub kind: BundleKind,
    pub profile: String,
    pub source: ModelSource,
    pub conversion: ConversionManifest,
    pub members: Vec<FileIdentity>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BundleIdentity(String);

impl BundleIdentity {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BundleIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BundleInspection {
    pub schema: String,
    pub kind: BundleKind,
    pub profile: String,
    pub bundle_id: BundleIdentity,
    pub model_bytes: u64,
    pub notice_bytes: u64,
    pub checkpoints: usize,
    pub channels: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Strand {
    Plus,
    Minus,
}

impl Strand {
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Plus => "+",
            Self::Minus => "-",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelContext(Vec<u8>);

impl ModelContext {
    pub fn new(bases: impl AsRef<[u8]>) -> Result<Self, ModelError> {
        let bases = bases.as_ref();
        if !(MIN_CONTEXT_LENGTH..=MAX_CONTEXT_LENGTH).contains(&bases.len()) {
            return Err(ModelError::ContextLength {
                observed: bases.len(),
            });
        }
        let mut normalized = Vec::with_capacity(bases.len());
        for (position, base) in bases.iter().copied().enumerate() {
            let base = base.to_ascii_uppercase();
            if !matches!(base, b'A' | b'C' | b'G' | b'T' | b'N') {
                return Err(ModelError::InvalidBase {
                    position,
                    byte: base,
                });
            }
            normalized.push(base);
        }
        Ok(Self(normalized))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn bases(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplicateScores {
    score_length: usize,
    values: Vec<f32>,
}

impl ReplicateScores {
    #[must_use]
    pub fn score_length(&self) -> usize {
        self.score_length
    }

    #[must_use]
    pub fn shape(&self) -> [usize; 3] {
        [1, CHANNELS, self.score_length]
    }

    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    #[must_use]
    pub fn channel(&self, ordinal: usize) -> Option<&[f32]> {
        let start = ordinal.checked_sub(1)?.checked_mul(self.score_length)?;
        self.values.get(start..start + self.score_length)
    }
}

#[derive(Debug)]
pub enum ModelError {
    Io {
        operation: &'static str,
        source: io::Error,
    },
    InvalidBundle(&'static str),
    IncompatibleBundle(&'static str),
    ContextLength {
        observed: usize,
    },
    InvalidBase {
        position: usize,
        byte: u8,
    },
    Runtime(String),
    OutputShape {
        expected: [usize; 3],
        observed: Vec<i64>,
    },
    OutputValue {
        index: usize,
        value: f32,
    },
}

impl fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
            Self::InvalidBundle(reason) => write!(formatter, "invalid model bundle: {reason}"),
            Self::IncompatibleBundle(reason) => {
                write!(formatter, "incompatible model bundle: {reason}")
            }
            Self::ContextLength { observed } => write!(
                formatter,
                "model context length {observed} is outside {MIN_CONTEXT_LENGTH}..={MAX_CONTEXT_LENGTH}"
            ),
            Self::InvalidBase { position, byte } => write!(
                formatter,
                "model context byte 0x{byte:02x} at zero-based position {position} is not A/C/G/T/N"
            ),
            Self::Runtime(reason) => write!(formatter, "ONNX Runtime failure: {reason}"),
            Self::OutputShape { expected, observed } => {
                write!(
                    formatter,
                    "model output shape {observed:?} does not match {expected:?}"
                )
            }
            Self::OutputValue { index, value } => {
                write!(
                    formatter,
                    "model output value {value} at flat index {index} is invalid"
                )
            }
        }
    }
}

impl std::error::Error for ModelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

struct AuthenticatedBundle {
    manifest: ModelManifest,
    identity: BundleIdentity,
    model_file: File,
    model_bytes: Vec<u8>,
}

pub struct ModelKernel {
    session: Session,
    identity: BundleIdentity,
    kind: BundleKind,
    profile: String,
    // Keep the authenticated descriptor alive for the entire session. The
    // graph was loaded from bytes read through this descriptor, not a reopened
    // pathname.
    _model_file: File,
}

impl ModelKernel {
    pub fn open(bundle: &Path) -> Result<Self, ModelError> {
        Self::open_with_cpu_policy(bundle, CpuPolicy::production_default())
    }

    /// Open an authenticated model bundle with an explicit CPU session policy.
    ///
    /// This low-level seam exists for qualification and callers that own their
    /// complete scheduling policy. Normal callers should use [`Self::open`].
    pub fn open_with_cpu_policy(bundle: &Path, policy: CpuPolicy) -> Result<Self, ModelError> {
        let authenticated = authenticate_bundle(bundle)?;
        let mut builder = Session::builder()
            .map_err(runtime_error)?
            .with_optimization_level(GraphOptimizationLevel::All)
            .map_err(runtime_error)?
            .with_parallel_execution(policy.execution == CpuExecutionMode::Parallel)
            .map_err(runtime_error)?
            .with_inter_threads(policy.inter_op.get())
            .map_err(runtime_error)?;
        if let IntraOpThreads::Fixed(threads) = policy.intra_op {
            builder = builder
                .with_intra_threads(threads.get())
                .map_err(runtime_error)?;
        }
        let session = builder
            .commit_from_memory(&authenticated.model_bytes)
            .map_err(runtime_error)?;
        validate_graph_contract(&session)?;
        let mut kernel = Self {
            session,
            identity: authenticated.identity,
            kind: authenticated.manifest.kind,
            profile: authenticated.manifest.profile,
            _model_file: authenticated.model_file,
        };
        let zeroes = ModelContext::new(vec![b'N'; MIN_CONTEXT_LENGTH])?;
        let probe = kernel.infer(&zeroes, Strand::Plus)?;
        if probe.shape() != [1, CHANNELS, 1] {
            return Err(ModelError::OutputShape {
                expected: [1, CHANNELS, 1],
                observed: probe
                    .shape()
                    .into_iter()
                    .map(|value| value as i64)
                    .collect(),
            });
        }
        Ok(kernel)
    }

    #[must_use]
    pub fn bundle_identity(&self) -> &BundleIdentity {
        &self.identity
    }

    #[must_use]
    pub fn bundle_kind(&self) -> BundleKind {
        self.kind
    }

    #[must_use]
    pub fn profile(&self) -> &str {
        &self.profile
    }

    pub fn infer(
        &mut self,
        context: &ModelContext,
        strand: Strand,
    ) -> Result<ReplicateScores, ModelError> {
        let score_length =
            context
                .len()
                .checked_sub(CONTEXT_FLANKS)
                .ok_or(ModelError::ContextLength {
                    observed: context.len(),
                })?;
        let encoded = encode_context(context, strand);
        let input =
            Tensor::<f32>::from_array(([1, 4, context.len()], encoded)).map_err(runtime_error)?;
        let outputs = self
            .session
            .run(ort::inputs! { INPUT_NAME => input })
            .map_err(runtime_error)?;
        let output = outputs
            .get(OUTPUT_NAME)
            .ok_or_else(|| ModelError::Runtime("missing replicate_scores output".to_owned()))?;
        let (shape, values) = output.try_extract_tensor::<f32>().map_err(runtime_error)?;
        let expected = [1, CHANNELS, score_length];
        let observed: Vec<i64> = shape.iter().copied().collect();
        if observed.as_slice() != [expected[0] as i64, expected[1] as i64, expected[2] as i64] {
            return Err(ModelError::OutputShape { expected, observed });
        }
        let mut values = values.to_vec();
        validate_output_values(&values)?;
        if strand == Strand::Minus {
            for channel in values.chunks_exact_mut(score_length) {
                channel.reverse();
            }
        }
        Ok(ReplicateScores {
            score_length,
            values,
        })
    }
}

pub fn canonical_manifest_bytes(manifest: &ModelManifest) -> Result<Vec<u8>, ModelError> {
    serde_jcs::to_vec(manifest).map_err(|_| ModelError::InvalidBundle("manifest encoding"))
}

#[must_use]
pub fn bundle_identity(manifest_bytes: &[u8]) -> BundleIdentity {
    BundleIdentity(format!("sha256:{:x}", Sha256::digest(manifest_bytes)))
}

pub fn inspect_bundle(bundle: &Path) -> Result<BundleInspection, ModelError> {
    let authenticated = authenticate_bundle(bundle)?;
    let model_bytes = member(&authenticated.manifest, "model.onnx")?.bytes;
    let notice_bytes = member(&authenticated.manifest, "NOTICE")?.bytes;
    Ok(BundleInspection {
        schema: authenticated.manifest.schema,
        kind: authenticated.manifest.kind,
        profile: authenticated.manifest.profile,
        bundle_id: authenticated.identity,
        model_bytes,
        notice_bytes,
        checkpoints: authenticated.manifest.source.checkpoints.len(),
        channels: authenticated.manifest.conversion.graph.channels.len(),
    })
}

pub fn parse_manifest_bytes(bytes: &[u8]) -> Result<ModelManifest, ModelError> {
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(ModelError::InvalidBundle("manifest size"));
    }
    let manifest: ModelManifest =
        serde_json::from_slice(bytes).map_err(|_| ModelError::InvalidBundle("manifest JSON"))?;
    if canonical_manifest_bytes(&manifest)? != bytes {
        return Err(ModelError::InvalidBundle("manifest canonical bytes"));
    }
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn authenticate_bundle(bundle: &Path) -> Result<AuthenticatedBundle, ModelError> {
    let root_metadata = fs::symlink_metadata(bundle)
        .map_err(|error| io_error("inspect bundle directory", error))?;
    if !root_metadata.file_type().is_dir() || root_metadata.file_type().is_symlink() {
        return Err(ModelError::InvalidBundle("bundle directory type"));
    }
    let observed = exact_member_set(bundle)?;
    if observed != EXACT_MEMBERS.into_iter().map(str::to_owned).collect() {
        return Err(ModelError::InvalidBundle("member set"));
    }

    let (mut manifest_file, manifest_size) =
        open_bounded_member(bundle, "manifest.json", MAX_MANIFEST_BYTES)?;
    let manifest_bytes = read_exact_member(
        &mut manifest_file,
        manifest_size,
        MAX_MANIFEST_BYTES,
        "read manifest",
    )?;
    let manifest = parse_manifest_bytes(&manifest_bytes)?;
    let identity = bundle_identity(&manifest_bytes);

    let (mut notice_file, notice_size) = open_bounded_member(bundle, "NOTICE", MAX_NOTICE_BYTES)?;
    let notice_bytes = read_exact_member(
        &mut notice_file,
        notice_size,
        MAX_NOTICE_BYTES,
        "read notice",
    )?;
    let (mut model_file, model_size) = open_bounded_member(bundle, "model.onnx", MAX_MODEL_BYTES)?;
    let model_bytes =
        read_exact_member(&mut model_file, model_size, MAX_MODEL_BYTES, "read model")?;

    let aggregate = manifest_size
        .checked_add(notice_size)
        .and_then(|value| value.checked_add(model_size))
        .ok_or(ModelError::InvalidBundle("aggregate size overflow"))?;
    if aggregate > MAX_AGGREGATE_BYTES {
        return Err(ModelError::InvalidBundle("aggregate size"));
    }
    validate_member_bytes(&manifest, "NOTICE", &notice_bytes)?;
    validate_member_bytes(&manifest, "model.onnx", &model_bytes)?;
    if exact_member_set(bundle)? != observed {
        return Err(ModelError::InvalidBundle("member set changed during open"));
    }
    model_file
        .seek(SeekFrom::Start(0))
        .map_err(|error| io_error("rewind model", error))?;

    Ok(AuthenticatedBundle {
        manifest,
        identity,
        model_file,
        model_bytes,
    })
}

fn exact_member_set(bundle: &Path) -> Result<BTreeSet<String>, ModelError> {
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(bundle).map_err(|error| io_error("read bundle directory", error))? {
        let entry = entry.map_err(|error| io_error("read bundle member", error))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| ModelError::InvalidBundle("non-UTF-8 member name"))?;
        if !names.insert(name) {
            return Err(ModelError::InvalidBundle("duplicate member name"));
        }
    }
    Ok(names)
}

fn open_bounded_member(bundle: &Path, name: &str, maximum: u64) -> Result<(File, u64), ModelError> {
    let path = bundle.join(name);
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc_flags::NOFOLLOW | libc_flags::CLOEXEC);
    let file = options
        .open(&path)
        .map_err(|error| io_error("open bundle member", error))?;
    let metadata = file
        .metadata()
        .map_err(|error| io_error("inspect bundle member", error))?;
    if !metadata.file_type().is_file() {
        return Err(ModelError::InvalidBundle("member is not a regular file"));
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(ModelError::InvalidBundle("member link count"));
    }
    if metadata.len() > maximum {
        return Err(ModelError::InvalidBundle("member size bound"));
    }
    Ok((file, metadata.len()))
}

fn read_exact_member(
    file: &mut File,
    expected: u64,
    maximum: u64,
    operation: &'static str,
) -> Result<Vec<u8>, ModelError> {
    let capacity =
        usize::try_from(expected).map_err(|_| ModelError::InvalidBundle("member size"))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(operation, error))?;
    if bytes.len() as u64 != expected {
        return Err(ModelError::InvalidBundle("member changed while reading"));
    }
    Ok(bytes)
}

fn validate_member_bytes(
    manifest: &ModelManifest,
    name: &str,
    bytes: &[u8],
) -> Result<(), ModelError> {
    let expected = member(manifest, name)?;
    if expected.bytes != bytes.len() as u64 {
        return Err(ModelError::InvalidBundle("member byte length"));
    }
    if expected.sha256 != sha256(bytes) {
        return Err(ModelError::InvalidBundle("member digest"));
    }
    Ok(())
}

fn member<'a>(manifest: &'a ModelManifest, name: &str) -> Result<&'a FileIdentity, ModelError> {
    manifest
        .members
        .iter()
        .find(|member| member.filename == name)
        .ok_or(ModelError::InvalidBundle("missing declared member"))
}

fn validate_manifest(manifest: &ModelManifest) -> Result<(), ModelError> {
    if manifest.schema != MODEL_BUNDLE_SCHEMA {
        return Err(ModelError::IncompatibleBundle("schema"));
    }
    match manifest.kind {
        BundleKind::Production => validate_production_source(manifest)?,
        BundleKind::SyntheticTest => {
            if manifest.profile != MINI_PROFILE {
                return Err(ModelError::IncompatibleBundle("synthetic profile"));
            }
            if !manifest.source.checkpoints.is_empty() {
                return Err(ModelError::InvalidBundle(
                    "synthetic bundle declares production checkpoints",
                ));
            }
        }
    }
    if manifest.members.len() != 2
        || manifest.members[0].filename != "NOTICE"
        || manifest.members[1].filename != "model.onnx"
    {
        return Err(ModelError::InvalidBundle("declared member order"));
    }
    if manifest.members[0].bytes > MAX_NOTICE_BYTES
        || manifest.members[1].bytes > MAX_MODEL_BYTES
        || manifest.members.iter().any(|item| !valid_sha(&item.sha256))
    {
        return Err(ModelError::InvalidBundle("declared member identity"));
    }
    validate_file_identity(&manifest.source.model_source)?;
    validate_file_identity(&manifest.conversion.converter)?;
    validate_file_identity(&manifest.conversion.checkpoint_inventory)?;
    validate_file_identity(&manifest.conversion.qualification_evidence)?;
    validate_graph(&manifest.conversion.graph)?;
    if manifest.conversion.environment.python.is_empty()
        || manifest.conversion.environment.pytorch.is_empty()
        || manifest.conversion.environment.numpy.is_empty()
        || manifest.conversion.environment.onnx.is_empty()
    {
        return Err(ModelError::InvalidBundle("conversion environment"));
    }
    Ok(())
}

fn validate_production_source(manifest: &ModelManifest) -> Result<(), ModelError> {
    if manifest.profile != PRODUCTION_PROFILE {
        return Err(ModelError::IncompatibleBundle("production profile"));
    }
    if manifest.source.upstream_commit != PRODUCTION_UPSTREAM_COMMIT
        || manifest.source.model_source.filename != "pangolin/model.py"
        || manifest.source.model_source.bytes != 3_011
        || manifest.source.model_source.sha256 != PRODUCTION_MODEL_SOURCE_SHA256
    {
        return Err(ModelError::InvalidBundle("production source identity"));
    }
    if manifest.source.checkpoints != production_checkpoints() {
        return Err(ModelError::InvalidBundle(
            "production checkpoint identities",
        ));
    }
    Ok(())
}

fn validate_file_identity(identity: &FileIdentity) -> Result<(), ModelError> {
    if identity.filename.is_empty()
        || identity.filename.contains('\0')
        || identity.filename.starts_with('/')
        || identity.filename.split('/').any(|part| part == "..")
        || !valid_sha(&identity.sha256)
    {
        return Err(ModelError::InvalidBundle("file identity"));
    }
    Ok(())
}

fn validate_graph(graph: &GraphContract) -> Result<(), ModelError> {
    if graph.opset != 17
        || graph.input.name != INPUT_NAME
        || graph.input.element_type != "f32"
        || graph.input.shape != ["1", "4", "N"]
        || graph.output.name != OUTPUT_NAME
        || graph.output.element_type != "f32"
        || graph.output.shape != ["1", "12", "N-10000"]
        || graph.exporter.dynamo
        || !graph.exporter.constant_folding
        || graph.exporter.dynamic_axis != 2
        || graph.channels.as_slice() != EXPECTED_CHANNEL_MAPPING
    {
        return Err(ModelError::InvalidBundle("graph contract"));
    }
    Ok(())
}

fn validate_graph_contract(session: &Session) -> Result<(), ModelError> {
    if session.inputs().len() != 1 || session.outputs().len() != 1 {
        return Err(ModelError::IncompatibleBundle("graph input/output count"));
    }
    let input = &session.inputs()[0];
    let output = &session.outputs()[0];
    if input.name() != INPUT_NAME || output.name() != OUTPUT_NAME {
        return Err(ModelError::IncompatibleBundle("graph input/output names"));
    }
    validate_outlet(
        input.dtype(),
        [1, 4, -1],
        ["", "", "N"],
        "graph input metadata",
    )?;
    validate_outlet(
        output.dtype(),
        [1, CHANNELS as i64, -1],
        ["", "", "N_minus_10000"],
        "graph output metadata",
    )
}

fn validate_outlet(
    value: &ValueType,
    expected_shape: [i64; 3],
    expected_symbols: [&str; 3],
    reason: &'static str,
) -> Result<(), ModelError> {
    let ValueType::Tensor {
        ty,
        shape,
        dimension_symbols,
    } = value
    else {
        return Err(ModelError::IncompatibleBundle(reason));
    };
    if *ty != TensorElementType::Float32
        || &**shape != expected_shape.as_slice()
        || !dimension_symbols
            .iter()
            .map(String::as_str)
            .eq(expected_symbols)
    {
        return Err(ModelError::IncompatibleBundle(reason));
    }
    Ok(())
}

fn validate_output_values(values: &[f32]) -> Result<(), ModelError> {
    for (index, value) in values.iter().copied().enumerate() {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ModelError::OutputValue { index, value });
        }
    }
    Ok(())
}

fn encode_context(context: &ModelContext, strand: Strand) -> Vec<f32> {
    let length = context.len();
    let mut encoded = vec![0.0; 4 * length];
    match strand {
        Strand::Plus => {
            for (position, base) in context.bases().iter().copied().enumerate() {
                if let Some(channel) = channel_for(base) {
                    encoded[channel * length + position] = 1.0;
                }
            }
        }
        Strand::Minus => {
            for (position, base) in context.bases().iter().rev().copied().enumerate() {
                if let Some(channel) = channel_for(complement(base)) {
                    encoded[channel * length + position] = 1.0;
                }
            }
        }
    }
    encoded
}

fn channel_for(base: u8) -> Option<usize> {
    match base {
        b'A' => Some(0),
        b'C' => Some(1),
        b'G' => Some(2),
        b'T' => Some(3),
        b'N' => None,
        _ => None,
    }
}

fn complement(base: u8) -> u8 {
    match base {
        b'A' => b'T',
        b'C' => b'G',
        b'G' => b'C',
        b'T' => b'A',
        b'N' => b'N',
        _ => base,
    }
}

fn valid_sha(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    })
}

#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn runtime_error(error: impl fmt::Display) -> ModelError {
    ModelError::Runtime(error.to_string())
}

fn io_error(operation: &'static str, source: io::Error) -> ModelError {
    ModelError::Io { operation, source }
}

#[cfg(unix)]
mod libc_flags {
    pub const NOFOLLOW: i32 = 0o400_000;
    pub const CLOEXEC: i32 = 0o2_000_000;
}

pub const EXPECTED_CHANNEL_MAPPING: &[ChannelMapping] = &[
    ChannelMapping {
        checkpoint_ordinal: 1,
        selected_channel: 1,
    },
    ChannelMapping {
        checkpoint_ordinal: 2,
        selected_channel: 1,
    },
    ChannelMapping {
        checkpoint_ordinal: 3,
        selected_channel: 1,
    },
    ChannelMapping {
        checkpoint_ordinal: 4,
        selected_channel: 4,
    },
    ChannelMapping {
        checkpoint_ordinal: 5,
        selected_channel: 4,
    },
    ChannelMapping {
        checkpoint_ordinal: 6,
        selected_channel: 4,
    },
    ChannelMapping {
        checkpoint_ordinal: 7,
        selected_channel: 7,
    },
    ChannelMapping {
        checkpoint_ordinal: 8,
        selected_channel: 7,
    },
    ChannelMapping {
        checkpoint_ordinal: 9,
        selected_channel: 7,
    },
    ChannelMapping {
        checkpoint_ordinal: 10,
        selected_channel: 10,
    },
    ChannelMapping {
        checkpoint_ordinal: 11,
        selected_channel: 10,
    },
    ChannelMapping {
        checkpoint_ordinal: 12,
        selected_channel: 10,
    },
];

#[must_use]
pub fn production_checkpoints() -> Vec<CheckpointIdentity> {
    const VALUES: [(u8, &str, &str); 12] = [
        (
            1,
            "final.1.0.3.v2",
            "f0478fab173b75f7f7e9fe96688bad6c50fa4a46d70557f423b110caaf565501",
        ),
        (
            2,
            "final.2.0.3.v2",
            "c4c6bb4880fa6fb28b14182ae3ea0600edb07056158f55325b5e6e6e48fc9f26",
        ),
        (
            3,
            "final.3.0.3.v2",
            "ec685a6e7105a4486c1f89a005458a13deb3fe7171f13d434f4877e386d10676",
        ),
        (
            4,
            "final.1.2.3.v2",
            "559c05de3e1ce65c2515ca3e92ef85edb0ec2e47686ca58060e25891ce06eb3a",
        ),
        (
            5,
            "final.2.2.3.v2",
            "48758ba8b95eee9aa9feea52672ef06ca1b34111299c27f8a710f734d8b9aae5",
        ),
        (
            6,
            "final.3.2.3.v2",
            "7cb576c2b24db4fdd6970c4ca4fb7c20ae1b1d8ae80645ebbe689848b5743129",
        ),
        (
            7,
            "final.1.4.3.v2",
            "c50b12e0c0af776d5674ca5e346493f8265783494d4df383364de9c1136657f6",
        ),
        (
            8,
            "final.2.4.3.v2",
            "e03303bed4fd6f135ec0f6c1b192cce954ea42d0646f44d17b4a6fbb2b1f610e",
        ),
        (
            9,
            "final.3.4.3.v2",
            "9476d2e25520d7ff15bece0cd5d3b657e3b1dd3cc5fcab1d9c3b62bea7a0c5b6",
        ),
        (
            10,
            "final.1.6.3.v2",
            "2aae563fa18a8a9b6699c6c96e0d32b8ec7543f8f805fb3bc9de77302cc9f66e",
        ),
        (
            11,
            "final.2.6.3.v2",
            "7d3c0b1b2a60067b940dec315567874fbc8bcd322f1b7c76bf969f51f0f53f7f",
        ),
        (
            12,
            "final.3.6.3.v2",
            "756e7721a382cace24e9bfea5b543af5623f2487d9a3efe7385e9c76367005fd",
        ),
    ];
    VALUES
        .into_iter()
        .map(|(ordinal, filename, sha256)| CheckpointIdentity {
            ordinal,
            filename: filename.to_owned(),
            bytes: 2_877_321,
            sha256: format!("sha256:{sha256}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_normalizes_case_and_rejects_positioned_non_acgtn() {
        let context =
            ModelContext::new(std::iter::repeat_n(b'n', MIN_CONTEXT_LENGTH).collect::<Vec<_>>())
                .expect("lowercase N is valid");
        assert!(context.bases().iter().all(|base| *base == b'N'));

        let mut invalid = vec![b'A'; MIN_CONTEXT_LENGTH];
        invalid[9] = b'R';
        assert!(matches!(
            ModelContext::new(invalid),
            Err(ModelError::InvalidBase {
                position: 9,
                byte: b'R'
            })
        ));
    }

    #[test]
    fn context_length_bounds_are_closed() {
        assert!(ModelContext::new(vec![b'A'; MIN_CONTEXT_LENGTH]).is_ok());
        assert!(ModelContext::new(vec![b'A'; MAX_CONTEXT_LENGTH]).is_ok());
        assert!(matches!(
            ModelContext::new(vec![b'A'; MIN_CONTEXT_LENGTH - 1]),
            Err(ModelError::ContextLength { .. })
        ));
        assert!(matches!(
            ModelContext::new(vec![b'A'; MAX_CONTEXT_LENGTH + 1]),
            Err(ModelError::ContextLength { .. })
        ));
    }

    #[test]
    fn strand_encoding_is_reverse_complement_and_n_is_zero() {
        let mut bases = vec![b'N'; MIN_CONTEXT_LENGTH];
        bases[..5].copy_from_slice(b"ACGTN");
        let context = ModelContext::new(bases).expect("context");
        let plus = encode_context(&context, Strand::Plus);
        assert_eq!(plus[0], 1.0);
        assert_eq!(plus[context.len() + 1], 1.0);
        assert_eq!(plus[2 * context.len() + 2], 1.0);
        assert_eq!(plus[3 * context.len() + 3], 1.0);
        assert_eq!(
            plus.iter()
                .enumerate()
                .filter(|(_, value)| **value == 1.0)
                .count(),
            4
        );

        let minus = encode_context(&context, Strand::Minus);
        let end = context.len() - 1;
        assert_eq!(minus[3 * context.len() + end], 1.0);
        assert_eq!(minus[2 * context.len() + end - 1], 1.0);
        assert_eq!(minus[context.len() + end - 2], 1.0);
        assert_eq!(minus[end - 3], 1.0);
    }

    #[test]
    fn production_checkpoint_contract_is_exact() {
        let checkpoints = production_checkpoints();
        assert_eq!(checkpoints.len(), CHANNELS);
        assert_eq!(
            checkpoints.iter().map(|item| item.bytes).sum::<u64>(),
            34_527_852
        );
    }

    #[test]
    fn graph_metadata_rejects_wrong_type_rank_and_fixed_dimensions() {
        let wrong_type = ValueType::Tensor {
            ty: TensorElementType::Int64,
            shape: [1_i64, 4, -1].into(),
            dimension_symbols: ort::value::SymbolicDimensions::new([
                String::new(),
                String::new(),
                "N".to_owned(),
            ]),
        };
        assert!(validate_outlet(&wrong_type, [1, 4, -1], ["", "", "N"], "input").is_err());

        let wrong_rank = ValueType::Tensor {
            ty: TensorElementType::Float32,
            shape: [1_i64, 4].into(),
            dimension_symbols: ort::value::SymbolicDimensions::empty(2),
        };
        assert!(validate_outlet(&wrong_rank, [1, 4, -1], ["", "", "N"], "input").is_err());

        let wrong_fixed_dimension = ValueType::Tensor {
            ty: TensorElementType::Float32,
            shape: [1_i64, 5, -1].into(),
            dimension_symbols: ort::value::SymbolicDimensions::new([
                String::new(),
                String::new(),
                "N".to_owned(),
            ]),
        };
        assert!(
            validate_outlet(&wrong_fixed_dimension, [1, 4, -1], ["", "", "N"], "input").is_err()
        );
    }

    #[test]
    fn graph_metadata_rejects_wrong_missing_and_fixed_axis_symbols() {
        for symbols in [
            ["", "", "wrong"],
            ["", "", ""],
            ["batch", "", "N"],
            ["", "channels", "N"],
        ] {
            let value = ValueType::Tensor {
                ty: TensorElementType::Float32,
                shape: [1_i64, 4, -1].into(),
                dimension_symbols: ort::value::SymbolicDimensions::new(symbols.map(str::to_owned)),
            };
            assert!(validate_outlet(&value, [1, 4, -1], ["", "", "N"], "input").is_err());
        }

        let exact = ValueType::Tensor {
            ty: TensorElementType::Float32,
            shape: [1_i64, 12, -1].into(),
            dimension_symbols: ort::value::SymbolicDimensions::new([
                String::new(),
                String::new(),
                "N_minus_10000".to_owned(),
            ]),
        };
        assert!(
            validate_outlet(
                &exact,
                [1, CHANNELS as i64, -1],
                ["", "", "N_minus_10000"],
                "output",
            )
            .is_ok()
        );
    }

    #[test]
    fn output_values_reject_nan_infinity_and_range() {
        assert!(validate_output_values(&[0.0, 0.5, 1.0]).is_ok());
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.1] {
            assert!(matches!(
                validate_output_values(&[value]),
                Err(ModelError::OutputValue { index: 0, .. })
            ));
        }
    }
}
