use pangopup_build::reference_candidates::{MINI_PROFILE, inspect_candidates};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/reference-candidates-mini")
}

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "pangopup-reference-candidate-test-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).expect("create scratch");
    path
}

#[test]
fn reference_candidates_checked_miniature_inspects_exactly() {
    let root = fixture();
    let outcome = inspect_candidates(&root.join("candidates"), &root.join("corpus"))
        .expect("inspect checked miniature");
    assert!(outcome.ok);
    assert_eq!(outcome.profile, MINI_PROFILE);
    assert_eq!(outcome.contexts_verified, 5);
    assert_eq!(outcome.members.len(), 3);
}

#[test]
fn reference_candidates_changed_member_and_cross_profile_fail_closed() {
    let source = fixture();
    let root = scratch();
    let candidates = root.join("candidates");
    fs::create_dir(&candidates).expect("create candidates");
    for name in [
        "manifest.json",
        "ascii8.pgr",
        "iupac4.pgr",
        "acgt2-rle-v1.pgr",
    ] {
        fs::copy(source.join("candidates").join(name), candidates.join(name)).expect("copy member");
    }
    let mut bytes = fs::read(candidates.join("iupac4.pgr")).expect("read member");
    bytes[4096] ^= 1;
    fs::write(candidates.join("iupac4.pgr"), bytes).expect("mutate member");
    let error =
        inspect_candidates(&candidates, &source.join("corpus")).expect_err("changed member");
    assert!(matches!(
        error.code(),
        "unsupported_profile" | "candidate_member"
    ));
    fs::remove_dir_all(root).expect("remove scratch");
}

#[test]
fn reference_candidates_cli_is_one_json_line_on_stdout() {
    let root = fixture();
    let output = Command::new(env!("CARGO_BIN_EXE_pangopup-build"))
        .args(["reference-candidates", "inspect", "--candidates"])
        .arg(root.join("candidates"))
        .args(["--corpus"])
        .arg(root.join("corpus"))
        .output()
        .expect("run inspect");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON result");
    assert_eq!(value["ok"], true);
    assert_eq!(value["profile"], MINI_PROFILE);

    let usage = Command::new(env!("CARGO_BIN_EXE_pangopup-build"))
        .args(["reference-candidates", "inspect", "--corpus"])
        .arg(root.join("corpus"))
        .output()
        .expect("run usage failure");
    assert_eq!(usage.status.code(), Some(2));
    assert!(usage.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&usage.stdout).expect("JSON failure");
    assert_eq!(value["error"]["code"], "usage");
}

#[test]
fn reference_candidates_literal_and_corpus_identity_are_independent() {
    let source = fixture();
    let root = scratch();
    let corpus = root.join("corpus");
    fs::create_dir(&corpus).expect("create corpus");
    fs::copy(
        source.join("corpus/manifest.json"),
        corpus.join("manifest.json"),
    )
    .expect("copy manifest");
    let mut cases = fs::read(source.join("corpus/cases.jsonl")).expect("read cases");
    let position = cases
        .iter()
        .position(|byte| *byte == b'A')
        .expect("literal base");
    cases[position] = b'C';
    fs::write(corpus.join("cases.jsonl"), cases).expect("write changed literal");
    let error = inspect_candidates(&source.join("candidates"), &corpus)
        .expect_err("changed literal corpus");
    assert_eq!(error.code(), "corpus_identity");
    fs::remove_dir_all(root).expect("remove scratch");
}

#[test]
fn reference_candidates_cli_rejects_duplicate_unknown_and_missing_flags_safely() {
    let root = fixture();
    for arguments in [
        vec![
            "reference-candidates",
            "inspect",
            "--corpus",
            "x",
            "--corpus",
            "x",
        ],
        vec![
            "reference-candidates",
            "inspect",
            "--unknown",
            "x",
            "--corpus",
            "x",
        ],
        vec!["reference-candidates", "inspect", "--candidates", "x"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_pangopup-build"))
            .args(arguments)
            .output()
            .expect("run usage");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout.iter().filter(|byte| **byte == b'\n').count(),
            1
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("usage JSON");
        assert_eq!(value["error"]["code"], "usage");
    }
    let secret = root.join("not-present-secret-name");
    let output = Command::new(env!("CARGO_BIN_EXE_pangopup-build"))
        .args(["reference-candidates", "inspect", "--candidates"])
        .arg(&secret)
        .args(["--corpus"])
        .arg(root.join("corpus"))
        .output()
        .expect("run operational failure");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).expect("UTF-8 failure");
    assert!(!text.contains(secret.to_str().expect("secret path")));
    let value: serde_json::Value = serde_json::from_str(&text).expect("failure JSON");
    assert_eq!(value["error"]["code"], "candidate_set");
}
