//! Offline preparation of the exact model-side runtime GitHub release.

use super::{
    AssetError, AssetErrorKind, ensure_output_absent, finish_staged,
    runtime_transport::{
        Encoding, parse_runtime_transport_manifest_for_release, read_runtime_transport_held_raw,
        verify_runtime_transport_held_frame,
    },
    sha256, sync_directory,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(any(test, feature = "test-read-audit"))]
use std::cell::RefCell;
use std::{
    collections::BTreeSet,
    ffi::CString,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    os::{
        fd::{AsRawFd, FromRawFd, RawFd},
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::Path,
};

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(test, feature = "test-read-audit"))]
pub enum RuntimeReleaseFaultPoint {
    Copy,
    FileSync,
    StageSync,
    Publication,
    ParentSync,
    SourceReplacement,
    VerifiedSourceReplacement,
    PathAdmissionFifoReplacement,
}

#[cfg(any(test, feature = "test-read-audit"))]
thread_local! {
    static FAULT: RefCell<Option<RuntimeReleaseFaultPoint>> = const { RefCell::new(None) };
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-read-audit"))]
pub fn set_runtime_release_fault(point: RuntimeReleaseFaultPoint) {
    FAULT.set(Some(point));
}

#[cfg(any(test, feature = "test-read-audit"))]
fn fail_at(point: RuntimeReleaseFaultPoint) -> bool {
    FAULT.with_borrow_mut(|fault| {
        if *fault == Some(point) {
            *fault = None;
            true
        } else {
            false
        }
    })
}

const SCHEMA: &str = "pangopup.runtime-release-profile.v1";
const REPOSITORY: &str = "genomoncology/pangopup";
const TAG: &str = "runtime-grch38-v1";
const TITLE: &str = "Pangopup GRCh38 Pangolin runtime v1";
const TRANSPORT_ID: &str =
    "sha256:415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3";
const PROFILE_ID: &str = "sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c";
const PRODUCTION_RELEASE_PROFILE: &[u8] =
    include_bytes!("../../../release-profiles/runtime-release-profile.json");
const PRODUCTION_TRANSPORT_MANIFEST: &[u8] =
    include_bytes!("../../../release-profiles/runtime-transport.json");
const PRODUCTION_RELEASE_PROFILE_SHA256: &str =
    "sha256:d1caf6346bb24378f720056416fa6286f1153ccaf0c6a0778494f557035ef59e";
const SNV_ID: &str = "sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3";
const MODEL_ID: &str = "sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43";
const REFERENCE_ID: &str =
    "sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f";
const MASK_ID: &str = "sha256:714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702";
const UPSTREAM_COMMIT: &str = "5cf94b8db938c658391b4305cd7ce33297d44ff7";
const PRODUCTION_MEMBERS: [ExpectedMember<'static>; 10] = [
    ExpectedMember {
        name: "runtime-transport.json",
        role: "runtime-transport-manifest",
        size: 3_179,
        sha256: TRANSPORT_ID,
    },
    ExpectedMember {
        name: "runtime-profile.json",
        role: "runtime-profile",
        size: 1_366,
        sha256: PROFILE_ID,
    },
    ExpectedMember {
        name: "model-manifest.json",
        role: "model-manifest",
        size: 3_823,
        sha256: MODEL_ID,
    },
    ExpectedMember {
        name: "model-NOTICE",
        role: "model-attribution",
        size: 648,
        sha256: "sha256:fbba767913348642351d7e95b8589619a8bb4a7f3738c5ea6fe266c21434107f",
    },
    ExpectedMember {
        name: "model.onnx.zst",
        role: "model-member",
        size: 31_144_867,
        sha256: "sha256:741642c98c0aae6a76d4096780c114ba9bd497122868ba0ecf2d85a30d8af568",
    },
    ExpectedMember {
        name: "reference-manifest.json",
        role: "reference-manifest",
        size: 3_719,
        sha256: REFERENCE_ID,
    },
    ExpectedMember {
        name: "reference-NOTICE",
        role: "reference-attribution",
        size: 793,
        sha256: "sha256:1e3ce49d78cd9089407c54ce92a9e6d3adb92a9f3267185ba9ea64df8a588499",
    },
    ExpectedMember {
        name: "reference.pgr.zst",
        role: "reference-member",
        size: 656_781_805,
        sha256: "sha256:e181eb31a76c8e05782415317450c92d5d7a148cb28afd184e0c7767aa42cc25",
    },
    ExpectedMember {
        name: "mask-NOTICE",
        role: "mask-attribution",
        size: 978,
        sha256: "sha256:d8ee279f7a97ae25d2bf502b42a4fb480234cc517c0b58f85d6cf6547995bbeb",
    },
    ExpectedMember {
        name: "domains.pgm.zst",
        role: "mask-member",
        size: 3_933_486,
        sha256: "sha256:e8353beba3820e3c4679acb46a673622080fe6f560a02b558bc6d75f50286747",
    },
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeReleaseProfile {
    pub schema: String,
    pub profile: String,
    pub repository: String,
    pub release: RuntimeRelease,
    pub runtime: RuntimeReleaseTuple,
    pub model_source: ModelSource,
    pub transport: RuntimeReleaseTransport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRelease {
    pub tag: String,
    pub title: String,
    pub target_commit: String,
    pub page_url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeReleaseTuple {
    pub profile_id: String,
    pub snv_bundle_id: String,
    pub model_bundle_id: String,
    pub reference_bundle_id: String,
    pub mask_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSource {
    pub license: String,
    pub upstream_repository: String,
    pub upstream_commit: String,
    pub model_py: SourceFile,
    pub license_file: SourceFile,
    pub checkpoint_set: String,
    pub checkpoints: Vec<Checkpoint>,
    pub converter_repository: String,
    pub converter_commit: String,
    pub converter_paths: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub ordinal: u8,
    pub name: String,
    pub size: u64,
    pub sha256: String,
    pub url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeReleaseTransport {
    pub schema: String,
    pub transport_id: String,
    pub members: Vec<RuntimeReleaseMember>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeReleaseMember {
    pub logical_path: String,
    pub role: String,
    pub asset_name: String,
    pub size: u64,
    pub sha256: String,
    pub url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PrepareRuntimeReleaseOutcome {
    pub status: &'static str,
    pub command: &'static str,
    pub tag: &'static str,
    pub target_commit: String,
    pub transport_id: String,
    pub runtime_profile_id: String,
    pub upload_asset_count: usize,
}

#[doc(hidden)]
#[derive(Clone, Copy)]
#[cfg(any(test, feature = "test-read-audit"))]
pub struct RuntimeReleasePreparationContract<'a> {
    pub transport_id: &'a str,
    pub runtime_profile_id: &'a str,
    pub members: &'a [RuntimeReleaseExpectedMember<'a>],
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(test, feature = "test-read-audit"))]
pub struct RuntimeReleaseExpectedMember<'a> {
    pub name: &'a str,
    pub role: &'a str,
    pub size: u64,
    pub sha256: &'a str,
}

#[derive(Clone, Copy)]
struct Contract<'a> {
    transport_id: &'a str,
    runtime_profile_id: &'a str,
    members: &'a [ExpectedMember<'a>],
}

#[derive(Clone, Copy)]
struct ExpectedMember<'a> {
    name: &'a str,
    role: &'a str,
    size: u64,
    sha256: &'a str,
}

pub fn parse_runtime_release_profile(bytes: &[u8]) -> Result<RuntimeReleaseProfile, AssetError> {
    parse_profile_with_contract(
        bytes,
        Contract {
            transport_id: TRANSPORT_ID,
            runtime_profile_id: PROFILE_ID,
            members: &PRODUCTION_MEMBERS,
        },
    )
}

pub(crate) fn production_runtime_release_profile()
-> Result<(&'static str, RuntimeReleaseProfile), AssetError> {
    if super::sha256(PRODUCTION_RELEASE_PROFILE) != PRODUCTION_RELEASE_PROFILE_SHA256
        || super::sha256(PRODUCTION_TRANSPORT_MANIFEST) != TRANSPORT_ID
    {
        return Err(release_invalid(
            "compiled runtime release authority identity mismatch",
        ));
    }
    let profile = parse_runtime_release_profile(PRODUCTION_RELEASE_PROFILE)?;
    let transport = parse_runtime_transport_manifest_for_release(PRODUCTION_TRANSPORT_MANIFEST)?;
    let manifest_member = profile
        .transport
        .members
        .first()
        .ok_or_else(|| release_invalid("runtime transport manifest member is missing"))?;
    if manifest_member.asset_name != "runtime-transport.json"
        || manifest_member.size != PRODUCTION_TRANSPORT_MANIFEST.len() as u64
        || manifest_member.sha256 != TRANSPORT_ID
        || transport.runtime_profile_id != profile.runtime.profile_id
        || transport.members.len() + 1 != profile.transport.members.len()
    {
        return Err(release_invalid("compiled runtime authorities disagree"));
    }
    for (outer, inner) in profile
        .transport
        .members
        .iter()
        .skip(1)
        .zip(&transport.members)
    {
        if outer.asset_name != inner.name
            || outer.logical_path != inner.name
            || outer.role != inner.role
            || outer.size != inner.stored_bytes
            || outer.sha256 != inner.stored_sha256
        {
            return Err(release_invalid("compiled runtime authorities disagree"));
        }
    }
    Ok((PRODUCTION_RELEASE_PROFILE_SHA256, profile))
}

pub(crate) fn production_runtime_transport_manifest() -> Result<&'static [u8], AssetError> {
    production_runtime_release_profile()?;
    Ok(PRODUCTION_TRANSPORT_MANIFEST)
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-read-audit"))]
pub fn parse_runtime_release_profile_with_contract(
    bytes: &[u8],
    contract: RuntimeReleasePreparationContract<'_>,
) -> Result<RuntimeReleaseProfile, AssetError> {
    let members: Vec<_> = contract
        .members
        .iter()
        .map(|member| ExpectedMember {
            name: member.name,
            role: member.role,
            size: member.size,
            sha256: member.sha256,
        })
        .collect();
    parse_profile_with_contract(
        bytes,
        Contract {
            transport_id: contract.transport_id,
            runtime_profile_id: contract.runtime_profile_id,
            members: &members,
        },
    )
}

fn parse_profile_with_contract(
    bytes: &[u8],
    contract: Contract<'_>,
) -> Result<RuntimeReleaseProfile, AssetError> {
    super::reject_duplicate_json(bytes)
        .map_err(|_| release_invalid("runtime release profile contains duplicate JSON"))?;
    let profile: RuntimeReleaseProfile = serde_json::from_slice(bytes)
        .map_err(|_| release_invalid("runtime release profile is not closed v1 JSON"))?;
    let canonical = serde_jcs::to_vec(&profile)
        .map_err(|_| release_invalid("cannot canonicalize runtime release profile"))?;
    if canonical != bytes {
        return Err(release_invalid(
            "runtime release profile is not canonical RFC 8785 JSON",
        ));
    }
    validate_profile(&profile, contract)?;
    Ok(profile)
}

pub fn prepare_runtime_release(
    transport: &Path,
    target_commit: &str,
    output: &Path,
) -> Result<PrepareRuntimeReleaseOutcome, AssetError> {
    prepare_with_contract(
        transport,
        target_commit,
        output,
        Contract {
            transport_id: TRANSPORT_ID,
            runtime_profile_id: PROFILE_ID,
            members: &PRODUCTION_MEMBERS,
        },
    )
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-read-audit"))]
pub fn prepare_runtime_release_with_contract(
    transport: &Path,
    target_commit: &str,
    output: &Path,
    contract: RuntimeReleasePreparationContract<'_>,
) -> Result<PrepareRuntimeReleaseOutcome, AssetError> {
    let members: Vec<_> = contract
        .members
        .iter()
        .map(|member| ExpectedMember {
            name: member.name,
            role: member.role,
            size: member.size,
            sha256: member.sha256,
        })
        .collect();
    prepare_with_contract(
        transport,
        target_commit,
        output,
        Contract {
            transport_id: contract.transport_id,
            runtime_profile_id: contract.runtime_profile_id,
            members: &members,
        },
    )
}

fn prepare_with_contract(
    transport: &Path,
    target_commit: &str,
    output: &Path,
    contract: Contract<'_>,
) -> Result<PrepareRuntimeReleaseOutcome, AssetError> {
    super::require_linux()?;
    if !valid_commit(target_commit) {
        return Err(release_invalid(
            "target commit must be exactly 40 lowercase hexadecimal characters",
        ));
    }
    ensure_output_absent(output)?;
    let source = SourceDirectory::open(transport)?;
    let inspection = source.inspect_and_verify(contract)?;
    if inspection.transport_id != contract.transport_id
        || inspection.runtime_profile_id != contract.runtime_profile_id
        || !inspection.matches_contract(contract)
    {
        return Err(release_invalid(
            "runtime transport does not match the reviewed release contract",
        ));
    }
    #[cfg(any(test, feature = "test-read-audit"))]
    if fail_at(RuntimeReleaseFaultPoint::VerifiedSourceReplacement) {
        replace_source_member_for_test(&source, inspection.members[0].name.as_str())?;
    }

    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| output_io("create output parent", error))?;
    let (stage, mut guard) = super::create_stage(output)?;
    let result = (|| {
        let mut copied = Vec::with_capacity(inspection.members.len());
        for member in &inspection.members {
            let destination = stage.join(&member.name);
            copy_member(&source, member, &destination)?;
            copied.push((member.name.as_str(), member.sha256.as_str()));
        }
        #[cfg(any(test, feature = "test-read-audit"))]
        if fail_at(RuntimeReleaseFaultPoint::SourceReplacement) {
            replace_source_member_for_test(&source, &inspection.members[0].name)?;
        }
        source.require_inventory(
            &inspection
                .members
                .iter()
                .map(|member| member.name.as_str())
                .collect::<Vec<_>>(),
        )?;
        for member in &inspection.members {
            source.validate_member(&member.name, &member.file, &member.metadata)?;
        }
        source.require_same_path(transport)?;

        let members = inspection
            .members
            .iter()
            .map(|member| RuntimeReleaseMember {
                logical_path: member.name.clone(),
                role: member.role.clone(),
                asset_name: member.name.clone(),
                size: member.size,
                sha256: member.sha256.clone(),
                url: format!(
                    "https://github.com/{REPOSITORY}/releases/download/{TAG}/{}",
                    member.name
                ),
            })
            .collect();
        let profile = production_profile(
            target_commit,
            contract.runtime_profile_id,
            contract.transport_id,
            members,
        );
        let profile_bytes = serde_jcs::to_vec(&profile)
            .map_err(|_| release_invalid("cannot serialize runtime release profile"))?;
        parse_profile_with_contract(&profile_bytes, contract)?;
        let profile_hash = sha256(&profile_bytes);
        write_private(&stage.join("runtime-release-profile.json"), &profile_bytes)?;
        let sums = sha256sums(&copied, &profile_hash);
        write_private(&stage.join("SHA256SUMS"), sums.as_bytes())?;
        write_private(
            &stage.join("RELEASE-NOTES.md"),
            release_notes(&profile).as_bytes(),
        )?;

        for entry in fs::read_dir(&stage).map_err(|error| output_io("list staging", error))? {
            let path = entry
                .map_err(|error| output_io("read staging entry", error))?
                .path();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o400))
                .map_err(|error| output_io("set staged file mode", error))?;
            let file = File::open(&path).map_err(|error| output_io("reopen staged file", error))?;
            #[cfg(any(test, feature = "test-read-audit"))]
            if fail_at(RuntimeReleaseFaultPoint::FileSync) {
                return Err(output_io(
                    "sync staged file",
                    io::Error::other("injected failure"),
                ));
            }
            file.sync_all()
                .map_err(|error| output_io("sync staged file", error))?;
        }
        #[cfg(any(test, feature = "test-read-audit"))]
        if fail_at(RuntimeReleaseFaultPoint::StageSync) {
            return Err(output_io(
                "sync staged directory",
                io::Error::other("injected failure"),
            ));
        }
        sync_directory(&stage)?;
        fs::set_permissions(&stage, fs::Permissions::from_mode(0o500))
            .map_err(|error| output_io("set staged directory mode", error))?;
        File::open(&stage)
            .and_then(|file| file.sync_all())
            .map_err(|error| output_io("sync staged directory", error))?;
        publish_read_only_stage(&stage, output, &mut guard)?;
        Ok(PrepareRuntimeReleaseOutcome {
            status: "ok",
            command: "runtime-release.prepare",
            tag: TAG,
            target_commit: target_commit.to_owned(),
            transport_id: inspection.transport_id,
            runtime_profile_id: inspection.runtime_profile_id,
            upload_asset_count: 12,
        })
    })();
    finish_staged(result, &mut guard)
}

