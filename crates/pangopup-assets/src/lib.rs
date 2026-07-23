//! Deterministic, bounded delivery of Pangopup's certified SNV bundle.

use pangopup_index::{
    BundleManifest, BundleOpen, IndexError, IndexReader, InputLocus, LogicalManifest,
    VisitAllError, parse_bundle_manifest_bytes,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, SeqAccess, Visitor},
};
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::BTreeSet,
    fmt,
    fs::{self, File},
    io::{self, BufRead, BufReader, ErrorKind, Read, Write},
    path::{Path, PathBuf},
    rc::Rc,
};

mod local;

pub use local::{
    ActiveBundle, DataPathInputs, InstallOutcome, LocalStatus, active_bundle, install_transport,
    local_status, open_active_bundle, resolve_data_root,
};

#[cfg(test)]
mod install_audit {
    use std::cell::RefCell;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Event {
        CompressedRead(usize),
        ScoreWriteComplete(usize),
        CheapOpenStart(usize),
        CheapOpenComplete(usize),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FaultPoint {
        MarkerChmod,
        MarkerSync,
        StageSync,
        StagingSync,
        CandidateChmod,
        CandidateSync,
        CandidateStageSync,
        ScoreWrite,
        ScoreChmod,
        ScoreSync,
        NoticeChmod,
        NoticeSync,
        ManifestChmod,
        ManifestSync,
        ReceiptChmod,
        ReceiptSync,
        BundleChmod,
        BundleSync,
        WrapperSync,
        PayloadSync,
        PrepublishStageSync,
        BundleRename,
        PublishedWrapperChmod,
        PublishedWrapperSync,
        BundlesSync,
        ActiveRename,
        RootSync,
        CleanupAfterCommit,
    }

    thread_local! {
        static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
        static FAULT: RefCell<Option<FaultPoint>> = const { RefCell::new(None) };
    }

    pub fn record(event: Event) {
        EVENTS.with_borrow_mut(|events| events.push(event));
    }

    pub fn take() -> Vec<Event> {
        EVENTS.take()
    }

    pub fn set_fault(point: FaultPoint) {
        FAULT.set(Some(point));
    }

    pub fn hit(point: FaultPoint) {
        if FAULT.with_borrow(|fault| *fault == Some(point)) {
            FAULT.set(None);
            panic!("simulated installer crash at {point:?}");
        }
    }

    pub fn fail(point: FaultPoint) -> bool {
        if FAULT.with_borrow(|fault| *fault == Some(point)) {
            FAULT.set(None);
            true
        } else {
            false
        }
    }
}

pub const NOTICE: &[u8] = include_bytes!("../../../NOTICE");
pub const NOTICE_SHA256: &str =
    "sha256:9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7";
pub const PART_SIZE: u64 = 1_000_000_000;
pub const MAX_PARTS: usize = 1_000;
pub const MAX_FIXED11_BYTES: u64 = 17_179_869_184;
const MAX_JSON_BYTES: u64 = 1024 * 1024;
const MAX_NOTICE_BYTES: u64 = 64 * 1024;
const MAX_SAFE_JSON_U64: u64 = 9_007_199_254_740_991;
const TRANSPORT_SCHEMA: &str = "pangopup.snv-transport.v1";
const COMPRESSION_FORMAT: &str = "zstd.frame.v1";
const MAX_ZSTD_WINDOW_LOG: u32 = 22;
const MAX_ZSTD_WINDOW_BYTES: u64 = 1 << MAX_ZSTD_WINDOW_LOG;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetErrorKind {
    InputIo,
    OutputIo,
    ManifestInvalid,
    TransportIncompatible,
    PartSetInvalid,
    TransportHashMismatch,
    CompressionInvalid,
    BundleInvalid,
    OutputConflict,
    UnsupportedPlatform,
    PathInvalid,
    PathUnavailable,
    AssetLocked,
    AssetIo,
    AssetStateInvalid,
    StagingInvalid,
    InstallConflict,
    AssetsMissing,
}

impl AssetErrorKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::InputIo => "INPUT_IO",
            Self::OutputIo => "OUTPUT_IO",
            Self::ManifestInvalid => "MANIFEST_INVALID",
            Self::TransportIncompatible => "TRANSPORT_INCOMPATIBLE",
            Self::PartSetInvalid => "PART_SET_INVALID",
            Self::TransportHashMismatch => "TRANSPORT_HASH_MISMATCH",
            Self::CompressionInvalid => "COMPRESSION_INVALID",
            Self::BundleInvalid => "BUNDLE_INVALID",
            Self::OutputConflict => "OUTPUT_CONFLICT",
            Self::UnsupportedPlatform => "UNSUPPORTED_PLATFORM",
            Self::PathInvalid => "PATH_INVALID",
            Self::PathUnavailable => "PATH_UNAVAILABLE",
            Self::AssetLocked => "ASSET_LOCKED",
            Self::AssetIo => "ASSET_IO",
            Self::AssetStateInvalid => "ASSET_STATE_INVALID",
            Self::StagingInvalid => "STAGING_INVALID",
            Self::InstallConflict => "INSTALL_CONFLICT",
            Self::AssetsMissing => "ASSETS_MISSING",
        }
    }
}

#[derive(Debug)]
pub struct AssetError {
    kind: AssetErrorKind,
    legacy_code: Option<&'static str>,
    message: String,
}

impl AssetError {
    pub fn new(kind: AssetErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            legacy_code: None,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> AssetErrorKind {
        self.kind
    }

    pub fn legacy_build_code(&self) -> Option<&'static str> {
        self.legacy_code
    }
}

