//! Deterministic local delivery set for the three model-side runtime assets.

use super::{
    AssetError, AssetErrorKind, create_stage, finish_encoder, finish_staged, open_regular,
    production_encoder, publish_stage, reject_duplicate_json, sha256, sync_directory,
    validate_frame_header, write_synced,
};
use pangopup_core::ReferenceProvider;
use pangopup_index::{
    mask::MaskDomainsOpen,
    reference::{ReferenceBundleOpen, ReferenceIndexError},
};
use pangopup_model::{ModelError, ModelRepresentation, inspect_runtime_profile_bundle};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

const SCHEMA: &str = "pangopup.runtime-transport.v1";
const FORMAT: &str = "zstd.frame.v1";
const MAX_JSON: u64 = 1024 * 1024;
const MAX_NOTICE: u64 = 64 * 1024;
const MAX_WINDOW_LOG: u32 = 22;
const MASK_NOTICE: &[u8] = include_bytes!("../../../assets/notices/GENCODE-v38-NOTICE");
const TRANSPORT_MANIFEST: &str = "runtime-transport.json";

#[cfg(test)]
mod direct_install_audit {
    use std::cell::RefCell;

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct Event {
        pub name: String,
        pub cached_read_bytes: u64,
        pub decoded_bytes: u64,
        pub final_write_bytes: u64,
    }

    thread_local! {
        static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    }

    pub fn reset() {
        EVENTS.take();
    }

    pub fn record(name: &str, stored: u64, decoded: u64, written: u64) {
        EVENTS.with_borrow_mut(|events| {
            events.push(Event {
                name: name.to_owned(),
                cached_read_bytes: stored,
                decoded_bytes: decoded,
                final_write_bytes: written,
            });
        });
    }

    pub fn take() -> Vec<Event> {
        EVENTS.take()
    }
}

#[cfg(test)]
fn record_direct_install(name: &str, stored: u64, decoded: u64, written: u64) {
    direct_install_audit::record(name, stored, decoded, written);
}

#[cfg(not(test))]
fn record_direct_install(_name: &str, _stored: u64, _decoded: u64, _written: u64) {}

