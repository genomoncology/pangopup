use pangopup_assets::{
    AssetErrorKind, MaskProfile, ModelProfile, ReferenceProfile, RuntimeProfile, ScoringProfile,
    SnvProfile, canonical_runtime_profile_bytes, pack_runtime_transport, runtime_profile_id,
    unpack_runtime_transport, verify_runtime_transport,
};
use pangopup_core::ReferenceProvider;
use pangopup_index::{mask::MaskDomainsOpen, reference::ReferenceBundleOpen};
use pangopup_model::{ModelRepresentation, inspect_runtime_profile_bundle};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use tempfile::tempdir;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn representation(value: ModelRepresentation) -> &'static str {
    match value {
        ModelRepresentation::Singleton => "singleton",
        ModelRepresentation::ZeroPaddedBatch => "zero-padded-batch",
        ModelRepresentation::PairedStrandBatch => "paired-strand-batch",
    }
}

fn write_profile(path: &Path) -> String {
    let root = fixture_root();
    let model_path = root.join("pangolin-model-kernel-mini/bundle");
    let reference_path = root.join("reference-route-test/bundle");
    let mask_path = root.join("gencode-mask-mini/domains.pgm");
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
            semantics: "miniature-runtime-transport-test".to_owned(),
            distance: 50,
            masking_policy: "miniature".to_owned(),
            cpu_policy: "sequential:1/1".to_owned(),
        },
    };
    let bytes = canonical_runtime_profile_bytes(&profile).expect("canonical profile");
    let identity = runtime_profile_id(&bytes).expect("identity").to_string();
    fs::write(path, bytes).expect("write profile");
    identity
}

fn pack_fixture(root: &Path, name: &str) -> (PathBuf, String) {
    let fixtures = fixture_root();
    let profile = root.join(format!("{name}.profile.json"));
    let profile_id = write_profile(&profile);
    let output = root.join(name);
    let packed = pack_runtime_transport(
        &profile,
        &fixtures.join("pangolin-model-kernel-mini/bundle"),
        &fixtures.join("reference-route-test/bundle"),
        &fixtures.join("gencode-mask-mini/domains.pgm"),
        &output,
    )
    .expect("pack");
    assert_eq!(packed.runtime_profile_id, profile_id);
    (output, profile_id)
}

fn inventory(path: &Path) -> Vec<String> {
    let mut values: Vec<_> = fs::read_dir(path)
        .expect("list")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("UTF-8")
        })
        .collect();
    values.sort();
    values
}

#[test]
fn deterministic_closed_transport_verifies_and_round_trips_exactly() {
    let temp = tempdir().expect("temp");
    let (first, profile_id) = pack_fixture(temp.path(), "first");
    let (second, _) = pack_fixture(temp.path(), "second");
    let expected: BTreeSet<_> = [
        "domains.pgm.zst",
        "mask-NOTICE",
        "model-NOTICE",
        "model-manifest.json",
        "model.onnx.zst",
        "reference-NOTICE",
        "reference-manifest.json",
        "reference.pgr.zst",
        "runtime-profile.json",
        "runtime-transport.json",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        inventory(&first).into_iter().collect::<BTreeSet<_>>(),
        expected
    );
    assert_eq!(inventory(&first), inventory(&second));
    for name in inventory(&first) {
        assert_eq!(
            fs::read(first.join(&name)).expect("first member"),
            fs::read(second.join(name)).expect("second member")
        );
    }
    let verified = verify_runtime_transport(&first).expect("verify");
    assert_eq!(verified.runtime_profile_id, profile_id);
    let unpacked = temp.path().join("unpacked");
    let outcome = unpack_runtime_transport(&first, &unpacked).expect("unpack");
    assert_eq!(outcome.runtime_profile_id, profile_id);

    let fixtures = fixture_root();
    for (actual, expected) in [
        (
            "runtime-profile.json",
            temp.path().join("first.profile.json"),
        ),
        (
            "model/manifest.json",
            fixtures.join("pangolin-model-kernel-mini/bundle/manifest.json"),
        ),
        (
            "model/NOTICE",
            fixtures.join("pangolin-model-kernel-mini/bundle/NOTICE"),
        ),
        (
            "model/model.onnx",
            fixtures.join("pangolin-model-kernel-mini/bundle/model.onnx"),
        ),
        (
            "reference/manifest.json",
            fixtures.join("reference-route-test/bundle/manifest.json"),
        ),
        (
            "reference/NOTICE",
            fixtures.join("reference-route-test/bundle/NOTICE"),
        ),
        (
            "reference/reference.pgr",
            fixtures.join("reference-route-test/bundle/reference.pgr"),
        ),
        (
            "mask/domains.pgm",
            fixtures.join("gencode-mask-mini/domains.pgm"),
        ),
    ] {
        assert_eq!(
            fs::read(unpacked.join(actual)).expect("unpacked bytes"),
            fs::read(expected).expect("source bytes"),
            "{actual}"
        );
    }
    assert_eq!(
        fs::read(unpacked.join("mask/NOTICE")).expect("mask notice"),
        include_bytes!("../../../assets/notices/GENCODE-v38-NOTICE")
    );
}