fn production_profile(
    target_commit: &str,
    runtime_profile_id: &str,
    transport_id: &str,
    members: Vec<RuntimeReleaseMember>,
) -> RuntimeReleaseProfile {
    RuntimeReleaseProfile {
        schema: SCHEMA.to_owned(),
        profile: TAG.to_owned(),
        repository: REPOSITORY.to_owned(),
        release: RuntimeRelease {
            tag: TAG.to_owned(),
            title: TITLE.to_owned(),
            target_commit: target_commit.to_owned(),
            page_url: format!("https://github.com/{REPOSITORY}/releases/tag/{TAG}"),
        },
        runtime: RuntimeReleaseTuple {
            profile_id: runtime_profile_id.to_owned(),
            snv_bundle_id: SNV_ID.to_owned(),
            model_bundle_id: MODEL_ID.to_owned(),
            reference_bundle_id: REFERENCE_ID.to_owned(),
            mask_sha256: MASK_ID.to_owned(),
        },
        model_source: model_source(target_commit),
        transport: RuntimeReleaseTransport {
            schema: "pangopup.runtime-transport.v1".to_owned(),
            transport_id: transport_id.to_owned(),
            members,
        },
    }
}

fn model_source(target_commit: &str) -> ModelSource {
    const DATA: [(&str, u64, &str); 12] = [
        (
            "final.1.0.3.v2",
            2_877_321,
            "f0478fab173b75f7f7e9fe96688bad6c50fa4a46d70557f423b110caaf565501",
        ),
        (
            "final.2.0.3.v2",
            2_877_321,
            "c4c6bb4880fa6fb28b14182ae3ea0600edb07056158f55325b5e6e6e48fc9f26",
        ),
        (
            "final.3.0.3.v2",
            2_877_321,
            "ec685a6e7105a4486c1f89a005458a13deb3fe7171f13d434f4877e386d10676",
        ),
        (
            "final.1.2.3.v2",
            2_877_321,
            "559c05de3e1ce65c2515ca3e92ef85edb0ec2e47686ca58060e25891ce06eb3a",
        ),
        (
            "final.2.2.3.v2",
            2_877_321,
            "48758ba8b95eee9aa9feea52672ef06ca1b34111299c27f8a710f734d8b9aae5",
        ),
        (
            "final.3.2.3.v2",
            2_877_321,
            "7cb576c2b24db4fdd6970c4ca4fb7c20ae1b1d8ae80645ebbe689848b5743129",
        ),
        (
            "final.1.4.3.v2",
            2_877_321,
            "c50b12e0c0af776d5674ca5e346493f8265783494d4df383364de9c1136657f6",
        ),
        (
            "final.2.4.3.v2",
            2_877_321,
            "e03303bed4fd6f135ec0f6c1b192cce954ea42d0646f44d17b4a6fbb2b1f610e",
        ),
        (
            "final.3.4.3.v2",
            2_877_321,
            "9476d2e25520d7ff15bece0cd5d3b657e3b1dd3cc5fcab1d9c3b62bea7a0c5b6",
        ),
        (
            "final.1.6.3.v2",
            2_877_321,
            "2aae563fa18a8a9b6699c6c96e0d32b8ec7543f8f805fb3bc9de77302cc9f66e",
        ),
        (
            "final.2.6.3.v2",
            2_877_321,
            "7d3c0b1b2a60067b940dec315567874fbc8bcd322f1b7c76bf969f51f0f53f7f",
        ),
        (
            "final.3.6.3.v2",
            2_877_321,
            "756e7721a382cace24e9bfea5b543af5623f2487d9a3efe7385e9c76367005fd",
        ),
    ];
    let source = |path: &str, size, digest: &str| SourceFile {
        path: path.to_owned(),
        size,
        sha256: format!("sha256:{digest}"),
        url: format!("https://raw.githubusercontent.com/tkzeng/Pangolin/{UPSTREAM_COMMIT}/{path}"),
    };
    ModelSource {
        license: "GPL-3.0-only".to_owned(),
        upstream_repository: "https://github.com/tkzeng/Pangolin".to_owned(),
        upstream_commit: UPSTREAM_COMMIT.to_owned(),
        model_py: source("pangolin/model.py", 3_011, "4a1c5c2570aafe1452bb43332255321677e6c6c817adf84b9dd438e3ca4be6f8"),
        license_file: source("LICENSE", 35_149, "3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986"),
        checkpoint_set: "pangolin-1.0.2-5cf94b8-checkpoints-v1".to_owned(),
        checkpoints: DATA
            .into_iter()
            .enumerate()
            .map(|(index, (name, size, digest))| Checkpoint {
                ordinal: (index + 1) as u8,
                name: name.to_owned(),
                size,
                sha256: format!("sha256:{digest}"),
                url: format!(
                    "https://raw.githubusercontent.com/tkzeng/Pangolin/{UPSTREAM_COMMIT}/pangolin/models/{name}"
                ),
            })
            .collect(),
        converter_repository: format!("https://github.com/{REPOSITORY}"),
        converter_commit: target_commit.to_owned(),
        converter_paths: vec![
            "tools/pangolin-model".to_owned(),
            "crates/pangopup-build".to_owned(),
        ],
    }
}

