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
    os::{
        fd::AsRawFd,
        unix::{ffi::OsStrExt, fs::FileTypeExt, fs::PermissionsExt},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
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

/// Longest a bounded `runtime-transport verify` child may take before this test
/// declares it stopped. Generous against a slow machine, finite against a
/// member the command can never finish opening.
const VERIFY_DEADLINE: Duration = Duration::from_secs(30);

struct BoundedVerify {
    returned: bool,
    code: Option<i32>,
    stderr: String,
}

/// Runs `runtime-transport verify` in a child process under a wall-clock
/// deadline. The child is killed and reaped when the deadline passes, so a
/// member the command cannot finish opening fails this test instead of
/// stopping the suite. Output goes to files rather than pipes so polling the
/// child can never block on a full pipe buffer.
fn verify_under_deadline(scratch: &Path, transport: &Path) -> BoundedVerify {
    let stderr_path = scratch.join("verify.stderr");
    let stdout_path = scratch.join("verify.stdout");
    let mut child = Command::new(env!("CARGO_BIN_EXE_pangopup-build"))
        .args(["runtime-transport", "verify", "--transport"])
        .arg(transport)
        .stdin(Stdio::null())
        .stdout(Stdio::from(fs::File::create(&stdout_path).expect("stdout")))
        .stderr(Stdio::from(fs::File::create(&stderr_path).expect("stderr")))
        .spawn()
        .expect("spawn verify");
    let deadline = Instant::now() + VERIFY_DEADLINE;
    let status = loop {
        match child.try_wait().expect("poll verify") {
            Some(status) => break Some(status),
            None if Instant::now() >= deadline => {
                child.kill().expect("kill stopped verify");
                child.wait().expect("reap stopped verify");
                break None;
            }
            None => std::thread::sleep(Duration::from_millis(25)),
        }
    };
    BoundedVerify {
        returned: status.is_some(),
        code: status.and_then(|status| status.code()),
        stderr: String::from_utf8(fs::read(&stderr_path).expect("read stderr")).expect("UTF-8"),
    }
}

/// A label, the member to spoil, how to spoil it, and the error code the
/// refusal must carry. The code is per row because the boundary row is here to
/// prove that the reclassification of a member that is not a regular file did
/// not swallow a genuine IO fault with it.
type ShapeCase = (&'static str, &'static str, fn(&Path, &str), &'static str);

fn replace_member_with_fifo(transport: &Path, member: &str) {
    let path = transport.join(member);
    fs::remove_file(&path).expect("remove member");
    let name = std::ffi::CString::new(path.as_os_str().as_bytes()).expect("member path");
    let made = unsafe { libc::mkfifo(name.as_ptr(), 0o644) };
    assert_eq!(made, 0, "mkfifo {}", path.display());
}

fn replace_member_with_directory(transport: &Path, member: &str) {
    let path = transport.join(member);
    fs::remove_file(&path).expect("remove member");
    fs::create_dir(&path).expect("directory member");
}

/// Binds through `/proc/self/fd` because a socket address is capped near 108
/// bytes and a temporary directory path can be longer than that on its own.
fn replace_member_with_socket(transport: &Path, member: &str) {
    fs::remove_file(transport.join(member)).expect("remove member");
    let held = fs::File::open(transport).expect("hold transport");
    let short = PathBuf::from(format!("/proc/self/fd/{}", held.as_raw_fd())).join(member);
    let listener = std::os::unix::net::UnixListener::bind(&short).expect("socket member");
    drop(listener);
    drop(held);
    let kind = fs::symlink_metadata(transport.join(member)).expect("socket member");
    assert!(
        kind.file_type().is_socket(),
        "the socket fixture did not leave a socket at {member}"
    );
}

/// A regular member the process cannot read. It is the boundary of the rule
/// under test: a member that is a regular file and fails to open is an IO
/// fault, and must stay one after a member that is not a regular file becomes
/// a bad member set.
fn make_member_unreadable(transport: &Path, member: &str) {
    let path = transport.join(member);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).expect("unreadable member");
    assert!(
        fs::read(&path).is_err(),
        "the unreadable fixture can still be read, so it measures nothing"
    );
}

/// A member that is not a regular file is refused by name, whatever kind of
/// non-regular file it is, and the command always returns. A FIFO is the kind
/// that stops the command today: `open` on it waits for a writer that never
/// arrives.
///
/// Verify reaches every member through the one inventory pass in
/// `require_transport_inventory_held`, which opens each name before either the
/// raw-member loop or the stored-frame reader runs. So both FIFO rows land on
/// the same open today. The stored row stays because a stored member has a
/// second opener behind that pass, and it must be refused as a bad member
/// rather than as a corrupt frame if the order ever changes.
///
/// The last row is the boundary. A member that is a regular file and cannot be
/// opened is still an IO fault, so widening the refusal to cover every member
/// that is not a regular file must not swallow it.
#[test]
fn a_member_that_is_not_a_regular_file_is_refused_by_name_and_verify_returns() {
    let temp = tempdir().expect("temp");
    let (packed, _) = pack_fixture(temp.path(), "shapes");

    let intact = verify_under_deadline(temp.path(), &packed);
    assert!(
        intact.returned,
        "verify did not return on an intact transport"
    );
    assert_eq!(
        intact.code,
        Some(0),
        "an intact transport must still verify: {}",
        intact.stderr
    );

    assert_ne!(
        unsafe { libc::geteuid() },
        0,
        "this test measures a permission boundary and cannot run as root"
    );

    let cases: [ShapeCase; 5] = [
        (
            "fifo-raw",
            "model-NOTICE",
            replace_member_with_fifo,
            "PART_SET_INVALID",
        ),
        (
            "fifo-stored",
            "model.onnx.zst",
            replace_member_with_fifo,
            "PART_SET_INVALID",
        ),
        (
            "directory",
            "mask-NOTICE",
            replace_member_with_directory,
            "PART_SET_INVALID",
        ),
        (
            "socket",
            "reference-NOTICE",
            replace_member_with_socket,
            "PART_SET_INVALID",
        ),
        (
            "unreadable",
            "reference-manifest.json",
            make_member_unreadable,
            "INPUT_IO",
        ),
    ];
    for (label, member, spoil, code) in cases {
        let transport = temp.path().join(format!("shapes-{label}"));
        copy_directory(&packed, &transport);
        spoil(&transport, member);

        let observed = verify_under_deadline(temp.path(), &transport);
        assert!(
            observed.returned,
            "{label}: verify never returned on a spoiled {member}"
        );
        assert_eq!(
            observed.code,
            Some(1),
            "{label}: verify must refuse a spoiled {member}, stderr {}",
            observed.stderr
        );
        assert!(
            observed.stderr.contains(code),
            "{label}: the refusal must carry {code}, got {}",
            observed.stderr
        );
        if code == "PART_SET_INVALID" {
            assert!(
                observed.stderr.contains(member),
                "{label}: the refusal must name the member, got {}",
                observed.stderr
            );
        } else {
            assert!(
                !observed.stderr.contains("PART_SET_INVALID"),
                "{label}: an unreadable regular member is an IO fault, not a bad member set, got {}",
                observed.stderr
            );
        }
    }
}
