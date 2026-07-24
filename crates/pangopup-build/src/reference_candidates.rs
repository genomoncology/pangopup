//! Authenticated preparation and inspection of benchmark reference candidates.

use crate::compatibility::inspect_corpus;
use pangopup_core::Grch38Contig;
use pangopup_index::reference_candidates::{
    CandidateCodec, CandidateError, CandidateReader, CandidateSetWriter, ContigPlan, sha256_file,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::fs::DirBuilderExt,
    os::unix::fs::MetadataExt,
    path::Path,
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

pub const REAL_PROFILE: &str = "refseq-grch38p14-compat-six-contigs-v1";
pub const MINI_PROFILE: &str = "pangopup-reference-candidates-mini-v1";
pub const CANDIDATE_SCHEMA: &str = "pangopup-reference-candidates-v1";
pub const CONTAINER_SCHEMA: &str = "pgrben01-v1";
const REAL_SOURCE_BYTES: u64 = 671_294_255;
const REAL_SOURCE_SHA256: &str = "81645a227efbbd196ae337f743f31a5b1c32979d6d7bb5713e0322402a70fafb";
const REAL_MANIFEST_BYTES: u64 = 5_337;
const REAL_MANIFEST_SHA256: &str =
    "fd12a0d6b503d1e572c0561eb43e66f19c55c4d073b25bced25be6303fd0553b";
const REAL_CASES_BYTES: u64 = 220_071;
const REAL_CASES_SHA256: &str = "2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8";
const REAL_CORPUS_PROFILE: &str = "pangolin-1.0.2-5cf94b8-grch38-v1";
const MANIFEST_MAX: u64 = 16_384;
const CASES_MAX: u64 = 4_000_000;
const FASTA_LINE_MAX: usize = 1_048_576;
static STAGING_SERIAL: AtomicU64 = AtomicU64::new(0);

// Filled from the independently authored checked fixture. These constants are
// deliberately not derived from a candidate manifest at inspection time.
const MINI_SOURCE_BYTES: u64 = 16_621;
const MINI_SOURCE_SHA256: &str = "57d45eee6e9c14b2ca170b7ac3014dd45100d797f4a763f08d5abc8a6a4fb1c8";
const MINI_MANIFEST_BYTES: u64 = 209;
const MINI_MANIFEST_SHA256: &str =
    "528a7b736112b3188ce9ab31fdbdbe14052e8ba5388433f3df7f18f82236474e";
const MINI_CASES_BYTES: u64 = 792;
const MINI_CASES_SHA256: &str = "8580670063e92dec05ca8e8637ffe5e730d355721c4df5a052ad609d99b12a23";
const MINI_MEMBER_IDENTITIES: [(&str, u64, &str); 4] = [
    (
        "manifest.json",
        987,
        "557cfa37dda0cb7d89b552d2e3cb2a3c31ebea26a937f386ff149e6ed17c08ff",
    ),
    (
        "ascii8.pgr",
        20_499,
        "f9729366e0bf5cb17f18540d1dffb2f01288541b595364cb6b0218f0fb271ae8",
    ),
    (
        "iupac4.pgr",
        12_298,
        "4a5091b53f5d00ba7c2734af98fbead9464e71c05461975e6dec59ab6bc9e97e",
    ),
    (
        "acgt2-rle-v1.pgr",
        8_416,
        "e948cfacbcdfba3b099cb57913607fdfb0f642b584ebcead6505ac3fca689fe1",
    ),
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateManifest {
    pub schema: String,
    pub profile: String,
    pub source: Identity,
    pub corpus: CorpusIdentity,
    pub container: ContainerIdentity,
    pub members: Vec<CandidateMember>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusIdentity {
    pub schema: String,
    pub profile: String,
    pub manifest_bytes: u64,
    pub manifest_sha256: String,
    pub cases_bytes: u64,
    pub cases_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerIdentity {
    pub schema: String,
    pub page_bytes: u64,
    pub contigs: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateMember {
    pub codec: CandidateCodec,
    pub filename: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LiteralContext {
    id: String,
    contig: String,
    start_1based: u64,
    bases: String,
    sha256: String,
}

#[derive(Deserialize)]
struct RealCaseHead {
    id: String,
    kind: String,
    context: Option<RealContext>,
}

#[derive(Deserialize)]
struct RealContext {
    start_1based: u64,
    bases: String,
    sha256: String,
}

#[derive(Deserialize)]
struct RealInputHead {
    input: RealInput,
}

#[derive(Deserialize)]
struct RealInput {
    contig: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PrepareOutcome {
    pub ok: bool,
    pub command: &'static str,
    pub profile: String,
    pub candidate_set_sha256: String,
    pub members: Vec<OutcomeMember>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectOutcome {
    pub ok: bool,
    pub command: &'static str,
    pub profile: String,
    pub candidate_set_sha256: String,
    pub source_sha256: String,
    pub corpus_manifest_sha256: String,
    pub contexts_verified: u64,
    pub members: Vec<OutcomeMember>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OutcomeMember {
    pub codec: CandidateCodec,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug)]
pub struct ReferenceCandidateError {
    code: &'static str,
    message: &'static str,
}

impl ReferenceCandidateError {
    pub const fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }
    pub const fn code(&self) -> &'static str {
        self.code
    }
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl std::fmt::Display for ReferenceCandidateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}
impl std::error::Error for ReferenceCandidateError {}

impl From<CandidateError> for ReferenceCandidateError {
    fn from(error: CandidateError) -> Self {
        Self::new(
            error.kind(),
            match error.kind() {
                "container" => "candidate container is invalid",
                "bounds" => "reference window is out of bounds",
                "oracle" => "literal reference oracle is invalid",
                "resource" => "candidate resource limit exceeded",
                _ => "candidate I/O failed",
            },
        )
    }
}

pub fn prepare_candidates(
    source: &Path,
    corpus: &Path,
    output: &Path,
) -> Result<PrepareOutcome, ReferenceCandidateError> {
    if output.exists() {
        return Err(ReferenceCandidateError::new(
            "already_exists",
            "output already exists",
        ));
    }
    let (plans, source_identity) = scan_fasta(source)?;
    if source_identity.bytes != REAL_SOURCE_BYTES || source_identity.sha256 != REAL_SOURCE_SHA256 {
        return Err(ReferenceCandidateError::new(
            "source_identity",
            "reference source identity mismatch",
        ));
    }
    inspect_corpus(corpus).map_err(|_| {
        ReferenceCandidateError::new("corpus_identity", "compatibility corpus identity mismatch")
    })?;
    let corpus_identity = authenticate_real_corpus(corpus)?;
    let contexts = real_contexts(corpus)?;
    prepare_with_plans(Preparation {
        source,
        output,
        profile: REAL_PROFILE,
        source_identity,
        corpus_identity,
        contexts: &contexts,
        plans: &plans,
        publish: true,
    })
}

pub fn inspect_candidates(
    candidates: &Path,
    corpus: &Path,
) -> Result<InspectOutcome, ReferenceCandidateError> {
    let bytes = read_bounded_regular(
        &candidates.join("manifest.json"),
        MANIFEST_MAX,
        "candidate_set",
    )?;
    let manifest: CandidateManifest = serde_json::from_slice(&bytes).map_err(|_| {
        ReferenceCandidateError::new("candidate_set", "candidate manifest is invalid")
    })?;
    let canonical = serde_jcs::to_vec(&manifest).map_err(|_| {
        ReferenceCandidateError::new("candidate_set", "candidate manifest is invalid")
    })?;
    if canonical != bytes {
        return Err(ReferenceCandidateError::new(
            "candidate_set",
            "candidate manifest is not canonical",
        ));
    }
    validate_directory(candidates)?;
    let contexts = match manifest.profile.as_str() {
        REAL_PROFILE => {
            inspect_corpus(corpus).map_err(|_| {
                ReferenceCandidateError::new(
                    "corpus_identity",
                    "compatibility corpus identity mismatch",
                )
            })?;
            let identity = authenticate_real_corpus(corpus)?;
            if manifest.source.bytes != REAL_SOURCE_BYTES
                || manifest.source.sha256 != REAL_SOURCE_SHA256
                || manifest.corpus != identity
            {
                return Err(ReferenceCandidateError::new(
                    "unsupported_profile",
                    "candidate profile is not registered",
                ));
            }
            real_contexts(corpus)?
        }
        MINI_PROFILE => {
            authenticate_mini_registry(candidates, corpus, &manifest)?;
            miniature_contexts(corpus)?
        }
        _ => {
            return Err(ReferenceCandidateError::new(
                "unsupported_profile",
                "candidate profile is not registered",
            ));
        }
    };
    validate_manifest_shape(&manifest)?;
    let members = inspect_members(candidates, &manifest, &contexts)?;
    Ok(InspectOutcome {
        ok: true,
        command: "reference-candidates.inspect",
        profile: manifest.profile,
        candidate_set_sha256: format!("{:x}", Sha256::digest(&bytes)),
        source_sha256: manifest.source.sha256,
        corpus_manifest_sha256: manifest.corpus.manifest_sha256,
        contexts_verified: contexts.len() as u64,
        members,
    })
}

#[cfg(test)]
fn prepare_authenticated(
    source: &Path,
    output: &Path,
    profile: &str,
    source_identity: Identity,
    corpus_identity: CorpusIdentity,
    contexts: &[LiteralContext],
    publish: bool,
) -> Result<PrepareOutcome, ReferenceCandidateError> {
    let (plans, scanned) = scan_fasta(source)?;
    if scanned != source_identity {
        return Err(ReferenceCandidateError::new(
            "source_identity",
            "reference source changed while reading",
        ));
    }
    prepare_with_plans(Preparation {
        source,
        output,
        profile,
        source_identity,
        corpus_identity,
        contexts,
        plans: &plans,
        publish,
    })
}

struct Preparation<'a> {
    source: &'a Path,
    output: &'a Path,
    profile: &'a str,
    source_identity: Identity,
    corpus_identity: CorpusIdentity,
    contexts: &'a [LiteralContext],
    plans: &'a [ContigPlan],
    publish: bool,
}

fn prepare_with_plans(
    preparation: Preparation<'_>,
) -> Result<PrepareOutcome, ReferenceCandidateError> {
    let Preparation {
        source,
        output,
        profile,
        source_identity,
        corpus_identity,
        contexts,
        plans,
        publish,
    } = preparation;
    let parent = output.parent().ok_or(ReferenceCandidateError::new(
        "path",
        "output requires a parent directory",
    ))?;
    if !output.is_absolute() {
        return Err(ReferenceCandidateError::new(
            "path",
            "output path must be absolute",
        ));
    }
    let filename =
        output
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(ReferenceCandidateError::new(
                "path",
                "output path is invalid",
            ))?;
    let staging = parent.join(format!(
        ".{filename}.staging-{}-{}",
        std::process::id(),
        STAGING_SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&staging)
        .map_err(|_| ReferenceCandidateError::new("io", "candidate staging failed"))?;
    let result = (|| {
        let mut writer = CandidateSetWriter::create(&staging, plans)?;
        let generated_identity = stream_fasta(source, |event| match event {
            FastaEvent::Start(contig) => writer.begin_contig(contig),
            FastaEvent::Bases(bytes) => writer.write_bases(bytes),
            FastaEvent::End => writer.end_contig(),
        })?;
        if generated_identity != source_identity {
            return Err(ReferenceCandidateError::new(
                "source_identity",
                "reference source changed during generation",
            ));
        }
        writer.finish()?;
        let members = member_outcomes(&staging)?;
        let manifest = CandidateManifest {
            schema: CANDIDATE_SCHEMA.into(),
            profile: profile.into(),
            source: source_identity.clone(),
            corpus: corpus_identity.clone(),
            container: ContainerIdentity {
                schema: CONTAINER_SCHEMA.into(),
                page_bytes: 4096,
                contigs: plans.iter().map(|plan| plan.contig.code()).collect(),
            },
            members: members
                .iter()
                .map(|member| CandidateMember {
                    codec: member.codec,
                    filename: member.codec.filename().into(),
                    bytes: member.bytes,
                    sha256: member.sha256.clone(),
                })
                .collect(),
        };
        let manifest_bytes = serde_jcs::to_vec(&manifest).map_err(|_| {
            ReferenceCandidateError::new("candidate_set", "candidate manifest serialization failed")
        })?;
        write_synced(&staging.join("manifest.json"), &manifest_bytes)?;
        sync_dir(&staging)?;
        let inspected_members = inspect_members(&staging, &manifest, contexts)?;
        if inspected_members != members {
            return Err(ReferenceCandidateError::new(
                "candidate_member",
                "candidate member changed during preparation",
            ));
        }
        let identity = format!("{:x}", Sha256::digest(&manifest_bytes));
        if publish {
            publish_candidate(&staging, parent, output)?;
        }
        Ok(PrepareOutcome {
            ok: true,
            command: "reference-candidates.prepare",
            profile: profile.into(),
            candidate_set_sha256: identity,
            members,
        })
    })();
    if result.is_err() || !publish {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn publish_candidate(
    staging: &Path,
    parent: &Path,
    output: &Path,
) -> Result<(), ReferenceCandidateError> {
    publish_candidate_with(
        || {
            rustix::fs::renameat_with(
                rustix::fs::CWD,
                staging,
                rustix::fs::CWD,
                output,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(|error| {
                if error == rustix::io::Errno::EXIST {
                    ReferenceCandidateError::new("already_exists", "output already exists")
                } else {
                    ReferenceCandidateError::new("io", "candidate publication failed")
                }
            })
        },
        || sync_dir(parent),
        || {
            fs::remove_dir_all(output).map_err(|_| {
                ReferenceCandidateError::new("io", "candidate publication rollback failed")
            })?;
            sync_dir(parent)
        },
    )
}

fn publish_candidate_with(
    rename: impl FnOnce() -> Result<(), ReferenceCandidateError>,
    sync_parent: impl FnOnce() -> Result<(), ReferenceCandidateError>,
    rollback: impl FnOnce() -> Result<(), ReferenceCandidateError>,
) -> Result<(), ReferenceCandidateError> {
    rename()?;
    if sync_parent().is_err() {
        rollback()?;
        return Err(ReferenceCandidateError::new(
            "io",
            "candidate parent sync failed",
        ));
    }
    Ok(())
}

impl PartialEq for OutcomeMember {
    fn eq(&self, other: &Self) -> bool {
        self.codec == other.codec && self.bytes == other.bytes && self.sha256 == other.sha256
    }
}

fn inspect_members(
    root: &Path,
    manifest: &CandidateManifest,
    contexts: &[LiteralContext],
) -> Result<Vec<OutcomeMember>, ReferenceCandidateError> {
    let mut result = Vec::new();
    for (expected_codec, member) in CandidateCodec::ALL.iter().zip(&manifest.members) {
        if member.codec != *expected_codec || member.filename != expected_codec.filename() {
            return Err(ReferenceCandidateError::new(
                "candidate_set",
                "candidate member order mismatch",
            ));
        }
        let identity =
            hash_regular_file(&root.join(&member.filename), u64::MAX, "candidate_member")?;
        if identity.bytes != member.bytes || identity.sha256 != member.sha256 {
            return Err(ReferenceCandidateError::new(
                "candidate_member",
                "candidate member identity mismatch",
            ));
        }
        let reader = CandidateReader::open(&root.join(&member.filename))?;
        if reader.codec() != member.codec {
            return Err(ReferenceCandidateError::new(
                "container",
                "candidate codec mismatch",
            ));
        }
        reader.inspect_payload()?;
        for context in contexts {
            let contig = Grch38Contig::from_str(&context.contig).map_err(|_| {
                ReferenceCandidateError::new("oracle", "literal context contig is invalid")
            })?;
            let mut actual = vec![0; context.bases.len()];
            reader.copy_window(contig, context.start_1based, &mut actual)?;
            if actual != context.bases.as_bytes()
                || format!("{:x}", Sha256::digest(&actual)) != context.sha256
            {
                return Err(ReferenceCandidateError::new(
                    "oracle",
                    "literal context mismatch",
                ));
            }
        }
        result.push(OutcomeMember {
            codec: member.codec,
            bytes: member.bytes,
            sha256: member.sha256.clone(),
        });
    }
    Ok(result)
}

fn validate_manifest_shape(manifest: &CandidateManifest) -> Result<(), ReferenceCandidateError> {
    if manifest.schema != CANDIDATE_SCHEMA
        || manifest.container.schema != CONTAINER_SCHEMA
        || manifest.container.page_bytes != 4096
        || manifest.members.len() != 3
        || manifest.source.sha256.len() != 64
        || manifest.corpus.manifest_sha256.len() != 64
        || manifest.corpus.cases_sha256.len() != 64
    {
        return Err(ReferenceCandidateError::new(
            "candidate_set",
            "candidate manifest shape mismatch",
        ));
    }
    let expected_contigs: &[u8] = match manifest.profile.as_str() {
        REAL_PROFILE => &[3, 10, 12, 13, 17, 25],
        MINI_PROFILE => &[3, 25],
        _ => {
            return Err(ReferenceCandidateError::new(
                "unsupported_profile",
                "candidate profile is not registered",
            ));
        }
    };
    if manifest.container.contigs != expected_contigs {
        return Err(ReferenceCandidateError::new(
            "candidate_set",
            "candidate contig registry mismatch",
        ));
    }
    Ok(())
}

fn validate_directory(root: &Path) -> Result<(), ReferenceCandidateError> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|_| ReferenceCandidateError::new("path", "candidate directory is unavailable"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ReferenceCandidateError::new(
            "path",
            "candidate path must be a directory",
        ));
    }
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(root)
        .map_err(|_| ReferenceCandidateError::new("io", "candidate directory read failed"))?
    {
        let entry = entry
            .map_err(|_| ReferenceCandidateError::new("io", "candidate directory read failed"))?;
        let file_type = entry
            .file_type()
            .map_err(|_| ReferenceCandidateError::new("io", "candidate member metadata failed"))?;
        if !file_type.is_file() || file_type.is_symlink() {
            return Err(ReferenceCandidateError::new(
                "candidate_set",
                "candidate directory contains a non-file",
            ));
        }
        names.insert(entry.file_name());
    }
    let expected: BTreeSet<OsString> = [
        "manifest.json",
        "ascii8.pgr",
        "iupac4.pgr",
        "acgt2-rle-v1.pgr",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    if names != expected {
        return Err(ReferenceCandidateError::new(
            "candidate_set",
            "candidate directory members mismatch",
        ));
    }
    Ok(())
}

fn authenticate_real_corpus(root: &Path) -> Result<CorpusIdentity, ReferenceCandidateError> {
    let manifest = hash_regular_file(&root.join("manifest.json"), 128 * 1024, "corpus_identity")?;
    let cases = hash_regular_file(&root.join("cases.jsonl"), CASES_MAX, "corpus_identity")?;
    if manifest.bytes != REAL_MANIFEST_BYTES
        || manifest.sha256 != REAL_MANIFEST_SHA256
        || cases.bytes != REAL_CASES_BYTES
        || cases.sha256 != REAL_CASES_SHA256
    {
        return Err(ReferenceCandidateError::new(
            "corpus_identity",
            "compatibility corpus identity mismatch",
        ));
    }
    Ok(CorpusIdentity {
        schema: "pangopup-compat-v1".into(),
        profile: REAL_CORPUS_PROFILE.into(),
        manifest_bytes: manifest.bytes,
        manifest_sha256: manifest.sha256,
        cases_bytes: cases.bytes,
        cases_sha256: cases.sha256,
    })
}

fn authenticate_mini_registry(
    candidates: &Path,
    corpus: &Path,
    manifest: &CandidateManifest,
) -> Result<(), ReferenceCandidateError> {
    let names: BTreeSet<OsString> = fs::read_dir(corpus)
        .map_err(|_| {
            ReferenceCandidateError::new("corpus_identity", "miniature corpus is unavailable")
        })?
        .map(|entry| {
            entry.map(|value| value.file_name()).map_err(|_| {
                ReferenceCandidateError::new("corpus_identity", "miniature corpus is unavailable")
            })
        })
        .collect::<Result<_, _>>()?;
    let expected: BTreeSet<OsString> = ["cases.jsonl", "manifest.json"]
        .into_iter()
        .map(OsString::from)
        .collect();
    if names != expected {
        return Err(ReferenceCandidateError::new(
            "corpus_identity",
            "miniature corpus members mismatch",
        ));
    }
    let source_identity = Identity {
        bytes: MINI_SOURCE_BYTES,
        sha256: MINI_SOURCE_SHA256.into(),
    };
    let corpus_identity = CorpusIdentity {
        schema: "pangopup-compat-v1".into(),
        profile: MINI_PROFILE.into(),
        manifest_bytes: MINI_MANIFEST_BYTES,
        manifest_sha256: MINI_MANIFEST_SHA256.into(),
        cases_bytes: MINI_CASES_BYTES,
        cases_sha256: MINI_CASES_SHA256.into(),
    };
    if manifest.source != source_identity || manifest.corpus != corpus_identity {
        return Err(ReferenceCandidateError::new(
            "unsupported_profile",
            "candidate profile is not registered",
        ));
    }
    for (name, bytes, sha) in MINI_MEMBER_IDENTITIES {
        let actual = hash_regular_file(
            &candidates.join(name),
            MANIFEST_MAX.max(bytes),
            "candidate_member",
        )?;
        if actual.bytes != bytes || actual.sha256 != sha {
            return Err(ReferenceCandidateError::new(
                "unsupported_profile",
                "miniature candidate identity mismatch",
            ));
        }
    }
    let manifest_actual = hash_regular_file(
        &corpus.join("manifest.json"),
        MANIFEST_MAX,
        "corpus_identity",
    )?;
    let cases_actual =
        hash_regular_file(&corpus.join("cases.jsonl"), CASES_MAX, "corpus_identity")?;
    if manifest_actual.bytes != MINI_MANIFEST_BYTES
        || manifest_actual.sha256 != MINI_MANIFEST_SHA256
        || cases_actual.bytes != MINI_CASES_BYTES
        || cases_actual.sha256 != MINI_CASES_SHA256
    {
        return Err(ReferenceCandidateError::new(
            "corpus_identity",
            "miniature corpus identity mismatch",
        ));
    }
    Ok(())
}

fn miniature_contexts(root: &Path) -> Result<Vec<LiteralContext>, ReferenceCandidateError> {
    decode_literal_contexts(&read_bounded_regular(
        &root.join("cases.jsonl"),
        CASES_MAX,
        "corpus_identity",
    )?)
}

fn real_contexts(root: &Path) -> Result<Vec<LiteralContext>, ReferenceCandidateError> {
    let bytes = read_bounded_regular(&root.join("cases.jsonl"), CASES_MAX, "corpus_identity")?;
    let text = std::str::from_utf8(&bytes).map_err(|_| {
        ReferenceCandidateError::new("corpus_identity", "compatibility cases are invalid")
    })?;
    let mut contexts = Vec::new();
    for line in text.lines().take(14) {
        let head: RealCaseHead = serde_json::from_str(line).map_err(|_| {
            ReferenceCandidateError::new("corpus_identity", "compatibility cases are invalid")
        })?;
        if head.kind != "model" {
            return Err(ReferenceCandidateError::new(
                "corpus_identity",
                "compatibility model order mismatch",
            ));
        }
        let input: RealInputHead = serde_json::from_str(line).map_err(|_| {
            ReferenceCandidateError::new("corpus_identity", "compatibility cases are invalid")
        })?;
        let context = head.context.ok_or(ReferenceCandidateError::new(
            "corpus_identity",
            "compatibility context is missing",
        ))?;
        contexts.push(LiteralContext {
            id: head.id,
            contig: input.input.contig,
            start_1based: context.start_1based,
            bases: context.bases,
            sha256: context.sha256,
        });
    }
    if contexts.len() != 14 {
        return Err(ReferenceCandidateError::new(
            "corpus_identity",
            "compatibility context count mismatch",
        ));
    }
    Ok(contexts)
}

fn decode_literal_contexts(bytes: &[u8]) -> Result<Vec<LiteralContext>, ReferenceCandidateError> {
    if !bytes.ends_with(b"\n") {
        return Err(ReferenceCandidateError::new(
            "oracle",
            "literal contexts require terminal newline",
        ));
    }
    let mut result = Vec::new();
    for line in bytes[..bytes.len() - 1].split(|byte| *byte == b'\n') {
        let context: LiteralContext = serde_json::from_slice(line).map_err(|_| {
            ReferenceCandidateError::new("oracle", "literal context schema is invalid")
        })?;
        let canonical = serde_jcs::to_vec(&context)
            .map_err(|_| ReferenceCandidateError::new("oracle", "literal context is invalid"))?;
        if canonical != line
            || context.bases.is_empty()
            || context.sha256 != format!("{:x}", Sha256::digest(context.bases.as_bytes()))
            || context
                .bases
                .bytes()
                .any(|base| pangopup_index::reference_candidates::iupac_code(base).is_none())
        {
            return Err(ReferenceCandidateError::new(
                "oracle",
                "literal context identity mismatch",
            ));
        }
        result.push(context);
    }
    if result.is_empty() {
        return Err(ReferenceCandidateError::new(
            "oracle",
            "literal context set is empty",
        ));
    }
    Ok(result)
}

enum FastaEvent<'a> {
    Start(Grch38Contig),
    Bases(&'a [u8]),
    End,
}

fn scan_fasta(path: &Path) -> Result<(Vec<ContigPlan>, Identity), ReferenceCandidateError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        ReferenceCandidateError::new("source_identity", "reference source is unavailable")
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1 {
        return Err(ReferenceCandidateError::new(
            "source_identity",
            "reference source identity is invalid",
        ));
    }
    let file = File::open(path)
        .map_err(|_| ReferenceCandidateError::new("io", "reference source open failed"))?;
    let mut reader = BufReader::with_capacity(128 * 1024, HashingRead::new(file));
    let mut plans = Vec::new();
    let mut current: Option<(Grch38Contig, u64)> = None;
    parse_fasta(&mut reader, |event| {
        match event {
            FastaEvent::Start(contig) => current = Some((contig, 0)),
            FastaEvent::Bases(bytes) => {
                if bytes
                    .iter()
                    .any(|base| pangopup_index::reference_candidates::iupac_code(*base).is_none())
                {
                    return Err(CandidateError::Input("unsupported IUPAC symbol"));
                }
                let value = current
                    .as_mut()
                    .ok_or(CandidateError::Input("sequence before header"))?;
                value.1 = value
                    .1
                    .checked_add(bytes.len() as u64)
                    .ok_or(CandidateError::Arithmetic("contig length"))?;
            }
            FastaEvent::End => {
                let (contig, bases) = current
                    .take()
                    .ok_or(CandidateError::Input("header without sequence"))?;
                plans.push(ContigPlan { contig, bases });
            }
        }
        Ok(())
    })?;
    let hashing = reader.into_inner();
    let identity = Identity {
        bytes: hashing.bytes,
        sha256: format!("{:x}", hashing.hasher.finalize()),
    };
    let after = fs::metadata(path)
        .map_err(|_| ReferenceCandidateError::new("io", "reference metadata failed"))?;
    if identity.bytes != metadata.len() || after.len() != identity.bytes || after.nlink() != 1 {
        return Err(ReferenceCandidateError::new(
            "source_identity",
            "reference source changed while reading",
        ));
    }
    Ok((plans, identity))
}

struct HashingRead<R> {
    inner: R,
    hasher: Sha256,
    bytes: u64,
}

impl<R> HashingRead<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes: 0,
        }
    }
}

impl<R: Read> Read for HashingRead<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buffer)?;
        self.hasher.update(&buffer[..count]);
        self.bytes = self
            .bytes
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("reference byte count overflow"))?;
        Ok(count)
    }
}