fn validate_profile(
    profile: &RuntimeReleaseProfile,
    contract: Contract<'_>,
) -> Result<(), AssetError> {
    if profile.schema != SCHEMA
        || profile.profile != TAG
        || profile.repository != REPOSITORY
        || profile.release.tag != TAG
        || profile.release.title != TITLE
        || !valid_commit(&profile.release.target_commit)
        || profile.release.page_url != format!("https://github.com/{REPOSITORY}/releases/tag/{TAG}")
        || profile.transport.schema != "pangopup.runtime-transport.v1"
        || profile.transport.transport_id != contract.transport_id
        || profile.transport.members.len() != contract.members.len()
        || contract.members.len() != 10
        || contract.members[0].sha256 != contract.transport_id
        || profile.runtime.profile_id != contract.runtime_profile_id
        || profile.runtime.snv_bundle_id != SNV_ID
        || profile.runtime.model_bundle_id != MODEL_ID
        || profile.runtime.reference_bundle_id != REFERENCE_ID
        || profile.runtime.mask_sha256 != MASK_ID
        || profile.model_source.converter_commit != profile.release.target_commit
        || profile.model_source != model_source(&profile.release.target_commit)
    {
        return Err(release_invalid("runtime release profile facts are invalid"));
    }
    let mut names = BTreeSet::new();
    for (member, expected) in profile.transport.members.iter().zip(contract.members) {
        if member.logical_path != member.asset_name
            || member.asset_name != expected.name
            || member.role != expected.role
            || member.size != expected.size
            || member.sha256 != expected.sha256
            || !safe_name(&member.asset_name)
            || !names.insert(&member.asset_name)
            || member.size == 0
            || !valid_sha(&member.sha256)
            || member.url
                != format!(
                    "https://github.com/{REPOSITORY}/releases/download/{TAG}/{}",
                    member.asset_name
                )
        {
            return Err(release_invalid("runtime release member is invalid"));
        }
    }
    Ok(())
}

