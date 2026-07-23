//! Strict bounded metadata used to prepare the public SNV data release.

#[cfg(any(test, feature = "test-read-audit"))]
use super::release_upload_linux::PayloadTestFaults;
use super::release_upload_linux::{
    LEASE_CLEANUP_DEADLINE, LeasedPayload, PayloadConfig, PayloadOperation, PendingUploadSignals,
    UploadSignals,
};
use super::{
    AssetError, AssetErrorKind, MAX_JSON_BYTES, MAX_NOTICE_BYTES, MAX_SAFE_JSON_U64,
    TransportInspection, VerifiedTransport, create_stage, ensure_output_absent, finish_staged,
    inspect_transport, open_regular, parse_transport_manifest, publish_stage,
    reject_duplicate_json, sha256, sync_directory, transport_inspection_from_verified,
    validate_bundle_metadata, write_synced,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    env,
    ffi::{CString, OsStr},
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, FromRawFd, RawFd},
        unix::{ffi::OsStrExt, fs::PermissionsExt, process::CommandExt},
    },
    path::{Component, Path},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const MAX_RECEIPT_BYTES: u64 = 64 * 1024;
const PROOF_SCHEMA: &str = "pangopup.proof-receipt.v1";
const PROFILE_SCHEMA: &str = "pangopup.release-profile.v1";
const PRODUCTION_RECEIPT: &[u8] =
    include_bytes!("../../../release-profiles/proofs/snv-grch38-v1.json");
const PRODUCTION_PROFILE: &[u8] = include_bytes!("../../../release-profiles/snv-grch38-v1.json");
const PRODUCTION_RECEIPT_SHA256: &str =
    "sha256:9ddae771d200fe73bda5f31f5a04a52227b77c5d3f225dc7ee52294cd9aea475";