impl fmt::Display for AssetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AssetError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleCertification {
    pub bundle_id: String,
    pub members_verified: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PackOutcome {
    pub status: &'static str,
    pub transport_id: String,
    pub bundle_id: String,
    pub part_count: usize,
    pub compressed_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct VerifyTransportOutcome {
    pub status: &'static str,
    pub transport_id: String,
    pub bundle_id: String,
    pub part_count: usize,
    pub compressed_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct UnpackOutcome {
    pub status: &'static str,
    pub transport_id: String,
    pub bundle_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransportManifest {
    schema: String,
    transport_id: String,
    bundle: TransportBundle,
    compression: CompressionManifest,
    payload: PayloadManifest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransportBundle {
    bundle_id: String,
    manifest: FileDescriptor,
    notice: FileDescriptor,
    scores: ScoreDescriptor,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileDescriptor {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScoreDescriptor {
    installed_path: String,
    size: u64,
    sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompressionManifest {
    format: String,
    level: i32,
    checksum: bool,
    content_size: bool,
    dictionary: bool,
    workers: u32,
    encoder_crate: String,
    libzstd_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadManifest {
    compressed_size: u64,
    compressed_sha256: String,
    part_size: u64,
    parts: Vec<PartDescriptor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartDescriptor {
    ordinal: u16,
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Serialize)]
struct UnsignedTransport<'a> {
    schema: &'a str,
    bundle: &'a TransportBundle,
    compression: &'a CompressionManifest,
    payload: &'a PayloadManifest,
}

#[derive(Deserialize)]
struct TransportDiscriminator {
    schema: String,
    compression: CompressionDiscriminator,
}

#[derive(Deserialize)]
struct CompressionDiscriminator {
    format: String,
}

/// Exhaustively certify an installed three-file bundle.
pub fn certify_bundle(path: &Path) -> Result<BundleCertification, AssetError> {
    preflight_bundle_files(path)?;
    let opened = BundleOpen::open(path).map_err(|error| match error {
        IndexError::Io(_) => AssetError {
            kind: AssetErrorKind::InputIo,
            legacy_code: Some("BUNDLE_INVALID"),
            message: error.to_string(),
        },
        _ => bundle_error(error.to_string()),
    })?;
    let notice_member = inner_member(opened.manifest(), "NOTICE")?;
    let scores_member = inner_member(opened.manifest(), "scores.pgi")?;
    if notice_member.size > MAX_NOTICE_BYTES || notice_member.size != NOTICE.len() as u64 {
        return Err(bundle_error_code(
            "BUNDLE_NOTICE",
            "NOTICE exceeds or differs from the exact fixed-v1 notice size",
        ));
    }
    if scores_member.size > MAX_FIXED11_BYTES {
        return Err(bundle_error_code(
            "BUNDLE_INDEX",
            "scores.pgi exceeds the fixed-v1 certification ceiling",
        ));
    }
    for member in &opened.manifest().members {
        let actual = hash_bundle_member(&path.join(&member.path))?;
        if actual != member.sha256 {
            return Err(bundle_error_code(
                "BUNDLE_MEMBER_HASH",
                format!("bundle member {} has the wrong SHA-256", member.path),
            ));
        }
    }
    let notice = read_bounded(
        &path.join("NOTICE"),
        MAX_NOTICE_BYTES,
        AssetErrorKind::InputIo,
        AssetErrorKind::BundleInvalid,
    )
    .map_err(with_legacy_io)?;
    if notice != NOTICE {
        return Err(bundle_error_code(
            "BUNDLE_NOTICE",
            "NOTICE does not match Pangopup's byte-exact embedded notice",
        ));
    }
    opened
        .index()
        .verify_canonical_structure()
        .map_err(|error| bundle_error_code("BUNDLE_INDEX", error.to_string()))?;
    let decoded = decode_reader(opened.index())?;
    if decoded.logical != opened.manifest().logical_decoded
        || opened.manifest().logical_source != opened.manifest().logical_decoded
    {
        return Err(bundle_error_code(
            "BUNDLE_LOGICAL_MISMATCH",
            "complete decoded logical stream does not match the manifest",
        ));
    }
    validate_decoded_counts(opened.manifest(), opened.index(), &decoded)?;
    Ok(BundleCertification {
        bundle_id: opened.bundle_id().to_owned(),
        members_verified: 2,
    })
}

fn preflight_bundle_files(path: &Path) -> Result<(), AssetError> {
    let expected = BTreeSet::from([
        "NOTICE".to_owned(),
        "manifest.json".to_owned(),
        "scores.pgi".to_owned(),
    ]);
    let mut actual = BTreeSet::new();
    for (count, entry) in fs::read_dir(path)
        .map_err(|error| bundle_input_io("read bundle directory", error))?
        .enumerate()
    {
        if count >= 3 {
            return Err(bundle_error_code(
                "BUNDLE_INVALID",
                "bundle contains more than three entries",
            ));
        }
        let entry = entry.map_err(|error| bundle_input_io("read bundle entry", error))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| bundle_error("bundle member name is not UTF-8"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| bundle_input_io("inspect bundle member", error))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(bundle_error("bundle members must be regular files"));
        }
        let limit = match name.as_str() {
            "manifest.json" => MAX_JSON_BYTES,
            "NOTICE" => MAX_NOTICE_BYTES,
            "scores.pgi" => MAX_FIXED11_BYTES,
            _ => {
                return Err(bundle_error_code(
                    "BUNDLE_INVALID",
                    "bundle member set mismatch",
                ));
            }
        };
        if metadata.len() > limit {
            let code = if name == "NOTICE" {
                "BUNDLE_NOTICE"
            } else if name == "scores.pgi" {
                "BUNDLE_INDEX"
            } else {
                "BUNDLE_INVALID"
            };
            return Err(bundle_error_code(code, "bundle member exceeds size limit"));
        }
        actual.insert(name);
    }
    if actual != expected {
        return Err(bundle_error_code(
            "BUNDLE_INVALID",
            "bundle member set mismatch",
        ));
    }
    Ok(())
}

pub fn pack_bundle(bundle: &Path, output: &Path) -> Result<PackOutcome, AssetError> {
    require_linux()?;
    let certification = certify_bundle(bundle)?;
    let manifest_bytes = read_bounded(
        &bundle.join("manifest.json"),
        MAX_JSON_BYTES,
        AssetErrorKind::InputIo,
        AssetErrorKind::BundleInvalid,
    )?;
    let inner = parse_bundle_manifest_bytes(&manifest_bytes)
        .map_err(|error| bundle_error(error.to_string()))?;
    let notice = read_bounded(
        &bundle.join("NOTICE"),
        MAX_NOTICE_BYTES,
        AssetErrorKind::InputIo,
        AssetErrorKind::BundleInvalid,
    )?;
    if sha256(&manifest_bytes) != certification.bundle_id {
        return Err(bundle_error(
            "certified manifest.json changed before packing",
        ));
    }
    if notice != NOTICE {
        return Err(bundle_error("certified NOTICE changed before packing"));
    }
    let scores_member = inner_member(&inner, "scores.pgi")?;
    let notice_member = inner_member(&inner, "NOTICE")?;
    if notice_member.size != notice.len() as u64 || notice_member.sha256 != sha256(&notice) {
        return Err(bundle_error(
            "reread NOTICE does not match the certified manifest",
        ));
    }
    if scores_member.size > MAX_FIXED11_BYTES {
        return Err(bundle_error(
            "fixed-v1 score member exceeds the transport ceiling",
        ));
    }
    let scores_path = bundle.join("scores.pgi");
    let (mut input, input_metadata) = open_regular(
        &scores_path,
        AssetErrorKind::InputIo,
        AssetErrorKind::BundleInvalid,
    )?;
    if input_metadata.len() != scores_member.size {
        return Err(bundle_error(
            "certified scores.pgi changed size before packing",
        ));
    }
    ensure_output_absent(output)?;
    let (stage, mut guard) = create_stage(output)?;
    let result = (|| {
        write_synced(&stage.join("bundle-manifest.json"), &manifest_bytes)?;
        write_synced(&stage.join("NOTICE"), &notice)?;
        let split = SplitWriter::new(&stage, PART_SIZE)?;
        let mut encoder = production_encoder(split, scores_member.size)?;
        stream_exact_member(
            &mut input,
            &mut encoder,
            scores_member.size,
            &scores_member.sha256,
        )?;
        let split = finish_encoder(encoder)?;
        let payload = split.finish()?;
        let transport_bundle = TransportBundle {
            bundle_id: certification.bundle_id.clone(),
            manifest: descriptor("bundle-manifest.json", &manifest_bytes),
            notice: descriptor("NOTICE", &notice),
            scores: ScoreDescriptor {
                installed_path: "scores.pgi".to_owned(),
                size: scores_member.size,
                sha256: scores_member.sha256.clone(),
            },
        };
        let compression = expected_compression();
        let unsigned = UnsignedTransport {
            schema: TRANSPORT_SCHEMA,
            bundle: &transport_bundle,
            compression: &compression,
            payload: &payload,
        };
        let unsigned_bytes = serde_jcs::to_vec(&unsigned)
            .map_err(|_| manifest_error("serialize transport identity"))?;
        let transport_id = sha256(&unsigned_bytes);
        let manifest = TransportManifest {
            schema: TRANSPORT_SCHEMA.to_owned(),
            transport_id: transport_id.clone(),
            bundle: transport_bundle,
            compression,
            payload,
        };
        let bytes = serde_jcs::to_vec(&manifest)
            .map_err(|_| manifest_error("serialize transport manifest"))?;
        write_synced(&stage.join("transport.json"), &bytes)?;
        sync_directory(&stage)?;
        publish_stage(&stage, output, &mut guard)?;
        Ok(PackOutcome {
            status: "packed",
            transport_id,
            bundle_id: certification.bundle_id,
            part_count: manifest.payload.parts.len(),
            compressed_bytes: manifest.payload.compressed_size,
        })
    })();
    finish_staged(result, &mut guard)
}

pub fn verify_transport(path: &Path) -> Result<VerifyTransportOutcome, AssetError> {
    let verified = verify_internal(path, None)?;
    Ok(VerifyTransportOutcome {
        status: "verified",
        transport_id: verified.manifest.transport_id,
        bundle_id: verified.manifest.bundle.bundle_id,
        part_count: verified.manifest.payload.parts.len(),
        compressed_bytes: verified.manifest.payload.compressed_size,
    })
}

pub fn unpack_transport(path: &Path, output: &Path) -> Result<UnpackOutcome, AssetError> {
    require_linux()?;
    ensure_output_absent(output)?;
    let (stage, mut guard) = create_stage(output)?;
    let result = (|| {
        let score_path = stage.join("scores.pgi");
        let mut score_file = File::create(&score_path)
            .map_err(|error| output_io("create reconstructed scores.pgi", error))?;
        let verified = verify_internal(path, Some(&mut score_file))?;
        score_file
            .sync_all()
            .map_err(|error| output_io("sync reconstructed scores.pgi", error))?;
        write_synced(
            &stage.join("manifest.json"),
            &verified.bundle_manifest_bytes,
        )?;
        write_synced(&stage.join("NOTICE"), &verified.notice)?;
        sync_directory(&stage)?;
        let certified = certify_bundle(&stage)?;
        if certified.bundle_id != verified.manifest.bundle.bundle_id {
            return Err(bundle_error("reconstructed bundle identity changed"));
        }
        publish_stage(&stage, output, &mut guard)?;
        Ok(UnpackOutcome {
            status: "unpacked",
            transport_id: verified.manifest.transport_id,
            bundle_id: certified.bundle_id,
        })
    })();
    finish_staged(result, &mut guard)
}

struct VerifiedTransport {
    manifest: TransportManifest,
    bundle_manifest_bytes: Vec<u8>,
    notice: Vec<u8>,
}

fn verify_internal(
    path: &Path,
    reconstructed: Option<&mut File>,
) -> Result<VerifiedTransport, AssetError> {
    let verified = inspect_transport(path)?;
    decode_parts(
        path,
        &verified.manifest,
        reconstructed.map(|file| file as &mut dyn Write),
    )?;
    Ok(verified)
}

fn inspect_transport(path: &Path) -> Result<VerifiedTransport, AssetError> {
    let manifest_bytes = read_bounded(
        &path.join("transport.json"),
        MAX_JSON_BYTES,
        AssetErrorKind::InputIo,
        AssetErrorKind::ManifestInvalid,
    )?;
    let manifest = parse_transport_manifest(&manifest_bytes)?;
    validate_directory(path, &manifest)?;
    let bundle_manifest_bytes = read_bounded(
        &path.join("bundle-manifest.json"),
        MAX_JSON_BYTES,
        AssetErrorKind::InputIo,
        AssetErrorKind::BundleInvalid,
    )?;
    let notice = read_bounded(
        &path.join("NOTICE"),
        MAX_NOTICE_BYTES,
        AssetErrorKind::InputIo,
        AssetErrorKind::BundleInvalid,
    )?;
    validate_bundle_metadata(&manifest, &bundle_manifest_bytes, &notice)?;
    Ok(VerifiedTransport {
        manifest,
        bundle_manifest_bytes,
        notice,
    })
}

fn parse_transport_manifest(bytes: &[u8]) -> Result<TransportManifest, AssetError> {
    reject_duplicate_json(bytes)?;
    let discriminator: TransportDiscriminator = serde_json::from_slice(bytes)
        .map_err(|_| manifest_error("transport manifest is not valid JSON"))?;
    if discriminator.schema != TRANSPORT_SCHEMA
        || discriminator.compression.format != COMPRESSION_FORMAT
    {
        return Err(AssetError::new(
            AssetErrorKind::TransportIncompatible,
            "unsupported transport or compression version",
        ));
    }
    let manifest: TransportManifest = serde_json::from_slice(bytes)
        .map_err(|_| manifest_error("transport manifest is not closed v1 JSON"))?;
    if serde_jcs::to_vec(&manifest).map_err(|_| manifest_error("canonicalize manifest"))? != bytes {
        return Err(manifest_error(
            "transport manifest is not canonical RFC 8785 JSON",
        ));
    }
    validate_transport_manifest(&manifest)?;
    let unsigned = UnsignedTransport {
        schema: &manifest.schema,
        bundle: &manifest.bundle,
        compression: &manifest.compression,
        payload: &manifest.payload,
    };
    let identity =
        sha256(&serde_jcs::to_vec(&unsigned).map_err(|_| manifest_error("transport identity"))?);
    if identity != manifest.transport_id {
        return Err(manifest_error(
            "transport identity does not match canonical content",
        ));
    }
    Ok(manifest)
}

fn validate_transport_manifest(manifest: &TransportManifest) -> Result<(), AssetError> {
    if manifest.schema != TRANSPORT_SCHEMA || manifest.compression != expected_compression() {
        return Err(manifest_error("unsupported fixed-v1 transport values"));
    }
    for hash in [
        &manifest.transport_id,
        &manifest.bundle.bundle_id,
        &manifest.bundle.manifest.sha256,
        &manifest.bundle.notice.sha256,
        &manifest.bundle.scores.sha256,
        &manifest.payload.compressed_sha256,
    ] {
        if !valid_sha256(hash) {
            return Err(manifest_error("invalid SHA-256 spelling"));
        }
    }
    if manifest.bundle.manifest.path != "bundle-manifest.json"
        || manifest.bundle.notice.path != "NOTICE"
        || manifest.bundle.scores.installed_path != "scores.pgi"
        || manifest.bundle.scores.size > MAX_FIXED11_BYTES
        || manifest.bundle.notice.size != NOTICE.len() as u64
        || manifest.bundle.notice.sha256 != NOTICE_SHA256
        || manifest.payload.part_size != PART_SIZE
    {
        return Err(manifest_error("invalid fixed-v1 transport fields"));
    }
    if manifest.payload.parts.is_empty() || manifest.payload.parts.len() > MAX_PARTS {
        return Err(part_error("invalid payload part count"));
    }
    let values = [
        manifest.bundle.manifest.size,
        manifest.bundle.notice.size,
        manifest.bundle.scores.size,
        manifest.payload.compressed_size,
        manifest.payload.part_size,
    ];
    if values.into_iter().any(|value| value > MAX_SAFE_JSON_U64) {
        return Err(manifest_error("integer exceeds JSON safe range"));
    }
    let mut total = 0_u64;
    for (position, part) in manifest.payload.parts.iter().enumerate() {
        let ordinal = u16::try_from(position).map_err(|_| manifest_error("part ordinal"))?;
        let expected = format!("payload.pgi.zst.part{ordinal:04}");
        let final_part = position + 1 == manifest.payload.parts.len();
        if !valid_sha256(&part.sha256) || part.size > MAX_SAFE_JSON_U64 {
            return Err(manifest_error("invalid part hash or integer"));
        }
        if part.ordinal != ordinal
            || part.path != expected
            || part.size == 0
            || part.size > PART_SIZE
            || (!final_part && part.size != PART_SIZE)
        {
            return Err(part_error("invalid payload part descriptor"));
        }
        total = total
            .checked_add(part.size)
            .ok_or_else(|| manifest_error("part size overflow"))?;
    }
    if total != manifest.payload.compressed_size {
        return Err(part_error("part sizes do not equal compressed size"));
    }
    Ok(())
}

fn validate_bundle_metadata(
    outer: &TransportManifest,
    manifest_bytes: &[u8],
    notice: &[u8],
) -> Result<(), AssetError> {
    let manifest_hash = sha256(manifest_bytes);
    if outer.bundle.bundle_id != manifest_hash
        || outer.bundle.manifest.sha256 != manifest_hash
        || outer.bundle.manifest.size != manifest_bytes.len() as u64
    {
        return Err(AssetError::new(
            AssetErrorKind::TransportHashMismatch,
            "copied bundle manifest identity mismatch",
        ));
    }
    let inner = parse_bundle_manifest_bytes(manifest_bytes)
        .map_err(|error| bundle_error(error.to_string()))?;
    let notice_member = inner_member(&inner, "NOTICE")?;
    let scores_member = inner_member(&inner, "scores.pgi")?;
    if sha256(notice) != outer.bundle.notice.sha256
        || outer.bundle.notice.size != notice.len() as u64
    {
        return Err(AssetError::new(
            AssetErrorKind::TransportHashMismatch,
            "copied notice identity mismatch",
        ));
    }
    if notice != NOTICE
        || outer.bundle.notice.size != notice_member.size
        || outer.bundle.notice.sha256 != notice_member.sha256
        || outer.bundle.scores.size != scores_member.size
        || outer.bundle.scores.sha256 != scores_member.sha256
    {
        return Err(bundle_error(
            "inner bundle members or exact CC BY notice do not match transport",
        ));
    }
    Ok(())
}

fn validate_directory(path: &Path, manifest: &TransportManifest) -> Result<(), AssetError> {
    let mut expected = BTreeSet::from([
        "NOTICE".to_owned(),
        "bundle-manifest.json".to_owned(),
        "transport.json".to_owned(),
    ]);
    expected.extend(manifest.payload.parts.iter().map(|part| part.path.clone()));
    let mut actual = BTreeSet::new();
    for (count, entry) in fs::read_dir(path)
        .map_err(|error| input_io("read transport directory", error))?
        .enumerate()
    {
        if count >= MAX_PARTS + 3 {
            return Err(part_error("transport has too many directory entries"));
        }
        let entry = entry.map_err(|error| input_io("read transport entry", error))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| part_error("transport entry name is not UTF-8"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| input_io("inspect transport entry", error))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(part_error("transport entries must be regular files"));
        }
        actual.insert(name);
    }
    if actual != expected {
        return Err(part_error("transport directory member set mismatch"));
    }
    Ok(())
}

fn decode_parts(
    path: &Path,
    manifest: &TransportManifest,
    output: Option<&mut dyn Write>,
) -> Result<(), AssetError> {
    let failure = Rc::new(RefCell::new(None));
    let reader = PartReader::new(path, &manifest.payload, Rc::clone(&failure));
    let mut buffered = BufReader::with_capacity(128 * 1024, reader);
    let header = buffered.fill_buf().map_err(|error| {
        take_part_failure(&failure).unwrap_or_else(|| input_io("read Zstandard header", error))
    })?;
    if let Err(header_error) = validate_frame_header(header, manifest.bundle.scores.size) {
        drain_compressed(&mut buffered, &failure)?;
        return Err(header_error);
    }
    let mut decoder = zstd::stream::read::Decoder::with_buffer(buffered)
        .map_err(|error| compression_error(error.to_string()))?;
    decoder
        .window_log_max(MAX_ZSTD_WINDOW_LOG)
        .map_err(|error| compression_error(error.to_string()))?;
    let mut decoder = decoder.single_frame();
    let mut hash = Sha256::new();
    let decoded_result =
        copy_decoded_limited(&mut decoder, manifest.bundle.scores.size, &mut hash, output);
    if let Some(part_error) = take_part_failure(&failure) {
        return Err(part_error);
    }
    if decoded_result
        .as_ref()
        .is_err_and(|error| error.kind() == AssetErrorKind::OutputIo)
    {
        return decoded_result.map(|_| ());
    }
    let mut remaining = decoder.finish();
    let trailing = drain_compressed(&mut remaining, &failure)?;
    let decoded = decoded_result?;
    if trailing {
        return Err(compression_error(
            "second frame or trailing compressed bytes",
        ));
    }
    if decoded != manifest.bundle.scores.size
        || format!("sha256:{:x}", hash.finalize()) != manifest.bundle.scores.sha256
    {
        return Err(AssetError::new(
            AssetErrorKind::TransportHashMismatch,
            "decompressed score identity mismatch",
        ));
    }
    Ok(())
}

fn drain_compressed(
    reader: &mut BufReader<PartReader<'_>>,
    failure: &Rc<RefCell<Option<AssetError>>>,
) -> Result<bool, AssetError> {
    let mut consumed_any = false;
    loop {
        let available = reader.fill_buf().map_err(|error| {
            take_part_failure(failure).unwrap_or_else(|| input_io("read payload part", error))
        })?;
        if available.is_empty() {
            break;
        }
        consumed_any = true;
        let length = available.len();
        reader.consume(length);
    }
    reader
        .get_mut()
        .finish_validation()
        .map_err(|error| take_part_failure(failure).unwrap_or(error))?;
    if let Some(error) = take_part_failure(failure) {
        return Err(error);
    }
    Ok(consumed_any)
}

fn copy_decoded_limited(
    input: &mut impl Read,
    expected_size: u64,
    hash: &mut Sha256,
    mut output: Option<&mut dyn Write>,
) -> Result<u64, AssetError> {
    let mut decoded = 0_u64;
    let mut buffer = vec![0_u8; 128 * 1024];
    loop {
        let remaining = expected_size.saturating_sub(decoded);
        let limit = usize::try_from((remaining + 1).min(buffer.len() as u64))
            .map_err(|_| compression_error("decompressed read bound"))?;
        let read = input
            .read(&mut buffer[..limit])
            .map_err(|error| compression_error(error.to_string()))?;
        if read == 0 {
            break;
        }
        decoded = decoded
            .checked_add(read as u64)
            .ok_or_else(|| compression_error("decompressed size overflow"))?;
        if decoded > expected_size {
            return Err(compression_error(
                "decompressed payload exceeds declared size",
            ));
        }
        hash.update(&buffer[..read]);
        if let Some(file) = output.as_deref_mut() {
            file.write_all(&buffer[..read])
                .map_err(|error| output_io("write reconstructed scores.pgi", error))?;
        }
    }
    Ok(decoded)
}

fn validate_frame_header(bytes: &[u8], expected_size: u64) -> Result<(), AssetError> {
    if bytes.len() < 6 || bytes[0..4] != [0x28, 0xb5, 0x2f, 0xfd] {
        return Err(compression_error(
            "payload is not a standard Zstandard frame",
        ));
    }
    let descriptor = bytes[4];
    if descriptor & 0x18 != 0 || descriptor & 0x04 == 0 || descriptor & 0x03 != 0 {
        return Err(compression_error(
            "frame must use checksum, no dictionary, and no reserved bits",
        ));
    }
    let single_segment = descriptor & 0x20 != 0;
    let fcs_flag = descriptor >> 6;
    let window_bytes = usize::from(!single_segment);
    if !single_segment {
        let Some(window_descriptor) = bytes.get(5).copied() else {
            return Err(compression_error("truncated frame window descriptor"));
        };
        let exponent = u32::from(window_descriptor >> 3);
        let mantissa = u64::from(window_descriptor & 7);
        let base = 1_u64
            .checked_shl(10 + exponent)
            .ok_or_else(|| compression_error("frame window size overflow"))?;
        let window_size = base
            .checked_add((base / 8) * mantissa)
            .ok_or_else(|| compression_error("frame window size overflow"))?;
        if window_size > MAX_ZSTD_WINDOW_BYTES {
            return Err(compression_error("frame window exceeds fixed-v1 limit"));
        }
    }
    let fcs_bytes = match (fcs_flag, single_segment) {
        (0, true) => 1,
        (0, false) => 0,
        (1, _) => 2,
        (2, _) => 4,
        (3, _) => 8,
        _ => return Err(compression_error("invalid frame content-size flag")),
    };
    let offset = 5 + window_bytes;
    if fcs_bytes == 0 || bytes.len() < offset + fcs_bytes {
        return Err(compression_error(
            "frame does not pledge a complete content size",
        ));
    }
    let mut encoded = [0_u8; 8];
    encoded[..fcs_bytes].copy_from_slice(&bytes[offset..offset + fcs_bytes]);
    let mut content_size = u64::from_le_bytes(encoded);
    if fcs_bytes == 2 {
        content_size += 256;
    }
    if content_size != expected_size {
        return Err(compression_error("frame pledged content size mismatch"));
    }
    if single_segment && content_size > MAX_ZSTD_WINDOW_BYTES {
        return Err(compression_error("frame window exceeds fixed-v1 limit"));
    }
    Ok(())
}

fn production_encoder<W: Write>(
    writer: W,
    pledged_size: u64,
) -> Result<zstd::stream::write::Encoder<'static, W>, AssetError> {
    if zstd_safe::version_string() != "1.5.7" {
        return Err(compression_error(format!(
            "linked libzstd is {}, expected 1.5.7",
            zstd_safe::version_string()
        )));
    }
    let mut encoder = zstd::stream::write::Encoder::new(writer, 9)
        .map_err(|error| compression_error(error.to_string()))?;
    encoder
        .include_checksum(true)
        .and_then(|_| encoder.include_contentsize(true))
        .and_then(|_| encoder.include_dictid(false))
        .and_then(|_| encoder.long_distance_matching(false))
        .and_then(|_| encoder.set_parameter(zstd_safe::CParameter::NbWorkers(0)))
        .and_then(|_| encoder.set_pledged_src_size(Some(pledged_size)))
        .map_err(|error| compression_error(error.to_string()))?;
    Ok(encoder)
}

fn finish_encoder<W: Write>(
    encoder: zstd::stream::write::Encoder<'static, W>,
) -> Result<W, AssetError> {
    encoder
        .finish()
        .map_err(|error| output_io("finish compressed scores.pgi", error))
}

struct SplitWriter {
    directory: PathBuf,
    part_size: u64,
    current: Option<File>,
    current_hash: Sha256,
    current_size: u64,
    whole_hash: Sha256,
    total: u64,
    parts: Vec<PartDescriptor>,
}

impl SplitWriter {
    fn new(directory: &Path, part_size: u64) -> Result<Self, AssetError> {
        if part_size == 0 || part_size > PART_SIZE {
            return Err(manifest_error("invalid split threshold"));
        }
        Ok(Self {
            directory: directory.to_owned(),
            part_size,
            current: None,
            current_hash: Sha256::new(),
            current_size: 0,
            whole_hash: Sha256::new(),
            total: 0,
            parts: Vec::new(),
        })
    }

    fn open_part(&mut self) -> io::Result<()> {
        let ordinal = self.parts.len();
        let path = format!("payload.pgi.zst.part{ordinal:04}");
        self.current = Some(File::create(self.directory.join(path))?);
        Ok(())
    }

    fn finish_part(&mut self) -> io::Result<()> {
        let Some(file) = self.current.take() else {
            return Ok(());
        };
        file.sync_all()?;
        let ordinal = u16::try_from(self.parts.len())
            .map_err(|_| io::Error::other("too many payload parts"))?;
        self.parts.push(PartDescriptor {
            ordinal,
            path: format!("payload.pgi.zst.part{ordinal:04}"),
            size: self.current_size,
            sha256: format!(
                "sha256:{:x}",
                std::mem::replace(&mut self.current_hash, Sha256::new()).finalize()
            ),
        });
        self.current_size = 0;
        Ok(())
    }

    fn finish(mut self) -> Result<PayloadManifest, AssetError> {
        self.finish_part()
            .map_err(|error| output_io("finish payload part", error))?;
        if self.parts.is_empty() || self.parts.len() > MAX_PARTS {
            return Err(part_error("compressed payload has invalid part count"));
        }
        Ok(PayloadManifest {
            compressed_size: self.total,
            compressed_sha256: format!("sha256:{:x}", self.whole_hash.finalize()),
            part_size: PART_SIZE,
            parts: self.parts,
        })
    }
}

impl Write for SplitWriter {
    fn write(&mut self, mut bytes: &[u8]) -> io::Result<usize> {
        let original = bytes.len();
        while !bytes.is_empty() {
            if self.current.is_none() {
                self.open_part()?;
            }
            let room = usize::try_from(self.part_size - self.current_size)
                .map_err(|_| io::Error::other("part size conversion"))?;
            let take = room.min(bytes.len());
            let chunk = &bytes[..take];
            self.current
                .as_mut()
                .ok_or_else(|| io::Error::other("missing payload part"))?
                .write_all(chunk)?;
            self.current_hash.update(chunk);
            self.whole_hash.update(chunk);
            self.current_size += take as u64;
            self.total = self
                .total
                .checked_add(take as u64)
                .ok_or_else(|| io::Error::other("compressed size overflow"))?;
            bytes = &bytes[take..];
            if self.current_size == self.part_size {
                self.finish_part()?;
            }
        }
        Ok(original)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(file) = &mut self.current {
            file.flush()?;
        }
        Ok(())
    }
}

struct PartReader<'a> {
    directory: &'a Path,
    payload: &'a PayloadManifest,
    index: usize,
    current: Option<File>,
    remaining: u64,
    current_hash: Sha256,
    whole_hash: Sha256,
    total: u64,
    failure: Rc<RefCell<Option<AssetError>>>,
}

impl<'a> PartReader<'a> {
    fn new(
        directory: &'a Path,
        payload: &'a PayloadManifest,
        failure: Rc<RefCell<Option<AssetError>>>,
    ) -> Self {
        Self {
            directory,
            payload,
            index: 0,
            current: None,
            remaining: 0,
            current_hash: Sha256::new(),
            whole_hash: Sha256::new(),
            total: 0,
            failure,
        }
    }

    fn fail(&self, error: AssetError) -> io::Error {
        let message = error.to_string();
        let mut failure = self.failure.borrow_mut();
        if failure.is_none() {
            *failure = Some(error);
        }
        io::Error::other(message)
    }

    fn open_next(&mut self) -> io::Result<bool> {
        let Some(part) = self.payload.parts.get(self.index) else {
            return Ok(false);
        };
        let path = self.directory.join(&part.path);
        let (file, metadata) = open_regular(
            &path,
            AssetErrorKind::InputIo,
            AssetErrorKind::PartSetInvalid,
        )
        .map_err(|error| self.fail(error))?;
        if metadata.len() != part.size {
            return Err(self.fail(part_error("payload part changed size before opening")));
        }
        self.current = Some(file);
        self.remaining = part.size;
        self.current_hash = Sha256::new();
        Ok(true)
    }

    fn finish_current(&mut self) -> io::Result<()> {
        let Some(mut file) = self.current.take() else {
            return Ok(());
        };
        let mut extra = [0_u8; 1];
        let extra_read = file
            .read(&mut extra)
            .map_err(|error| self.fail(input_io("read payload part", error)))?;
        if extra_read != 0 {
            return Err(self.fail(part_error("payload part grew while read")));
        }
        let part = &self.payload.parts[self.index];
        let actual = format!(
            "sha256:{:x}",
            std::mem::replace(&mut self.current_hash, Sha256::new()).finalize()
        );
        self.index += 1;
        if actual != part.sha256 {
            return Err(self.fail(AssetError::new(
                AssetErrorKind::TransportHashMismatch,
                "payload part hash mismatch",
            )));
        }
        Ok(())
    }

    fn finish_validation(&mut self) -> Result<(), AssetError> {
        if self.remaining != 0 {
            return Err(part_error("payload part was not completely consumed"));
        }
        if self.current.is_some() {
            self.finish_current().map_err(|error| {
                take_part_failure(&self.failure)
                    .unwrap_or_else(|| input_io("finish payload part", error))
            })?;
        }
        if self.index != self.payload.parts.len() {
            return Err(part_error("not every payload part was consumed"));
        }
        let whole = format!(
            "sha256:{:x}",
            std::mem::replace(&mut self.whole_hash, Sha256::new()).finalize()
        );
        if self.total != self.payload.compressed_size || whole != self.payload.compressed_sha256 {
            return Err(AssetError::new(
                AssetErrorKind::TransportHashMismatch,
                "whole compressed payload hash mismatch",
            ));
        }
        Ok(())
    }
}

impl Read for PartReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        loop {
            if self.current.is_none() && !self.open_next()? {
                return Ok(0);
            }
            if self.remaining == 0 {
                self.finish_current()?;
                continue;
            }
            let take = usize::try_from(self.remaining.min(buffer.len() as u64))
                .map_err(|_| io::Error::other("part read size"))?;
            let result = self
                .current
                .as_mut()
                .ok_or_else(|| io::Error::other("missing part handle"))?
                .read(&mut buffer[..take]);
            let read = result.map_err(|error| self.fail(input_io("read payload part", error)))?;
            if read == 0 {
                return Err(self.fail(part_error("payload part was truncated")));
            }
            self.remaining -= read as u64;
            self.current_hash.update(&buffer[..read]);
            self.whole_hash.update(&buffer[..read]);
            self.total = self
                .total
                .checked_add(read as u64)
                .ok_or_else(|| self.fail(manifest_error("compressed size overflow")))?;
            #[cfg(test)]
            install_audit::record(install_audit::Event::CompressedRead(read));
            return Ok(read);
        }
    }
}

fn take_part_failure(failure: &Rc<RefCell<Option<AssetError>>>) -> Option<AssetError> {
    failure.borrow_mut().take()
}

struct DecodedFacts {
    logical: LogicalManifest,
    genes: u64,
    loci: u64,
    source_segments: u64,
    index_segments: u64,
    gaps: u64,
    omitted_bases: u64,
    n_ref_loci: u64,
    n_omit_a: u64,
    n_omit_t: u64,
}

fn decode_reader(reader: &IndexReader) -> Result<DecodedFacts, AssetError> {
    let mut hash = HashSink::new();
    let mut facts = DecodedFacts {
        logical: LogicalManifest {
            records: 0,
            sha256: String::new(),
        },
        genes: 0,
        loci: 0,
        source_segments: 0,
        index_segments: 0,
        gaps: 0,
        omitted_bases: 0,
        n_ref_loci: 0,
        n_omit_a: 0,
        n_omit_t: 0,
    };
    let mut previous: Option<(u64, u8, u32)> = None;
    let mut previous_ordinary: Option<(u64, u8, u32)> = None;
    reader
        .visit_all(|locus| {
            write_logical_text(&mut hash, locus)?;
            add(&mut facts.logical.records, 3)?;
            add(&mut facts.loci, 1)?;
            let (gene, contig, position) = match locus {
                InputLocus::Ordinary(value) => {
                    let current = (
                        value.gene.numeric(),
                        value.contig.code(),
                        value.position.get(),
                    );
                    if previous_ordinary.is_none_or(|prior| {
                        prior.0 != current.0
                            || prior.1 != current.1
                            || prior.2.checked_add(1) != Some(current.2)
                    }) {
                        add(&mut facts.index_segments, 1)?;
                    }
                    previous_ordinary = Some(current);
                    current
                }
                InputLocus::Ambiguous(value) => {
                    add(&mut facts.n_ref_loci, 1)?;
                    match value.omitted.to_string().as_str() {
                        "A" => add(&mut facts.n_omit_a, 1)?,
                        "T" => add(&mut facts.n_omit_t, 1)?,
                        _ => return Err(io::Error::other("invalid omitted exception base")),
                    }
                    (
                        value.gene.numeric(),
                        value.contig.code(),
                        value.position.get(),
                    )
                }
            };
            match previous {
                None => {
                    add(&mut facts.genes, 1)?;
                    add(&mut facts.source_segments, 1)?;
                }
                Some((prior_gene, _, _)) if prior_gene != gene => {
                    add(&mut facts.genes, 1)?;
                    add(&mut facts.source_segments, 1)?;
                }
                Some((_, prior_contig, prior_position)) => {
                    if prior_contig != contig || position <= prior_position {
                        return Err(io::Error::other("decoded logical order"));
                    }
                    let distance = u64::from(position - prior_position);
                    if distance > 1 {
                        add(&mut facts.gaps, 1)?;
                        add(&mut facts.omitted_bases, distance - 1)?;
                        add(&mut facts.source_segments, 1)?;
                    }
                }
            }
            previous = Some((gene, contig, position));
            Ok::<_, io::Error>(())
        })
        .map_err(|error| match error {
            VisitAllError::Index(error) => bundle_error_code("BUNDLE_INDEX", error.to_string()),
            VisitAllError::Visitor(error) => bundle_error(error.to_string()),
        })?;
    facts.logical.sha256 = hash.finish();
    Ok(facts)
}

fn validate_decoded_counts(
    manifest: &BundleManifest,
    reader: &IndexReader,
    decoded: &DecodedFacts,
) -> Result<(), AssetError> {
    let counts = manifest.counts;
    let directions = counts
        .ascending_members
        .checked_add(counts.descending_members)
        .ok_or_else(|| bundle_error_code("BUNDLE_COUNTS", "bundle count overflow"))?;
    let shapes = counts
        .n_omit_a
        .checked_add(counts.n_omit_t)
        .ok_or_else(|| bundle_error_code("BUNDLE_COUNTS", "bundle count overflow"))?;
    let rows = counts
        .gene_loci
        .checked_mul(3)
        .ok_or_else(|| bundle_error_code("BUNDLE_COUNTS", "bundle count overflow"))?;
    if counts.source_rows != decoded.logical.records
        || rows != counts.source_rows
        || counts.gene_loci != decoded.loci
        || counts.genes != decoded.genes
        || counts.genes != directions
        || manifest.source.observed_member_count != counts.genes
        || counts.source_segments != decoded.source_segments
        || counts.gap_transitions != decoded.gaps
        || counts.omitted_bases != decoded.omitted_bases
        || counts.index_segments != decoded.index_segments
        || decoded.index_segments != reader.segment_count()
        || counts.n_ref_loci != reader.exception_count()
        || counts.n_ref_loci != decoded.n_ref_loci
        || counts.n_omit_a != decoded.n_omit_a
        || counts.n_omit_t != decoded.n_omit_t
        || shapes != counts.n_ref_loci
    {
        return Err(bundle_error_code(
            "BUNDLE_COUNTS",
            "manifest counts do not agree with complete index decode",
        ));
    }
    Ok(())
}

struct HashSink(Sha256);

impl HashSink {
    fn new() -> Self {
        Self(Sha256::new())
    }
    fn finish(self) -> String {
        format!("sha256:{:x}", self.0.finalize())
    }
}

impl Write for HashSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn write_logical_text(output: &mut impl Write, locus: InputLocus) -> io::Result<()> {
    let (kind, gene, contig, position, reference, mut alternatives, omitted) = match locus {
        InputLocus::Ordinary(value) => (
            "O",
            value.gene,
            value.contig,
            value.position,
            value.reference.to_string(),
            value.alternatives,
            None,
        ),
        InputLocus::Ambiguous(value) => (
            "N",
            value.gene,
            value.contig,
            value.position,
            "N".to_owned(),
            value.alternatives,
            Some(value.omitted),
        ),
    };
    alternatives.sort_by_key(|value| value.alternate);
    for alternative in alternatives {
        write!(
            output,
            "{kind}\t{gene}\t{contig}\t{position}\t{reference}\t{}\t{}\t{}\t{}\t{}",
            alternative.alternate,
            alternative.score.gain().hundredths(),
            alternative.score.gain_position().get(),
            alternative.score.loss().hundredths(),
            alternative.score.loss_position().get()
        )?;
        if let Some(omitted) = omitted {
            write!(output, "\t{omitted}")?;
        }
        writeln!(output)?;
    }
    Ok(())
}

fn add(target: &mut u64, amount: u64) -> io::Result<()> {
    *target = target
        .checked_add(amount)
        .ok_or_else(|| io::Error::other("decoded count overflow"))?;
    Ok(())
}

fn expected_compression() -> CompressionManifest {
    CompressionManifest {
        format: COMPRESSION_FORMAT.to_owned(),
        level: 9,
        checksum: true,
        content_size: true,
        dictionary: false,
        workers: 0,
        encoder_crate: "zstd/0.13.3".to_owned(),
        libzstd_version: "1.5.7".to_owned(),
    }
}

fn inner_member<'a>(
    manifest: &'a BundleManifest,
    path: &str,
) -> Result<&'a pangopup_index::MemberManifest, AssetError> {
    manifest
        .members
        .iter()
        .find(|member| member.path == path)
        .ok_or_else(|| bundle_error(format!("inner manifest lacks {path}")))
}