fn stream_fasta<F>(path: &Path, visitor: F) -> Result<Identity, ReferenceCandidateError>
where
    F: for<'a> FnMut(FastaEvent<'a>) -> Result<(), CandidateError>,
{
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        ReferenceCandidateError::new("source_identity", "reference source is unavailable")
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1 {
        return Err(ReferenceCandidateError::new(
            "source_identity",
            "reference source identity is invalid",
        ));
    }
    let file = File::open(path)
        .map_err(|_| ReferenceCandidateError::new("io", "reference source open failed"))?;
    let mut reader = BufReader::with_capacity(128 * 1024, HashingRead::new(file));
    parse_fasta(&mut reader, visitor)?;
    let hashing = reader.into_inner();
    let identity = Identity {
        bytes: hashing.bytes,
        sha256: format!("{:x}", hashing.hasher.finalize()),
    };
    let after = fs::metadata(path)
        .map_err(|_| ReferenceCandidateError::new("io", "reference metadata failed"))?;
    if identity.bytes != metadata.len() || after.len() != identity.bytes || after.nlink() != 1 {
        return Err(ReferenceCandidateError::new(
            "source_identity",
            "reference source changed while generating",
        ));
    }
    Ok(identity)
}

fn parse_fasta<R, F>(reader: &mut R, mut visitor: F) -> Result<(), ReferenceCandidateError>
where
    R: BufRead,
    F: for<'a> FnMut(FastaEvent<'a>) -> Result<(), CandidateError>,
{
    let mut line = Vec::new();
    let mut active = false;
    loop {
        line.clear();
        let count = reader
            .read_until(b'\n', &mut line)
            .map_err(|_| ReferenceCandidateError::new("io", "reference source read failed"))?;
        if count == 0 {
            break;
        }
        if line.len() > FASTA_LINE_MAX {
            return Err(ReferenceCandidateError::new(
                "resource",
                "reference line exceeds limit",
            ));
        }
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if line.starts_with(b">") {
            if active {
                visitor(FastaEvent::End)?;
            }
            let name = std::str::from_utf8(&line[1..]).map_err(|_| {
                ReferenceCandidateError::new("oracle", "reference header is invalid")
            })?;
            if name.contains(char::is_whitespace) || name.is_empty() {
                return Err(ReferenceCandidateError::new(
                    "oracle",
                    "reference header is invalid",
                ));
            }
            let contig = Grch38Contig::from_str(name).map_err(|_| {
                ReferenceCandidateError::new("oracle", "reference contig is unsupported")
            })?;
            visitor(FastaEvent::Start(contig))?;
            active = true;
        } else {
            if line.is_empty() {
                return Err(ReferenceCandidateError::new(
                    "oracle",
                    "reference contains an empty line",
                ));
            }
            visitor(FastaEvent::Bases(&line))?;
        }
    }
    if !active {
        return Err(ReferenceCandidateError::new("oracle", "reference is empty"));
    }
    visitor(FastaEvent::End)?;
    Ok(())
}