const PRODUCTION_PROFILE_SHA256: &str =
    "sha256:63f3842ea6cb40ebc0a2b6ca23fba4f35d53f829d96c33f597a2c5bcac238ca6";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofReceipt {
    pub schema: String,
    pub source: ProofSource,
    pub reference: ProofReference,
    pub bundle: ProofBundle,
    pub transport: ProofTransport,
    pub tool: ProofTool,
    pub verify: ProofVerify,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofSource {
    pub archive_name: String,
    pub archive_size: u64,
    pub archive_md5: String,
    pub observed_member_count: u64,
    pub observed_members_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofReference {
    pub assembly_accession: String,
    pub input_size: u64,
    pub input_sha256: String,
    pub sequence_set_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofBundle {
    pub bundle_id: String,
    pub builder_version: String,
    pub builder_source_sha256: String,
    pub manifest: ProofIdentity,
    pub members: Vec<ProofMember>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofIdentity {
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofMember {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofTransport {
    pub transport_id: String,
    pub manifest: ProofIdentity,
    pub compressed: ProofIdentity,
    pub parts: Vec<ProofPart>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofPart {
    pub ordinal: u16,
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofTool {
    pub implementation_commit: String,
    pub encoder_crate: String,
    pub libzstd_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofVerify {
    pub bundle: Vec<String>,
    pub transport: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseProfile {
    pub schema: String,
    pub profile: String,
    pub repository: String,
    pub release: ProfileRelease,
    pub source: ProfileSource,
    pub reference_compatibility: ProfileReference,
    pub bundle: ProfileBundle,
    pub transport: ProfileTransport,
    pub proof: ProfileProof,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileRelease {
    pub tag: String,
    pub title: String,
    pub target_commit: String,
    pub page_url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileSource {
    pub title: String,
    pub creators: Vec<String>,
    pub doi: String,
    pub license: String,
    pub archive: ProfileArchive,
    pub assembly: String,
    pub masked: bool,
    pub window: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileArchive {
    pub name: String,
    pub size: u64,
    pub md5: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileReference {
    pub assembly: String,
    pub assembly_accession: String,
    pub input_size: u64,
    pub input_sha256: String,
    pub sequence_set_sha256: String,
    pub ordinary_ref_mismatches: u64,
    pub preserved_ref_n_loci: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileBundle {
    pub schema: String,
    pub index_format: String,
    pub bundle_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileTransport {
    pub schema: String,
    pub transport_id: String,
    pub members: Vec<ProfileMember>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileMember {
    pub logical_path: String,
    pub asset_name: String,
    pub size: u64,
    pub sha256: String,
    pub url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileProof {
    pub schema: String,
    pub asset_name: String,
    pub size: u64,
    pub sha256: String,
}

/// Injectable release contract used only by bounded miniature tests.
#[doc(hidden)]
#[derive(Clone, Copy)]
#[cfg(any(test, feature = "test-read-audit"))]
pub struct ReleasePreparationContract<'a> {
    pub receipt_bytes: &'a [u8],
    pub receipt_sha256: &'a str,
    pub profile_bytes: &'a [u8],
}

#[derive(Clone, Copy)]
struct PreparationContract<'a> {
    receipt_bytes: &'a [u8],
    receipt_sha256: &'a str,
    profile_bytes: &'a [u8],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PrepareReleaseOutcome {
    pub status: &'static str,
    pub repository: String,
    pub tag: String,
    pub transport_id: String,
    pub bundle_id: String,
    pub asset_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UploadAssetOutcome {
    pub status: &'static str,
    pub asset: String,
    pub size: u64,
    pub digest: Option<String>,
}

#[derive(Clone, Copy)]
struct GhExecutableContract<'a> {
    size: u64,
    sha256: &'a str,
}

#[doc(hidden)]
#[derive(Clone, Copy)]
#[cfg(any(test, feature = "test-read-audit"))]
pub struct ReleaseUploadTestContract<'a> {
    pub receipt_bytes: &'a [u8],
    pub receipt_sha256: &'a str,
    pub profile_bytes: &'a [u8],
    pub gh_size: u64,
    pub gh_sha256: &'a str,
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-read-audit"))]
pub type BeforeChildSpawnHook<'a> = Option<&'a dyn Fn(&[PayloadOperation], i64)>;

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(test, feature = "test-read-audit"))]
pub enum ChildPreExecBarrierPhase {
    BeforeParentDeathSignal,
    AfterParentDeathSignal,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
#[cfg(any(test, feature = "test-read-audit"))]
pub struct ReleaseUploadTestBarrier {
    pub ready_fd: RawFd,
    pub release_fd: RawFd,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
#[cfg(any(test, feature = "test-read-audit"))]
pub struct ReleaseUploadChildBarrier {
    pub phase: ChildPreExecBarrierPhase,
    pub ready_fd: RawFd,
    pub release_fd: RawFd,
}

#[doc(hidden)]
#[derive(Clone, Copy, Default)]
#[cfg(any(test, feature = "test-read-audit"))]
pub struct ReleaseUploadTestHooks<'a> {
    pub after_gh_validation: Option<&'a dyn Fn()>,
    pub after_asset_open: Option<&'a dyn Fn()>,
    pub after_contract_validation: Option<&'a dyn Fn()>,
    pub before_child_spawn: BeforeChildSpawnHook<'a>,
    pub child_deadline: Option<Duration>,
    pub payload_faults: PayloadTestFaults,
    pub supervision_barrier: Option<ReleaseUploadTestBarrier>,
    pub child_pre_exec_barrier: Option<ReleaseUploadChildBarrier>,
}

type InternalBeforeChildSpawnHook<'a> = Option<&'a dyn Fn(&[PayloadOperation], i64)>;
type InternalChildBoundaryHook<'a> = (&'a dyn Fn(&[PayloadOperation], i64), i64);

#[derive(Clone, Copy)]
struct UploadHooks<'a> {
    after_gh_validation: Option<&'a dyn Fn()>,
    after_asset_open: Option<&'a dyn Fn()>,
    after_contract_validation: Option<&'a dyn Fn()>,
    before_child_spawn: InternalBeforeChildSpawnHook<'a>,
    child_deadline: Duration,
    payload_config: PayloadConfig,
    supervision_barrier: Option<InternalTestBarrier>,
    child_pre_exec_barrier: Option<InternalChildBarrier>,
}

#[derive(Clone, Copy)]
struct InternalTestBarrier {
    ready_fd: RawFd,
    release_fd: RawFd,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum InternalChildBarrierPhase {
    BeforeParentDeathSignal,
    AfterParentDeathSignal,
}

#[derive(Clone, Copy)]
struct InternalChildBarrier {
    phase: InternalChildBarrierPhase,
    ready_fd: RawFd,
    release_fd: RawFd,
}

struct ChildRunConfig<'a> {
    release_id: u64,
    asset: &'a ReviewedAsset,
    deadline: Duration,
    before_child_spawn: Option<InternalChildBoundaryHook<'a>>,
    supervision_barrier: Option<InternalTestBarrier>,
    child_pre_exec_barrier: Option<InternalChildBarrier>,
}

impl Default for UploadHooks<'_> {
    fn default() -> Self {
        Self {
            after_gh_validation: None,
            after_asset_open: None,
            after_contract_validation: None,
            before_child_spawn: None,
            child_deadline: Duration::from_secs(21_600),
            payload_config: PayloadConfig::default(),
            supervision_barrier: None,
            child_pre_exec_barrier: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AssetSource {
    Transport,
    Prepared,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReviewedAsset {
    name: String,
    size: u64,
    sha256: String,
    source: AssetSource,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UploadResponse {
    name: String,
    size: u64,
    state: String,
    digest: Option<String>,
}

/// Parse one closed, canonical proof receipt without accepting extensions.
pub fn parse_proof_receipt(bytes: &[u8]) -> Result<ProofReceipt, AssetError> {
    let Some(prefix) = bytes.strip_suffix(b"\n") else {
        return Err(release_error("proof receipt must end with exactly one LF"));
    };
    parse_canonical(prefix, "proof receipt JSON prefix").and_then(|receipt: ProofReceipt| {
        validate_receipt(&receipt)?;
        Ok(receipt)
    })
}

/// Parse one closed, canonical release profile without accepting extensions.
pub fn parse_release_profile(bytes: &[u8]) -> Result<ReleaseProfile, AssetError> {
    parse_canonical(bytes, "release profile").and_then(|profile: ReleaseProfile| {
        validate_profile(&profile)?;
        Ok(profile)
    })
}

fn parse_canonical<T>(bytes: &[u8], label: &str) -> Result<T, AssetError>
where
    T: for<'de> Deserialize<'de>,
{
    reject_duplicate_json(bytes)
        .map_err(|_| release_error(format!("{label} contains invalid or duplicate JSON")))?;
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| release_error(format!("{label} is not valid JSON")))?;
    let canonical = serde_jcs::to_vec(&value)
        .map_err(|_| release_error(format!("cannot canonicalize {label}")))?;
    if canonical != bytes {
        return Err(release_error(format!(
            "{label} is not canonical RFC 8785 JSON"
        )));
    }
    let parsed: T = serde_json::from_value(value)
        .map_err(|_| release_error(format!("{label} is not closed v1 JSON")))?;
    Ok(parsed)
}

fn validate_receipt(receipt: &ProofReceipt) -> Result<(), AssetError> {
    if receipt.schema != PROOF_SCHEMA
        || receipt.bundle.members.len() != 2
        || receipt.bundle.members[0].path != "NOTICE"
        || receipt.bundle.members[1].path != "scores.pgi"
        || receipt.transport.parts.is_empty()
        || receipt.verify.bundle.is_empty()
        || receipt.verify.transport.is_empty()
    {
        return Err(release_error("invalid closed proof-receipt v1 shape"));
    }
    let integers = [
        receipt.source.archive_size,
        receipt.source.observed_member_count,
        receipt.reference.input_size,
        receipt.bundle.manifest.size,
        receipt.bundle.members[0].size,
        receipt.bundle.members[1].size,
        receipt.transport.manifest.size,
        receipt.transport.compressed.size,
    ];
    if integers.into_iter().any(|value| value > MAX_SAFE_JSON_U64) {
        return Err(release_error(
            "proof receipt integer exceeds JSON safe range",
        ));
    }
    let hashes = [
        &receipt.source.observed_members_sha256,
        &receipt.reference.input_sha256,
        &receipt.reference.sequence_set_sha256,
        &receipt.bundle.bundle_id,
        &receipt.bundle.builder_source_sha256,
        &receipt.bundle.manifest.sha256,
        &receipt.bundle.members[0].sha256,
        &receipt.bundle.members[1].sha256,
        &receipt.transport.transport_id,
        &receipt.transport.manifest.sha256,
        &receipt.transport.compressed.sha256,
    ];
    if hashes.into_iter().any(|value| !valid_identity(value))
        || !valid_md5(&receipt.source.archive_md5)
        || !valid_commit(&receipt.tool.implementation_commit)
    {
        return Err(release_error("proof receipt identity spelling is invalid"));
    }
    let mut total = 0_u64;
    for (position, part) in receipt.transport.parts.iter().enumerate() {
        let ordinal = u16::try_from(position)
            .map_err(|_| release_error("proof receipt has too many parts"))?;
        if part.ordinal != ordinal
            || part.path != format!("payload.pgi.zst.part{ordinal:04}")
            || part.size == 0
            || part.size > MAX_SAFE_JSON_U64
            || !valid_identity(&part.sha256)
        {
            return Err(release_error("proof receipt part descriptor is invalid"));
        }
        total = total
            .checked_add(part.size)
            .ok_or_else(|| release_error("proof receipt part size overflow"))?;
    }
    if total != receipt.transport.compressed.size {
        return Err(release_error(
            "proof receipt part sizes do not match compressed size",
        ));
    }
    Ok(())
}

fn validate_profile(profile: &ReleaseProfile) -> Result<(), AssetError> {
    let expected_page_url = format!(
        "https://github.com/{}/releases/tag/{}",
        profile.repository, profile.release.tag
    );
    if profile.schema != PROFILE_SCHEMA
        || profile.profile != profile.release.tag
        || profile.release.page_url != expected_page_url
        || profile.proof.schema != PROOF_SCHEMA
        || profile.proof.asset_name != "proof-receipt.json"
        || profile.proof.size == 0
        || profile.transport.schema != "pangopup.snv-transport.v1"
        || profile.bundle.schema != "pangopup.bundle.v1"
        || profile.bundle.index_format != "pangopup.fixed11.v1"
        || profile.transport.members.len() < 4
        || profile.source.creators.len() != 2
        || !valid_commit(&profile.release.target_commit)
    {
        return Err(release_error("invalid closed release-profile v1 shape"));
    }
    let mut expected_url_prefix = String::from("https://github.com/");
    expected_url_prefix.push_str(&profile.repository);
    expected_url_prefix.push_str("/releases/download/");
    expected_url_prefix.push_str(&profile.release.tag);
    expected_url_prefix.push('/');
    for (position, member) in profile.transport.members.iter().enumerate() {
        let expected_name = match position {
            0 => "transport.json".to_owned(),
            1 => "bundle-manifest.json".to_owned(),
            2 => "NOTICE".to_owned(),
            part => format!("payload.pgi.zst.part{:04}", part - 3),
        };
        if member.logical_path != member.asset_name
            || member.asset_name != expected_name
            || member.size == 0
            || member.size > MAX_SAFE_JSON_U64
            || !valid_identity(&member.sha256)
            || member.url != format!("{expected_url_prefix}{}", member.asset_name)
        {
            return Err(release_error("release profile member is invalid"));
        }
    }
    let values = [
        profile.source.archive.size,
        profile.reference_compatibility.input_size,
        profile.reference_compatibility.ordinary_ref_mismatches,
        profile.reference_compatibility.preserved_ref_n_loci,
        profile.proof.size,
    ];
    if values.into_iter().any(|value| value > MAX_SAFE_JSON_U64)
        || !valid_identity(&profile.transport.transport_id)
        || !valid_identity(&profile.bundle.bundle_id)
        || !valid_identity(&profile.proof.sha256)
        || !valid_identity(&profile.reference_compatibility.input_sha256)
        || !valid_identity(&profile.reference_compatibility.sequence_set_sha256)
        || !valid_md5(&profile.source.archive.md5)
    {
        return Err(release_error(
            "release profile identity or integer is invalid",
        ));
    }
    Ok(())
}

fn validate_production_contract() -> Result<(ProofReceipt, ReleaseProfile), AssetError> {
    validate_production_contract_bytes(PRODUCTION_RECEIPT, PRODUCTION_PROFILE)
}

fn validate_production_contract_bytes(
    receipt_bytes: &[u8],
    profile_bytes: &[u8],
) -> Result<(ProofReceipt, ReleaseProfile), AssetError> {
    if receipt_bytes.len() != 2_194
        || sha256(receipt_bytes) != PRODUCTION_RECEIPT_SHA256
        || profile_bytes.len() != 2_821
        || sha256(profile_bytes) != PRODUCTION_PROFILE_SHA256
    {
        return Err(release_error(
            "production release contract identity mismatch",
        ));
    }
    let receipt = parse_proof_receipt(receipt_bytes)?;
    let profile = parse_release_profile(profile_bytes)?;
    let expected_parts = [
        (
            0,
            "payload.pgi.zst.part0000",
            1_000_000_000,
            "sha256:07c1f9a2e33e1a5bd929500eefd00b84764c82d56e3f573c35d380419e4ed42a",
        ),
        (
            1,
            "payload.pgi.zst.part0001",
            931_687_706,
            "sha256:87580144fd828676d7adb269059cf2b425b342fe5ccee442888e0b93994adc74",
        ),
    ];
    let parts_match = receipt.transport.parts.len() == expected_parts.len()
        && receipt.transport.parts.iter().zip(expected_parts).all(
            |(actual, (ordinal, path, size, digest))| {
                actual.ordinal == ordinal
                    && actual.path == path
                    && actual.size == size
                    && actual.sha256 == digest
            },
        );
    if receipt.schema != PROOF_SCHEMA
        || receipt.source.archive_name != "Pangolin_hg38_snvs_masked.zip"
        || receipt.source.archive_size != 12_988_141_317
        || receipt.source.archive_md5 != "md5:679ef0b50e511b6102b4b88fbf811108"
        || receipt.source.observed_member_count != 19_913
        || receipt.source.observed_members_sha256
            != "sha256:0e40ee8e0527210cb64c26a6637117aea7d41d696e7bd95f3bb9545ee16782f6"
        || receipt.reference.assembly_accession != "GCF_000001405.40"
        || receipt.reference.input_size != 972_898_531
        || receipt.reference.input_sha256
            != "sha256:11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3"
        || receipt.reference.sequence_set_sha256
            != "sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4"
        || receipt.bundle.bundle_id
            != "sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3"
        || receipt.bundle.builder_version != "0.1.0"
        || receipt.bundle.builder_source_sha256
            != "sha256:10fd5d7715a611f9b7f20040887391502535ac7860bc6a1eda2bfdda79682b64"
        || receipt.bundle.manifest.size != 3_589
        || receipt.bundle.manifest.sha256 != receipt.bundle.bundle_id
        || receipt.bundle.members[0].path != "NOTICE"
        || receipt.bundle.members[0].size != 1_709
        || receipt.bundle.members[0].sha256
            != "sha256:9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7"
        || receipt.bundle.members[1].path != "scores.pgi"
        || receipt.bundle.members[1].size != 15_033_158_255
        || receipt.bundle.members[1].sha256
            != "sha256:6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27"
        || receipt.transport.transport_id
            != "sha256:3a2f4901b8f3dece302640d0257cc98aa50010a45fe61c5ef77c64a62f4660aa"
        || receipt.transport.manifest.size != 1_266
        || receipt.transport.manifest.sha256
            != "sha256:f9b7501087226fb35cbfa66fa9b903cc21eb8bbbacb067363b9eeef487ee9e9a"
        || receipt.transport.compressed.size != 1_931_687_706
        || receipt.transport.compressed.sha256
            != "sha256:8b00b8b39cb07d0b5443e506bde097406c0533e50b5e1056ca026ea92d28134d"
        || !parts_match
        || receipt.tool.implementation_commit != "4161679b362805b706a5bfd2a8b24a25df5e23fb"
        || receipt.tool.encoder_crate != "zstd/0.13.3"
        || receipt.tool.libzstd_version != "1.5.7"
        || receipt.verify.bundle
            != [
                "pangopup-build",
                "verify",
                "bundles/sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3",
            ]
        || receipt.verify.transport
            != [
                "pangopup-build",
                "transport",
                "verify",
                "--transport",
                "transports/sha256:3a2f4901b8f3dece302640d0257cc98aa50010a45fe61c5ef77c64a62f4660aa",
            ]
    {
        return Err(release_error("production proof receipt values mismatch"));
    }

    if profile.schema != PROFILE_SCHEMA
        || profile.profile != "snv-grch38-v1"
        || profile.repository != "genomoncology/pangopup"
        || profile.release.tag != "snv-grch38-v1"
        || profile.release.title != "Pangopup GRCh38 SNV scores v1"
        || profile.release.target_commit != "851f57d6ffb75a2c099a3d1263b1e94b60aad0e8"
        || profile.release.page_url
            != "https://github.com/genomoncology/pangopup/releases/tag/snv-grch38-v1"
        || profile.source.title != "Pangolin precomputed scores"
        || profile.source.creators != ["Nils Wagner", "Aleksandr Neverov"]
        || profile.source.doi != "10.5281/zenodo.15649338"
        || profile.source.license != "CC-BY-4.0"
        || profile.source.archive.name != receipt.source.archive_name
        || profile.source.archive.size != receipt.source.archive_size
        || profile.source.archive.md5 != receipt.source.archive_md5
        || profile.source.assembly != "GRCh38"
        || !profile.source.masked
        || profile.source.window != 50
        || profile.reference_compatibility.assembly != "GRCh38.p14"
        || profile.reference_compatibility.assembly_accession
            != receipt.reference.assembly_accession
        || profile.reference_compatibility.input_size != receipt.reference.input_size
        || profile.reference_compatibility.input_sha256 != receipt.reference.input_sha256
        || profile.reference_compatibility.sequence_set_sha256
            != receipt.reference.sequence_set_sha256
        || profile.reference_compatibility.ordinary_ref_mismatches != 0
        || profile.reference_compatibility.preserved_ref_n_loci != 30
        || profile.bundle.schema != "pangopup.bundle.v1"
        || profile.bundle.index_format != "pangopup.fixed11.v1"
        || profile.bundle.bundle_id != receipt.bundle.bundle_id
        || profile.transport.schema != "pangopup.snv-transport.v1"
        || profile.transport.transport_id != receipt.transport.transport_id
        || profile.proof.schema != PROOF_SCHEMA
        || profile.proof.asset_name != "proof-receipt.json"
        || profile.proof.size != 2_194
        || profile.proof.sha256 != PRODUCTION_RECEIPT_SHA256
    {
        return Err(release_error("production release profile values mismatch"));
    }
    let expected_profile_members = [
        (
            "transport.json",
            1_266,
            receipt.transport.manifest.sha256.as_str(),
        ),
        (
            "bundle-manifest.json",
            3_589,
            receipt.bundle.bundle_id.as_str(),
        ),
        ("NOTICE", 1_709, receipt.bundle.members[0].sha256.as_str()),
        (
            expected_parts[0].1,
            expected_parts[0].2,
            expected_parts[0].3,
        ),
        (
            expected_parts[1].1,
            expected_parts[1].2,
            expected_parts[1].3,
        ),
    ];
    if profile.transport.members.len() != expected_profile_members.len()
        || profile
            .transport
            .members
            .iter()
            .zip(expected_profile_members)
            .any(|(member, (name, size, digest))| {
                member.logical_path != name
                    || member.asset_name != name
                    || member.size != size
                    || member.sha256 != digest
                    || member.url
                        != format!(
                            "https://github.com/genomoncology/pangopup/releases/download/snv-grch38-v1/{name}"
                        )
            })
    {
        return Err(release_error("production release profile members mismatch"));
    }
    Ok((receipt, profile))
}

pub fn prepare_release(
    transport: &Path,
    receipt: &Path,
    output: &Path,
) -> Result<PrepareReleaseOutcome, AssetError> {
    validate_production_contract()?;
    prepare_release_contract(
        transport,
        receipt,
        output,
        PreparationContract {
            receipt_bytes: PRODUCTION_RECEIPT,
            receipt_sha256: PRODUCTION_RECEIPT_SHA256,
            profile_bytes: PRODUCTION_PROFILE,
        },
    )
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-read-audit"))]
pub fn prepare_release_with_contract(
    transport_path: &Path,
    receipt_path: &Path,
    output: &Path,
    contract: ReleasePreparationContract<'_>,
) -> Result<PrepareReleaseOutcome, AssetError> {
    prepare_release_contract(
        transport_path,
        receipt_path,
        output,
        PreparationContract {
            receipt_bytes: contract.receipt_bytes,
            receipt_sha256: contract.receipt_sha256,
            profile_bytes: contract.profile_bytes,
        },
    )
}

fn prepare_release_contract(
    transport_path: &Path,
    receipt_path: &Path,
    output: &Path,
    contract: PreparationContract<'_>,
) -> Result<PrepareReleaseOutcome, AssetError> {
    super::require_linux()?;
    ensure_output_absent(output)?;
    let supplied_receipt = read_release_input(receipt_path, MAX_RECEIPT_BYTES)?;
    if supplied_receipt != contract.receipt_bytes
        || sha256(&supplied_receipt) != contract.receipt_sha256
    {
        return Err(release_error(
            "supplied proof receipt does not match the reviewed release contract",
        ));
    }
    let receipt = parse_proof_receipt(&supplied_receipt)?;
    let profile = parse_release_profile(contract.profile_bytes)?;
    let generated_profile = canonical_profile_bytes(&profile)?;
    if generated_profile != contract.profile_bytes {
        return Err(release_error(
            "generated release profile differs from the reviewed profile",
        ));
    }
    let inspection = inspect_transport(transport_path)?;
    compare_receipt(&receipt, &inspection)?;
    compare_profile(&profile, &receipt, &inspection, contract)?;

    let sums = sha256sums(&inspection, &supplied_receipt, &generated_profile);
    let notes = release_notes(&profile, &receipt);
    let (stage, mut guard) = create_stage(output)?;
    let result = (|| {
        write_synced(&stage.join("proof-receipt.json"), &supplied_receipt)?;
        write_synced(&stage.join("release-profile.json"), &generated_profile)?;
        write_synced(&stage.join("SHA256SUMS"), sums.as_bytes())?;
        write_synced(&stage.join("release-notes.md"), notes.as_bytes())?;
        sync_directory(&stage)?;
        publish_stage(&stage, output, &mut guard)?;
        Ok(PrepareReleaseOutcome {
            status: "prepared",
            repository: profile.repository,
            tag: profile.release.tag,
            transport_id: inspection.transport_id,
            bundle_id: inspection.bundle_id,
            asset_count: inspection.parts.len() + 6,
        })
    })();
    finish_staged(result, &mut guard)
}

pub fn upload_release_asset(
    transport: &Path,
    prepared: &Path,
    gh: &Path,
    release_id: u64,
    asset_name: &str,
) -> Result<UploadAssetOutcome, AssetError> {
    super::require_linux()?;
    validate_production_contract()?;
    upload_release_asset_contract(
        transport,
        prepared,
        gh,
        release_id,
        asset_name,
        PreparationContract {
            receipt_bytes: PRODUCTION_RECEIPT,
            receipt_sha256: PRODUCTION_RECEIPT_SHA256,
            profile_bytes: PRODUCTION_PROFILE,
        },
        GhExecutableContract {
            size: 43_495_424,
            sha256: "sha256:d4a46368912cfc7b9f0a897a613910e34562ef033fc6029e0bea52c43b440fa4",
        },
        UploadHooks::default(),
    )
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-read-audit"))]
pub fn upload_release_asset_with_contract(
    transport: &Path,
    prepared: &Path,
    gh: &Path,
    release_id: u64,
    asset_name: &str,
    contract: ReleaseUploadTestContract<'_>,
    hooks: ReleaseUploadTestHooks<'_>,
) -> Result<UploadAssetOutcome, AssetError> {
    upload_release_asset_contract(
        transport,
        prepared,
        gh,
        release_id,
        asset_name,
        PreparationContract {
            receipt_bytes: contract.receipt_bytes,
            receipt_sha256: contract.receipt_sha256,
            profile_bytes: contract.profile_bytes,
        },
        GhExecutableContract {
            size: contract.gh_size,
            sha256: contract.gh_sha256,
        },
        UploadHooks {
            after_gh_validation: hooks.after_gh_validation,
            after_asset_open: hooks.after_asset_open,
            after_contract_validation: hooks.after_contract_validation,
            before_child_spawn: hooks.before_child_spawn,
            child_deadline: hooks
                .child_deadline
                .unwrap_or_else(|| Duration::from_secs(5)),
            payload_config: PayloadConfig {
                faults: hooks.payload_faults,
            },
            supervision_barrier: hooks
                .supervision_barrier
                .map(|barrier| InternalTestBarrier {
                    ready_fd: barrier.ready_fd,
                    release_fd: barrier.release_fd,
                }),
            child_pre_exec_barrier: hooks.child_pre_exec_barrier.map(|barrier| {
                InternalChildBarrier {
                    phase: match barrier.phase {
                        ChildPreExecBarrierPhase::BeforeParentDeathSignal => {
                            InternalChildBarrierPhase::BeforeParentDeathSignal
                        }
                        ChildPreExecBarrierPhase::AfterParentDeathSignal => {
                            InternalChildBarrierPhase::AfterParentDeathSignal
                        }
                    },
                    ready_fd: barrier.ready_fd,
                    release_fd: barrier.release_fd,
                }
            }),
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn upload_release_asset_contract(
    transport_path: &Path,
    prepared_path: &Path,
    gh_path: &Path,
    release_id: u64,
    asset_name: &str,
    contract: PreparationContract<'_>,
    gh_contract: GhExecutableContract<'_>,
    hooks: UploadHooks<'_>,
) -> Result<UploadAssetOutcome, AssetError> {
    if release_id == 0 {
        return Err(upload_error("release ID must be positive"));
    }
    if !gh_path.is_absolute() {
        return Err(upload_error("GitHub CLI path must be absolute"));
    }

    // The executable is the first filesystem object resolved by this command.
    let mut gh_source = open_secure_regular_path(gh_path, "GitHub CLI")?;
    let gh_metadata = validate_held_regular(&gh_source)?;
    if gh_metadata.permissions().mode() & 0o111 == 0 {
        return Err(upload_error("held GitHub CLI is not executable"));
    }
    let gh_file = sealed_snapshot(&mut gh_source, gh_contract.size, true)?;
    validate_held_identity(&gh_file, gh_contract.size, gh_contract.sha256, true)?;
    drop(gh_source);
    if let Some(hook) = hooks.after_gh_validation {
        hook();
    }

    let selected_source = reviewed_source_for_name(asset_name)
        .ok_or_else(|| upload_error("asset name is not in the reviewed release set"))?;
    let mut upload_signals =
        UploadSignals::block().map_err(|_| upload_error("establish upload signal supervision"))?;
    let selected_root_path = match selected_source {
        AssetSource::Transport => transport_path,
        AssetSource::Prepared => prepared_path,
    };
    let selected_root = open_secure_directory_path(selected_root_path, "selected asset root")?;
    let selected_is_payload = asset_name.starts_with("payload.pgi.zst.part");
    let mut selected_file = None;
    let mut selected_payload = None;
    if selected_is_payload {
        selected_payload = Some(
            LeasedPayload::open(&selected_root, asset_name, hooks.payload_config)
                .map_err(|_| upload_error("acquire stable payload read lease"))?,
        );
    } else {
        let mut source = open_held_member(&selected_root, asset_name)?;
        validate_held_regular(&source)?;
        selected_file = Some(sealed_snapshot(&mut source, MAX_JSON_BYTES, false)?);
    }
    if let Some(hook) = hooks.after_asset_open {
        hook();
    }

    let receipt = parse_proof_receipt(contract.receipt_bytes)?;
    let profile = parse_release_profile(contract.profile_bytes)?;
    let assets = reviewed_assets(&profile, &receipt, contract.profile_bytes)?;
    let selected = assets
        .iter()
        .find(|asset| asset.name == asset_name && asset.source == selected_source)
        .ok_or_else(|| upload_error("asset name is not in the reviewed release set"))?
        .clone();
    if let Some(file) = selected_file.as_ref() {
        validate_held_shape(file, selected.size)?;
    } else if selected_payload
        .as_mut()
        .ok_or_else(|| upload_error("missing leased payload"))?
        .size()
        .map_err(|_| upload_error("inspect leased payload metadata"))?
        != selected.size
    {
        return Err(upload_error("leased payload size mismatch"));
    }

    let other_root_path = match selected.source {
        AssetSource::Transport => prepared_path,
        AssetSource::Prepared => transport_path,
    };
    let other_root = open_secure_directory_path(other_root_path, "release asset root")?;
    let (transport_root, prepared_root) = match selected.source {
        AssetSource::Transport => (&selected_root, &other_root),
        AssetSource::Prepared => (&other_root, &selected_root),
    };
    let inspection = inspect_transport_held(
        transport_root,
        (selected.source == AssetSource::Transport).then_some(selected.name.as_str()),
        &mut selected_file,
    )?;
    compare_receipt(&receipt, &inspection)?;
    compare_profile(&profile, &receipt, &inspection, contract)?;
    let profile_bytes = canonical_profile_bytes(&profile)?;
    let sums = sha256sums(&inspection, contract.receipt_bytes, &profile_bytes);
    let notes = release_notes(&profile, &receipt);
    validate_prepared_held(
        prepared_root,
        (selected.source == AssetSource::Prepared).then_some(selected.name.as_str()),
        &mut selected_file,
        contract.receipt_bytes,
        &profile_bytes,
        sums.as_bytes(),
        notes.as_bytes(),
    )?;
    if let Some(hook) = hooks.after_contract_validation {
        hook();
    }

    let child_result = if let Some(mut selected_file) = selected_file {
        selected_file
            .seek(SeekFrom::Start(0))
            .map_err(|_| upload_error("rewind selected asset"))?;
        run_upload_child(
            &gh_file,
            Stdio::from(selected_file),
            None,
            &mut upload_signals,
            ChildRunConfig {
                release_id,
                asset: &selected,
                deadline: hooks.child_deadline,
                before_child_spawn: None,
                supervision_barrier: hooks.supervision_barrier,
                child_pre_exec_barrier: hooks.child_pre_exec_barrier,
            },
        )
    } else {
        let payload = selected_payload
            .as_mut()
            .ok_or_else(|| upload_error("missing leased payload"))?;
        let stdin = payload
            .child_stdin()
            .map_err(|_| upload_error("duplicate leased payload into child stdin"))?;
        let offset = payload
            .verify_zero_offset()
            .map_err(|_| upload_error("verify zero payload offset before spawn"))?;
        run_upload_child(
            &gh_file,
            stdin,
            Some(payload),
            &mut upload_signals,
            ChildRunConfig {
                release_id,
                asset: &selected,
                deadline: hooks.child_deadline,
                before_child_spawn: hooks.before_child_spawn.map(|hook| (hook, offset)),
                supervision_barrier: hooks.supervision_barrier,
                child_pre_exec_barrier: hooks.child_pre_exec_barrier,
            },
        )
    };
    if let Some(payload) = selected_payload.as_mut() {
        let released = payload
            .release()
            .map_err(|_| upload_error("release payload read lease"));
        if child_result.is_ok() {
            released?;
        }
    }
    child_result
}

fn reviewed_source_for_name(name: &str) -> Option<AssetSource> {
    match name {
        "transport.json"
        | "bundle-manifest.json"
        | "NOTICE"
        | "payload.pgi.zst.part0000"
        | "payload.pgi.zst.part0001" => Some(AssetSource::Transport),
        "proof-receipt.json" | "release-profile.json" | "SHA256SUMS" => Some(AssetSource::Prepared),
        _ => None,
    }
}

fn reviewed_assets(
    profile: &ReleaseProfile,
    receipt: &ProofReceipt,
    profile_bytes: &[u8],
) -> Result<Vec<ReviewedAsset>, AssetError> {
    let mut assets = profile
        .transport
        .members
        .iter()
        .map(|member| ReviewedAsset {
            name: member.asset_name.clone(),
            size: member.size,
            sha256: member.sha256.clone(),
            source: AssetSource::Transport,
        })
        .collect::<Vec<_>>();
    assets.push(ReviewedAsset {
        name: "proof-receipt.json".to_owned(),
        size: profile.proof.size,
        sha256: profile.proof.sha256.clone(),
        source: AssetSource::Prepared,
    });
    assets.push(ReviewedAsset {
        name: "release-profile.json".to_owned(),
        size: profile_bytes.len() as u64,
        sha256: sha256(profile_bytes),
        source: AssetSource::Prepared,
    });
    let sums = sha256sums_from_contract(profile, receipt, profile_bytes)?;
    assets.push(ReviewedAsset {
        name: "SHA256SUMS".to_owned(),
        size: sums.len() as u64,
        sha256: sha256(sums.as_bytes()),
        source: AssetSource::Prepared,
    });
    Ok(assets)
}

fn sha256sums_from_contract(
    profile: &ReleaseProfile,
    receipt: &ProofReceipt,
    profile_bytes: &[u8],
) -> Result<String, AssetError> {
    if profile.transport.members.len() != receipt.transport.parts.len() + 3 {
        return Err(release_error("release contract member count mismatch"));
    }
    let mut output = String::new();
    for member in &profile.transport.members {
        append_sum(&mut output, &member.sha256, &member.asset_name)?;
    }
    append_sum(&mut output, &profile.proof.sha256, "proof-receipt.json")?;
    append_sum(&mut output, &sha256(profile_bytes), "release-profile.json")?;
    Ok(output)
}

fn append_sum(output: &mut String, identity: &str, name: &str) -> Result<(), AssetError> {
    let digest = identity
        .strip_prefix("sha256:")
        .ok_or_else(|| release_error("checksum identity is not SHA-256"))?;
    output.push_str(digest);
    output.push_str("  ");
    output.push_str(name);
    output.push('\n');
    Ok(())
}

fn canonical_profile_bytes(profile: &ReleaseProfile) -> Result<Vec<u8>, AssetError> {
    let value = serde_json::to_value(profile)
        .map_err(|_| release_error("cannot materialize release profile"))?;
    serde_jcs::to_vec(&value).map_err(|_| release_error("cannot canonicalize release profile"))
}

fn read_release_input(path: &Path, limit: u64) -> Result<Vec<u8>, AssetError> {
    let (file, metadata) = open_regular(
        path,
        AssetErrorKind::InputIo,
        AssetErrorKind::ReleaseInvalid,
    )?;
    if metadata.len() > limit {
        return Err(release_error("release input exceeds bounded size limit"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| AssetError::new(AssetErrorKind::InputIo, error.to_string()))?;
    if bytes.len() as u64 > limit {
        return Err(release_error("release input exceeds bounded size limit"));
    }
    if bytes.len() as u64 != metadata.len() {
        return Err(release_error("release input changed while reading"));
    }
    Ok(bytes)
}

fn inspect_transport_held(
    root: &File,
    selected_name: Option<&str>,
    selected: &mut Option<File>,
) -> Result<TransportInspection, AssetError> {
    let transport_bytes = read_held_member_bytes(
        root,
        "transport.json",
        MAX_JSON_BYTES,
        selected_name,
        selected,
    )?;
    let manifest = parse_transport_manifest(&transport_bytes)?;
    let mut expected = vec![
        ("transport.json".to_owned(), transport_bytes.len() as u64),
        (
            "bundle-manifest.json".to_owned(),
            manifest.bundle.manifest.size,
        ),
        ("NOTICE".to_owned(), manifest.bundle.notice.size),
    ];
    expected.extend(
        manifest
            .payload
            .parts
            .iter()
            .map(|part| (part.path.clone(), part.size)),
    );
    validate_closed_held_directory(root, &expected, selected_name, selected)?;
    let bundle_manifest_bytes = read_held_member_bytes(
        root,
        "bundle-manifest.json",
        MAX_JSON_BYTES,
        selected_name,
        selected,
    )?;
    let notice = read_held_member_bytes(root, "NOTICE", MAX_NOTICE_BYTES, selected_name, selected)?;
    validate_bundle_metadata(&manifest, &bundle_manifest_bytes, &notice)?;
    Ok(transport_inspection_from_verified(VerifiedTransport {
        manifest,
        transport_bytes,
        bundle_manifest_bytes,
        notice,
    }))
}

#[allow(clippy::too_many_arguments)]
fn validate_prepared_held(
    root: &File,
    selected_name: Option<&str>,
    selected: &mut Option<File>,
    receipt: &[u8],
    profile: &[u8],
    sums: &[u8],
    notes: &[u8],
) -> Result<(), AssetError> {
    let expected_bytes = [
        ("proof-receipt.json", receipt),
        ("release-profile.json", profile),
        ("SHA256SUMS", sums),
        ("release-notes.md", notes),
    ];
    let expected = expected_bytes
        .iter()
        .map(|(name, bytes)| ((*name).to_owned(), bytes.len() as u64))
        .collect::<Vec<_>>();
    validate_closed_held_directory(root, &expected, selected_name, selected)?;
    for (name, bytes) in expected_bytes {
        let actual = read_held_member_bytes(root, name, MAX_JSON_BYTES, selected_name, selected)?;
        if actual != bytes {
            return Err(upload_error(
                "prepared release member differs from reviewed bytes",
            ));
        }
    }
    Ok(())
}

fn validate_closed_held_directory(
    root: &File,
    expected: &[(String, u64)],
    selected_name: Option<&str>,
    selected: &Option<File>,
) -> Result<(), AssetError> {
    let expected_names = expected
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<BTreeSet<_>>();
    let actual_names = read_held_names(root)?.into_iter().collect::<BTreeSet<_>>();
    if actual_names != expected_names {
        return Err(upload_error("release asset directory member set mismatch"));
    }
    for (name, size) in expected {
        if selected_name == Some(name.as_str()) {
            if let Some(selected) = selected {
                validate_held_shape(selected, *size)?;
            }
        } else {
            validate_member_metadata(root, name, *size)?;
        }
    }
    Ok(())
}

fn read_held_member_bytes(
    root: &File,
    name: &str,
    cap: u64,
    selected_name: Option<&str>,
    selected: &mut Option<File>,
) -> Result<Vec<u8>, AssetError> {
    if selected_name == Some(name) {
        let selected = selected
            .as_mut()
            .ok_or_else(|| upload_error("payload content access is forbidden"))?;
        read_bounded_held(selected, cap)
    } else {
        let mut file = open_held_member(root, name)?;
        read_bounded_held(&mut file, cap)
    }
}

fn read_bounded_held(file: &mut File, cap: u64) -> Result<Vec<u8>, AssetError> {
    let metadata = file
        .metadata()
        .map_err(|_| upload_error("inspect held metadata file"))?;
    if !metadata.file_type().is_file() || metadata.len() > cap {
        return Err(upload_error("held metadata file shape or size is invalid"));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| upload_error("rewind held metadata file"))?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    (&mut *file)
        .take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| upload_error("read held metadata file"))?;
    if bytes.len() as u64 != metadata.len() || bytes.len() as u64 > cap {
        return Err(upload_error("held metadata file changed while reading"));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| upload_error("rewind validated metadata file"))?;
    Ok(bytes)
}

fn sealed_snapshot(
    source: &mut File,
    maximum_size: u64,
    executable: bool,
) -> Result<File, AssetError> {
    let metadata = validate_held_regular(source)?;
    if metadata.len() > maximum_size {
        return Err(upload_error("snapshot source exceeds reviewed size bound"));
    }
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| upload_error("rewind snapshot source"))?;
    let name = CString::new("pangopup-release-snapshot").expect("static memfd name");
    let fd =
        unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING) };
    let mut snapshot = file_from_raw_fd(fd).map_err(|_| upload_error("create sealed snapshot"))?;
    let mut total = 0_u64;
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|_| upload_error("read snapshot source"))?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| upload_error("snapshot size overflow"))?;
        if total > maximum_size {
            return Err(upload_error("snapshot source exceeds reviewed size bound"));
        }
        snapshot
            .write_all(&buffer[..read])
            .map_err(|_| upload_error("write sealed snapshot"))?;
    }
    if total != metadata.len() {
        return Err(upload_error("snapshot source changed while copying"));
    }
    if executable {
        let result = unsafe { libc::fchmod(snapshot.as_raw_fd(), 0o500) };
        if result == -1 {
            return Err(upload_error("make executable snapshot"));
        }
    }
    let seals = libc::F_SEAL_WRITE | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_SEAL;
    if unsafe { libc::fcntl(snapshot.as_raw_fd(), libc::F_ADD_SEALS, seals) } == -1 {
        return Err(upload_error("seal release snapshot"));
    }
    if unsafe { libc::fcntl(snapshot.as_raw_fd(), libc::F_GET_SEALS) } & seals != seals {
        return Err(upload_error("verify release snapshot seals"));
    }
    snapshot
        .seek(SeekFrom::Start(0))
        .map_err(|_| upload_error("rewind sealed snapshot"))?;
    Ok(snapshot)
}

fn validate_held_identity(
    file: &File,
    expected_size: u64,
    expected_sha256: &str,
    require_executable: bool,
) -> Result<(), AssetError> {
    validate_held_shape(file, expected_size)?;
    let metadata = file
        .metadata()
        .map_err(|_| upload_error("inspect held file identity"))?;
    if require_executable && metadata.permissions().mode() & 0o111 == 0 {
        return Err(upload_error("held GitHub CLI is not executable"));
    }
    let mut reader = file
        .try_clone()
        .map_err(|_| upload_error("duplicate held file for hashing"))?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|_| upload_error("rewind held file for hashing"))?;
    let mut hash = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| upload_error("hash held GitHub CLI"))?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| upload_error("held file size overflow"))?;
        if total > expected_size {
            return Err(upload_error("held file exceeds reviewed size"));
        }
        hash.update(&buffer[..read]);
    }
    let actual = format!("sha256:{:x}", hash.finalize());
    if total != expected_size || actual != expected_sha256 {
        return Err(upload_error("held GitHub CLI identity mismatch"));
    }
    Ok(())
}

fn validate_held_shape(file: &File, expected_size: u64) -> Result<(), AssetError> {
    let metadata = validate_held_regular(file)?;
    if metadata.len() != expected_size {
        return Err(upload_error("held release asset shape or size mismatch"));
    }
    Ok(())
}

fn validate_held_regular(file: &File) -> Result<fs::Metadata, AssetError> {
    let metadata = file
        .metadata()
        .map_err(|_| upload_error("inspect held release asset"))?;
    if !metadata.file_type().is_file() {
        return Err(upload_error("held release asset shape or size mismatch"));
    }
    Ok(metadata)
}

fn open_secure_directory_path(path: &Path, label: &str) -> Result<File, AssetError> {
    open_secure_path(path, true).map_err(|_| upload_error(format!("open {label} without symlinks")))
}

fn open_secure_regular_path(path: &Path, label: &str) -> Result<File, AssetError> {
    open_secure_path(path, false)
        .map_err(|_| upload_error(format!("open {label} without symlinks")))
}

fn open_secure_path(path: &Path, final_is_directory: bool) -> io::Result<File> {
    let mut current = open_path_start(path.is_absolute())?;
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => names.push(name.to_owned()),
            Component::ParentDir | Component::Prefix(_) => {
                return Err(io::Error::other("unsafe path component"));
            }
        }
    }
    if names.is_empty() {
        return if final_is_directory {
            Ok(current)
        } else {
            Err(io::Error::other("missing file name"))
        };
    }
    for (position, name) in names.iter().enumerate() {
        let final_component = position + 1 == names.len();
        let flags = if !final_component || final_is_directory {
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC
        } else {
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC
        };
        current = openat2_held(current.as_raw_fd(), name, flags)?;
    }
    Ok(current)
}

fn open_path_start(absolute: bool) -> io::Result<File> {
    let name = if absolute { b"/\0" } else { b".\0" };
    // SAFETY: both byte strings are statically NUL terminated.
    let fd = unsafe {
        libc::open(
            name.as_ptr().cast(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    file_from_raw_fd(fd)
}

fn open_held_member(root: &File, name: &str) -> Result<File, AssetError> {
    openat2_held(
        root.as_raw_fd(),
        OsStr::new(name),
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
    )
    .map_err(|_| upload_error("open selected release asset without symlinks"))
}

#[repr(C)]
struct ReleaseOpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}

fn openat2_held(dirfd: RawFd, name: &OsStr, flags: i32) -> io::Result<File> {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes == b"." || bytes == b".." || bytes.contains(&b'/') {
        return Err(io::Error::other("invalid direct member"));
    }
    let name = CString::new(bytes).map_err(|_| io::Error::other("NUL in path"))?;
    let how = ReleaseOpenHow {
        flags: flags as u64,
        mode: 0,
        resolve: 0x02 | 0x04 | 0x08,
    };
    // SAFETY: the held dirfd, CString, and open_how remain valid for this syscall.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            dirfd,
            name.as_ptr(),
            &how,
            std::mem::size_of::<ReleaseOpenHow>(),
        ) as i32
    };
    file_from_raw_fd(fd)
}

fn file_from_raw_fd(fd: i32) -> io::Result<File> {
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: a successful open/openat2 returns one newly owned descriptor.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn validate_member_metadata(root: &File, name: &str, expected_size: u64) -> Result<(), AssetError> {
    let name = CString::new(name).map_err(|_| upload_error("invalid asset member name"))?;
    let mut stat = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: root and name are held; stat points to writable storage.
    let result = unsafe {
        libc::fstatat(
            root.as_raw_fd(),
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result != 0 {
        return Err(upload_error("inspect release asset member"));
    }
    // SAFETY: successful fstatat initialized the structure.
    let stat = unsafe { stat.assume_init() };
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG
        || stat.st_size < 0
        || stat.st_size as u64 != expected_size
    {
        return Err(upload_error("release asset member shape or size mismatch"));
    }
    Ok(())
}

fn read_held_names(root: &File) -> Result<Vec<String>, AssetError> {
    let cursor = root
        .try_clone()
        .map_err(|_| upload_error("duplicate release asset directory"))?;
    let mut buffer = [MaybeUninit::<u8>::uninit(); 8192];
    let mut entries = rustix::fs::RawDir::new(cursor, &mut buffer);
    let mut names = Vec::new();
    while let Some(entry) = entries.next() {
        let entry = entry.map_err(|_| upload_error("read release asset directory"))?;
        let bytes = entry.file_name().to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        names.push(
            String::from_utf8(bytes.to_vec())
                .map_err(|_| upload_error("release asset name is not UTF-8"))?,
        );
    }
    Ok(names)
}

fn run_upload_child(
    gh_file: &File,
    asset_stdin: Stdio,
    mut payload: Option<&mut LeasedPayload>,
    upload_signals: &mut UploadSignals,
    config: ChildRunConfig<'_>,
) -> Result<UploadAssetOutcome, AssetError> {
    let url = format!(
        "https://uploads.github.com/repos/genomoncology/pangopup/releases/{}/assets?name={}",
        config.release_id,
        percent_encode(&config.asset.name)
    );
    let content_length = format!("Content-Length:{}", config.asset.size);
    let argv = [
        "gh".to_owned(),
        "api".to_owned(),
        url,
        "--method".to_owned(),
        "POST".to_owned(),
        "--header".to_owned(),
        "Accept:application/vnd.github+json".to_owned(),
        "--header".to_owned(),
        "X-GitHub-Api-Version:2022-11-28".to_owned(),
        "--header".to_owned(),
        "Content-Type:application/octet-stream".to_owned(),
        "--header".to_owned(),
        content_length,
        "--input".to_owned(),
        "-".to_owned(),
        "--jq".to_owned(),
        r#"{"name":.name,"size":.size,"state":.state,"digest":.digest}"#.to_owned(),
    ];
    let mut environment = Vec::new();
    for name in [
        "HOME",
        "XDG_CONFIG_HOME",
        "GH_CONFIG_DIR",
        "GH_TOKEN",
        "GITHUB_TOKEN",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "HTTPS_PROXY",
        "NO_PROXY",
        "LANG",
        "LC_ALL",
    ] {
        if let Some(value) = env::var_os(name) {
            let mut assignment = name.as_bytes().to_vec();
            assignment.push(b'=');
            assignment.extend_from_slice(value.as_bytes());
            environment.push(assignment);
        }
    }
    environment.extend([
        b"GH_PROMPT_DISABLED=1".to_vec(),
        b"GH_PAGER=cat".to_vec(),
        b"PAGER=cat".to_vec(),
        b"NO_COLOR=1".to_vec(),
    ]);

    let mut command = Command::new("/proc/self/exe");
    command.env_clear();
    command.stdin(asset_stdin);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let expected_parent_pid = unsafe { libc::getpid() };
    configure_execveat(
        &mut command,
        gh_file.as_raw_fd(),
        &argv,
        &environment,
        expected_parent_pid,
        upload_signals.original_mask(),
        config.child_pre_exec_barrier,
    )?;
    let pending = if let Some(payload) = payload.as_deref_mut() {
        payload
            .drain_before_spawn(upload_signals)
            .map_err(|_| upload_error("drain upload signals before child spawn"))?
    } else {
        upload_signals
            .drain()
            .map_err(|_| upload_error("drain upload signals before child spawn"))?
    };
    if pending.interrupt.is_some() {
        return Err(upload_error("GitHub CLI upload interrupted"));
    }
    if pending.lease_break {
        return Err(upload_error("payload lease break interrupted upload"));
    }
    if let Some((hook, offset)) = config.before_child_spawn {
        let operations = payload
            .as_deref()
            .expect("payload hook is present only for a leased payload")
            .operations();
        hook(operations, offset);
    }
    let mut child = command
        .spawn()
        .map_err(|_| upload_error("start sealed GitHub CLI"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| upload_error("capture GitHub CLI stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| upload_error("capture GitHub CLI stderr"))?;
    let capture_cancelled = Arc::new(AtomicBool::new(false));
    let (signal_send, signal_receive) = mpsc::channel();
    let stdout_thread =
        match capture_bounded(stdout, signal_send.clone(), Arc::clone(&capture_cancelled)) {
            Ok(thread) => thread,
            Err(error) => {
                let _ = kill_process_group_and_reap(&mut child, Instant::now());
                return Err(error);
            }
        };
    let stderr_thread = match capture_bounded(stderr, signal_send, Arc::clone(&capture_cancelled)) {
        Ok(thread) => thread,
        Err(error) => {
            capture_cancelled.store(true, Ordering::Release);
            let _ = kill_process_group_and_reap(&mut child, Instant::now());
            let _ = stdout_thread.join();
            return Err(error);
        }
    };
    let started = Instant::now();
    let mut failure = match config.supervision_barrier {
        Some(barrier) => supervise_test_barrier(barrier)
            .err()
            .map(|_| "GitHub CLI supervision failed"),
        None => None,
    };
    let mut status = None;
    let mut captures_complete = 0_u8;
    let status = loop {
        while let Ok(signal) = signal_receive.try_recv() {
            match signal {
                CaptureSignal::Overflow => {
                    failure = Some("GitHub CLI output exceeded 64 KiB");
                }
                CaptureSignal::Complete => captures_complete = captures_complete.saturating_add(1),
            }
        }
        if failure.is_some() {
            break status;
        }
        let pending = if let Some(payload) = payload.as_deref_mut() {
            payload.break_pending(upload_signals)
        } else {
            upload_signals.drain()
        };
        match pending {
            Ok(PendingUploadSignals {
                interrupt: Some(_), ..
            }) => {
                failure = Some("GitHub CLI upload interrupted");
                break None;
            }
            Ok(PendingUploadSignals {
                lease_break: true, ..
            }) => {
                failure = Some("payload lease break interrupted upload");
                break None;
            }
            Ok(_) => {}
            Err(_) => {
                failure = Some("payload lease supervision failed");
                break None;
            }
        }
        if started.elapsed() >= config.deadline {
            failure = Some("GitHub CLI upload deadline exceeded");
            break None;
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(Some(child_status)) => status = Some(child_status),
                Ok(None) => {}
                Err(_) => {
                    failure = Some("GitHub CLI supervision failed");
                    break None;
                }
            }
        }
        if status.is_some() && captures_complete == 2 {
            break status;
        }
        thread::sleep(Duration::from_millis(2));
    };
    let mut cleanup_started = None;
    let mut cleanup_exhausted = false;
    if failure.is_some() {
        capture_cancelled.store(true, Ordering::Release);
        let cleanup = Instant::now();
        cleanup_started = Some(cleanup);
        cleanup_exhausted = kill_process_group_and_reap(&mut child, cleanup);
    }
    let stdout = stdout_thread
        .join()
        .map_err(|_| upload_error("join GitHub CLI stdout reader"))?
        .map_err(|_| upload_error("read GitHub CLI stdout"))?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| upload_error("join GitHub CLI stderr reader"))?
        .map_err(|_| upload_error("read GitHub CLI stderr"))?;
    if cleanup_started.is_some_and(|started| started.elapsed() > LEASE_CLEANUP_DEADLINE)
        || payload
            .as_deref()
            .is_some_and(LeasedPayload::cleanup_deadline_exhausted)
    {
        cleanup_exhausted = true;
    }
    if cleanup_exhausted {
        return Err(upload_error(
            "GitHub CLI process-group cleanup exceeded five seconds",
        ));
    }
    if let Some(message) = failure {
        return Err(upload_error(message));
    }
    if stdout.overflow || stderr.overflow || status.is_none() {
        return Err(upload_error("GitHub CLI output exceeded 64 KiB"));
    }
    if !status.is_some_and(|status| status.success()) {
        return Err(upload_error("GitHub CLI upload request failed"));
    }
    let final_pending = if let Some(payload) = payload {
        payload
            .final_check(upload_signals)
            .map_err(|_| upload_error("payload read lease was not stable through upload"))?
    } else {
        upload_signals
            .drain()
            .map_err(|_| upload_error("GitHub CLI signal supervision failed"))?
    };
    if final_pending.interrupt.is_some() {
        return Err(upload_error("GitHub CLI upload interrupted"));
    }
    if final_pending.lease_break {
        return Err(upload_error(
            "payload read lease was not stable through upload",
        ));
    }
    let response = parse_upload_response(&stdout.bytes)?;
    if response.name != config.asset.name
        || response.size != config.asset.size
        || response.state != "uploaded"
        || response
            .digest
            .as_ref()
            .is_some_and(|digest| digest != &config.asset.sha256)
    {
        return Err(upload_error(
            "GitHub upload response differs from reviewed asset",
        ));
    }
    Ok(UploadAssetOutcome {
        status: "uploaded",
        asset: response.name,
        size: response.size,
        digest: response.digest,
    })
}

fn configure_execveat(
    command: &mut Command,
    executable_fd: RawFd,
    argv: &[String],
    environment: &[Vec<u8>],
    expected_parent_pid: libc::pid_t,
    original_signal_mask: libc::sigset_t,
    child_barrier: Option<InternalChildBarrier>,
) -> Result<(), AssetError> {
    let argv_storage = argv
        .iter()
        .map(|value| CString::new(value.as_bytes()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| upload_error("GitHub CLI argv contains NUL"))?;
    let environment_storage = environment
        .iter()
        .map(|value| CString::new(value.as_slice()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| upload_error("GitHub CLI environment contains NUL"))?;
    let mut argv_pointers = argv_storage
        .iter()
        .map(|value| value.as_ptr() as usize)
        .collect::<Vec<_>>();
    argv_pointers.push(0);
    let mut environment_pointers = environment_storage
        .iter()
        .map(|value| value.as_ptr() as usize)
        .collect::<Vec<_>>();
    environment_pointers.push(0);
    unsafe {
        command.pre_exec(move || {
            let _keep_argv_alive = &argv_storage;
            let _keep_environment_alive = &environment_storage;
            if libc::setpgid(0, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            if let Some(barrier) = child_barrier
                && barrier.phase == InternalChildBarrierPhase::BeforeParentDeathSignal
            {
                child_test_barrier(barrier)?;
            }
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                return Err(io::Error::last_os_error());
            }
            if let Some(barrier) = child_barrier
                && barrier.phase == InternalChildBarrierPhase::AfterParentDeathSignal
            {
                child_test_barrier(barrier)?;
            }
            if libc::getppid() != expected_parent_pid {
                return Err(io::Error::from_raw_os_error(libc::ECHILD));
            }
            let mask_result = libc::pthread_sigmask(
                libc::SIG_SETMASK,
                &original_signal_mask,
                std::ptr::null_mut(),
            );
            if mask_result != 0 {
                return Err(io::Error::from_raw_os_error(mask_result));
            }
            let empty = b"\0";
            libc::execveat(
                executable_fd,
                empty.as_ptr().cast(),
                argv_pointers.as_ptr().cast(),
                environment_pointers.as_ptr().cast(),
                libc::AT_EMPTY_PATH,
            );
            Err(io::Error::last_os_error())
        });
    }
    Ok(())
}

fn child_test_barrier(barrier: InternalChildBarrier) -> io::Result<()> {
    let pid = unsafe { libc::getpid() };
    write_fd_bytes(barrier.ready_fd, &pid.to_ne_bytes())?;
    read_fd_byte(barrier.release_fd)
}

fn write_fd_bytes(fd: RawFd, bytes: &[u8]) -> io::Result<()> {
    let mut written_total = 0;
    while written_total < bytes.len() {
        let remaining = &bytes[written_total..];
        let written = unsafe { libc::write(fd, remaining.as_ptr().cast(), remaining.len()) };
        if written > 0 {
            written_total += written as usize;
            continue;
        }
        if written == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn read_fd_byte(fd: RawFd) -> io::Result<()> {
    loop {
        let mut release = 0_u8;
        let read = unsafe {
            libc::read(
                fd,
                (&mut release as *mut u8).cast(),
                std::mem::size_of::<u8>(),
            )
        };
        if read == 1 {
            return Ok(());
        }
        if read == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        return Err(io::Error::last_os_error());
    }
}

fn supervise_test_barrier(barrier: InternalTestBarrier) -> io::Result<()> {
    let tid = unsafe { libc::syscall(libc::SYS_gettid) as libc::pid_t };
    write_fd_bytes(barrier.ready_fd, &tid.to_ne_bytes())?;
    read_fd_byte(barrier.release_fd)
}

fn kill_process_group_and_reap(child: &mut std::process::Child, started: Instant) -> bool {
    let group = -(child.id() as libc::pid_t);
    let killed = unsafe { libc::kill(group, libc::SIGKILL) };
    let mut cleanup_exhausted =
        killed == -1 && io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() <= LEASE_CLEANUP_DEADLINE => {
                thread::sleep(Duration::from_millis(2));
            }
            Ok(None) => {
                cleanup_exhausted = true;
                let _ = child.wait();
                break;
            }
            Err(_) => {
                cleanup_exhausted = true;
                let _ = child.wait();
                break;
            }
        }
    }
    cleanup_exhausted
}

struct CapturedOutput {
    bytes: Vec<u8>,
    overflow: bool,
}

#[derive(Clone, Copy)]
enum CaptureSignal {
    Overflow,
    Complete,
}

fn capture_bounded(
    mut input: impl Read + AsRawFd + Send + 'static,
    signal: mpsc::Sender<CaptureSignal>,
    cancelled: Arc<AtomicBool>,
) -> Result<thread::JoinHandle<io::Result<CapturedOutput>>, AssetError> {
    let flags = unsafe { libc::fcntl(input.as_raw_fd(), libc::F_GETFL) };
    if flags == -1
        || unsafe { libc::fcntl(input.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1
    {
        return Err(upload_error("make GitHub CLI output pipe nonblocking"));
    }
    Ok(thread::spawn(move || {
        let result = (|| {
            const CAP: usize = 64 * 1024;
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 8192];
            let mut overflow = false;
            loop {
                if cancelled.load(Ordering::Acquire) {
                    break;
                }
                let read = match input.read(&mut buffer) {
                    Ok(read) => read,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                if read == 0 {
                    break;
                }
                if bytes.len() < CAP {
                    let retained = (CAP - bytes.len()).min(read);
                    bytes.extend_from_slice(&buffer[..retained]);
                    if retained < read {
                        overflow = true;
                        let _ = signal.send(CaptureSignal::Overflow);
                    }
                } else if !overflow {
                    overflow = true;
                    let _ = signal.send(CaptureSignal::Overflow);
                }
            }
            Ok(CapturedOutput { bytes, overflow })
        })();
        let _ = signal.send(CaptureSignal::Complete);
        result
    }))
}

fn parse_upload_response(bytes: &[u8]) -> Result<UploadResponse, AssetError> {
    reject_duplicate_json(bytes).map_err(|_| upload_error("GitHub upload response is invalid"))?;
    serde_json::from_slice(bytes)
        .map_err(|_| upload_error("GitHub upload response is not closed JSON"))
}

fn percent_encode(name: &str) -> String {
    let mut encoded = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn compare_receipt(
    receipt: &ProofReceipt,
    inspection: &TransportInspection,
) -> Result<(), AssetError> {
    let notice = &receipt.bundle.members[0];
    let scores = &receipt.bundle.members[1];
    let parts_match = receipt.transport.parts.len() == inspection.parts.len()
        && receipt
            .transport
            .parts
            .iter()
            .zip(&inspection.parts)
            .all(|(left, right)| {
                left.ordinal == right.ordinal
                    && left.path == right.path
                    && left.size == right.size
                    && left.sha256 == right.sha256
            });
    if receipt.bundle.bundle_id != inspection.bundle_id
        || receipt.bundle.manifest.size != inspection.bundle_manifest_size
        || receipt.bundle.manifest.sha256 != inspection.bundle_manifest_sha256
        || notice.size != inspection.notice_size
        || notice.sha256 != inspection.notice_sha256
        || scores.size != inspection.score_size
        || scores.sha256 != inspection.score_sha256
        || receipt.transport.transport_id != inspection.transport_id
        || receipt.transport.manifest.size != inspection.transport_bytes.len() as u64
        || receipt.transport.manifest.sha256 != inspection.transport_sha256
        || receipt.transport.compressed.size != inspection.compressed_size
        || receipt.transport.compressed.sha256 != inspection.compressed_sha256
        || receipt.tool.encoder_crate != inspection.compression.encoder_crate
        || receipt.tool.libzstd_version != inspection.compression.libzstd_version
        || !parts_match
    {
        return Err(release_error(
            "proof receipt does not match inspected transport metadata",
        ));
    }
    Ok(())
}

fn compare_profile(
    profile: &ReleaseProfile,
    receipt: &ProofReceipt,
    inspection: &TransportInspection,
    contract: PreparationContract<'_>,
) -> Result<(), AssetError> {
    let expected = [
        (
            "transport.json",
            inspection.transport_bytes.len() as u64,
            &inspection.transport_sha256,
        ),
        (
            "bundle-manifest.json",
            inspection.bundle_manifest_size,
            &inspection.bundle_manifest_sha256,
        ),
        ("NOTICE", inspection.notice_size, &inspection.notice_sha256),
    ];
    let fixed_match =
        expected
            .iter()
            .zip(&profile.transport.members)
            .all(|((name, size, digest), member)| {
                member.asset_name == *name && member.size == *size && member.sha256 == **digest
            });
    let parts_match = inspection
        .parts
        .iter()
        .zip(profile.transport.members.iter().skip(3))
        .all(|(part, member)| {
            part.path == member.asset_name
                && part.size == member.size
                && part.sha256 == member.sha256
        });
    if profile.transport.members.len() != inspection.parts.len() + 3
        || !fixed_match
        || !parts_match
        || profile.transport.transport_id != inspection.transport_id
        || profile.bundle.bundle_id != inspection.bundle_id
        || profile.proof.size != contract.receipt_bytes.len() as u64
        || profile.proof.sha256 != contract.receipt_sha256
        || profile.source.archive.name != receipt.source.archive_name
        || profile.source.archive.size != receipt.source.archive_size
        || profile.source.archive.md5 != receipt.source.archive_md5
        || profile.reference_compatibility.assembly_accession
            != receipt.reference.assembly_accession
        || profile.reference_compatibility.input_size != receipt.reference.input_size
        || profile.reference_compatibility.input_sha256 != receipt.reference.input_sha256
        || profile.reference_compatibility.sequence_set_sha256
            != receipt.reference.sequence_set_sha256
    {
        return Err(release_error(
            "release profile does not match the receipt and transport",
        ));
    }
    Ok(())
}

fn sha256sums(inspection: &TransportInspection, receipt: &[u8], profile: &[u8]) -> String {
    let mut entries = vec![
        (inspection.transport_sha256.as_str(), "transport.json"),
        (
            inspection.bundle_manifest_sha256.as_str(),
            "bundle-manifest.json",
        ),
        (inspection.notice_sha256.as_str(), "NOTICE"),
    ];
    entries.extend(
        inspection
            .parts
            .iter()
            .map(|part| (part.sha256.as_str(), part.path.as_str())),
    );
    let receipt_hash = sha256(receipt);
    let profile_hash = sha256(profile);
    entries.push((receipt_hash.as_str(), "proof-receipt.json"));
    entries.push((profile_hash.as_str(), "release-profile.json"));
    let mut output = String::new();
    for (identity, name) in entries {
        let digest = identity
            .strip_prefix("sha256:")
            .expect("validated SHA-256 identity");
        output.push_str(digest);
        output.push_str("  ");
        output.push_str(name);
        output.push('\n');
    }
    output
}

fn release_notes(profile: &ReleaseProfile, receipt: &ProofReceipt) -> String {
    let transport_member_count = match profile.transport.members.len() {
        5 => "five".to_owned(),
        count => count.to_string(),
    };
    let downloads = profile
        .transport
        .members
        .iter()
        .map(|member| {
            format!(
                "curl --fail --location --output \"$transport_dir/{}\" '{}'",
                member.asset_name, member.url
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "# {}\n\n\
         Source: **{}**, by {} and {}, DOI [{}](https://doi.org/{}), licensed CC BY 4.0.\n\n\
         The publisher identifies these as masked, window-50 precomputed SNV data for hg38, but does not name an exact FASTA/patch release or GENCODE release. Separately, Pangopup exhaustively certified all ordinary reference alleles against RefSeq GRCh38.p14 (`GCF_000001405.40`) with zero mismatches while preserving the 30 published `REF=N` loci.\n\n\
         Pangopup transformed the per-gene TSV rows into its deterministic fixed-v1 lookup representation, preserving gene-specific scores and source attribution.\n\n\
         - Bundle: `{}`\n\
         - Transport: `{}`\n\
         - Proof receipt: `{}`\n\n\
         This release does not contain model weights, reference or mask assets, binaries, non-SNV inference, remote sync, HTTP, or Docker support.\n\n\
         ## Manual installation\n\n\
         This copy/paste recipe creates a new transport directory, downloads exactly the {} transport members, and installs them. Keep `proof-receipt.json`, `release-profile.json`, and `SHA256SUMS` outside this directory; downloading all release assets there is invalid because the installer enforces a closed transport set.\n\n\
         ```sh\n\
         transport_dir=\"$PWD/pangopup-snv-grch38-v1\"\n\
         mkdir -- \"$transport_dir\"\n\
         {}\n\
         pangopup assets install --transport \"$transport_dir\"\n\
         ```\n",
        profile.release.title,
        profile.source.title,
        profile.source.creators[0],
        profile.source.creators[1],
        profile.source.doi,
        profile.source.doi,
        receipt.bundle.bundle_id,
        receipt.transport.transport_id,
        profile.proof.sha256,
        transport_member_count,
        downloads,
    )
}

fn valid_identity(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn valid_md5(value: &str) -> bool {
    value.strip_prefix("md5:").is_some_and(|hex| {
        hex.len() == 32
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn valid_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn release_error(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::ReleaseInvalid, message)
}

fn upload_error(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::ReleaseUpload, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_receipt_and_profile_are_exact_canonical_contracts() {
        assert_eq!(PRODUCTION_RECEIPT.len(), 2_194);
        assert_eq!(sha256(PRODUCTION_RECEIPT), PRODUCTION_RECEIPT_SHA256);
        assert_eq!(PRODUCTION_PROFILE.len(), 2_821);
        assert_eq!(sha256(PRODUCTION_PROFILE), PRODUCTION_PROFILE_SHA256);
        let (receipt, profile) = validate_production_contract().expect("exact production contract");
        assert_eq!(profile.bundle.bundle_id, receipt.bundle.bundle_id);
        assert_eq!(
            profile.transport.transport_id,
            receipt.transport.transport_id
        );
        let expected_names = [
            "transport.json",
            "bundle-manifest.json",
            "NOTICE",
            "payload.pgi.zst.part0000",
            "payload.pgi.zst.part0001",
        ];
        assert_eq!(
            profile
                .transport
                .members
                .iter()
                .map(|member| member.asset_name.as_str())
                .collect::<Vec<_>>(),
            expected_names
        );
        for member in &profile.transport.members {
            assert_eq!(
                member.url,
                format!(
                    "https://github.com/genomoncology/pangopup/releases/download/snv-grch38-v1/{}",
                    member.asset_name
                )
            );
        }
        let notes = release_notes(&profile, &receipt);
        assert_eq!(notes.matches("curl --fail --location --output").count(), 5);
        assert!(notes.contains("pangopup assets install --transport \"$transport_dir\""));
        assert!(
            notes
                .lines()
                .filter(|line| line.starts_with("curl "))
                .all(|line| !line.contains("proof-receipt.json")
                    && !line.contains("release-profile.json")
                    && !line.contains("SHA256SUMS"))
        );
    }

    #[test]
    fn profile_internal_consistency_and_production_digest_fail_closed() {
        let profile = parse_release_profile(PRODUCTION_PROFILE).expect("production profile");
        let mut mutations = Vec::new();

        let mut wrong_profile = profile.clone();
        wrong_profile.profile = "different-tag".to_owned();
        mutations.push(wrong_profile);

        let mut wrong_page = profile.clone();
        wrong_page.release.page_url = "https://example.invalid/release".to_owned();
        mutations.push(wrong_page);

        let mut wrong_proof = profile.clone();
        wrong_proof.proof.asset_name = "other-proof.json".to_owned();
        mutations.push(wrong_proof);

        let mut reordered = profile.clone();
        reordered.transport.members.swap(0, 1);
        mutations.push(reordered);

        let mut wrong_url = profile.clone();
        wrong_url.transport.members[0].url = "https://example.invalid/member".to_owned();
        mutations.push(wrong_url);

        for mutation in mutations {
            let bytes = serde_jcs::to_vec(&mutation).expect("canonical mutated profile");
            assert!(parse_release_profile(&bytes).is_err());
        }

        let mut changed_profile = PRODUCTION_PROFILE.to_vec();
        changed_profile[100] ^= 1;
        assert!(validate_production_contract_bytes(PRODUCTION_RECEIPT, &changed_profile).is_err());
        let mut changed_receipt = PRODUCTION_RECEIPT.to_vec();
        changed_receipt[100] ^= 1;
        assert!(validate_production_contract_bytes(&changed_receipt, PRODUCTION_PROFILE).is_err());
    }

    #[test]
    fn receipt_and_profile_reject_duplicates_extensions_and_noncanonical_bytes() {
        let receipt = String::from_utf8(PRODUCTION_RECEIPT.to_vec()).expect("UTF-8 receipt");
        let duplicate = receipt.replacen(
            "{\"bundle\":",
            "{\"schema\":\"pangopup.proof-receipt.v1\",\"bundle\":",
            1,
        );
        assert!(parse_proof_receipt(duplicate.as_bytes()).is_err());
        let mut extended: serde_json::Value =
            serde_json::from_slice(PRODUCTION_RECEIPT).expect("receipt value");
        extended["future"] = serde_json::Value::Bool(true);
        let bytes = serde_jcs::to_vec(&extended).expect("canonical extended receipt");
        let mut bytes_with_lf = bytes;
        bytes_with_lf.push(b'\n');
        assert!(parse_proof_receipt(&bytes_with_lf).is_err());

        assert!(parse_proof_receipt(&PRODUCTION_RECEIPT[..PRODUCTION_RECEIPT.len() - 1]).is_err());
        let mut crlf = PRODUCTION_RECEIPT[..PRODUCTION_RECEIPT.len() - 1].to_vec();
        crlf.extend_from_slice(b"\r\n");
        assert!(parse_proof_receipt(&crlf).is_err());
        let mut two_lf = PRODUCTION_RECEIPT.to_vec();
        two_lf.push(b'\n');
        assert!(parse_proof_receipt(&two_lf).is_err());
        let mut noncanonical = PRODUCTION_PROFILE.to_vec();
        noncanonical.push(b'\n');
        assert!(parse_release_profile(&noncanonical).is_err());
    }
}