fn descriptor(path: &str, bytes: &[u8]) -> FileDescriptor {
    FileDescriptor {
        path: path.to_owned(),
        size: bytes.len() as u64,
        sha256: sha256(bytes),
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn copy_hash(
    reader: &mut impl Read,
    hash: &mut Sha256,
    mut second: Option<&mut Sha256>,
) -> io::Result<u64> {
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; 128 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
        if let Some(other) = second.as_deref_mut() {
            other.update(&buffer[..read]);
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::other("hash size overflow"))?;
    }
    Ok(total)
}

fn stream_exact_member(
    input: &mut impl Read,
    output: &mut impl Write,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), AssetError> {
    let mut buffer = vec![0_u8; 128 * 1024];
    let mut total = 0_u64;
    let mut hash = Sha256::new();
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| input_io("read scores.pgi", error))?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| bundle_error("scores.pgi size overflow while packing"))?;
        if total > expected_size {
            return Err(bundle_error("certified scores.pgi grew before packing"));
        }
        hash.update(&buffer[..read]);
        output
            .write_all(&buffer[..read])
            .map_err(|error| output_io("write compressed scores.pgi", error))?;
    }
    if total != expected_size || format!("sha256:{:x}", hash.finalize()) != expected_sha256 {
        return Err(bundle_error(
            "certified scores.pgi identity changed before packing",
        ));
    }
    Ok(())
}