struct InspectedTransport {
    transport_id: String,
    runtime_profile_id: String,
    members: Vec<HeldMember>,
}

impl InspectedTransport {
    fn matches_contract(&self, contract: Contract<'_>) -> bool {
        self.members.len() == contract.members.len()
            && self
                .members
                .iter()
                .zip(contract.members)
                .all(|(actual, expected)| {
                    actual.name == expected.name
                        && actual.role == expected.role
                        && actual.size == expected.size
                        && actual.sha256 == expected.sha256
                })
    }
}

struct HeldMember {
    name: String,
    role: String,
    size: u64,
    sha256: String,
    file: File,
    metadata: fs::Metadata,
}

fn copy_member(
    source: &SourceDirectory,
    member: &HeldMember,
    destination: &Path,
) -> Result<(), AssetError> {
    let mut input = member
        .file
        .try_clone()
        .map_err(|error| input_io("clone runtime transport member for copy", error))?;
    input
        .seek(SeekFrom::Start(0))
        .map_err(|error| input_io("rewind runtime transport member for copy", error))?;
    let mut output =
        File::create_new(destination).map_err(|error| output_io("create staged member", error))?;
    let mut hash = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|error| input_io("read runtime transport member", error))?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > member.size {
            return Err(release_invalid("runtime transport member grew during copy"));
        }
        hash.update(&buffer[..count]);
        output
            .write_all(&buffer[..count])
            .map_err(|error| output_io("copy runtime transport member", error))?;
        #[cfg(any(test, feature = "test-read-audit"))]
        if fail_at(RuntimeReleaseFaultPoint::Copy) {
            return Err(output_io(
                "copy runtime transport member",
                io::Error::other("injected failure"),
            ));
        }
    }
    output
        .sync_all()
        .map_err(|error| output_io("sync copied runtime transport member", error))?;
    source.validate_member(&member.name, &member.file, &member.metadata)?;
    if size != member.size || format!("sha256:{:x}", hash.finalize()) != member.sha256 {
        return Err(release_invalid(
            "runtime transport member identity changed during copy",
        ));
    }
    Ok(())
}