#[test]
fn corruption_truncation_substitution_and_extra_members_fail_closed() {
    let temp = tempdir().expect("temp");
    for (ordinal, mutation) in ["corrupt", "truncate", "substitute", "extra"]
        .into_iter()
        .enumerate()
    {
        let (transport, _) = pack_fixture(temp.path(), &format!("case-{ordinal}"));
        match mutation {
            "corrupt" => {
                let path = transport.join("model.onnx.zst");
                let mut bytes = fs::read(&path).expect("read");
                let last = bytes.len() - 1;
                bytes[last] ^= 1;
                fs::write(path, bytes).expect("corrupt");
            }
            "truncate" => {
                let path = transport.join("reference.pgr.zst");
                let file = fs::OpenOptions::new().write(true).open(path).expect("open");
                let length = file.metadata().expect("metadata").len();
                file.set_len(length - 1).expect("truncate");
            }
            "substitute" => {
                fs::write(transport.join("model-NOTICE"), b"substitute").expect("substitute");
            }
            "extra" => fs::write(transport.join("extra"), b"x").expect("extra"),
            _ => unreachable!(),
        }
        assert!(verify_runtime_transport(&transport).is_err(), "{mutation}");
        let output = temp.path().join(format!("out-{ordinal}"));
        assert!(
            unpack_runtime_transport(&transport, &output).is_err(),
            "{mutation}"
        );
        assert!(!output.exists(), "{mutation} published output");
    }
}

#[test]
fn unsafe_shapes_and_output_conflicts_fail_without_replacement() {
    let temp = tempdir().expect("temp");
    let (transport, _) = pack_fixture(temp.path(), "unsafe");
    let target = transport.join("model-NOTICE");
    let saved = fs::read(&target).expect("notice");
    fs::remove_file(&target).expect("remove");
    std::os::unix::fs::symlink("mask-NOTICE", &target).expect("symlink");
    assert!(verify_runtime_transport(&transport).is_err());
    fs::remove_file(&target).expect("remove symlink");
    fs::write(&target, saved).expect("restore");

    let output = temp.path().join("occupied");
    fs::create_dir(&output).expect("occupied");
    fs::write(output.join("sentinel"), b"keep").expect("sentinel");
    assert!(unpack_runtime_transport(&transport, &output).is_err());
    assert_eq!(
        fs::read(output.join("sentinel")).expect("sentinel"),
        b"keep"
    );

    let fixtures = fixture_root();
    let profile = temp.path().join("profile.json");
    write_profile(&profile);
    let linked = temp.path().join("linked-model");
    copy_directory(&fixtures.join("pangolin-model-kernel-mini/bundle"), &linked);
    fs::remove_file(linked.join("model.onnx")).expect("remove model");
    std::os::unix::fs::symlink(
        fixtures.join("pangolin-model-kernel-mini/bundle/model.onnx"),
        linked.join("model.onnx"),
    )
    .expect("model link");
    assert!(
        pack_runtime_transport(
            &profile,
            &linked,
            &fixtures.join("reference-route-test/bundle"),
            &fixtures.join("gencode-mask-mini/domains.pgm"),
            &temp.path().join("never"),
        )
        .is_err()
    );
}

fn copy_directory(source: &Path, target: &Path) {
    fs::create_dir(target).expect("target");
    for entry in fs::read_dir(source).expect("source") {
        let entry = entry.expect("entry");
        fs::copy(entry.path(), target.join(entry.file_name())).expect("copy");
    }
}