const EXPECTED: [(&str, &str, Encoding); 9] = [
    ("runtime-profile.json", "runtime-profile", Encoding::Raw),
    ("model-manifest.json", "model-manifest", Encoding::Raw),
    ("model-NOTICE", "model-attribution", Encoding::Raw),
    ("model.onnx.zst", "model-member", Encoding::Zstd),
    (
        "reference-manifest.json",
        "reference-manifest",
        Encoding::Raw,
    ),
    ("reference-NOTICE", "reference-attribution", Encoding::Raw),
    ("reference.pgr.zst", "reference-member", Encoding::Zstd),
    ("mask-NOTICE", "mask-attribution", Encoding::Raw),
    ("domains.pgm.zst", "mask-member", Encoding::Zstd),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Encoding {
    Raw,
    Zstd,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Encoder {
    format: String,
    level: i32,
    checksum: bool,
    content_size: bool,
    dictionary: bool,
    long_distance: bool,
    workers: u32,
    encoder_crate: String,
    libzstd_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Member {
    pub name: String,
    pub role: String,
    pub encoding: Encoding,
    pub uncompressed_bytes: u64,
    pub uncompressed_sha256: String,
    pub stored_bytes: u64,
    pub stored_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Attribution {
    model: String,
    reference: String,
    mask: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    schema: String,
    pub runtime_profile_id: String,
    encoder: Encoder,
    attribution: Attribution,
    pub members: Vec<Member>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PackRuntimeTransportOutcome {
    pub status: &'static str,
    pub command: &'static str,
    pub transport_id: String,
    pub runtime_profile_id: String,
    pub compressed_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerifyRuntimeTransportOutcome {
    pub status: &'static str,
    pub command: &'static str,
    pub transport_id: String,
    pub runtime_profile_id: String,
    pub compressed_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UnpackRuntimeTransportOutcome {
    pub status: &'static str,
    pub command: &'static str,
    pub transport_id: String,
    pub runtime_profile_id: String,
}

struct Source<'a> {
    source: &'a Path,
    output: &'static str,
    role: &'static str,
    expected_bytes: u64,
    expected_sha256: &'a str,
}

pub fn pack_runtime_transport(
    profile_path: &Path,
    model_bundle: &Path,
    reference_bundle: &Path,
    mask_path: &Path,
    output: &Path,
) -> Result<PackRuntimeTransportOutcome, AssetError> {
    super::require_linux()?;
    super::ensure_output_absent(output)?;

    let profile_bytes = read_checked(profile_path, 64 * 1024)?;
    let profile = super::parse_runtime_profile(&profile_bytes).map_err(profile_error)?;
    let profile_id = super::runtime_profile_id(&profile_bytes)
        .map_err(profile_error)?
        .to_string();

    authenticate_components(&profile, model_bundle, reference_bundle, mask_path)?;
    require_exact_directory(model_bundle, &["NOTICE", "manifest.json", "model.onnx"])?;
    require_exact_directory(
        reference_bundle,
        &["NOTICE", "manifest.json", "reference.pgr"],
    )?;

    let model_manifest = authenticated_metadata(
        &model_bundle.join("manifest.json"),
        MAX_JSON,
        &profile.model.bundle_id,
    )?;
    let model_notice = authenticated_notice(
        &model_bundle.join("NOTICE"),
        MAX_NOTICE,
        &model_manifest,
        "NOTICE",
    )?;
    let reference_manifest = authenticated_metadata(
        &reference_bundle.join("manifest.json"),
        MAX_JSON,
        &profile.reference.bundle_id,
    )?;
    let reference_notice = authenticated_notice(
        &reference_bundle.join("NOTICE"),
        MAX_NOTICE,
        &reference_manifest,
        "NOTICE",
    )?;

    let (stage, mut guard) = create_stage(output)?;
    let result = (|| {
        let mut members = Vec::with_capacity(EXPECTED.len());
        for (name, role, bytes) in [
            (
                "runtime-profile.json",
                "runtime-profile",
                profile_bytes.as_slice(),
            ),
            (
                "model-manifest.json",
                "model-manifest",
                model_manifest.as_slice(),
            ),
            ("model-NOTICE", "model-attribution", model_notice.as_slice()),
        ] {
            write_synced(&stage.join(name), bytes)?;
            members.push(raw_member(name, role, bytes));
        }
        members.push(pack_payload(
            &stage,
            Source {
                source: &model_bundle.join("model.onnx"),
                output: "model.onnx.zst",
                role: "model-member",
                expected_bytes: profile.model.member_bytes,
                expected_sha256: &profile.model.member_sha256,
            },
        )?);
        for (name, role, bytes) in [
            (
                "reference-manifest.json",
                "reference-manifest",
                reference_manifest.as_slice(),
            ),
            (
                "reference-NOTICE",
                "reference-attribution",
                reference_notice.as_slice(),
            ),
        ] {
            write_synced(&stage.join(name), bytes)?;
            members.push(raw_member(name, role, bytes));
        }
        members.push(pack_payload(
            &stage,
            Source {
                source: &reference_bundle.join("reference.pgr"),
                output: "reference.pgr.zst",
                role: "reference-member",
                expected_bytes: profile.reference.member_bytes,
                expected_sha256: &profile.reference.member_sha256,
            },
        )?);
        write_synced(&stage.join("mask-NOTICE"), MASK_NOTICE)?;
        members.push(raw_member("mask-NOTICE", "mask-attribution", MASK_NOTICE));
        members.push(pack_payload(
            &stage,
            Source {
                source: mask_path,
                output: "domains.pgm.zst",
                role: "mask-member",
                expected_bytes: profile.mask.member_bytes,
                expected_sha256: &profile.mask.member_sha256,
            },
        )?);

        let manifest = Manifest {
            schema: SCHEMA.to_owned(),
            runtime_profile_id: profile_id.clone(),
            encoder: expected_encoder()?,
            attribution: Attribution {
                model: "model-NOTICE".to_owned(),
                reference: "reference-NOTICE".to_owned(),
                mask: "mask-NOTICE".to_owned(),
            },
            members,
        };
        validate_manifest(&manifest)?;
        let manifest_bytes = serde_jcs::to_vec(&manifest)
            .map_err(|_| invalid_manifest("serialize runtime transport manifest"))?;
        write_synced(&stage.join(TRANSPORT_MANIFEST), &manifest_bytes)?;
        sync_directory(&stage)?;
        publish_stage(&stage, output, &mut guard)?;
        Ok(PackRuntimeTransportOutcome {
            status: "ok",
            command: "runtime-transport.pack",
            transport_id: sha256(&manifest_bytes),
            runtime_profile_id: profile_id,
            compressed_bytes: manifest
                .members
                .iter()
                .filter(|member| member.encoding == Encoding::Zstd)
                .map(|member| member.stored_bytes)
                .sum(),
        })
    })();
    finish_staged(result, &mut guard)
}

pub fn verify_runtime_transport(
    transport: &Path,
) -> Result<VerifyRuntimeTransportOutcome, AssetError> {
    let opened = open_transport(transport)?;
    let mut compressed_bytes = 0;
    for member in &opened.manifest.members {
        let path = transport.join(&member.name);
        match member.encoding {
            Encoding::Raw => verify_raw(&path, member)?,
            Encoding::Zstd => {
                verify_frame(&path, member, None)?;
                compressed_bytes += member.stored_bytes;
            }
        }
    }
    Ok(VerifyRuntimeTransportOutcome {
        status: "ok",
        command: "runtime-transport.verify",
        transport_id: sha256(&opened.bytes),
        runtime_profile_id: opened.manifest.runtime_profile_id,
        compressed_bytes,
    })
}

pub fn unpack_runtime_transport(
    transport: &Path,
    output: &Path,
) -> Result<UnpackRuntimeTransportOutcome, AssetError> {
    super::require_linux()?;
    super::ensure_output_absent(output)?;
    let opened = open_transport(transport)?;
    let transport_id = sha256(&opened.bytes);
    let profile_id = opened.manifest.runtime_profile_id.clone();
    let (stage, mut guard) = create_stage(output)?;
    let result = (|| {
        for directory in ["model", "reference", "mask"] {
            fs::create_dir(stage.join(directory))
                .map_err(|error| output_io("create reconstructed directory", error))?;
        }
        for member in &opened.manifest.members {
            let source = transport.join(&member.name);
            let destination = unpack_path(&stage, &member.name)?;
            match member.encoding {
                Encoding::Raw => {
                    let bytes = read_and_verify_raw(&source, member)?;
                    write_synced(&destination, &bytes)?;
                }
                Encoding::Zstd => {
                    let mut file = File::create(&destination)
                        .map_err(|error| output_io("create reconstructed member", error))?;
                    verify_frame(&source, member, Some(&mut file))?;
                    file.sync_all()
                        .map_err(|error| output_io("sync reconstructed member", error))?;
                }
            }
        }
        for directory in ["model", "reference", "mask"] {
            sync_directory(&stage.join(directory))?;
        }
        sync_directory(&stage)?;
        publish_stage(&stage, output, &mut guard)?;
        Ok(UnpackRuntimeTransportOutcome {
            status: "ok",
            command: "runtime-transport.unpack",
            transport_id,
            runtime_profile_id: profile_id,
        })
    })();
    finish_staged(result, &mut guard)
}

struct OpenedTransport {
    bytes: Vec<u8>,
    manifest: Manifest,
    raw_members: BTreeMap<String, Vec<u8>>,
}

pub(crate) fn parse_runtime_transport_manifest_for_release(
    bytes: &[u8],
) -> Result<Manifest, AssetError> {
    reject_duplicate_json(bytes)?;
    let manifest: Manifest = serde_json::from_slice(bytes)
        .map_err(|_| invalid_manifest("invalid runtime transport JSON"))?;
    let canonical = serde_jcs::to_vec(&manifest)
        .map_err(|_| invalid_manifest("serialize runtime transport manifest"))?;
    if canonical != bytes {
        return Err(invalid_manifest(
            "runtime transport manifest is not canonical",
        ));
    }
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub(crate) fn read_runtime_transport_held_raw(
    file: &File,
    before: &fs::Metadata,
    member: &Member,
) -> Result<Vec<u8>, AssetError> {
    let mut input = file
        .try_clone()
        .map_err(|error| input_io("clone held runtime member", error))?;
    input
        .seek(SeekFrom::Start(0))
        .map_err(|error| input_io("rewind held runtime member", error))?;
    let cap = MAX_JSON.max(MAX_NOTICE);
    if member.stored_bytes > cap || before.len() != member.stored_bytes {
        return Err(part_set("raw runtime transport member exceeds bound"));
    }
    let mut bytes = Vec::with_capacity(member.stored_bytes as usize);
    Read::by_ref(&mut input)
        .take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| input_io("read held raw runtime member", error))?;
    validate_held_metadata(file, before)?;
    if bytes.len() as u64 != member.stored_bytes
        || member.stored_bytes != member.uncompressed_bytes
        || sha256(&bytes) != member.stored_sha256
        || member.stored_sha256 != member.uncompressed_sha256
    {
        return Err(AssetError::new(
            AssetErrorKind::TransportHashMismatch,
            "raw runtime transport member identity mismatch",
        ));
    }
    Ok(bytes)
}

pub(crate) fn verify_runtime_transport_held_frame(
    file: &File,
    before: &fs::Metadata,
    member: &Member,
) -> Result<(), AssetError> {
    let mut input = file
        .try_clone()
        .map_err(|error| input_io("clone held runtime member", error))?;
    input
        .seek(SeekFrom::Start(0))
        .map_err(|error| input_io("rewind held runtime member", error))?;
    verify_frame_open(input, before, member, None, None).map(|_| ())
}

fn open_transport(path: &Path) -> Result<OpenedTransport, AssetError> {
    let descriptor_path = path.parent() == Some(Path::new("/proc/self/fd"))
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_digit()));
    let root = if descriptor_path {
        fs::metadata(path)
    } else {
        fs::symlink_metadata(path)
    }
    .map_err(|error| input_io("inspect transport", error))?;
    if (!descriptor_path && root.file_type().is_symlink()) || !root.file_type().is_dir() {
        return Err(part_set("transport is not a regular directory"));
    }
    let bytes = read_checked(&path.join(TRANSPORT_MANIFEST), MAX_JSON)?;
    let manifest = parse_runtime_transport_manifest_for_release(&bytes)?;
    require_transport_inventory(path, &manifest)?;
    let mut raw_members = BTreeMap::new();
    for member in &manifest.members {
        if member.encoding == Encoding::Raw {
            raw_members.insert(
                member.name.clone(),
                read_and_verify_raw(&path.join(&member.name), member)?,
            );
        }
    }
    let profile_bytes = raw_members
        .get("runtime-profile.json")
        .ok_or_else(|| invalid_manifest("runtime profile member is missing"))?;
    let observed_profile_id = super::runtime_profile_id(profile_bytes)
        .map_err(profile_error)?
        .to_string();
    if observed_profile_id != manifest.runtime_profile_id {
        return Err(invalid_manifest(
            "runtime profile identity does not match transport manifest",
        ));
    }
    Ok(OpenedTransport {
        bytes,
        manifest,
        raw_members,
    })
}

/// Install an authenticated cached runtime transport without materializing a
/// second decoded transport tree.
pub(crate) fn install_cached_runtime_transport(
    transport: &Path,
    data_root: &Path,
) -> Result<super::RuntimeInstallOutcome, AssetError> {
    install_cached_runtime_transport_with_policy(transport, data_root, true)
}

fn install_cached_runtime_transport_with_policy(
    transport: &Path,
    data_root: &Path,
    require_production: bool,
) -> Result<super::RuntimeInstallOutcome, AssetError> {
    super::require_linux()?;
    let opened = open_transport(transport)?;
    if require_production {
        require_production_manifest(&opened.bytes)?;
    }
    let profile_bytes = opened
        .raw_members
        .get("runtime-profile.json")
        .ok_or_else(|| invalid_manifest("runtime profile member is missing"))?;
    let profile = super::parse_runtime_profile(profile_bytes).map_err(profile_error)?;
    if require_production {
        profile
            .require_trusted_production()
            .map_err(profile_error)?;
    }
    let manifest = &opened.manifest;
    super::runtime_install::install_with_stager(
        profile_bytes,
        &profile,
        data_root,
        |model, reference, mask| {
            stage_cached_transport(
                transport,
                manifest,
                &opened.raw_members,
                model,
                reference,
                mask,
            )
        },
    )
}

fn require_production_manifest(bytes: &[u8]) -> Result<(), AssetError> {
    if bytes != super::runtime_release::production_runtime_transport_manifest()? {
        Err(invalid_manifest(
            "cached runtime transport does not match compiled production authority",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn install_cached_runtime_transport_for_test(
    transport: &Path,
    data_root: &Path,
) -> Result<super::RuntimeInstallOutcome, AssetError> {
    install_cached_runtime_transport_with_policy(transport, data_root, false)
}

fn stage_cached_transport(
    transport: &Path,
    manifest: &Manifest,
    raw: &BTreeMap<String, Vec<u8>>,
    model: &Path,
    reference: &Path,
    mask: &Path,
) -> Result<(), AssetError> {
    for directory in [model, reference, mask] {
        fs::create_dir(directory)
            .map_err(|error| output_io("create runtime installation staging directory", error))?;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .map_err(|error| output_io("protect runtime installation staging directory", error))?;
    }
    for member in &manifest.members {
        let destination = match member.name.as_str() {
            "runtime-profile.json" => continue,
            "model-manifest.json" => model.join("manifest.json"),
            "model-NOTICE" => model.join("NOTICE"),
            "model.onnx.zst" => model.join("model.onnx"),
            "reference-manifest.json" => reference.join("manifest.json"),
            "reference-NOTICE" => reference.join("NOTICE"),
            "reference.pgr.zst" => reference.join("reference.pgr"),
            "mask-NOTICE" => continue,
            "domains.pgm.zst" => mask.join("domains.pgm"),
            _ => return Err(invalid_manifest("unexpected runtime transport member")),
        };
        match member.encoding {
            Encoding::Raw => {
                let bytes = raw
                    .get(&member.name)
                    .ok_or_else(|| part_set("raw runtime transport member is missing"))?;
                write_install_member(&destination, bytes)?;
            }
            Encoding::Zstd => {
                let mut output = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&destination)
                    .map_err(|error| output_io("create decoded runtime member", error))?;
                let (metrics, final_write_bytes) = {
                    let mut counted = CountingWriter::new(&mut output);
                    let metrics =
                        verify_frame(&transport.join(&member.name), member, Some(&mut counted))?;
                    (metrics, counted.bytes)
                };
                record_direct_install(
                    &member.name,
                    metrics.cached_read_bytes,
                    metrics.decoded_bytes,
                    final_write_bytes,
                );
                output
                    .set_permissions(fs::Permissions::from_mode(0o444))
                    .map_err(|error| output_io("protect decoded runtime member", error))?;
                output
                    .sync_all()
                    .map_err(|error| output_io("sync decoded runtime member", error))?;
            }
        }
    }
    for directory in [model, reference, mask] {
        sync_directory(directory)?;
    }
    Ok(())
}

fn write_install_member(path: &Path, bytes: &[u8]) -> Result<(), AssetError> {
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| output_io("create staged runtime member", error))?;
    output
        .write_all(bytes)
        .and_then(|_| output.set_permissions(fs::Permissions::from_mode(0o444)))
        .and_then(|_| output.sync_all())
        .map_err(|error| output_io("write staged runtime member", error))
}

fn authenticate_components(
    profile: &super::RuntimeProfile,
    model_bundle: &Path,
    reference_bundle: &Path,
    mask_path: &Path,
) -> Result<(), AssetError> {
    let model = inspect_runtime_profile_bundle(model_bundle).map_err(model_error)?;
    if model.bundle_id.as_str() != profile.model.bundle_id
        || model.profile != profile.model.profile
        || representation(model.representation) != profile.model.representation
        || model.member_bytes != profile.model.member_bytes
        || model.member_sha256 != profile.model.member_sha256
    {
        return Err(bundle_invalid(
            "model bundle does not match runtime profile",
        ));
    }
    let reference =
        ReferenceBundleOpen::open_identified(reference_bundle).map_err(reference_error)?;
    let provenance = reference.provenance();
    if provenance.bundle_id() != profile.reference.bundle_id
        || provenance.profile() != profile.reference.profile
        || provenance.format() != profile.reference.format
        || provenance.assembly() != profile.reference.assembly
        || provenance.assembly_accession() != profile.reference.assembly_accession
        || provenance.sequence_set_sha256() != profile.reference.sequence_set_sha256
        || reference.identity().bytes() != profile.reference.member_bytes
        || reference.identity().sha256() != profile.reference.member_sha256
    {
        return Err(bundle_invalid(
            "reference bundle does not match runtime profile",
        ));
    }
    let mask = MaskDomainsOpen::open_identified(mask_path).map_err(mask_error)?;
    if profile.mask.format != "pangopup.gencode-v38-domains.v1"
        || mask.identity().bytes() != profile.mask.member_bytes
        || format!("sha256:{}", mask.identity().sha256()) != profile.mask.member_sha256
    {
        return Err(bundle_invalid("mask does not match runtime profile"));
    }
    Ok(())
}

fn pack_payload(stage: &Path, source: Source<'_>) -> Result<Member, AssetError> {
    let (mut input, before) = open_regular(
        source.source,
        AssetErrorKind::InputIo,
        AssetErrorKind::BundleInvalid,
    )?;
    require_single_link(&before)?;
    if before.len() != source.expected_bytes {
        return Err(bundle_invalid("runtime member size changed before packing"));
    }
    let output = File::create(stage.join(source.output))
        .map_err(|error| output_io("create compressed runtime member", error))?;
    let hashing = HashingWriter::new(output);
    let mut encoder = production_encoder(hashing, source.expected_bytes)?;
    let mut hash = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|error| input_io("read runtime member", error))?;
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(count as u64)
            .ok_or_else(|| bundle_invalid("runtime member size overflow"))?;
        if observed > source.expected_bytes {
            return Err(bundle_invalid("runtime member grew before packing"));
        }
        hash.update(&buffer[..count]);
        encoder
            .write_all(&buffer[..count])
            .map_err(|error| output_io("compress runtime member", error))?;
    }
    let hashing = finish_encoder(encoder)?;
    hashing
        .inner
        .sync_all()
        .map_err(|error| output_io("sync compressed runtime member", error))?;
    validate_retained_path(source.source, &input, &before)?;
    let uncompressed_sha256 = format!("sha256:{:x}", hash.finalize());
    if observed != source.expected_bytes || uncompressed_sha256 != source.expected_sha256 {
        return Err(bundle_invalid(
            "runtime member identity changed before packing",
        ));
    }
    Ok(Member {
        name: source.output.to_owned(),
        role: source.role.to_owned(),
        encoding: Encoding::Zstd,
        uncompressed_bytes: observed,
        uncompressed_sha256,
        stored_bytes: hashing.bytes,
        stored_sha256: format!("sha256:{:x}", hashing.hash.finalize()),
    })
}

struct FrameMetrics {
    cached_read_bytes: u64,
    decoded_bytes: u64,
}

struct CountingWriter<'a> {
    inner: &'a mut File,
    bytes: u64,
}

impl<'a> CountingWriter<'a> {
    fn new(inner: &'a mut File) -> Self {
        Self { inner, bytes: 0 }
    }
}

impl Write for CountingWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let count = self.inner.write(buffer)?;
        self.bytes = self
            .bytes
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("runtime destination byte count overflow"))?;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn verify_frame(
    path: &Path,
    member: &Member,
    output: Option<&mut dyn Write>,
) -> Result<FrameMetrics, AssetError> {
    let (input, before) = open_regular(
        path,
        AssetErrorKind::InputIo,
        AssetErrorKind::PartSetInvalid,
    )?;
    require_single_link(&before)?;
    verify_frame_open(input, &before, member, output, Some(path))
}

fn verify_frame_open(
    input: File,
    before: &fs::Metadata,
    member: &Member,
    output: Option<&mut dyn Write>,
    path: Option<&Path>,
) -> Result<FrameMetrics, AssetError> {
    if before.len() != member.stored_bytes {
        return Err(part_set("compressed member size does not match manifest"));
    }
    let hashing = HashingReader::new(input);
    let mut buffered = BufReader::with_capacity(128 * 1024, hashing);
    let header = buffered
        .fill_buf()
        .map_err(|error| input_io("read compressed runtime header", error))?;
    validate_frame_header(header, member.uncompressed_bytes)?;
    let mut decoder = zstd::stream::read::Decoder::with_buffer(buffered)
        .map_err(|error| compression_invalid(error.to_string()))?;
    decoder
        .window_log_max(MAX_WINDOW_LOG)
        .map_err(|error| compression_invalid(error.to_string()))?;
    let mut decoder = decoder.single_frame();
    let mut output = output;
    let mut decoded_hash = Sha256::new();
    let mut decoded = 0_u64;
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let remaining = member.uncompressed_bytes.saturating_sub(decoded);
        let limit = usize::try_from((remaining + 1).min(buffer.len() as u64))
            .map_err(|_| compression_invalid("decoded size bound"))?;
        let count = decoder
            .read(&mut buffer[..limit])
            .map_err(|error| compression_invalid(error.to_string()))?;
        if count == 0 {
            break;
        }
        decoded += count as u64;
        if decoded > member.uncompressed_bytes {
            return Err(compression_invalid("decoded member exceeds declared size"));
        }
        decoded_hash.update(&buffer[..count]);
        if let Some(file) = output.as_deref_mut() {
            file.write_all(&buffer[..count])
                .map_err(|error| output_io("write reconstructed runtime member", error))?;
        }
    }
    let mut remaining = decoder.finish();
    let mut trailing = false;
    loop {
        let count = remaining
            .read(&mut buffer)
            .map_err(|error| input_io("drain compressed runtime member", error))?;
        if count == 0 {
            break;
        }
        trailing = true;
    }
    let hashing = remaining.into_inner();
    validate_held_metadata(&hashing.inner, before)?;
    if let Some(path) = path {
        validate_retained_path(path, &hashing.inner, before)?;
    }
    if trailing {
        return Err(compression_invalid(
            "compressed member has a second frame or trailing bytes",
        ));
    }
    let cached_read_bytes = hashing.bytes;
    if cached_read_bytes != member.stored_bytes
        || format!("sha256:{:x}", hashing.hash.finalize()) != member.stored_sha256
    {
        return Err(AssetError::new(
            AssetErrorKind::TransportHashMismatch,
            "compressed runtime member identity mismatch",
        ));
    }
    if decoded != member.uncompressed_bytes
        || format!("sha256:{:x}", decoded_hash.finalize()) != member.uncompressed_sha256
    {
        return Err(AssetError::new(
            AssetErrorKind::TransportHashMismatch,
            "decompressed runtime member identity mismatch",
        ));
    }
    Ok(FrameMetrics {
        cached_read_bytes,
        decoded_bytes: decoded,
    })
}

fn validate_held_metadata(file: &File, before: &fs::Metadata) -> Result<(), AssetError> {
    let after = file
        .metadata()
        .map_err(|error| input_io("reinspect held runtime member", error))?;
    if before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.len() != after.len()
        || before.nlink() != after.nlink()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(bundle_invalid("held runtime member changed while read"));
    }
    Ok(())
}

