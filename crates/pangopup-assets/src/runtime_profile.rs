//! Canonical, path-free compatibility statement for Pangopup runtime assets.

use pangopup_index::{IndexError, bundle_id, parse_bundle_manifest_bytes};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, SeqAccess, Visitor},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

pub const RUNTIME_PROFILE_SCHEMA: &str = "pangopup.runtime-profile.v1";
pub const MAX_RUNTIME_PROFILE_BYTES: usize = 64 * 1024;
const MAX_SAFE_JSON_U64: u64 = 9_007_199_254_740_991;
const MAX_SNV_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_SNV_NOTICE_BYTES: u64 = 64 * 1024;
const EXPECTED_SNV_MEMBERS: [&str; 3] = ["NOTICE", "manifest.json", "scores.pgi"];

const SNV_BUNDLE_ID: &str =
    "sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3";
const SNV_MEMBER_BYTES: u64 = 15_033_158_255;
const SNV_MEMBER_SHA256: &str =
    "sha256:6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27";
const MODEL_BUNDLE_ID: &str =
    "sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43";
const MODEL_MEMBER_SHA256: &str =
    "sha256:3c2760472ce0af5feb693f562716b6cdc6887a7d0a00b7b5ec8ddad2a2d31f6b";
const REFERENCE_BUNDLE_ID: &str =
    "sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f";
const REFERENCE_SEQUENCE_SET_SHA256: &str =
    "sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4";
const REFERENCE_MEMBER_SHA256: &str =
    "sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82";