fn member_outcomes(root: &Path) -> Result<Vec<OutcomeMember>, ReferenceCandidateError> {
    CandidateCodec::ALL
        .into_iter()
        .map(|codec| {
            let identity =
                hash_regular_file(&root.join(codec.filename()), u64::MAX, "candidate_member")?;
            Ok(OutcomeMember {
                codec,
                bytes: identity.bytes,
                sha256: identity.sha256,
            })
        })
        .collect()
}

fn hash_regular_file(
    path: &Path,
    max: u64,
    code: &'static str,
) -> Result<Identity, ReferenceCandidateError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| ReferenceCandidateError::new(code, "required file is unavailable"))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.len() > max
    {
        return Err(ReferenceCandidateError::new(
            code,
            "required file identity is invalid",
        ));
    }
    let (bytes, sha256) =
        sha256_file(path).map_err(|_| ReferenceCandidateError::new("io", "file hashing failed"))?;
    let after = fs::metadata(path)
        .map_err(|_| ReferenceCandidateError::new("io", "file metadata failed"))?;
    if bytes != metadata.len() || after.len() != bytes || after.nlink() != 1 {
        return Err(ReferenceCandidateError::new(
            code,
            "file changed while reading",
        ));
    }
    Ok(Identity { bytes, sha256 })
}