fn authenticated_metadata(
    path: &Path,
    cap: u64,
    expected_identity: &str,
) -> Result<Vec<u8>, AssetError> {
    let bytes = read_checked(path, cap)?;
    if sha256(&bytes) != expected_identity {
        return Err(bundle_invalid("component manifest identity changed"));
    }
    Ok(bytes)
}

fn authenticated_notice(
    path: &Path,
    cap: u64,
    manifest: &[u8],
    member_name: &str,
) -> Result<Vec<u8>, AssetError> {
    let expected = manifest_member_identity(manifest, member_name)?;
    let bytes = read_checked(path, cap)?;
    if bytes.len() as u64 != expected.0 || sha256(&bytes) != expected.1 {
        return Err(bundle_invalid("component notice identity changed"));
    }
    Ok(bytes)
}

fn manifest_member_identity(bytes: &[u8], name: &str) -> Result<(u64, String), AssetError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| bundle_invalid("component manifest JSON"))?;
    let members = value
        .get("members")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| bundle_invalid("component manifest members"))?;
    let member = members
        .iter()
        .find(|member| {
            member
                .get("filename")
                .or_else(|| member.get("path"))
                .and_then(serde_json::Value::as_str)
                == Some(name)
        })
        .ok_or_else(|| bundle_invalid("component manifest notice"))?;
    let bytes = member
        .get("bytes")
        .or_else(|| member.get("size"))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| bundle_invalid("component manifest notice size"))?;
    let digest = member
        .get("sha256")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| bundle_invalid("component manifest notice digest"))?;
    Ok((bytes, digest.to_owned()))
}