fn hash_bundle_member(path: &Path) -> Result<String, AssetError> {
    let (mut file, _) = open_regular(path, AssetErrorKind::InputIo, AssetErrorKind::BundleInvalid)
        .map_err(with_legacy_io)?;
    let mut hash = Sha256::new();
    copy_hash(&mut file, &mut hash, None).map_err(|error| AssetError {
        kind: AssetErrorKind::InputIo,
        legacy_code: Some("IO"),
        message: error.to_string(),
    })?;
    Ok(format!("sha256:{:x}", hash.finalize()))
}

fn read_bounded(
    path: &Path,
    cap: u64,
    io_kind: AssetErrorKind,
    invalid_kind: AssetErrorKind,
) -> Result<Vec<u8>, AssetError> {
    let (file, metadata) = open_regular(path, io_kind, invalid_kind)?;
    if metadata.len() > cap {
        return Err(AssetError::new(
            invalid_kind,
            "bounded input exceeds size limit",
        ));
    }
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| AssetError::new(invalid_kind, "bounded input size conversion"))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| AssetError::new(io_kind, error.to_string()))?;
    if bytes.len() as u64 > cap {
        return Err(AssetError::new(
            invalid_kind,
            "bounded input grew beyond size limit",
        ));
    }
    Ok(bytes)
}

