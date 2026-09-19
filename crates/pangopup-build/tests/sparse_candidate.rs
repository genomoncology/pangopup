use pangopup_assets::MAX_FIXED11_BYTES;
use pangopup_build::{SparseCandidateArguments, build_sparse_candidate, verify_bundle};
use pangopup_index::{
    BundleManifest, IndexReader, VisitAllError, canonical_manifest_bytes,
    sparse_writer::SPARSE_INDEX_FORMAT,
};
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn prepare_bundle(temp: &TempDir) -> PathBuf {
    let bundle = temp.path().join("bundle");
    copy_bundle(&fixture("snv-regression/bundle"), &bundle);
    bundle
}

fn arguments(temp: &TempDir, bundle: &Path, label: &str) -> SparseCandidateArguments {
    SparseCandidateArguments {
        fixed_bundle: bundle.to_owned(),
        scratch: temp.path().join(format!("{label}.scratch")),
        candidate: temp.path().join(format!("{label}.pgi")),
        report: temp.path().join(format!("{label}.json")),
        expected_bundle_id: Some(verify_bundle(bundle).expect("verify bundle").bundle_id),
    }
}

fn copy_bundle(source: &Path, destination: &Path) {
    fs::create_dir(destination).expect("copy bundle directory");
    for member in ["NOTICE", "manifest.json", "scores.pgi"] {
        fs::copy(source.join(member), destination.join(member)).expect("copy member");
    }
}

fn rewrite_manifest(bundle: &Path, edit: impl FnOnce(&mut BundleManifest)) {
    let path = bundle.join("manifest.json");
    let mut manifest: BundleManifest =
        serde_json::from_slice(&fs::read(&path).expect("manifest")).expect("manifest JSON");
    edit(&mut manifest);
    fs::write(
        path,
        canonical_manifest_bytes(&manifest).expect("canonical manifest"),
    )
    .expect("write manifest");
}

#[test]
fn miniature_build_is_exact_deterministic_and_bounded() {
    let temp = TempDir::new().expect("temp");
    let bundle = prepare_bundle(&temp);
    let first = arguments(&temp, &bundle, "first");
    let second = arguments(&temp, &bundle, "second");
    let first_outcome = build_sparse_candidate(&first).expect("first candidate");
    let second_outcome = build_sparse_candidate(&second).expect("second candidate");

    assert_eq!(
        fs::read(&first.candidate).expect("first candidate bytes"),
        fs::read(&second.candidate).expect("second candidate bytes")
    );
    assert_eq!(
        fs::read(&first.report).expect("first report bytes"),
        fs::read(&second.report).expect("second report bytes")
    );
    assert_eq!(first_outcome, second_outcome);
    assert!(!first.scratch.exists());
    assert!(!PathBuf::from(format!("{}.exceptions", first.scratch.display())).exists());

    let report_bytes = fs::read(&first.report).expect("report");
    let report: serde_json::Value = serde_json::from_slice(&report_bytes).expect("report JSON");
    assert_eq!(
        report_bytes,
        serde_jcs::to_vec(&report).expect("canonical report JSON")
    );
    assert!(!String::from_utf8_lossy(&report_bytes).contains(&temp.path().display().to_string()));
    assert_eq!(report["schema"], "pangopup.sparse-candidate-report.v1");
    assert_eq!(
        report["input_bundle_id"],
        first.expected_bundle_id.expect("expected bundle ID")
    );
    assert_eq!(report["candidate_sha256"], first_outcome.candidate_sha256);
    assert_eq!(report["candidate_bytes"], first_outcome.candidate_bytes);
    let final_candidate = fs::read(&first.candidate).expect("final candidate bytes");
    assert_eq!(
        report["candidate_sha256"],
        format!("sha256:{:x}", Sha256::digest(&final_candidate))
    );
    assert_eq!(
        report["candidate_bytes"],
        u64::try_from(final_candidate.len()).expect("candidate byte count")
    );
    assert_eq!(
        report["logical_source_sha256"],
        report["logical_decoded_sha256"]
    );
    assert_eq!(
        report["writer_counts"]["loci"],
        report["decoded_counts"]["loci"]
    );
    assert!(
        report["gene_buffer"]["maximum_length"]
            .as_u64()
            .expect("maximum length")
            > 0
    );
    assert!(
        report["gene_buffer"]["maximum_capacity_bytes"]
            .as_u64()
            .expect("maximum capacity bytes")
            < 512 * 1024 * 1024
    );
}