fn read_checked(path: &Path, cap: u64) -> Result<Vec<u8>, AssetError> {
    let (mut file, before) =
        open_regular(path, AssetErrorKind::InputIo, AssetErrorKind::BundleInvalid)?;
    require_single_link(&before)?;
    if before.len() > cap {
        return Err(bundle_invalid("bounded component member is too large"));
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    Read::by_ref(&mut file)
        .take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| input_io("read component member", error))?;
    validate_retained_path(path, &file, &before)?;
    if bytes.len() as u64 != before.len() {
        return Err(bundle_invalid("component member changed while read"));
    }
    Ok(bytes)
}

fn raw_member(name: &str, role: &str, bytes: &[u8]) -> Member {
    Member {
        name: name.to_owned(),
        role: role.to_owned(),
        encoding: Encoding::Raw,
        uncompressed_bytes: bytes.len() as u64,
        uncompressed_sha256: sha256(bytes),
        stored_bytes: bytes.len() as u64,
        stored_sha256: sha256(bytes),
    }
}

fn read_and_verify_raw(path: &Path, member: &Member) -> Result<Vec<u8>, AssetError> {
    let bytes = read_checked_transport(path, member.stored_bytes)?;
    if bytes.len() as u64 != member.stored_bytes
        || member.stored_bytes != member.uncompressed_bytes
        || sha256(&bytes) != member.stored_sha256
        || member.stored_sha256 != member.uncompressed_sha256
    {
        return Err(AssetError::new(
            AssetErrorKind::TransportHashMismatch,
            "raw runtime transport member identity mismatch",
        ));
    }
    Ok(bytes)
}