fn open_regular(
    path: &Path,
    io_kind: AssetErrorKind,
    invalid_kind: AssetErrorKind,
) -> Result<(File, fs::Metadata), AssetError> {
    let before = fs::symlink_metadata(path).map_err(|error| {
        AssetError::new(io_kind, format!("inspect {}: {error}", path.display()))
    })?;
    if before.file_type().is_symlink() || !before.file_type().is_file() {
        return Err(AssetError::new(
            invalid_kind,
            "required input is not a regular file",
        ));
    }
    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
    };
    #[cfg(not(unix))]
    let file = File::open(path);
    let file = file.map_err(|error| AssetError::new(io_kind, error.to_string()))?;
    let metadata = file
        .metadata()
        .map_err(|error| AssetError::new(io_kind, error.to_string()))?;
    if !metadata.file_type().is_file() {
        return Err(AssetError::new(
            invalid_kind,
            "opened input is not a regular file",
        ));
    }
    Ok((file, metadata))
}

fn reject_duplicate_json(bytes: &[u8]) -> Result<(), AssetError> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    TransportNoDuplicateJson::deserialize(&mut decoder)
        .map_err(|_| manifest_error("transport manifest contains invalid or duplicate JSON"))?;
    decoder
        .end()
        .map_err(|_| manifest_error("transport manifest contains trailing JSON"))?;
    Ok(())
}