struct SourceDirectory {
    file: File,
    metadata: fs::Metadata,
}

impl SourceDirectory {
    fn open(path: &Path) -> Result<Self, AssetError> {
        let file = open_path_directory(path)
            .map_err(|error| input_io("open runtime transport directory", error))?;
        let metadata = file
            .metadata()
            .map_err(|error| input_io("inspect runtime transport directory", error))?;
        Ok(Self { file, metadata })
    }

    fn inspect_and_verify(&self, contract: Contract<'_>) -> Result<InspectedTransport, AssetError> {
        if contract.members.len() != 10
            || contract.members[0].name != "runtime-transport.json"
            || contract.members[0].role != "runtime-transport-manifest"
            || contract.members[0].sha256 != contract.transport_id
            || contract
                .members
                .iter()
                .any(|member| !safe_name(member.name) || !valid_sha(member.sha256))
        {
            return Err(release_invalid(
                "runtime release preparation contract is invalid",
            ));
        }
        self.require_inventory(
            &contract
                .members
                .iter()
                .map(|member| member.name)
                .collect::<Vec<_>>(),
        )?;
        let mut held = Vec::with_capacity(contract.members.len());
        for expected in contract.members {
            let (file, metadata) = self.open_member(expected.name)?;
            held.push(HeldMember {
                name: expected.name.to_owned(),
                role: expected.role.to_owned(),
                size: expected.size,
                sha256: expected.sha256.to_owned(),
                file,
                metadata,
            });
        }
        let manifest_bytes = read_held_exact(&held[0])?;
        let transport_id = sha256(&manifest_bytes);
        let manifest = parse_runtime_transport_manifest_for_release(&manifest_bytes)?;
        if transport_id != contract.transport_id
            || manifest.runtime_profile_id != contract.runtime_profile_id
            || manifest.members.len() + 1 != held.len()
        {
            return Err(release_invalid(
                "runtime transport does not match the reviewed release contract",
            ));
        }
        let mut runtime_profile_bytes = None;
        for (descriptor, member) in held.iter().skip(1).zip(&manifest.members) {
            if descriptor.name != member.name
                || descriptor.role != member.role
                || descriptor.size != member.stored_bytes
                || descriptor.sha256 != member.stored_sha256
            {
                return Err(release_invalid(
                    "runtime transport member does not match the reviewed release contract",
                ));
            }
            match member.encoding {
                Encoding::Raw => {
                    let bytes = read_runtime_transport_held_raw(
                        &descriptor.file,
                        &descriptor.metadata,
                        member,
                    )?;
                    if member.name == "runtime-profile.json" {
                        runtime_profile_bytes = Some(bytes);
                    }
                }
                Encoding::Zstd => verify_runtime_transport_held_frame(
                    &descriptor.file,
                    &descriptor.metadata,
                    member,
                )?,
            }
        }
        let runtime_profile_bytes = runtime_profile_bytes
            .ok_or_else(|| release_invalid("runtime profile member is missing"))?;
        let runtime_profile_id = super::runtime_profile_id(&runtime_profile_bytes)
            .map_err(|error| release_invalid(error.to_string()))?
            .to_string();
        if runtime_profile_id != manifest.runtime_profile_id {
            return Err(release_invalid(
                "runtime profile identity does not match transport manifest",
            ));
        }
        for member in &held {
            self.validate_member(&member.name, &member.file, &member.metadata)?;
        }
        Ok(InspectedTransport {
            transport_id,
            runtime_profile_id,
            members: held,
        })
    }