fn verify_raw(path: &Path, member: &Member) -> Result<(), AssetError> {
    read_and_verify_raw(path, member).map(|_| ())
}

fn read_checked_transport(path: &Path, expected: u64) -> Result<Vec<u8>, AssetError> {
    let cap = MAX_JSON.max(MAX_NOTICE);
    if expected > cap {
        return Err(part_set("raw runtime transport member exceeds bound"));
    }
    let (mut file, before) = open_regular(
        path,
        AssetErrorKind::InputIo,
        AssetErrorKind::PartSetInvalid,
    )?;
    require_single_link(&before)?;
    if before.len() != expected {
        return Err(part_set("raw runtime transport member size mismatch"));
    }
    let mut bytes = Vec::with_capacity(expected as usize);
    Read::by_ref(&mut file)
        .take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| input_io("read raw runtime transport member", error))?;
    validate_retained_path(path, &file, &before)?;
    Ok(bytes)
}

fn validate_manifest(manifest: &Manifest) -> Result<(), AssetError> {
    if manifest.schema != SCHEMA
        || manifest.encoder != expected_encoder()?
        || manifest.attribution
            != (Attribution {
                model: "model-NOTICE".to_owned(),
                reference: "reference-NOTICE".to_owned(),
                mask: "mask-NOTICE".to_owned(),
            })
        || !valid_sha(&manifest.runtime_profile_id)
        || manifest.members.len() != EXPECTED.len()
    {
        return Err(invalid_manifest("runtime transport facts are incompatible"));
    }
    for (member, (name, role, encoding)) in manifest.members.iter().zip(EXPECTED) {
        if member.name != name
            || member.role != role
            || member.encoding != encoding
            || member.uncompressed_bytes > 9_007_199_254_740_991
            || member.stored_bytes > 9_007_199_254_740_991
            || member.uncompressed_bytes == 0
            || member.stored_bytes == 0
            || !valid_sha(&member.uncompressed_sha256)
            || !valid_sha(&member.stored_sha256)
            || (encoding == Encoding::Raw
                && (member.uncompressed_bytes != member.stored_bytes
                    || member.uncompressed_sha256 != member.stored_sha256))
        {
            return Err(invalid_manifest(
                "runtime transport member order or facts are incompatible",
            ));
        }
    }
    Ok(())
}

