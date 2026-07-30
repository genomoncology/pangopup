use pangopup_assets::{
    MaskProfile, ModelProfile, ReferenceProfile, RuntimeProfile, RuntimeReleaseExpectedMember,
    RuntimeReleaseFaultPoint, RuntimeReleasePreparationContract, ScoringProfile, SnvProfile,
    canonical_runtime_profile_bytes, pack_runtime_transport,
    parse_runtime_release_profile_with_contract, prepare_runtime_release,
    prepare_runtime_release_with_contract, runtime_profile_id, set_runtime_release_fault,
    verify_runtime_transport,
};
use pangopup_core::ReferenceProvider;
use pangopup_index::{mask::MaskDomainsOpen, reference::ReferenceBundleOpen};
use pangopup_model::{ModelRepresentation, inspect_runtime_profile_bundle};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};
use tempfile::tempdir;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn representation(value: ModelRepresentation) -> &'static str {
    match value {
        ModelRepresentation::Singleton => "singleton",
        ModelRepresentation::ZeroPaddedBatch => "zero-padded-batch",
        ModelRepresentation::PairedStrandBatch => "paired-strand-batch",
    }
}

struct PackedFixture {
    transport: PathBuf,
    transport_id: String,
    runtime_profile_id: String,
    members: Vec<OwnedExpectedMember>,
}

struct OwnedExpectedMember {
    name: String,
    role: String,
    size: u64,
    sha256: String,
}