struct TransportNoDuplicateJson;

impl<'de> Deserialize<'de> for TransportNoDuplicateJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(TransportNoDuplicateVisitor)
    }
}

struct TransportNoDuplicateVisitor;

impl<'de> Visitor<'de> for TransportNoDuplicateVisitor {
    type Value = TransportNoDuplicateJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(serde::de::Error::custom("duplicate object key"));
            }
            map.next_value::<TransportNoDuplicateJson>()?;
        }
        Ok(TransportNoDuplicateJson)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence
            .next_element::<TransportNoDuplicateJson>()?
            .is_some()
        {}
        Ok(TransportNoDuplicateJson)
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(TransportNoDuplicateJson)
    }
    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(TransportNoDuplicateJson)
    }
    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(TransportNoDuplicateJson)
    }
    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(TransportNoDuplicateJson)
    }
    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(TransportNoDuplicateJson)
    }
    fn visit_borrowed_str<E>(self, _value: &'de str) -> Result<Self::Value, E> {
        Ok(TransportNoDuplicateJson)
    }
    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(TransportNoDuplicateJson)
    }
    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(TransportNoDuplicateJson)
    }
    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(TransportNoDuplicateJson)
    }
    fn visit_some<D>(self, decoder: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        TransportNoDuplicateJson::deserialize(decoder)
    }
    fn visit_newtype_struct<D>(self, decoder: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        TransportNoDuplicateJson::deserialize(decoder)
    }
}

struct StageGuard {
    path: PathBuf,
    armed: bool,
}

impl StageGuard {
    fn published(&mut self) {
        self.armed = false;
    }
    fn cleanup(&mut self) -> Result<(), AssetError> {
        if !self.armed {
            return Ok(());
        }
        self.armed = false;
        fs::remove_dir_all(&self.path)
            .map_err(|error| output_io("remove invocation staging", error))
    }
}