    fn open_member(&self, name: &str) -> Result<(File, fs::Metadata), AssetError> {
        if !safe_name(name) {
            return Err(release_invalid("unsafe runtime transport member name"));
        }
        let path_descriptor = open_at(self.file.as_raw_fd(), name, libc::O_PATH)
            .map_err(|error| input_io("inspect runtime transport member path", error))?;
        let path_metadata = path_descriptor
            .metadata()
            .map_err(|error| input_io("inspect runtime transport member", error))?;
        if !path_metadata.is_file() || path_metadata.nlink() != 1 {
            return Err(release_invalid(
                "runtime transport member must be a regular singly linked file",
            ));
        }
        #[cfg(any(test, feature = "test-read-audit"))]
        if fail_at(RuntimeReleaseFaultPoint::PathAdmissionFifoReplacement) {
            self.replace_with_fifo_for_test(name)?;
        }
        let file = open_at(
            self.file.as_raw_fd(),
            name,
            libc::O_RDONLY | libc::O_NONBLOCK,
        )
        .map_err(|error| input_io("open runtime transport member", error))?;
        let metadata = file
            .metadata()
            .map_err(|error| input_io("inspect runtime transport member", error))?;
        if !same_metadata(&path_metadata, &metadata) {
            return Err(release_invalid(
                "runtime transport member changed while it was opened",
            ));
        }
        Ok((file, metadata))
    }

    #[cfg(any(test, feature = "test-read-audit"))]
    fn replace_with_fifo_for_test(&self, name: &str) -> Result<(), AssetError> {
        let name = CString::new(name)
            .map_err(|_| release_invalid("invalid runtime transport member name"))?;
        let removed = unsafe { libc::unlinkat(self.file.as_raw_fd(), name.as_ptr(), 0) };
        if removed != 0 {
            return Err(input_io(
                "replace admitted runtime member",
                io::Error::last_os_error(),
            ));
        }
        let created =
            unsafe { libc::mkfifoat(self.file.as_raw_fd(), name.as_ptr(), 0o600 as libc::mode_t) };
        if created != 0 {
            return Err(input_io(
                "create replacement runtime FIFO",
                io::Error::last_os_error(),
            ));
        }
        Ok(())
    }