fn rebind_stored_member(transport: &Path, name: &str) {
    let payload = fs::read(transport.join(name)).expect("payload");
    let manifest_path = transport.join("runtime-transport.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest")).expect("JSON");
    let members = manifest["members"].as_array_mut().expect("members");
    let member = members
        .iter_mut()
        .find(|member| member["name"] == name)
        .expect("selected member");
    member["stored_bytes"] = serde_json::Value::from(payload.len() as u64);
    member["stored_sha256"] =
        serde_json::Value::String(format!("sha256:{:x}", Sha256::digest(&payload)));
    fs::write(
        manifest_path,
        serde_jcs::to_vec(&manifest).expect("canonical manifest"),
    )
    .expect("rewrite manifest");
}

#[test]
fn manifest_consistent_trailing_bytes_and_second_frames_are_compression_errors() {
    let temp = tempdir().expect("temp");
    let (trailing, _) = pack_fixture(temp.path(), "trailing");
    let trailing_member = trailing.join("model.onnx.zst");
    let mut bytes = fs::read(&trailing_member).expect("frame");
    bytes.extend_from_slice(b"trailing");
    fs::write(&trailing_member, bytes).expect("append");
    rebind_stored_member(&trailing, "model.onnx.zst");
    assert_eq!(
        verify_runtime_transport(&trailing)
            .expect_err("trailing rejected")
            .kind(),
        AssetErrorKind::CompressionInvalid
    );

    let (concatenated, _) = pack_fixture(temp.path(), "concatenated");
    let member = concatenated.join("model.onnx.zst");
    let mut bytes = fs::read(&member).expect("first frame");
    let second = bytes.clone();
    bytes.extend_from_slice(&second);
    fs::write(&member, bytes).expect("second frame");
    rebind_stored_member(&concatenated, "model.onnx.zst");
    assert_eq!(
        verify_runtime_transport(&concatenated)
            .expect_err("second frame rejected")
            .kind(),
        AssetErrorKind::CompressionInvalid
    );
}

#[test]
fn late_unpack_failure_removes_private_stage_and_never_publishes() {
    let temp = tempdir().expect("temp");
    let (transport, _) = pack_fixture(temp.path(), "late");
    let member = transport.join("domains.pgm.zst");
    let mut bytes = fs::read(&member).expect("frame");
    bytes.extend_from_slice(b"late");
    fs::write(&member, bytes).expect("append");
    rebind_stored_member(&transport, "domains.pgm.zst");

    let output = temp.path().join("late-output");
    assert_eq!(
        unpack_runtime_transport(&transport, &output)
            .expect_err("late failure")
            .kind(),
        AssetErrorKind::CompressionInvalid
    );
    assert!(!output.exists());
    let hidden_prefix = ".late-output.pangopup-stage-";
    assert!(
        fs::read_dir(temp.path())
            .expect("parent")
            .all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(hidden_prefix)),
        "private stage was not cleaned"
    );
}

#[test]
fn hardlinked_profile_and_payload_inputs_are_rejected() {
    let temp = tempdir().expect("temp");
    let fixtures = fixture_root();
    let profile = temp.path().join("profile.json");
    write_profile(&profile);
    let profile_link = temp.path().join("profile-link.json");
    fs::hard_link(&profile, &profile_link).expect("profile hard link");
    assert!(
        pack_runtime_transport(
            &profile,
            &fixtures.join("pangolin-model-kernel-mini/bundle"),
            &fixtures.join("reference-route-test/bundle"),
            &fixtures.join("gencode-mask-mini/domains.pgm"),
            &temp.path().join("profile-never"),
        )
        .is_err()
    );

    fs::remove_file(profile_link).expect("remove profile link");
    let linked_model = temp.path().join("hardlinked-model");
    copy_directory(
        &fixtures.join("pangolin-model-kernel-mini/bundle"),
        &linked_model,
    );
    let source = temp.path().join("model-source.onnx");
    fs::copy(linked_model.join("model.onnx"), &source).expect("source");
    fs::remove_file(linked_model.join("model.onnx")).expect("remove copied payload");
    fs::hard_link(&source, linked_model.join("model.onnx")).expect("payload hard link");
    assert!(
        pack_runtime_transport(
            &profile,
            &linked_model,
            &fixtures.join("reference-route-test/bundle"),
            &fixtures.join("gencode-mask-mini/domains.pgm"),
            &temp.path().join("payload-never"),
        )
        .is_err()
    );
}