fn with_contract<T>(
    fixture: &PackedFixture,
    use_contract: impl FnOnce(RuntimeReleasePreparationContract<'_>) -> T,
) -> T {
    let members: Vec<_> = fixture
        .members
        .iter()
        .map(|member| RuntimeReleaseExpectedMember {
            name: &member.name,
            role: &member.role,
            size: member.size,
            sha256: &member.sha256,
        })
        .collect();
    use_contract(RuntimeReleasePreparationContract {
        transport_id: &fixture.transport_id,
        runtime_profile_id: &fixture.runtime_profile_id,
        members: &members,
    })
}

fn with_profile_contract<T>(
    profile: &serde_json::Value,
    use_contract: impl FnOnce(RuntimeReleasePreparationContract<'_>) -> T,
) -> T {
    let transport_id = profile["transport"]["transport_id"]
        .as_str()
        .expect("transport id");
    let runtime_profile_id = profile["runtime"]["profile_id"]
        .as_str()
        .expect("profile id");
    let owned: Vec<_> = profile["transport"]["members"]
        .as_array()
        .expect("members")
        .iter()
        .map(|member| OwnedExpectedMember {
            name: member["asset_name"].as_str().expect("name").to_owned(),
            role: member["role"].as_str().expect("role").to_owned(),
            size: member["size"].as_u64().expect("size"),
            sha256: member["sha256"].as_str().expect("sha").to_owned(),
        })
        .collect();
    let members: Vec<_> = owned
        .iter()
        .map(|member| RuntimeReleaseExpectedMember {
            name: &member.name,
            role: &member.role,
            size: member.size,
            sha256: &member.sha256,
        })
        .collect();
    use_contract(RuntimeReleasePreparationContract {
        transport_id,
        runtime_profile_id,
        members: &members,
    })
}

fn pack_fixture(root: &Path, name: &str) -> PackedFixture {
    let fixtures = fixtures();
    let model_path = fixtures.join("pangolin-model-kernel-mini/bundle");
    let reference_path = fixtures.join("reference-route-test/bundle");
    let mask_path = fixtures.join("gencode-mask-mini/domains.pgm");
    let model = inspect_runtime_profile_bundle(&model_path).expect("model");
    let reference = ReferenceBundleOpen::open_identified(&reference_path).expect("reference");
    let mask = MaskDomainsOpen::open_identified(&mask_path).expect("mask");
    let provenance = reference.provenance();
    let profile = RuntimeProfile {
        schema: "pangopup.runtime-profile.v1".to_owned(),
        snv: SnvProfile {
            bundle_id: format!("sha256:{}", "1".repeat(64)),
            format: "miniature.snv.v1".to_owned(),
            member_bytes: 15_000_000_000,
            member_sha256: format!("sha256:{}", "2".repeat(64)),
        },
        model: ModelProfile {
            bundle_id: model.bundle_id.to_string(),
            profile: model.profile,
            representation: representation(model.representation).to_owned(),
            member_bytes: model.member_bytes,
            member_sha256: model.member_sha256,
        },
        reference: ReferenceProfile {
            bundle_id: provenance.bundle_id().to_owned(),
            profile: provenance.profile().to_owned(),
            format: provenance.format().to_owned(),
            assembly: provenance.assembly().to_owned(),
            assembly_accession: provenance.assembly_accession().to_owned(),
            sequence_set_sha256: provenance.sequence_set_sha256().to_owned(),
            member_bytes: reference.identity().bytes(),
            member_sha256: reference.identity().sha256().to_owned(),
        },
        mask: MaskProfile {
            format: "pangopup.gencode-v38-domains.v1".to_owned(),
            member_bytes: mask.identity().bytes(),
            member_sha256: format!("sha256:{}", mask.identity().sha256()),
        },
        scoring: ScoringProfile {
            assembly: "GRCh38".to_owned(),
            semantics: "miniature-runtime-release-test".to_owned(),
            distance: 50,
            masking_policy: "miniature".to_owned(),
            cpu_policy: "sequential:1/1".to_owned(),
        },
    };
    let profile_bytes = canonical_runtime_profile_bytes(&profile).expect("profile");
    let profile_id = runtime_profile_id(&profile_bytes)
        .expect("profile id")
        .to_string();
    let profile_path = root.join(format!("{name}.profile.json"));
    fs::write(&profile_path, profile_bytes).expect("write profile");
    let transport = root.join(name);
    let outcome = pack_runtime_transport(
        &profile_path,
        &model_path,
        &reference_path,
        &mask_path,
        &transport,
    )
    .expect("pack transport");
    let manifest_bytes = fs::read(transport.join("runtime-transport.json")).expect("manifest");
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).expect("manifest");
    let mut members = vec![OwnedExpectedMember {
        name: "runtime-transport.json".to_owned(),
        role: "runtime-transport-manifest".to_owned(),
        size: manifest_bytes.len() as u64,
        sha256: outcome.transport_id.clone(),
    }];
    members.extend(
        manifest["members"]
            .as_array()
            .expect("members")
            .iter()
            .map(|member| OwnedExpectedMember {
                name: member["name"].as_str().expect("name").to_owned(),
                role: member["role"].as_str().expect("role").to_owned(),
                size: member["stored_bytes"].as_u64().expect("size"),
                sha256: member["stored_sha256"].as_str().expect("sha").to_owned(),
            }),
    );
    PackedFixture {
        transport,
        transport_id: outcome.transport_id,
        runtime_profile_id: profile_id,
        members,
    }
}

fn prepare_fixture(root: &Path, name: &str) -> PathBuf {
    let fixture = pack_fixture(root, &format!("{name}-transport"));
    let output = root.join(name);
    let outcome = with_contract(&fixture, |contract| {
        prepare_runtime_release_with_contract(&fixture.transport, COMMIT, &output, contract)
    })
    .expect("prepare release");
    assert_eq!(outcome.status, "ok");
    assert_eq!(outcome.upload_asset_count, 12);
    output
}

fn inventory(path: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(path)
        .expect("list")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("UTF-8")
        })
        .collect();
    names.sort();
    names
}