    fn validate_member(
        &self,
        name: &str,
        held: &File,
        before: &fs::Metadata,
    ) -> Result<(), AssetError> {
        let held_after = held
            .metadata()
            .map_err(|error| input_io("reinspect held runtime member", error))?;
        let (current, current_metadata) = self.open_member(name)?;
        drop(current);
        if !same_metadata(before, &held_after) || !same_metadata(before, &current_metadata) {
            return Err(release_invalid(
                "runtime transport member changed during preparation",
            ));
        }
        Ok(())
    }

    fn require_inventory(&self, expected: &[&str]) -> Result<(), AssetError> {
        let cursor = open_at(
            self.file.as_raw_fd(),
            ".",
            libc::O_RDONLY | libc::O_DIRECTORY,
        )
        .map_err(|error| input_io("open runtime transport inventory", error))?;
        let mut buffer = [std::mem::MaybeUninit::<u8>::uninit(); 8192];
        let mut entries = rustix::fs::RawDir::new(cursor, &mut buffer);
        let mut observed = BTreeSet::new();
        while let Some(entry) = entries.next() {
            let entry = entry
                .map_err(|error| input_io("read runtime transport inventory", error.into()))?;
            let bytes = entry.file_name().to_bytes();
            if bytes != b"." && bytes != b".." {
                observed.insert(
                    String::from_utf8(bytes.to_vec())
                        .map_err(|_| release_invalid("non-UTF-8 runtime transport member"))?,
                );
            }
        }
        let expected: BTreeSet<_> = expected.iter().map(|name| (*name).to_owned()).collect();
        if observed != expected {
            return Err(release_invalid(
                "runtime transport directory member set mismatch",
            ));
        }
        Ok(())
    }

    fn require_same_path(&self, path: &Path) -> Result<(), AssetError> {
        let current = fs::symlink_metadata(path)
            .map_err(|error| input_io("reinspect runtime transport directory", error))?;
        let held = self
            .file
            .metadata()
            .map_err(|error| input_io("reinspect held runtime transport directory", error))?;
        if current.file_type().is_symlink()
            || !same_metadata(&self.metadata, &held)
            || self.metadata.dev() != current.dev()
            || self.metadata.ino() != current.ino()
        {
            return Err(release_invalid(
                "runtime transport directory changed during preparation",
            ));
        }
        Ok(())
    }
}

fn read_held_exact(member: &HeldMember) -> Result<Vec<u8>, AssetError> {
    if member.size > 1024 * 1024 {
        return Err(release_invalid("runtime transport manifest is too large"));
    }
    let mut file = member
        .file
        .try_clone()
        .map_err(|error| input_io("clone runtime transport manifest", error))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| input_io("rewind runtime transport manifest", error))?;
    let mut bytes = Vec::with_capacity(member.size as usize);
    file.take(member.size + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| input_io("read runtime transport manifest", error))?;
    if bytes.len() as u64 != member.size || sha256(&bytes) != member.sha256 {
        return Err(release_invalid(
            "runtime transport manifest identity mismatch",
        ));
    }
    Ok(bytes)
}

fn same_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.nlink() == right.nlink()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), AssetError> {
    let mut file =
        File::create_new(path).map_err(|error| output_io("create publication metadata", error))?;
    file.write_all(bytes)
        .map_err(|error| output_io("write publication metadata", error))?;
    file.sync_all()
        .map_err(|error| output_io("sync publication metadata", error))
}

fn publish_read_only_stage(
    stage: &Path,
    output: &Path,
    guard: &mut super::StageGuard,
) -> Result<(), AssetError> {
    #[cfg(any(test, feature = "test-read-audit"))]
    if fail_at(RuntimeReleaseFaultPoint::Publication) {
        make_stage_removable(stage)?;
        return Err(output_io(
            "publish staged runtime release",
            io::Error::other("injected failure"),
        ));
    }
    if let Err(error) = rustix::fs::renameat_with(
        rustix::fs::CWD,
        stage,
        rustix::fs::CWD,
        output,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)
    {
        make_stage_removable(stage)?;
        return Err(
            if matches!(
                error.kind(),
                io::ErrorKind::AlreadyExists | io::ErrorKind::DirectoryNotEmpty
            ) {
                AssetError::new(
                    AssetErrorKind::OutputConflict,
                    "output publication race lost",
                )
            } else {
                output_io("publish staged runtime release", error)
            },
        );
    }
    guard.published();
    #[cfg(any(test, feature = "test-read-audit"))]
    if fail_at(RuntimeReleaseFaultPoint::ParentSync) {
        return Err(AssetError::new(
            AssetErrorKind::OutputIo,
            "runtime release published but parent durability is unconfirmed: injected failure",
        ));
    }
    if let Err(error) = sync_directory(output.parent().unwrap_or_else(|| Path::new("."))) {
        return Err(AssetError::new(
            AssetErrorKind::OutputIo,
            format!("runtime release published but parent durability is unconfirmed: {error}"),
        ));
    }
    Ok(())
}

fn make_stage_removable(stage: &Path) -> Result<(), AssetError> {
    fs::set_permissions(stage, fs::Permissions::from_mode(0o700))
        .map_err(|error| output_io("restore failed staging permissions", error))
}