impl Drop for StageGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn create_stage(output: &Path) -> Result<(PathBuf, StageGuard), AssetError> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| output_io("create output parent", error))?;
    let name = output
        .file_name()
        .ok_or_else(|| output_io("invalid output name", io::Error::other("missing name")))?
        .to_string_lossy();
    for _ in 0..32 {
        let mut random = [0_u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut random))
            .map_err(|error| output_io("obtain staging randomness", error))?;
        let suffix = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = parent.join(format!(".{name}.pangopup-stage-{suffix}"));
        match fs::create_dir(&path) {
            Ok(()) => {
                return Ok((path.clone(), StageGuard { path, armed: true }));
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(output_io("create output staging", error)),
        }
    }
    Err(output_io(
        "create unique output staging",
        io::Error::new(ErrorKind::AlreadyExists, "random name collisions"),
    ))
}

fn ensure_output_absent(output: &Path) -> Result<(), AssetError> {
    match fs::symlink_metadata(output) {
        Ok(_) => Err(AssetError::new(
            AssetErrorKind::OutputConflict,
            "output already exists",
        )),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(output_io("inspect output destination", error)),
    }
}

fn publish_stage(stage: &Path, output: &Path, guard: &mut StageGuard) -> Result<(), AssetError> {
    #[cfg(target_os = "linux")]
    {
        rustix::fs::renameat_with(
            rustix::fs::CWD,
            stage,
            rustix::fs::CWD,
            output,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(io::Error::from)
        .map_err(|error| {
            if matches!(
                error.kind(),
                ErrorKind::AlreadyExists | ErrorKind::DirectoryNotEmpty
            ) {
                AssetError::new(
                    AssetErrorKind::OutputConflict,
                    "output publication race lost",
                )
            } else {
                output_io("publish staged output", error)
            }
        })?;
        guard.published();
        sync_directory(output.parent().unwrap_or_else(|| Path::new(".")))?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (stage, output, guard);
        Err(AssetError::new(
            AssetErrorKind::UnsupportedPlatform,
            "atomic no-replace publication requires Linux",
        ))
    }
}

fn finish_staged<T>(
    result: Result<T, AssetError>,
    guard: &mut StageGuard,
) -> Result<T, AssetError> {
    let cleanup = guard.cleanup();
    match (result, cleanup) {
        (_, Err(error)) => Err(error),
        (result, Ok(())) => result,
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), AssetError> {
    let mut file = File::create(path).map_err(|error| output_io("create output member", error))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| output_io("write output member", error))
}

fn sync_directory(path: &Path) -> Result<(), AssetError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| output_io("sync output directory", error))
}

fn require_linux() -> Result<(), AssetError> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(AssetError::new(
            AssetErrorKind::UnsupportedPlatform,
            "pack and unpack require Linux",
        ))
    }
}

fn input_io(action: &str, error: io::Error) -> AssetError {
    AssetError::new(AssetErrorKind::InputIo, format!("{action}: {error}"))
}
fn bundle_input_io(action: &str, error: io::Error) -> AssetError {
    AssetError {
        kind: AssetErrorKind::InputIo,
        legacy_code: Some("BUNDLE_INVALID"),
        message: format!("{action}: {error}"),
    }
}
fn with_legacy_io(mut error: AssetError) -> AssetError {
    if error.kind == AssetErrorKind::InputIo {
        error.legacy_code = Some("IO");
    }
    error
}
fn output_io(action: &str, error: io::Error) -> AssetError {
    AssetError::new(AssetErrorKind::OutputIo, format!("{action}: {error}"))
}
fn manifest_error(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::ManifestInvalid, message)
}
fn part_error(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::PartSetInvalid, message)
}
fn compression_error(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::CompressionInvalid, message)
}
fn bundle_error(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::BundleInvalid, message)
}