#[test]
fn sparse_source_is_rejected_without_output_or_scratch() {
    let temp = TempDir::new().expect("temp");
    let fixed = prepare_bundle(&temp);
    let seed = arguments(&temp, &fixed, "seed");
    build_sparse_candidate(&seed).expect("seed sparse candidate");

    let sparse = temp.path().join("sparse-source");
    fs::create_dir(&sparse).expect("sparse bundle directory");
    fs::copy(fixed.join("NOTICE"), sparse.join("NOTICE")).expect("copy notice");
    fs::copy(&seed.candidate, sparse.join("scores.pgi")).expect("copy sparse scores");
    fs::copy(fixed.join("manifest.json"), sparse.join("manifest.json")).expect("copy manifest");
    rewrite_manifest(&sparse, |manifest| {
        let scores = fs::read(sparse.join("scores.pgi")).expect("sparse scores");
        manifest.index_format = SPARSE_INDEX_FORMAT.to_owned();
        manifest.members[1].media_type = "application/vnd.pangopup.sparse-direct".to_owned();
        manifest.members[1].size = scores.len() as u64;
        manifest.members[1].sha256 = format!("sha256:{:x}", Sha256::digest(scores));
    });

    let rejected = arguments(&temp, &sparse, "rejected");
    let sidecar = PathBuf::from(format!("{}.exceptions", rejected.scratch.display()));
    let error = build_sparse_candidate(&rejected).expect_err("sparse source format");
    assert_eq!(error.code, "SPARSE_INPUT_FORMAT");
    assert!(!rejected.scratch.exists());
    assert!(!sidecar.exists());
    assert!(!rejected.candidate.exists());
    assert!(!rejected.report.exists());
}

#[test]
fn certification_rejects_rehashed_manifest_lies_and_member_changes() {
    let temp = TempDir::new().expect("temp");
    let good = prepare_bundle(&temp);
    for (label, edit) in [
        ("counts", 0_u8),
        ("logical-source", 1_u8),
        ("logical-decoded", 2_u8),
    ] {
        let bundle = temp.path().join(label);
        copy_bundle(&good, &bundle);
        rewrite_manifest(&bundle, |manifest| match edit {
            0 => manifest.counts.gene_loci += 1,
            1 => manifest.logical_source.sha256 = format!("sha256:{}", "0".repeat(64)),
            2 => manifest.logical_decoded.sha256 = format!("sha256:{}", "1".repeat(64)),
            _ => unreachable!(),
        });
        let mut args = arguments(&temp, &good, label);
        args.fixed_bundle = bundle;
        args.expected_bundle_id = None;
        assert!(build_sparse_candidate(&args).is_err(), "{label}");
        assert!(!args.candidate.exists());
        assert!(!args.report.exists());
    }

    let notice_bundle = temp.path().join("notice");
    copy_bundle(&good, &notice_bundle);
    fs::write(notice_bundle.join("NOTICE"), b"wrong notice").expect("wrong notice");
    rewrite_manifest(&notice_bundle, |manifest| {
        let notice = manifest
            .members
            .iter_mut()
            .find(|member| member.path == "NOTICE")
            .expect("NOTICE member");
        notice.size = 12;
        notice.sha256 = format!("sha256:{:x}", Sha256::digest(b"wrong notice"));
    });
    let mut args = arguments(&temp, &good, "notice");
    args.fixed_bundle = notice_bundle;
    args.expected_bundle_id = None;
    assert!(build_sparse_candidate(&args).is_err());

    let corrupt_bundle = temp.path().join("corrupt");
    copy_bundle(&good, &corrupt_bundle);
    let scores = corrupt_bundle.join("scores.pgi");
    let mut bytes = fs::read(&scores).expect("scores");
    *bytes.last_mut().expect("score byte") ^= 1;
    fs::write(scores, bytes).expect("corrupt scores");
    let mut args = arguments(&temp, &good, "corrupt");
    args.fixed_bundle = corrupt_bundle;
    args.expected_bundle_id = None;
    assert!(build_sparse_candidate(&args).is_err());
}

#[test]
fn allocation_limit_fails_before_visiting_the_gene() {
    let temp = TempDir::new().expect("temp");
    let bundle = prepare_bundle(&temp);
    let reader = IndexReader::open(&bundle.join("scores.pgi")).expect("reader");
    let visited = Cell::new(false);
    let error = reader
        .visit_genes_bounded(1, 512 * 1024 * 1024, |_| {
            visited.set(true);
            Ok::<_, ()>(())
        })
        .expect_err("gene limit");
    assert!(!visited.get());
    assert!(
        matches!(error, VisitAllError::Index(error) if error.to_string().contains("allocation limit"))
    );
}