fn read_bounded_regular(
    path: &Path,
    max: u64,
    code: &'static str,
) -> Result<Vec<u8>, ReferenceCandidateError> {
    let identity = hash_regular_file(path, max, code)?;
    let capacity = usize::try_from(identity.bytes)
        .map_err(|_| ReferenceCandidateError::new("resource", "file is not addressable"))?;
    let mut bytes = Vec::with_capacity(capacity);
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|_| ReferenceCandidateError::new("io", "file read failed"))?;
    if bytes.len() != capacity {
        return Err(ReferenceCandidateError::new(
            code,
            "file changed while reading",
        ));
    }
    Ok(bytes)
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), ReferenceCandidateError> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|_| ReferenceCandidateError::new("io", "candidate write failed"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| ReferenceCandidateError::new("io", "candidate write failed"))
}

fn sync_dir(path: &Path) -> Result<(), ReferenceCandidateError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|_| ReferenceCandidateError::new("io", "directory sync failed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_candidate_literal_decoder_rejects_changed_expectation() {
        let changed = b"{\"id\":\"C1\",\"contig\":\"chr3\",\"start_1based\":1,\"bases\":\"A\",\"sha256\":\"00\"}\n";
        assert_eq!(
            decode_literal_contexts(changed)
                .expect_err("changed literal")
                .code(),
            "oracle"
        );
    }

    #[test]
    fn reference_candidate_registry_rejects_unregistered_profile() {
        let manifest = CandidateManifest {
            schema: CANDIDATE_SCHEMA.into(),
            profile: "other".into(),
            source: Identity {
                bytes: 1,
                sha256: "0".repeat(64),
            },
            corpus: CorpusIdentity {
                schema: "pangopup-compat-v1".into(),
                profile: "other".into(),
                manifest_bytes: 1,
                manifest_sha256: "0".repeat(64),
                cases_bytes: 1,
                cases_sha256: "0".repeat(64),
            },
            container: ContainerIdentity {
                schema: CONTAINER_SCHEMA.into(),
                page_bytes: 4096,
                contigs: vec![3],
            },
            members: vec![],
        };
        assert!(validate_manifest_shape(&manifest).is_err());
    }

    #[test]
    fn reference_candidate_real_registry_requires_exact_contigs() {
        let manifest = CandidateManifest {
            schema: CANDIDATE_SCHEMA.into(),
            profile: REAL_PROFILE.into(),
            source: Identity {
                bytes: REAL_SOURCE_BYTES,
                sha256: REAL_SOURCE_SHA256.into(),
            },
            corpus: CorpusIdentity {
                schema: "pangopup-compat-v1".into(),
                profile: REAL_CORPUS_PROFILE.into(),
                manifest_bytes: REAL_MANIFEST_BYTES,
                manifest_sha256: REAL_MANIFEST_SHA256.into(),
                cases_bytes: REAL_CASES_BYTES,
                cases_sha256: REAL_CASES_SHA256.into(),
            },
            container: ContainerIdentity {
                schema: CONTAINER_SCHEMA.into(),
                page_bytes: 4096,
                contigs: vec![3, 10, 12, 13, 17, 24],
            },
            members: CandidateCodec::ALL
                .into_iter()
                .map(|codec| CandidateMember {
                    codec,
                    filename: codec.filename().into(),
                    bytes: 1,
                    sha256: "0".repeat(64),
                })
                .collect(),
        };
        assert_eq!(
            validate_manifest_shape(&manifest)
                .expect_err("wrong real contigs")
                .code(),
            "candidate_set"
        );
    }

    #[test]
    fn reference_candidate_publication_rolls_back_after_parent_sync_failure() {
        let rolled_back = std::cell::Cell::new(false);
        let error = publish_candidate_with(
            || Ok(()),
            || Err(ReferenceCandidateError::new("io", "injected sync")),
            || {
                rolled_back.set(true);
                Ok(())
            },
        )
        .expect_err("parent sync failure");
        assert_eq!(error.code(), "io");
        assert!(rolled_back.get());
    }

    #[test]
    fn reference_candidate_generation_authenticates_second_read() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let fixture = repository.join("tests/fixtures/reference-candidates-mini");
        let source = fixture.join("source.fa");
        let corpus = fixture.join("corpus");
        let root = std::env::temp_dir().join(format!(
            "pangopup-generation-auth-{}-{}",
            std::process::id(),
            STAGING_SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("scratch root");
        let (plans, mut identity) = scan_fasta(&source).expect("scan source");
        identity.sha256 = "0".repeat(64);
        let manifest_identity = hash_regular_file(
            &corpus.join("manifest.json"),
            MANIFEST_MAX,
            "corpus_identity",
        )
        .expect("manifest");
        let cases_identity =
            hash_regular_file(&corpus.join("cases.jsonl"), CASES_MAX, "corpus_identity")
                .expect("cases");
        let corpus_identity = CorpusIdentity {
            schema: "pangopup-compat-v1".into(),
            profile: MINI_PROFILE.into(),
            manifest_bytes: manifest_identity.bytes,
            manifest_sha256: manifest_identity.sha256,
            cases_bytes: cases_identity.bytes,
            cases_sha256: cases_identity.sha256,
        };
        let contexts = miniature_contexts(&corpus).expect("contexts");
        let output = root.join("output");
        let error = prepare_with_plans(Preparation {
            source: &source,
            output: &output,
            profile: MINI_PROFILE,
            source_identity: identity,
            corpus_identity,
            contexts: &contexts,
            plans: &plans,
            publish: true,
        })
        .expect_err("second read identity mismatch");
        assert_eq!(error.code(), "source_identity");
        assert!(!output.exists());
        fs::remove_dir_all(root).expect("remove scratch");
    }

    #[test]
    fn reference_candidate_changed_source_cannot_define_its_own_oracle() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let fixture = repository.join("tests/fixtures/reference-candidates-mini");
        let corpus = fixture.join("corpus");
        let root = std::env::temp_dir().join(format!(
            "pangopup-source-oracle-{}-{}",
            std::process::id(),
            STAGING_SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("scratch root");
        let source = root.join("changed.fa");
        let mut bytes = fs::read(fixture.join("source.fa")).expect("read source");
        let first_base = bytes
            .iter()
            .position(|byte| *byte == b'a')
            .expect("lowercase source base");
        bytes[first_base] = b'c';
        fs::write(&source, bytes).expect("write changed source");
        let source_identity =
            hash_regular_file(&source, u64::MAX, "source_identity").expect("source identity");
        let manifest_identity = hash_regular_file(
            &corpus.join("manifest.json"),
            MANIFEST_MAX,
            "corpus_identity",
        )
        .expect("manifest");
        let cases_identity =
            hash_regular_file(&corpus.join("cases.jsonl"), CASES_MAX, "corpus_identity")
                .expect("cases");
        let corpus_identity = CorpusIdentity {
            schema: "pangopup-compat-v1".into(),
            profile: MINI_PROFILE.into(),
            manifest_bytes: manifest_identity.bytes,
            manifest_sha256: manifest_identity.sha256,
            cases_bytes: cases_identity.bytes,
            cases_sha256: cases_identity.sha256,
        };
        let contexts = miniature_contexts(&corpus).expect("independent contexts");
        let output = root.join("output");
        let error = prepare_authenticated(
            &source,
            &output,
            MINI_PROFILE,
            source_identity,
            corpus_identity,
            &contexts,
            true,
        )
        .expect_err("independent oracle must reject changed source");
        assert_eq!(error.code(), "oracle");
        assert!(!output.exists());
        fs::remove_dir_all(root).expect("remove scratch");
    }

    #[test]
    fn reference_candidate_miniature_preparation_is_byte_deterministic() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let fixture = repository.join("tests/fixtures/reference-candidates-mini");
        let source = fixture.join("source.fa");
        let corpus = fixture.join("corpus");
        let root = std::env::temp_dir().join(format!(
            "pangopup-mini-determinism-{}-{}",
            std::process::id(),
            STAGING_SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("scratch root");
        let contexts = miniature_contexts(&corpus).expect("literal contexts");
        let source_identity =
            hash_regular_file(&source, u64::MAX, "source_identity").expect("source identity");
        let manifest_identity = hash_regular_file(
            &corpus.join("manifest.json"),
            MANIFEST_MAX,
            "corpus_identity",
        )
        .expect("manifest identity");
        let cases_identity =
            hash_regular_file(&corpus.join("cases.jsonl"), CASES_MAX, "corpus_identity")
                .expect("cases identity");
        let corpus_identity = CorpusIdentity {
            schema: "pangopup-compat-v1".into(),
            profile: MINI_PROFILE.into(),
            manifest_bytes: manifest_identity.bytes,
            manifest_sha256: manifest_identity.sha256,
            cases_bytes: cases_identity.bytes,
            cases_sha256: cases_identity.sha256,
        };
        for name in ["one", "two"] {
            prepare_authenticated(
                &source,
                &root.join(name),
                MINI_PROFILE,
                source_identity.clone(),
                corpus_identity.clone(),
                &contexts,
                true,
            )
            .expect("prepare candidate set");
        }
        for name in [
            "manifest.json",
            "ascii8.pgr",
            "iupac4.pgr",
            "acgt2-rle-v1.pgr",
        ] {
            assert_eq!(
                fs::read(root.join("one").join(name)).expect("first bytes"),
                fs::read(root.join("two").join(name)).expect("second bytes")
            );
            assert_eq!(
                fs::read(root.join("one").join(name)).expect("generated bytes"),
                fs::read(fixture.join("candidates").join(name)).expect("checked bytes")
            );
        }
        fs::remove_dir_all(root).expect("remove scratch");
    }
}