fn require_transport_inventory(path: &Path, manifest: &Manifest) -> Result<(), AssetError> {
    let expected: BTreeSet<_> = std::iter::once(TRANSPORT_MANIFEST.to_owned())
        .chain(manifest.members.iter().map(|member| member.name.clone()))
        .collect();
    let mut observed = BTreeSet::new();
    for entry in fs::read_dir(path).map_err(|error| input_io("read transport directory", error))? {
        let entry = entry.map_err(|error| input_io("read transport entry", error))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| part_set("non-UTF-8 transport member"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| input_io("inspect transport member", error))?;
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_file()
            || metadata.nlink() != 1
        {
            return Err(part_set(
                "transport member is not a regular single-link file",
            ));
        }
        observed.insert(name);
    }
    if observed != expected {
        return Err(part_set("runtime transport directory member set mismatch"));
    }
    Ok(())
}

fn require_exact_directory(path: &Path, expected: &[&str]) -> Result<(), AssetError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| input_io("inspect bundle", error))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(bundle_invalid(
            "component bundle is not a regular directory",
        ));
    }
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(path).map_err(|error| input_io("read component bundle", error))? {
        let entry = entry.map_err(|error| input_io("read component member", error))?;
        actual.insert(
            entry
                .file_name()
                .into_string()
                .map_err(|_| bundle_invalid("component member name is not UTF-8"))?,
        );
    }
    let expected: BTreeSet<_> = expected.iter().map(|name| (*name).to_owned()).collect();
    if actual != expected {
        return Err(bundle_invalid("component bundle member set mismatch"));
    }
    Ok(())
}