#[test]
fn held_member_size_ceilings_reject_before_mapping_or_decoding() {
    let temp = TempDir::new().expect("temp");
    for (label, member, size, expected_code) in [
        ("manifest-too-large", "manifest.json", 1024 * 1024 + 1, None),
        (
            "notice-too-large",
            "NOTICE",
            64 * 1024 + 1,
            Some("BUNDLE_NOTICE"),
        ),
        (
            "scores-too-large",
            "scores.pgi",
            MAX_FIXED11_BYTES + 1,
            Some("BUNDLE_INDEX"),
        ),
    ] {
        let bundle = temp.path().join(label);
        copy_bundle(&fixture("snv-regression/bundle"), &bundle);
        fs::OpenOptions::new()
            .write(true)
            .open(bundle.join(member))
            .expect("open oversized member")
            .set_len(size)
            .expect("set sparse oversized length");
        let args = SparseCandidateArguments {
            fixed_bundle: bundle,
            scratch: temp.path().join(format!("{label}.scratch")),
            candidate: temp.path().join(format!("{label}.pgi")),
            report: temp.path().join(format!("{label}.json")),
            expected_bundle_id: None,
        };
        let error = build_sparse_candidate(&args).expect_err(label);
        if let Some(expected_code) = expected_code {
            assert_eq!(error.code, expected_code, "{label}");
        }
        assert!(error.message.contains("size") || error.message.contains("ceiling"));
        assert!(!args.scratch.exists(), "{label}");
        assert!(!args.candidate.exists(), "{label}");
        assert!(!args.report.exists(), "{label}");
    }
}

#[test]
fn existing_paths_and_derived_exception_collision_are_preserved() {
    let temp = TempDir::new().expect("temp");
    let bundle = prepare_bundle(&temp);
    let mut wrong_identity = arguments(&temp, &bundle, "wrong-identity");
    wrong_identity.expected_bundle_id = Some(format!("sha256:{}", "0".repeat(64)));
    let error = build_sparse_candidate(&wrong_identity).expect_err("wrong fixed bundle identity");
    assert_eq!(error.code, "SPARSE_INPUT_IDENTITY");
    assert!(!wrong_identity.candidate.exists());
    assert!(!wrong_identity.report.exists());

    for target in ["scratch", "candidate", "report"] {
        let args = arguments(&temp, &bundle, target);
        let path = match target {
            "scratch" => &args.scratch,
            "candidate" => &args.candidate,
            "report" => &args.report,
            _ => unreachable!(),
        };
        fs::write(path, b"keep").expect("existing path");
        assert!(build_sparse_candidate(&args).is_err());
        assert_eq!(fs::read(path).expect("preserved"), b"keep");
    }

    let args = arguments(&temp, &bundle, "sidecar");
    let sidecar = PathBuf::from(format!("{}.exceptions", args.scratch.display()));
    fs::write(&sidecar, b"keep sidecar").expect("sidecar");
    assert!(build_sparse_candidate(&args).is_err());
    assert_eq!(
        fs::read(sidecar).expect("preserved sidecar"),
        b"keep sidecar"
    );
    assert!(!args.scratch.exists());
    assert!(!args.candidate.exists());
    assert!(!args.report.exists());
}

#[test]
fn maintainer_cli_requires_identity_and_publishes_both_outputs() {
    let temp = TempDir::new().expect("temp");
    let bundle = prepare_bundle(&temp);
    let bundle_id = verify_bundle(&bundle).expect("verify fixture").bundle_id;
    let scratch = temp.path().join("cli.scratch");
    let candidate = temp.path().join("cli.pgi");
    let report = temp.path().join("cli.json");
    let output = Command::new(env!("CARGO_BIN_EXE_pangopup-build"))
        .args(["sparse-candidate", "build", "--fixed-bundle"])
        .arg(&bundle)
        .args(["--expected-bundle-id", &bundle_id, "--scratch"])
        .arg(&scratch)
        .arg("--candidate")
        .arg(&candidate)
        .arg("--report")
        .arg(&report)
        .output()
        .expect("run sparse candidate CLI");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let outcome: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("CLI outcome JSON");
    assert_eq!(outcome["status"], "built");
    assert!(candidate.exists());
    assert!(report.exists());
    assert!(!scratch.exists());

    let missing_identity = Command::new(env!("CARGO_BIN_EXE_pangopup-build"))
        .args(["sparse-candidate", "build", "--fixed-bundle"])
        .arg(&bundle)
        .arg("--scratch")
        .arg(temp.path().join("missing.scratch"))
        .arg("--candidate")
        .arg(temp.path().join("missing.pgi"))
        .arg("--report")
        .arg(temp.path().join("missing.json"))
        .output()
        .expect("run incomplete sparse candidate CLI");
    assert_eq!(missing_identity.status.code(), Some(2));
}
