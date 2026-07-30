//! Strict bounded metadata used to prepare the public SNV data release.

use super::{
    AssetError, AssetErrorKind, MAX_SAFE_JSON_U64, TransportInspection, create_stage,
    ensure_output_absent, finish_staged, inspect_transport, open_regular, publish_stage,
    reject_duplicate_json, sha256, sync_directory, write_synced,
};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};

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

pub(crate) fn production_profile()
-> Result<(&'static [u8], &'static str, ReleaseProfile), AssetError> {
    let (_, profile) = validate_production_contract()?;
    Ok((PRODUCTION_PROFILE, PRODUCTION_PROFILE_SHA256, profile))
}

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