fn unpack_path(stage: &Path, name: &str) -> Result<PathBuf, AssetError> {
    let relative = match name {
        "runtime-profile.json" => "runtime-profile.json",
        "model-manifest.json" => "model/manifest.json",
        "model-NOTICE" => "model/NOTICE",
        "model.onnx.zst" => "model/model.onnx",
        "reference-manifest.json" => "reference/manifest.json",
        "reference-NOTICE" => "reference/NOTICE",
        "reference.pgr.zst" => "reference/reference.pgr",
        "mask-NOTICE" => "mask/NOTICE",
        "domains.pgm.zst" => "mask/domains.pgm",
        _ => return Err(invalid_manifest("unknown runtime transport member")),
    };
    Ok(stage.join(relative))
}

fn expected_encoder() -> Result<Encoder, AssetError> {
    if zstd_safe::version_string() != "1.5.7" {
        return Err(compression_invalid(format!(
            "linked libzstd is {}, expected 1.5.7",
            zstd_safe::version_string()
        )));
    }
    Ok(Encoder {
        format: FORMAT.to_owned(),
        level: 9,
        checksum: true,
        content_size: true,
        dictionary: false,
        long_distance: false,
        workers: 0,
        encoder_crate: "zstd/0.13.3".to_owned(),
        libzstd_version: "1.5.7".to_owned(),
    })
}

fn valid_sha(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn representation(value: ModelRepresentation) -> &'static str {
    match value {
        ModelRepresentation::Singleton => "singleton",
        ModelRepresentation::ZeroPaddedBatch => "zero-padded-batch",
        ModelRepresentation::PairedStrandBatch => "paired-strand-batch",
    }
}

fn require_single_link(metadata: &fs::Metadata) -> Result<(), AssetError> {
    if metadata.nlink() != 1 {
        Err(bundle_invalid("input member must have one hard link"))
    } else {
        Ok(())
    }
}

fn validate_retained_path(
    path: &Path,
    file: &File,
    before: &fs::Metadata,
) -> Result<(), AssetError> {
    let held = file
        .metadata()
        .map_err(|error| input_io("inspect held member", error))?;
    let current =
        fs::symlink_metadata(path).map_err(|error| input_io("reinspect member path", error))?;
    if current.file_type().is_symlink()
        || before.dev() != held.dev()
        || before.ino() != held.ino()
        || before.len() != held.len()
        || before.nlink() != held.nlink()
        || before.dev() != current.dev()
        || before.ino() != current.ino()
        || before.len() != current.len()
        || before.nlink() != current.nlink()
    {
        return Err(bundle_invalid("input member changed while read"));
    }
    Ok(())
}

struct HashingWriter {
    inner: File,
    hash: Sha256,
    bytes: u64,
}

impl HashingWriter {
    fn new(inner: File) -> Self {
        Self {
            inner,
            hash: Sha256::new(),
            bytes: 0,
        }
    }
}

impl Write for HashingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.inner.write(bytes)?;
        self.hash.update(&bytes[..count]);
        self.bytes += count as u64;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct HashingReader {
    inner: File,
    hash: Sha256,
    bytes: u64,
}

impl HashingReader {
    fn new(inner: File) -> Self {
        Self {
            inner,
            hash: Sha256::new(),
            bytes: 0,
        }
    }
}

impl Read for HashingReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buffer)?;
        self.hash.update(&buffer[..count]);
        self.bytes += count as u64;
        Ok(count)
    }
}

fn profile_error(error: super::RuntimeProfileError) -> AssetError {
    AssetError::new(AssetErrorKind::BundleInvalid, error.to_string())
}

fn model_error(error: ModelError) -> AssetError {
    AssetError::new(AssetErrorKind::BundleInvalid, error.to_string())
}

fn reference_error(error: ReferenceIndexError) -> AssetError {
    AssetError::new(AssetErrorKind::BundleInvalid, error.to_string())
}

fn mask_error(error: pangopup_index::mask::MaskError) -> AssetError {
    AssetError::new(AssetErrorKind::BundleInvalid, error.to_string())
}

fn invalid_manifest(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::ManifestInvalid, message)
}

fn bundle_invalid(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::BundleInvalid, message)
}

fn part_set(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::PartSetInvalid, message)
}