#[test]
fn miniature_preparation_is_deterministic_closed_and_read_only() {
    let temp = tempdir().expect("temp");
    let first = prepare_fixture(temp.path(), "first");
    let second = prepare_fixture(temp.path(), "second");
    assert_eq!(inventory(&first), inventory(&second));
    assert_eq!(inventory(&first).len(), 13);
    for name in inventory(&first) {
        assert_eq!(
            fs::read(first.join(&name)).expect("first"),
            fs::read(second.join(&name)).expect("second"),
            "{name}"
        );
        let metadata = fs::metadata(first.join(name)).expect("metadata");
        assert_eq!(metadata.permissions().mode() & 0o777, 0o400);
        assert_eq!(metadata.nlink(), 1);
    }
    assert_eq!(
        fs::metadata(&first)
            .expect("directory")
            .permissions()
            .mode()
            & 0o777,
        0o500
    );

    let profile_bytes = fs::read(first.join("runtime-release-profile.json")).expect("profile");
    let raw: serde_json::Value = serde_json::from_slice(&profile_bytes).expect("raw profile");
    let profile = with_profile_contract(&raw, |contract| {
        parse_runtime_release_profile_with_contract(&profile_bytes, contract)
    })
    .expect("parse");
    assert_eq!(profile.release.target_commit, COMMIT);
    assert_eq!(profile.transport.members.len(), 10);
    assert_eq!(profile.model_source.checkpoints.len(), 12);
    assert_eq!(
        profile.transport.members[0].role,
        "runtime-transport-manifest"
    );
    let sums = fs::read_to_string(first.join("SHA256SUMS")).expect("sums");
    assert_eq!(sums.lines().count(), 11);
    assert!(
        sums.lines()
            .next()
            .expect("first")
            .ends_with("  runtime-transport.json")
    );
    assert!(
        sums.lines()
            .last()
            .expect("last")
            .ends_with("  runtime-release-profile.json")
    );
    let notes = fs::read_to_string(first.join("RELEASE-NOTES.md")).expect("notes");
    for excluded in [
        "Raw Zenodo data",
        "NCBI FASTA",
        "GENCODE GTF/SQLite",
        "original checkpoint containers",
        "qualification fixtures",
    ] {
        assert!(notes.contains(excluded), "{excluded}");
    }
}

#[test]
fn production_entry_rejects_a_valid_nonproduction_transport() {
    let temp = tempdir().expect("temp");
    let fixture = pack_fixture(temp.path(), "transport");
    let output = temp.path().join("output");
    let error = prepare_runtime_release(&fixture.transport, COMMIT, &output).expect_err("reject");
    assert_eq!(
        error.kind(),
        pangopup_assets::AssetErrorKind::ReleaseInvalid
    );
    assert!(!output.exists());
}

#[test]
fn invalid_targets_and_occupied_output_fail_without_a_partial_result() {
    let temp = tempdir().expect("temp");
    let fixture = pack_fixture(temp.path(), "transport");
    for (index, target) in [
        "",
        "ABCDEF0123456789abcdef0123456789abcdef01",
        "0123456789abcdef0123456789abcdef0123456g",
        "0123456789abcdef0123456789abcdef012345678",
    ]
    .into_iter()
    .enumerate()
    {
        let output = temp.path().join(format!("invalid-{index}"));
        assert!(
            with_contract(&fixture, |contract| {
                prepare_runtime_release_with_contract(&fixture.transport, target, &output, contract)
            })
            .is_err()
        );
        assert!(!output.exists());
    }
    let occupied = temp.path().join("occupied");
    fs::create_dir(&occupied).expect("occupied");
    assert!(
        with_contract(&fixture, |contract| {
            prepare_runtime_release_with_contract(&fixture.transport, COMMIT, &occupied, contract)
        })
        .is_err()
    );
}

#[test]
fn source_shape_and_identity_fail_closed() {
    let temp = tempdir().expect("temp");
    for (index, mutation) in [
        "missing", "extra", "corrupt", "truncate", "symlink", "hardlink", "fifo",
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = pack_fixture(temp.path(), &format!("transport-{index}"));
        let member = fixture.transport.join("model-NOTICE");
        match mutation {
            "missing" => fs::remove_file(member).expect("remove"),
            "extra" => fs::write(fixture.transport.join("extra"), b"x").expect("extra"),
            "corrupt" => fs::write(member, b"wrong").expect("corrupt"),
            "truncate" => {
                let file = fs::OpenOptions::new()
                    .write(true)
                    .open(member)
                    .expect("open");
                file.set_len(1).expect("truncate");
            }
            "symlink" => {
                fs::remove_file(&member).expect("remove");
                std::os::unix::fs::symlink("runtime-profile.json", member).expect("symlink");
            }
            "hardlink" => {
                fs::hard_link(&member, fixture.transport.join("linked")).expect("link");
            }
            "fifo" => {
                fs::remove_file(&member).expect("remove");
                let path = CStringPath::new(&member);
                assert_eq!(unsafe { libc::mkfifo(path.0.as_ptr(), 0o600) }, 0);
            }
            _ => unreachable!(),
        }
        let output = temp.path().join(format!("output-{index}"));
        assert!(
            with_contract(&fixture, |contract| {
                prepare_runtime_release_with_contract(&fixture.transport, COMMIT, &output, contract)
            })
            .is_err(),
            "{mutation}"
        );
        assert!(!output.exists(), "{mutation}");
    }
}