const MASK_MEMBER_SHA256: &str =
    "sha256:714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProfile {
    pub schema: String,
    pub snv: SnvProfile,
    pub model: ModelProfile,
    pub reference: ReferenceProfile,
    pub mask: MaskProfile,
    pub scoring: ScoringProfile,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnvProfile {
    pub bundle_id: String,
    pub format: String,
    pub member_bytes: u64,
    pub member_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelProfile {
    pub bundle_id: String,
    pub profile: String,
    pub representation: String,
    pub member_bytes: u64,
    pub member_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceProfile {
    pub bundle_id: String,
    pub profile: String,
    pub format: String,
    pub assembly: String,
    pub assembly_accession: String,
    pub sequence_set_sha256: String,
    pub member_bytes: u64,
    pub member_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskProfile {
    pub format: String,
    pub member_bytes: u64,
    pub member_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoringProfile {
    pub assembly: String,
    pub semantics: String,
    pub distance: u64,
    pub masking_policy: String,
    pub cpu_policy: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeProfileId(String);

impl RuntimeProfileId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuntimeProfileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnvBundleInspection {
    pub bundle_id: String,
    pub format: String,
    pub member_bytes: u64,
    pub member_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeProfileError {
    TooLarge,
    InvalidJson,
    NonCanonical,
    InvalidFacts,
    Incompatible,
    UnsafeInput,
    InputIo,
}

impl fmt::Display for RuntimeProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "runtime profile exceeds its size bound",
            Self::InvalidJson => "runtime profile JSON is invalid",
            Self::NonCanonical => "runtime profile is not canonical RFC 8785 JSON",
            Self::InvalidFacts => "runtime profile facts are invalid",
            Self::Incompatible => "runtime profile is not the trusted production tuple",
            Self::UnsafeInput => "runtime profile input is unsafe or changed",
            Self::InputIo => "runtime profile input I/O failed",
        })
    }
}

impl std::error::Error for RuntimeProfileError {}

pub fn production_runtime_profile() -> RuntimeProfile {
    RuntimeProfile {
        schema: RUNTIME_PROFILE_SCHEMA.to_owned(),
        snv: SnvProfile {
            bundle_id: SNV_BUNDLE_ID.to_owned(),
            format: "pangopup.fixed11.v1".to_owned(),
            member_bytes: SNV_MEMBER_BYTES,
            member_sha256: SNV_MEMBER_SHA256.to_owned(),
        },
        model: ModelProfile {
            bundle_id: MODEL_BUNDLE_ID.to_owned(),
            profile: "pangolin-1.0.2-5cf94b8-onnx-cpu-v1".to_owned(),
            representation: "singleton".to_owned(),
            member_bytes: 33_867_142,
            member_sha256: MODEL_MEMBER_SHA256.to_owned(),
        },
        reference: ReferenceProfile {
            bundle_id: REFERENCE_BUNDLE_ID.to_owned(),
            profile: "refseq-grch38p14-primary-v1".to_owned(),
            format: "pangopup.reference.acgt2-rle.v1".to_owned(),
            assembly: "GRCh38.p14".to_owned(),
            assembly_accession: "GCF_000001405.40".to_owned(),
            sequence_set_sha256: REFERENCE_SEQUENCE_SET_SHA256.to_owned(),
            member_bytes: 772_091_760,
            member_sha256: REFERENCE_MEMBER_SHA256.to_owned(),
        },
        mask: MaskProfile {
            format: "pangopup.gencode-v38-domains.v1".to_owned(),
            member_bytes: 6_703_320,
            member_sha256: MASK_MEMBER_SHA256.to_owned(),
        },
        scoring: ScoringProfile {
            assembly: "GRCh38".to_owned(),
            semantics: "pangopup-variant-score-v1".to_owned(),
            distance: 50,
            masking_policy: "pangolin-gencode-v38-order-sensitive-v1".to_owned(),
            cpu_policy: "sequential:1/1".to_owned(),
        },
    }
}

pub fn canonical_runtime_profile_bytes(
    profile: &RuntimeProfile,
) -> Result<Vec<u8>, RuntimeProfileError> {
    validate_profile(profile)?;
    let bytes = serde_jcs::to_vec(profile).map_err(|_| RuntimeProfileError::InvalidFacts)?;
    if bytes.len() > MAX_RUNTIME_PROFILE_BYTES {
        return Err(RuntimeProfileError::TooLarge);
    }
    Ok(bytes)
}

pub fn parse_runtime_profile(bytes: &[u8]) -> Result<RuntimeProfile, RuntimeProfileError> {
    if bytes.len() > MAX_RUNTIME_PROFILE_BYTES {
        return Err(RuntimeProfileError::TooLarge);
    }
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(RuntimeProfileError::InvalidJson);
    }
    let mut duplicate_check = serde_json::Deserializer::from_slice(bytes);
    NoDuplicateJson::deserialize(&mut duplicate_check)
        .map_err(|_| RuntimeProfileError::InvalidJson)?;
    duplicate_check
        .end()
        .map_err(|_| RuntimeProfileError::InvalidJson)?;
    let profile: RuntimeProfile =
        serde_json::from_slice(bytes).map_err(|_| RuntimeProfileError::InvalidJson)?;
    validate_profile(&profile)?;
    if canonical_runtime_profile_bytes(&profile)? != bytes {
        return Err(RuntimeProfileError::NonCanonical);
    }
    Ok(profile)
}

pub fn runtime_profile_id(bytes: &[u8]) -> Result<RuntimeProfileId, RuntimeProfileError> {
    parse_runtime_profile(bytes)?;
    Ok(RuntimeProfileId(format!(
        "sha256:{:x}",
        Sha256::digest(bytes)
    )))
}

impl RuntimeProfile {
    pub fn require_trusted_production(&self) -> Result<(), RuntimeProfileError> {
        if self == &production_runtime_profile() {
            Ok(())
        } else {
            Err(RuntimeProfileError::Incompatible)
        }
    }
}

fn validate_profile(profile: &RuntimeProfile) -> Result<(), RuntimeProfileError> {
    let integers = [
        profile.snv.member_bytes,
        profile.model.member_bytes,
        profile.reference.member_bytes,
        profile.mask.member_bytes,
        profile.scoring.distance,
    ];
    if profile.schema != RUNTIME_PROFILE_SCHEMA
        || integers.into_iter().any(|value| value > MAX_SAFE_JSON_U64)
        || !valid_sha(&profile.snv.bundle_id)
        || !valid_sha(&profile.snv.member_sha256)
        || !valid_sha(&profile.model.bundle_id)
        || !valid_sha(&profile.model.member_sha256)
        || !valid_sha(&profile.reference.bundle_id)
        || !valid_sha(&profile.reference.sequence_set_sha256)
        || !valid_sha(&profile.reference.member_sha256)
        || !valid_sha(&profile.mask.member_sha256)
        || profile.snv.format.is_empty()
        || profile.model.profile.is_empty()
        || profile.model.representation.is_empty()
        || profile.reference.profile.is_empty()
        || profile.reference.format.is_empty()
        || profile.reference.assembly.is_empty()
        || profile.reference.assembly_accession.is_empty()
        || profile.mask.format.is_empty()
        || profile.scoring.assembly.is_empty()
        || profile.scoring.semantics.is_empty()
        || profile.scoring.masking_policy.is_empty()
        || profile.scoring.cpu_policy.is_empty()
    {
        return Err(RuntimeProfileError::InvalidFacts);
    }
    Ok(())
}

fn valid_sha(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Inspect only the bounded SNV bundle metadata and score-member file shape.
///
/// The score descriptor is retained and checked for pathname replacement, but
/// its 15 GB payload is never read, hashed, decoded, or mapped here.
pub fn inspect_snv_bundle(path: &Path) -> Result<SnvBundleInspection, RuntimeProfileError> {
    inspect_snv_bundle_with(path, || {})
}

fn inspect_snv_bundle_with(
    path: &Path,
    after_members_open: impl FnOnce(),
) -> Result<SnvBundleInspection, RuntimeProfileError> {
    let root_before = fs::symlink_metadata(path).map_err(|_| RuntimeProfileError::InputIo)?;
    if !root_before.file_type().is_dir() || root_before.file_type().is_symlink() {
        return Err(RuntimeProfileError::UnsafeInput);
    }
    let mut names = BTreeSet::new();
    for (index, entry) in fs::read_dir(path)
        .map_err(|_| RuntimeProfileError::InputIo)?
        .enumerate()
    {
        if index >= EXPECTED_SNV_MEMBERS.len() {
            return Err(RuntimeProfileError::UnsafeInput);
        }
        let name = entry
            .map_err(|_| RuntimeProfileError::InputIo)?
            .file_name()
            .into_string()
            .map_err(|_| RuntimeProfileError::UnsafeInput)?;
        names.insert(name);
    }
    if names
        != EXPECTED_SNV_MEMBERS
            .into_iter()
            .map(str::to_owned)
            .collect()
    {
        return Err(RuntimeProfileError::UnsafeInput);
    }

    let (mut manifest_file, manifest_before) =
        open_held(path.join("manifest.json"), MAX_SNV_MANIFEST_BYTES)?;
    let (mut notice_file, notice_before) = open_held(path.join("NOTICE"), MAX_SNV_NOTICE_BYTES)?;
    let (scores_file, scores_before) = open_held(path.join("scores.pgi"), SNV_MEMBER_BYTES)?;
    after_members_open();
    let manifest_bytes = read_held(&mut manifest_file, &manifest_before, MAX_SNV_MANIFEST_BYTES)?;
    let notice_bytes = read_held(&mut notice_file, &notice_before, MAX_SNV_NOTICE_BYTES)?;
    let manifest = parse_bundle_manifest_bytes(&manifest_bytes).map_err(|error| match error {
        IndexError::Io(_) => RuntimeProfileError::InputIo,
        IndexError::Incompatible(_) => RuntimeProfileError::Incompatible,
        IndexError::InvalidInput(_) | IndexError::Corrupt(_) | IndexError::Arithmetic(_) => {
            RuntimeProfileError::InvalidFacts
        }
    })?;
    let notice = manifest
        .members
        .iter()
        .find(|member| member.path == "NOTICE")
        .ok_or(RuntimeProfileError::InvalidFacts)?;
    let scores = manifest
        .members
        .iter()
        .find(|member| member.path == "scores.pgi")
        .ok_or(RuntimeProfileError::InvalidFacts)?;
    if notice.size != notice_bytes.len() as u64
        || notice.sha256 != format!("sha256:{:x}", Sha256::digest(&notice_bytes))
        || scores.size != scores_before.len()
        || !valid_sha(&scores.sha256)
    {
        return Err(RuntimeProfileError::InvalidFacts);
    }
    validate_path(path.join("manifest.json"), &manifest_file, &manifest_before)?;
    validate_path(path.join("NOTICE"), &notice_file, &notice_before)?;
    validate_path(path.join("scores.pgi"), &scores_file, &scores_before)?;
    let root_after = fs::symlink_metadata(path).map_err(|_| RuntimeProfileError::InputIo)?;
    if !same_inode(&root_before, &root_after) {
        return Err(RuntimeProfileError::UnsafeInput);
    }
    Ok(SnvBundleInspection {
        bundle_id: bundle_id(&manifest_bytes),
        format: manifest.index_format,
        member_bytes: scores.size,
        member_sha256: scores.sha256.clone(),
    })
}

fn open_held(
    path: impl AsRef<Path>,
    maximum: u64,
) -> Result<(File, fs::Metadata), RuntimeProfileError> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let file = options.open(path).map_err(|error| {
        if error.raw_os_error() == Some(libc::ELOOP) {
            RuntimeProfileError::UnsafeInput
        } else {
            RuntimeProfileError::InputIo
        }
    })?;
    let metadata = file.metadata().map_err(|_| RuntimeProfileError::InputIo)?;
    if !metadata.file_type().is_file() || metadata.nlink() != 1 || metadata.len() > maximum {
        return Err(RuntimeProfileError::UnsafeInput);
    }
    Ok((file, metadata))
}

fn read_held(
    file: &mut File,
    expected: &fs::Metadata,
    maximum: u64,
) -> Result<Vec<u8>, RuntimeProfileError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_| RuntimeProfileError::InputIo)?;
    let mut bytes = Vec::with_capacity(expected.len() as usize);
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| RuntimeProfileError::InputIo)?;
    if bytes.len() as u64 != expected.len() {
        return Err(RuntimeProfileError::UnsafeInput);
    }
    Ok(bytes)
}