fn compression_invalid(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::CompressionInvalid, message)
}

fn input_io(operation: &str, error: io::Error) -> AssetError {
    AssetError::new(AssetErrorKind::InputIo, format!("{operation}: {error}"))
}

fn output_io(operation: &str, error: io::Error) -> AssetError {
    AssetError::new(AssetErrorKind::OutputIo, format!("{operation}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_member((name, role, encoding): (&str, &str, Encoding)) -> Member {
        Member {
            name: name.to_owned(),
            role: role.to_owned(),
            encoding,
            uncompressed_bytes: 1,
            uncompressed_sha256: format!("sha256:{}", "1".repeat(64)),
            stored_bytes: 1,
            stored_sha256: format!("sha256:{}", "1".repeat(64)),
        }
    }

    fn test_manifest() -> Manifest {
        Manifest {
            schema: SCHEMA.to_owned(),
            runtime_profile_id: format!("sha256:{}", "2".repeat(64)),
            encoder: expected_encoder().expect("encoder"),
            attribution: Attribution {
                model: "model-NOTICE".to_owned(),
                reference: "reference-NOTICE".to_owned(),
                mask: "mask-NOTICE".to_owned(),
            },
            members: EXPECTED.into_iter().map(test_member).collect(),
        }
    }

    #[test]
    fn canonical_member_order_and_encoder_are_closed() {
        let manifest = test_manifest();
        validate_manifest(&manifest).expect("valid");

        let mut reordered = manifest.clone();
        reordered.members.swap(0, 1);
        assert!(validate_manifest(&reordered).is_err());

        let mut changed_encoder = manifest;
        changed_encoder.encoder.workers = 1;
        assert!(validate_manifest(&changed_encoder).is_err());
    }

    #[test]
    fn raw_members_cannot_claim_distinct_stored_identity() {
        let mut manifest = test_manifest();
        manifest.members[0].stored_bytes = 2;
        assert!(validate_manifest(&manifest).is_err());
    }

    #[test]
    fn cached_transport_decodes_directly_into_atomic_runtime_installation() {
        let temp = TempDir::new().expect("temp");
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let snv_source = fixtures.join("snv-regression/bundle");
        let snv_transport = temp.path().join("snv-transport");
        crate::pack_bundle(&snv_source, &snv_transport).expect("pack SNV");
        let data = temp.path().join("data");
        crate::install_transport(&snv_transport, &data).expect("install SNV");
        let snv = crate::inspect_snv_bundle(&snv_source).expect("inspect SNV");

        let mut profile = crate::parse_runtime_profile(
            &fs::read(fixtures.join("runtime-transport-mini/runtime-profile.json"))
                .expect("profile fixture"),
        )
        .expect("parse profile fixture");
        profile.snv.bundle_id = snv.bundle_id;
        profile.snv.format = snv.format;
        profile.snv.member_bytes = snv.member_bytes;
        profile.snv.member_sha256 = snv.member_sha256;
        let profile_path = temp.path().join("runtime-profile.json");
        fs::write(
            &profile_path,
            crate::canonical_runtime_profile_bytes(&profile).expect("canonical profile"),
        )
        .expect("write profile");
        let transport = temp.path().join("runtime-transport");
        pack_runtime_transport(
            &profile_path,
            &fixtures.join("pangolin-model-kernel-mini/bundle"),
            &fixtures.join("reference-route-test/bundle"),
            &fixtures.join("gencode-mask-mini/domains.pgm"),
            &transport,
        )
        .expect("pack runtime");

        direct_install_audit::reset();
        let installed = install_cached_runtime_transport_for_test(&transport, &data)
            .expect("direct cached install");
        assert_eq!(installed.status, "installed");
        assert!(data.join("runtime/active.json").is_file());
        assert!(!transport.join("model").exists());
        assert!(!transport.join("reference").exists());
        assert!(!transport.join("mask").exists());
        assert_eq!(
            fs::read_dir(&transport)
                .expect("transport inventory")
                .count(),
            10
        );
        let events = direct_install_audit::take();
        assert_eq!(events.len(), 3);
        for event in events {
            let member = parse_runtime_transport_manifest_for_release(
                &fs::read(transport.join(TRANSPORT_MANIFEST)).expect("manifest"),
            )
            .expect("manifest")
            .members
            .into_iter()
            .find(|member| member.name == event.name)
            .expect("audited member");
            assert_eq!(event.cached_read_bytes, member.stored_bytes);
            assert_eq!(event.decoded_bytes, member.uncompressed_bytes);
            assert_eq!(event.final_write_bytes, member.uncompressed_bytes);
        }
    }

    #[test]
    fn production_manifest_substitution_fails_before_notice_can_be_ignored() {
        let authority =
            crate::runtime_release::production_runtime_transport_manifest().expect("authority");
        let mut value: serde_json::Value =
            serde_json::from_slice(authority).expect("transport manifest");
        let notice = value["members"]
            .as_array_mut()
            .expect("members")
            .iter_mut()
            .find(|member| member["name"] == "mask-NOTICE")
            .expect("mask notice");
        notice["stored_sha256"] = serde_json::Value::String(format!("sha256:{}", "a".repeat(64)));
        notice["uncompressed_sha256"] =
            serde_json::Value::String(format!("sha256:{}", "a".repeat(64)));
        let substituted = serde_jcs::to_vec(&value).expect("canonical substitution");
        assert_ne!(substituted, authority);
        let error = require_production_manifest(&substituted).expect_err("substituted authority");
        assert_eq!(error.kind(), AssetErrorKind::ManifestInvalid);
        require_production_manifest(authority).expect("exact authority");
    }
}