struct CStringPath(std::ffi::CString);

impl CStringPath {
    fn new(path: &Path) -> Self {
        use std::os::unix::ffi::OsStrExt;
        Self(std::ffi::CString::new(path.as_os_str().as_bytes()).expect("path"))
    }
}

#[test]
fn injected_prepublication_failures_and_source_replacement_leave_no_output() {
    let temp = tempdir().expect("temp");
    for (index, point) in [
        RuntimeReleaseFaultPoint::Copy,
        RuntimeReleaseFaultPoint::FileSync,
        RuntimeReleaseFaultPoint::StageSync,
        RuntimeReleaseFaultPoint::Publication,
        RuntimeReleaseFaultPoint::SourceReplacement,
        RuntimeReleaseFaultPoint::VerifiedSourceReplacement,
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = pack_fixture(temp.path(), &format!("transport-{index}"));
        let output = temp.path().join(format!("output-{index}"));
        set_runtime_release_fault(point);
        let error = with_contract(&fixture, |contract| {
            prepare_runtime_release_with_contract(&fixture.transport, COMMIT, &output, contract)
        })
        .expect_err("injected failure");
        assert!(!error.to_string().is_empty());
        assert!(!output.exists(), "{point:?}");
        let output_name = output.file_name().expect("name").to_string_lossy();
        assert!(
            fs::read_dir(temp.path()).expect("list").all(|entry| {
                !entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&format!(".{output_name}.pangopup-stage-"))
            }),
            "{point:?}"
        );
    }
}

#[test]
fn fifo_replacement_between_path_admission_and_readable_open_is_rejected() {
    let temp = tempdir().expect("temp");
    let fixture = pack_fixture(temp.path(), "transport");
    let output = temp.path().join("output");
    set_runtime_release_fault(RuntimeReleaseFaultPoint::PathAdmissionFifoReplacement);
    let error = with_contract(&fixture, |contract| {
        prepare_runtime_release_with_contract(&fixture.transport, COMMIT, &output, contract)
    })
    .expect_err("FIFO replacement must not block or pass");
    assert!(
        error
            .to_string()
            .contains("runtime transport member changed while it was opened")
    );
    assert!(!output.exists());
}

#[test]
fn parent_sync_failure_reports_visible_but_unconfirmed_publication() {
    let temp = tempdir().expect("temp");
    let fixture = pack_fixture(temp.path(), "transport");
    let output = temp.path().join("output");
    set_runtime_release_fault(RuntimeReleaseFaultPoint::ParentSync);
    let error = with_contract(&fixture, |contract| {
        prepare_runtime_release_with_contract(&fixture.transport, COMMIT, &output, contract)
    })
    .expect_err("parent sync");
    assert!(
        error
            .to_string()
            .contains("published but parent durability is unconfirmed")
    );
    assert!(output.exists());
}