fn bundle_error_code(code: &'static str, message: impl Into<String>) -> AssetError {
    AssetError {
        kind: AssetErrorKind::BundleInvalid,
        legacy_code: Some(code),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct ToggleFailWriter {
        fail: bool,
        bytes: Vec<u8>,
    }

    impl Write for ToggleFailWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail {
                return Err(io::Error::other("injected output failure"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn signed_manifest() -> TransportManifest {
        let bundle = TransportBundle {
            bundle_id: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
            manifest: FileDescriptor {
                path: "bundle-manifest.json".to_owned(),
                size: 10,
                sha256: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
            },
            notice: FileDescriptor {
                path: "NOTICE".to_owned(),
                size: NOTICE.len() as u64,
                sha256: NOTICE_SHA256.to_owned(),
            },
            scores: ScoreDescriptor {
                installed_path: "scores.pgi".to_owned(),
                size: 100,
                sha256: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_owned(),
            },
        };
        let compression = expected_compression();
        let payload = PayloadManifest {
            compressed_size: 20,
            compressed_sha256:
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned(),
            part_size: PART_SIZE,
            parts: vec![PartDescriptor {
                ordinal: 0,
                path: "payload.pgi.zst.part0000".to_owned(),
                size: 20,
                sha256: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    .to_owned(),
            }],
        };
        let unsigned = UnsignedTransport {
            schema: TRANSPORT_SCHEMA,
            bundle: &bundle,
            compression: &compression,
            payload: &payload,
        };
        let transport_id = sha256(&serde_jcs::to_vec(&unsigned).expect("unsigned manifest"));
        TransportManifest {
            schema: TRANSPORT_SCHEMA.to_owned(),
            transport_id,
            bundle,
            compression,
            payload,
        }
    }

    fn encode(input: &[u8]) -> Vec<u8> {
        let mut encoder = production_encoder(Vec::new(), input.len() as u64).expect("encoder");
        encoder.write_all(input).expect("compress");
        encoder.finish().expect("finish")
    }

    #[test]
    fn transport_manifest_is_canonical_closed_bounded_and_versioned() {
        let manifest = signed_manifest();
        let canonical = serde_jcs::to_vec(&manifest).expect("canonical");
        assert_eq!(
            parse_transport_manifest(&canonical).expect("parse"),
            manifest
        );

        let mut unknown: serde_json::Value =
            serde_json::from_slice(&canonical).expect("manifest value");
        unknown
            .as_object_mut()
            .expect("object")
            .insert("future".to_owned(), serde_json::Value::Bool(true));
        assert_eq!(
            parse_transport_manifest(&serde_jcs::to_vec(&unknown).expect("unknown canonical"))
                .expect_err("v1 is closed")
                .kind(),
            AssetErrorKind::ManifestInvalid
        );

        let mut future = unknown;
        future["schema"] = serde_json::Value::String("pangopup.snv-transport.v2".to_owned());
        assert_eq!(
            parse_transport_manifest(&serde_jcs::to_vec(&future).expect("future canonical"))
                .expect_err("future version")
                .kind(),
            AssetErrorKind::TransportIncompatible
        );

        let text = String::from_utf8(canonical.clone()).expect("UTF-8");
        let duplicate = text.replacen(
            "\"schema\":\"pangopup.snv-transport.v1\"",
            "\"schema\":\"pangopup.snv-transport.v1\",\"schema\":\"pangopup.snv-transport.v1\"",
            1,
        );
        assert_eq!(
            parse_transport_manifest(duplicate.as_bytes())
                .expect_err("duplicate discriminator")
                .kind(),
            AssetErrorKind::ManifestInvalid
        );
        let nested_future_duplicate =
            String::from_utf8(serde_jcs::to_vec(&future).expect("future manifest"))
                .expect("UTF-8")
                .replacen(
                    "\"future\":true",
                    "\"future\":{\"nested\":1,\"nested\":2}",
                    1,
                );
        assert_eq!(
            parse_transport_manifest(nested_future_duplicate.as_bytes())
                .expect_err("nested duplicate in future manifest")
                .kind(),
            AssetErrorKind::ManifestInvalid
        );
        let mut noncanonical = canonical;
        noncanonical.push(b'\n');
        assert_eq!(
            parse_transport_manifest(&noncanonical)
                .expect_err("noncanonical")
                .kind(),
            AssetErrorKind::ManifestInvalid
        );

        let mut invalid_part: serde_json::Value =
            serde_json::from_slice(&serde_jcs::to_vec(&manifest).expect("manifest"))
                .expect("manifest value");
        invalid_part["payload"]["parts"][0]["ordinal"] = serde_json::Value::from(1);
        assert_eq!(
            parse_transport_manifest(
                &serde_jcs::to_vec(&invalid_part).expect("invalid part canonical")
            )
            .expect_err("invalid part set")
            .kind(),
            AssetErrorKind::PartSetInvalid
        );
    }

    #[test]
    fn embedded_notice_identity_is_pinned() {
        assert_eq!(NOTICE.len(), 1_709);
        assert_eq!(sha256(NOTICE), NOTICE_SHA256);
    }

    #[test]
    fn publication_platform_contract_is_explicit() {
        #[cfg(target_os = "linux")]
        require_linux().expect("Linux publication support");
        #[cfg(not(target_os = "linux"))]
        assert_eq!(
            require_linux()
                .expect_err("non-Linux publication is unsupported")
                .kind(),
            AssetErrorKind::UnsupportedPlatform
        );
    }

    #[test]
    fn pinned_encoder_emits_a_stable_checked_frame() {
        let input = b"pangopup deterministic zstandard fixture\n".repeat(64);
        let mut encoder = production_encoder(Vec::new(), input.len() as u64).expect("encoder");
        encoder.write_all(&input).expect("compress");
        let bytes = encoder.finish().expect("finish");
        let golden = [
            40, 181, 47, 253, 100, 64, 9, 149, 1, 0, 148, 2, 112, 97, 110, 103, 111, 112, 117, 112,
            32, 100, 101, 116, 101, 114, 109, 105, 110, 105, 115, 116, 105, 99, 32, 122, 115, 116,
            97, 110, 100, 97, 114, 100, 32, 102, 105, 120, 116, 117, 114, 101, 10, 1, 0, 161, 16,
            243, 85, 25, 160, 145, 177, 210,
        ];
        assert_eq!(bytes, golden);
        assert_eq!(
            sha256(&bytes),
            "sha256:2cc3b395a6086ed4814302d49394c136a6f293fc842c46bc5c39bfd33e79cc2d"
        );
        assert_eq!(&bytes[..8], &[40, 181, 47, 253, 100, 64, 9, 149]);
        validate_frame_header(&bytes, input.len() as u64).expect("header");
    }

    #[test]
    fn final_encoder_write_failure_is_output_io() {
        let input = b"finish must write the frame trailer";
        let mut encoder =
            production_encoder(ToggleFailWriter::default(), input.len() as u64).expect("encoder");
        encoder.write_all(input).expect("buffer input");
        encoder.get_mut().fail = true;
        assert_eq!(
            finish_encoder(encoder)
                .expect_err("finish output failure")
                .kind(),
            AssetErrorKind::OutputIo
        );
    }

    #[test]
    fn frame_header_rejects_every_fixed_v1_flag_violation() {
        let input = b"header contract".repeat(64);
        let valid = encode(&input);
        validate_frame_header(&valid, input.len() as u64).expect("valid frame");
        let mut cases = Vec::new();
        let mut magic = valid.clone();
        magic[0] ^= 1;
        cases.push(magic);
        let mut checksum = valid.clone();
        checksum[4] &= !0x04;
        cases.push(checksum);
        let mut dictionary = valid.clone();
        dictionary[4] |= 0x01;
        cases.push(dictionary);
        let mut reserved = valid.clone();
        reserved[4] |= 0x08;
        cases.push(reserved);
        let mut content_size = valid.clone();
        content_size[5] ^= 1;
        cases.push(content_size);
        let mut over_window = valid.clone();
        over_window[4] &= !0x20;
        over_window.insert(5, 13 << 3);
        cases.push(over_window);
        cases.push(valid[..5].to_vec());
        for bytes in cases {
            assert_eq!(
                validate_frame_header(&bytes, input.len() as u64)
                    .expect_err("invalid header")
                    .kind(),
                AssetErrorKind::CompressionInvalid
            );
        }
    }

    #[test]
    fn decoder_rejects_trailing_bytes_and_a_second_frame() {
        let input = b"single frame only".repeat(1024);
        let compressed = encode(&input);
        let root = std::env::temp_dir().join(format!("pangopup-frame-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).expect("temporary directory");
        let mut manifest = signed_manifest();
        manifest.bundle.scores.size = input.len() as u64;
        manifest.bundle.scores.sha256 = sha256(&input);
        manifest.payload.parts[0].size = compressed.len() as u64;
        for suffix in [b"x".as_slice(), compressed.as_slice()] {
            let mut invalid = compressed.clone();
            invalid.extend_from_slice(suffix);
            manifest.payload.parts[0].size = invalid.len() as u64;
            manifest.payload.parts[0].sha256 = sha256(&invalid);
            manifest.payload.compressed_size = invalid.len() as u64;
            manifest.payload.compressed_sha256 = sha256(&invalid);
            fs::write(root.join("payload.pgi.zst.part0000"), invalid).expect("payload");
            assert_eq!(
                decode_parts(&root, &manifest, None)
                    .expect_err("trailing input")
                    .kind(),
                AssetErrorKind::CompressionInvalid
            );
        }
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn split_writer_uses_exact_boundaries() {
        let root = std::env::temp_dir().join(format!("pangopup-split-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).expect("temporary directory");
        let mut writer = SplitWriter::new(&root, 7).expect("writer");
        writer.write_all(b"abcdefghijklmnop").expect("write");
        let payload = writer.finish().expect("finish");
        assert_eq!(
            payload
                .parts
                .iter()
                .map(|part| part.size)
                .collect::<Vec<_>>(),
            [7, 7, 2]
        );
        assert_eq!(
            fs::read(root.join(&payload.parts[1].path)).expect("part"),
            b"hijklmn"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn part_reader_visits_all_one_thousand_synthetic_handles() {
        let root = std::env::temp_dir().join(format!("pangopup-parts-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).expect("temporary directory");
        let mut parts = Vec::with_capacity(MAX_PARTS);
        let mut expected = Vec::with_capacity(MAX_PARTS);
        for ordinal in 0..MAX_PARTS {
            let byte = (ordinal % 251) as u8;
            let path = format!("payload.pgi.zst.part{ordinal:04}");
            fs::write(root.join(&path), [byte]).expect("synthetic part");
            expected.push(byte);
            parts.push(PartDescriptor {
                ordinal: ordinal as u16,
                path,
                size: 1,
                sha256: sha256(&[byte]),
            });
        }
        let payload = PayloadManifest {
            compressed_size: expected.len() as u64,
            compressed_sha256: sha256(&expected),
            part_size: PART_SIZE,
            parts,
        };
        let failure = Rc::new(RefCell::new(None));
        let mut reader = PartReader::new(&root, &payload, Rc::clone(&failure));
        let mut actual = Vec::new();
        reader.read_to_end(&mut actual).expect("iterate every part");
        reader.finish_validation().expect("complete payload");
        assert_eq!(actual, expected);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn decoded_limit_reads_only_the_declared_size_plus_one() {
        let mut input = io::Cursor::new(b"abcd".to_vec());
        let mut hash = Sha256::new();
        let error =
            copy_decoded_limited(&mut input, 3, &mut hash, None).expect_err("one-byte expansion");
        assert_eq!(error.kind(), AssetErrorKind::CompressionInvalid);
        assert_eq!(input.position(), 4);
    }

    #[test]
    fn pack_stream_binds_the_certified_member_identity() {
        let bytes = b"certified score bytes";
        let identity = sha256(bytes);
        let mut exact = io::Cursor::new(bytes);
        let mut output = Vec::new();
        stream_exact_member(&mut exact, &mut output, bytes.len() as u64, &identity)
            .expect("exact member");
        assert_eq!(output, bytes);

        for changed in [b"short".as_slice(), b"certified score bytes!".as_slice()] {
            let mut input = io::Cursor::new(changed);
            assert_eq!(
                stream_exact_member(&mut input, &mut Vec::new(), bytes.len() as u64, &identity,)
                    .expect_err("changed member")
                    .kind(),
                AssetErrorKind::BundleInvalid
            );
        }
        let mut same_size = bytes.to_vec();
        same_size[0] ^= 1;
        assert_eq!(
            stream_exact_member(
                &mut io::Cursor::new(same_size),
                &mut Vec::new(),
                bytes.len() as u64,
                &identity,
            )
            .expect_err("same-size replacement")
            .kind(),
            AssetErrorKind::BundleInvalid
        );
    }

    #[cfg(unix)]
    #[test]
    fn part_reader_keeps_the_inspected_handle_when_the_path_is_replaced() {
        let root =
            std::env::temp_dir().join(format!("pangopup-same-handle-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).expect("temporary directory");
        let path = "payload.pgi.zst.part0000";
        let original = b"original";
        fs::write(root.join(path), original).expect("original part");
        let payload = PayloadManifest {
            compressed_size: original.len() as u64,
            compressed_sha256: sha256(original),
            part_size: PART_SIZE,
            parts: vec![PartDescriptor {
                ordinal: 0,
                path: path.to_owned(),
                size: original.len() as u64,
                sha256: sha256(original),
            }],
        };
        let failure = Rc::new(RefCell::new(None));
        let mut reader = PartReader::new(&root, &payload, failure);
        let mut actual = vec![0_u8; 1];
        reader
            .read_exact(&mut actual)
            .expect("open original handle");
        let replacement = root.join("replacement");
        fs::write(&replacement, b"replaced").expect("replacement part");
        fs::rename(&replacement, root.join(path)).expect("replace path");
        reader
            .read_to_end(&mut actual)
            .expect("finish original handle");
        reader.finish_validation().expect("original identity");
        assert_eq!(actual, original);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