#[cfg(any(test, feature = "test-read-audit"))]
fn replace_source_member_for_test(source: &SourceDirectory, name: &str) -> Result<(), AssetError> {
    let (mut original, _) = source.open_member(name)?;
    let replacement = format!("{name}.replacement");
    let descriptor = open_at_create(
        source.file.as_raw_fd(),
        &replacement,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        0o600,
    )
    .map_err(|error| input_io("create replacement runtime member", error))?;
    let mut replacement_file = descriptor;
    io::copy(&mut original, &mut replacement_file)
        .map_err(|error| input_io("copy replacement runtime member", error))?;
    replacement_file
        .sync_all()
        .map_err(|error| input_io("sync replacement runtime member", error))?;
    rustix::fs::renameat(&source.file, replacement.as_str(), &source.file, name)
        .map_err(io::Error::from)
        .map_err(|error| input_io("replace runtime member", error))
}

#[cfg(any(test, feature = "test-read-audit"))]
fn open_at_create(dir: RawFd, name: &str, flags: i32, mode: u32) -> io::Result<File> {
    let name = CString::new(name).map_err(|_| io::Error::other("NUL in component"))?;
    let descriptor = unsafe {
        libc::openat(
            dir,
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            mode as libc::mode_t,
        )
    };
    file_from_fd(descriptor)
}

fn sha256sums(members: &[(&str, &str)], profile_hash: &str) -> String {
    let mut output = String::new();
    for (name, identity) in members.iter().copied().chain(std::iter::once((
        "runtime-release-profile.json",
        profile_hash,
    ))) {
        output.push_str(
            identity
                .strip_prefix("sha256:")
                .expect("validated identity"),
        );
        output.push_str("  ");
        output.push_str(name);
        output.push('\n');
    }
    output
}

fn release_notes(profile: &RuntimeReleaseProfile) -> String {
    format!(
        "# {TITLE}\n\n\
This release contains the exact compressed Pangolin model, compact RefSeq GRCh38.p14 reference, and GENCODE v38 splice-mask runtime selected by Pangopup.\n\n\
- Runtime profile: `{}`\n\
- Model bundle: `{MODEL_ID}`\n\
- Reference bundle: `{REFERENCE_ID}`\n\
- Splice mask: `{MASK_ID}`\n\
- Compatible SNV bundle: `{SNV_ID}` from release `snv-grch38-v1`\n\n\
The included attribution files are `model-NOTICE`, `reference-NOTICE`, and `mask-NOTICE`.\n\n\
Raw Zenodo data, NCBI FASTA, GENCODE GTF/SQLite files, original checkpoint containers, and qualification fixtures are not included.\n\n\
Preferred source for modifying the model is the twelve authenticated `.v2` checkpoint containers plus `pangolin/model.py` at upstream Pangolin commit `{UPSTREAM_COMMIT}`. Pangopup's authenticated converter and lockfile are in `tools/pangolin-model` and `crates/pangopup-build` at commit `{}`.\n",
        profile.runtime.profile_id, profile.release.target_commit
    )
}

fn open_path_directory(path: &Path) -> io::Result<File> {
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::other("NUL in directory path"))?;
    let descriptor = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    file_from_fd(descriptor)
}

fn open_at(dir: RawFd, name: &str, flags: i32) -> io::Result<File> {
    let name = CString::new(name).map_err(|_| io::Error::other("NUL in component"))?;
    let descriptor = unsafe {
        libc::openat(
            dir,
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    file_from_fd(descriptor)
}

fn file_from_fd(descriptor: i32) -> io::Result<File> {
    if descriptor < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }
}

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.as_bytes().contains(&0)
}

fn valid_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_sha(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn release_invalid(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::ReleaseInvalid, message)
}

fn input_io(operation: &str, error: io::Error) -> AssetError {
    AssetError::new(AssetErrorKind::InputIo, format!("{operation}: {error}"))
}

fn output_io(operation: &str, error: io::Error) -> AssetError {
    AssetError::new(AssetErrorKind::OutputIo, format!("{operation}: {error}"))
}

#[cfg(test)]
mod production_contract_tests {
    use super::*;

    #[test]
    fn checked_release_and_transport_authorities_are_exact_and_closed() {
        let (digest, profile) = production_runtime_release_profile().expect("production contract");
        assert_eq!(digest, PRODUCTION_RELEASE_PROFILE_SHA256);
        assert_eq!(profile.profile, TAG);
        assert_eq!(profile.repository, REPOSITORY);
        assert_eq!(
            profile.release.target_commit,
            "e6d8497aaf1e3db521360ad969252a2ec6fd14e4"
        );
        assert_eq!(profile.transport.transport_id, TRANSPORT_ID);
        assert_eq!(profile.runtime.profile_id, PROFILE_ID);
        assert_eq!(profile.transport.members.len(), 10);
        assert_eq!(
            profile
                .transport
                .members
                .iter()
                .map(|member| member.asset_name.as_str())
                .collect::<Vec<_>>(),
            [
                "runtime-transport.json",
                "runtime-profile.json",
                "model-manifest.json",
                "model-NOTICE",
                "model.onnx.zst",
                "reference-manifest.json",
                "reference-NOTICE",
                "reference.pgr.zst",
                "mask-NOTICE",
                "domains.pgm.zst",
            ]
        );
    }
}