#[test]
fn release_profile_rejects_extensions_and_noncanonical_bytes() {
    let temp = tempdir().expect("temp");
    let output = prepare_fixture(temp.path(), "output");
    let bytes = fs::read(output.join("runtime-release-profile.json")).expect("profile");
    let raw: serde_json::Value = serde_json::from_slice(&bytes).expect("raw profile");
    let mut extension = raw.clone();
    extension["extension"] = serde_json::json!(true);
    let extension = serde_jcs::to_vec(&extension).expect("canonical");
    assert!(
        with_profile_contract(&raw, |contract| {
            parse_runtime_release_profile_with_contract(&extension, contract)
        })
        .is_err()
    );
    let mut changed_identity = raw.clone();
    changed_identity["runtime"]["model_bundle_id"] =
        serde_json::json!(format!("sha256:{}", "0".repeat(64)));
    let changed_identity = serde_jcs::to_vec(&changed_identity).expect("canonical");
    assert!(
        with_profile_contract(&raw, |contract| {
            parse_runtime_release_profile_with_contract(&changed_identity, contract)
        })
        .is_err()
    );
    let mut changed_size = raw.clone();
    changed_size["transport"]["members"][4]["size"] = serde_json::json!(
        changed_size["transport"]["members"][4]["size"]
            .as_u64()
            .expect("size")
            + 1
    );
    let changed_size = serde_jcs::to_vec(&changed_size).expect("canonical");
    assert!(
        with_profile_contract(&raw, |contract| {
            parse_runtime_release_profile_with_contract(&changed_size, contract)
        })
        .is_err()
    );
    let mut changed_digest = raw.clone();
    changed_digest["transport"]["members"][4]["sha256"] =
        serde_json::json!(format!("sha256:{}", "0".repeat(64)));
    let changed_digest = serde_jcs::to_vec(&changed_digest).expect("canonical");
    assert!(
        with_profile_contract(&raw, |contract| {
            parse_runtime_release_profile_with_contract(&changed_digest, contract)
        })
        .is_err()
    );
    let mut rebound_manifest = raw.clone();
    rebound_manifest["transport"]["members"][0]["sha256"] =
        serde_json::json!(format!("sha256:{}", "0".repeat(64)));
    let rebound_manifest = serde_jcs::to_vec(&rebound_manifest).expect("canonical");
    assert!(
        with_profile_contract(&raw, |contract| {
            parse_runtime_release_profile_with_contract(&rebound_manifest, contract)
        })
        .is_err()
    );
    let mut reordered = raw.clone();
    reordered["transport"]["members"]
        .as_array_mut()
        .expect("members")
        .swap(0, 1);
    let reordered = serde_jcs::to_vec(&reordered).expect("canonical");
    assert!(
        with_profile_contract(&raw, |contract| {
            parse_runtime_release_profile_with_contract(&reordered, contract)
        })
        .is_err()
    );
    let mut rebound_source = raw.clone();
    rebound_source["model_source"]["checkpoints"][0]["sha256"] =
        serde_json::json!(format!("sha256:{}", "0".repeat(64)));
    let rebound_source = serde_jcs::to_vec(&rebound_source).expect("canonical");
    assert!(
        with_profile_contract(&raw, |contract| {
            // The expected transport records are unchanged; the model-source
            // rebinding must still fail its independently frozen source contract.
            parse_runtime_release_profile_with_contract(&rebound_source, contract)
        })
        .is_err()
    );
    let mut trailing = bytes;
    trailing.push(b'\n');
    assert!(
        with_profile_contract(
            &serde_json::from_slice(&trailing[..trailing.len() - 1]).expect("raw"),
            |contract| parse_runtime_release_profile_with_contract(&trailing, contract)
        )
        .is_err()
    );
}

#[test]
fn copied_transport_members_remain_byte_identical() {
    let temp = tempdir().expect("temp");
    let fixture = pack_fixture(temp.path(), "transport");
    verify_runtime_transport(&fixture.transport).expect("verified");
    let output = temp.path().join("output");
    with_contract(&fixture, |contract| {
        prepare_runtime_release_with_contract(&fixture.transport, COMMIT, &output, contract)
    })
    .expect("prepare");
    for name in inventory(&fixture.transport) {
        let source = fs::read(fixture.transport.join(&name)).expect("source");
        let copied = fs::read(output.join(&name)).expect("copied");
        assert_eq!(source, copied);
        assert_eq!(
            format!("{:x}", Sha256::digest(&source)),
            format!("{:x}", Sha256::digest(&copied))
        );
    }
}