fn validate_path(
    path: impl AsRef<Path>,
    file: &File,
    before: &fs::Metadata,
) -> Result<(), RuntimeProfileError> {
    let held_after = file.metadata().map_err(|_| RuntimeProfileError::InputIo)?;
    let path_after =
        fs::symlink_metadata(path.as_ref()).map_err(|_| RuntimeProfileError::UnsafeInput)?;
    if !same_inode(before, &held_after)
        || !same_inode(before, &path_after)
        || path_after.file_type().is_symlink()
    {
        return Err(RuntimeProfileError::UnsafeInput);
    }
    Ok(())
}

fn same_inode(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.nlink() == right.nlink()
}

struct NoDuplicateJson;

impl<'de> Deserialize<'de> for NoDuplicateJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(NoDuplicateVisitor)
    }
}

struct NoDuplicateVisitor;

impl<'de> Visitor<'de> for NoDuplicateVisitor {
    type Value = NoDuplicateJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut names = BTreeSet::new();
        while let Some(name) = map.next_key::<String>()? {
            if !names.insert(name) {
                return Err(serde::de::Error::custom("duplicate key"));
            }
            map.next_value::<NoDuplicateJson>()?;
        }
        Ok(NoDuplicateJson)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<NoDuplicateJson>()?.is_some() {}
        Ok(NoDuplicateJson)
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(NoDuplicateJson)
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(NoDuplicateJson)
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(NoDuplicateJson)
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(NoDuplicateJson)
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(NoDuplicateJson)
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(NoDuplicateJson)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicateJson)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        NoDuplicateJson::deserialize(deserializer)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicateJson)
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        NoDuplicateJson::deserialize(deserializer)
    }

    fn visit_bytes<E>(self, _value: &[u8]) -> Result<Self::Value, E> {
        Ok(NoDuplicateJson)
    }

    fn visit_byte_buf<E>(self, _value: Vec<u8>) -> Result<Self::Value, E> {
        Ok(NoDuplicateJson)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;
    use tempfile::tempdir;

    fn fixture_bundle() -> &'static Path {
        Path::new("../../tests/fixtures/snv-regression/bundle")
    }

    #[test]
    fn production_profile_is_canonical_round_trips_and_has_external_identity() {
        let profile = production_runtime_profile();
        let first = canonical_runtime_profile_bytes(&profile).expect("canonical");
        let second = canonical_runtime_profile_bytes(&profile).expect("canonical again");
        assert_eq!(first, second);
        assert_eq!(parse_runtime_profile(&first).expect("parse"), profile);
        assert_eq!(
            runtime_profile_id(&first).expect("identity").as_str(),
            format!("sha256:{:x}", Sha256::digest(&first))
        );
        assert!(!first.ends_with(b"\n"));
        profile.require_trusted_production().expect("trusted tuple");
    }

    #[test]
    fn every_leaf_fact_changes_the_profile_identity() {
        let bytes =
            canonical_runtime_profile_bytes(&production_runtime_profile()).expect("profile");
        let baseline = runtime_profile_id(&bytes).expect("identity");
        let value: Value = serde_json::from_slice(&bytes).expect("value");
        let paths = [
            "/snv/bundle_id",
            "/snv/format",
            "/snv/member_bytes",
            "/snv/member_sha256",
            "/model/bundle_id",
            "/model/profile",
            "/model/representation",
            "/model/member_bytes",
            "/model/member_sha256",
            "/reference/bundle_id",
            "/reference/profile",
            "/reference/format",
            "/reference/assembly",
            "/reference/assembly_accession",
            "/reference/sequence_set_sha256",
            "/reference/member_bytes",
            "/reference/member_sha256",
            "/mask/format",
            "/mask/member_bytes",
            "/mask/member_sha256",
            "/scoring/assembly",
            "/scoring/semantics",
            "/scoring/distance",
            "/scoring/masking_policy",
            "/scoring/cpu_policy",
        ];
        for path in paths {
            let mut changed = value.clone();
            let slot = changed.pointer_mut(path).expect("leaf");
            if slot.is_number() {
                *slot = Value::from(slot.as_u64().expect("u64") + 1);
            } else if path.ends_with("sha256") || path.ends_with("bundle_id") {
                *slot = Value::from(
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                );
            } else {
                *slot = Value::from(format!("{}-changed", slot.as_str().expect("string")));
            }
            let candidate: RuntimeProfile = serde_json::from_value(changed).expect("typed");
            let candidate_bytes =
                canonical_runtime_profile_bytes(&candidate).expect("candidate bytes");
            assert_eq!(
                candidate.require_trusted_production(),
                Err(RuntimeProfileError::Incompatible),
                "{path}"
            );
            assert_ne!(
                runtime_profile_id(&candidate_bytes).expect("candidate identity"),
                baseline,
                "{path}"
            );
        }
    }

    #[test]
    fn parser_rejects_extensions_duplicates_noncanonical_and_unsafe_integers() {
        let bytes =
            canonical_runtime_profile_bytes(&production_runtime_profile()).expect("profile");
        let mut value: Value = serde_json::from_slice(&bytes).expect("value");
        value
            .as_object_mut()
            .expect("object")
            .insert("extension".to_owned(), Value::Bool(true));
        assert_eq!(
            parse_runtime_profile(&serde_jcs::to_vec(&value).expect("extended")),
            Err(RuntimeProfileError::InvalidJson)
        );
        let duplicate = String::from_utf8(bytes.clone()).expect("UTF-8").replacen(
            "{",
            "{\"schema\":\"pangopup.runtime-profile.v1\",",
            1,
        );
        assert_eq!(
            parse_runtime_profile(duplicate.as_bytes()),
            Err(RuntimeProfileError::InvalidJson)
        );
        let mut newline = bytes.clone();
        newline.push(b'\n');
        assert_eq!(
            parse_runtime_profile(&newline),
            Err(RuntimeProfileError::NonCanonical)
        );
        let mut unsafe_integer: Value = serde_json::from_slice(&bytes).expect("value");
        unsafe_integer["scoring"]["distance"] = Value::from(MAX_SAFE_JSON_U64 + 1);
        assert_eq!(
            parse_runtime_profile(&serde_jcs::to_vec(&unsafe_integer).expect("unsafe")),
            Err(RuntimeProfileError::InvalidFacts)
        );
        assert_eq!(
            parse_runtime_profile(&vec![b' '; MAX_RUNTIME_PROFILE_BYTES + 1]),
            Err(RuntimeProfileError::TooLarge)
        );
    }

    #[test]
    fn grammar_valid_synthetic_profile_is_not_trusted() {
        let mut profile = production_runtime_profile();
        profile.scoring.cpu_policy = "sequential:2/1".to_owned();
        let bytes = canonical_runtime_profile_bytes(&profile).expect("synthetic");
        let parsed = parse_runtime_profile(&bytes).expect("grammar valid");
        assert_eq!(
            parsed.require_trusted_production(),
            Err(RuntimeProfileError::Incompatible)
        );
    }

    #[test]
    fn snv_inspection_reads_metadata_not_score_payload_and_reports_declared_identity() {
        let inspected = inspect_snv_bundle(fixture_bundle()).expect("inspect fixture");
        let manifest_bytes =
            fs::read(fixture_bundle().join("manifest.json")).expect("fixture manifest");
        let manifest = parse_bundle_manifest_bytes(&manifest_bytes).expect("parse fixture");
        let scores = manifest
            .members
            .iter()
            .find(|member| member.path == "scores.pgi")
            .expect("scores");
        assert_eq!(inspected.bundle_id, bundle_id(&manifest_bytes));
        assert_eq!(inspected.format, "pangopup.fixed11.v1");
        assert_eq!(inspected.member_bytes, scores.size);
        assert_eq!(inspected.member_sha256, scores.sha256);
    }

    #[test]
    fn snv_inspection_rejects_pathname_replacement_after_descriptors_are_held() {
        let temp = tempdir().expect("temp");
        let bundle = temp.path().join("bundle");
        fs::create_dir(&bundle).expect("bundle");
        for name in EXPECTED_SNV_MEMBERS {
            fs::copy(fixture_bundle().join(name), bundle.join(name)).expect("copy fixture");
        }
        let result = inspect_snv_bundle_with(&bundle, || {
            let scores = bundle.join("scores.pgi");
            let held = bundle.join("held.pgi");
            fs::rename(&scores, held).expect("hold original");
            fs::copy(fixture_bundle().join("scores.pgi"), scores).expect("replacement");
        });
        assert_eq!(result, Err(RuntimeProfileError::UnsafeInput));
    }

    #[test]
    fn snv_member_enumeration_stops_at_the_fourth_entry() {
        let temp = tempdir().expect("temp");
        for ordinal in 0..64 {
            fs::write(temp.path().join(format!("member-{ordinal:02}")), b"x").expect("member");
        }
        assert_eq!(
            inspect_snv_bundle(temp.path()),
            Err(RuntimeProfileError::UnsafeInput)
        );
    }
}
