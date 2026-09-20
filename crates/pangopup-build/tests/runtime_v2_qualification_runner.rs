#![cfg(feature = "runtime-v2-qualification")]

use pangopup_assets::{pack_runtime_transport, verify_runtime_transport};
use std::{path::Path, process::Command};

fn command_error(binary: &str, arguments: &[&std::ffi::OsStr]) -> serde_json::Value {
    let rejected = Command::new(binary)
        .args(arguments)
        .output()
        .expect("qualification command rejection");
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    serde_json::from_slice(&rejected.stderr).expect("JSON error")
}

#[test]
fn qualification_runner_is_separate_and_requires_closed_explicit_inputs() {
    let binary = env!("CARGO_BIN_EXE_pangopup-runtime-v2-qualify");
    let help = Command::new(binary).arg("--help").output().expect("help");
    assert!(help.status.success());
    assert_eq!(
        String::from_utf8(help.stdout).expect("UTF-8 help"),
        concat!(
            "Usage: pangopup-runtime-v2-qualify prepare --v1-transport <ABSOLUTE_DIR> --sparse-bundle <ABSOLUTE_DIR> --scratch <ABSENT_ABSOLUTE_DIR> --output <ABSENT_ABSOLUTE_DIR>\n",
            "       pangopup-runtime-v2-qualify verify --transport <ABSOLUTE_DIR>\n",
            "       pangopup-runtime-v2-qualify install --transport <ABSOLUTE_DIR> --data-dir <ABSOLUTE_DIR>\n",
            "       pangopup-runtime-v2-qualify admit --data-dir <ABSOLUTE_DIR> --expected-snv <SHA256_ID>\n",
        )
    );
    assert!(help.stderr.is_empty());

    let rejected = Command::new(binary)
        .args([
            "prepare",
            "--v1-transport",
            "relative-v1",
            "--sparse-bundle",
            "relative-sparse",
            "--scratch",
            "relative-scratch",
            "--output",
            "relative-output",
        ])
        .output()
        .expect("rejection");
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&rejected.stderr).expect("JSON error");
    assert_eq!(error["status"], "error");
    assert_eq!(error["code"], "QUALIFICATION_PATH");

    for arguments in [
        vec!["verify", "--transport", "relative-transport"],
        vec![
            "install",
            "--transport",
            "relative-transport",
            "--data-dir",
            "relative-data",
        ],
        vec![
            "admit",
            "--data-dir",
            "relative-data",
            "--expected-snv",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ],
    ] {
        let rejected = Command::new(binary)
            .args(arguments)
            .output()
            .expect("qualification command rejection");
        assert!(!rejected.status.success());
        let error: serde_json::Value =
            serde_json::from_slice(&rejected.stderr).expect("JSON error");
        assert_eq!(error["code"], "QUALIFICATION_PATH");
    }
}

#[test]
fn qualification_verify_and_install_route_only_to_checked_authority() {
    let binary = env!("CARGO_BIN_EXE_pangopup-runtime-v2-qualify");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let temp = tempfile::TempDir::new().expect("temporary root");
    let miniature = temp.path().join("miniature-transport");
    pack_runtime_transport(
        &fixtures.join("runtime-transport-mini/runtime-profile.json"),
        &fixtures.join("pangolin-model-kernel-mini/bundle"),
        &fixtures.join("reference-route-test/bundle"),
        &fixtures.join("gencode-mask-mini/domains.pgm"),
        &miniature,
    )
    .expect("complete miniature transport");
    verify_runtime_transport(&miniature).expect("ordinary structural verification");
    let data = temp.path().join("data");

    let error = command_error(
        binary,
        &[
            "verify".as_ref(),
            "--transport".as_ref(),
            miniature.as_os_str(),
        ],
    );
    assert_eq!(error["code"], "MANIFEST_INVALID");
    assert_eq!(
        error["message"],
        "runtime v2 profile does not match checked qualification authority"
    );

    let error = command_error(
        binary,
        &[
            "install".as_ref(),
            "--transport".as_ref(),
            miniature.as_os_str(),
            "--data-dir".as_ref(),
            data.as_os_str(),
        ],
    );
    assert_eq!(error["code"], "MANIFEST_INVALID");
    assert_eq!(
        error["message"],
        "runtime v2 profile does not match checked qualification authority"
    );
    assert!(
        !data.exists(),
        "rejection must precede destination creation"
    );
}

#[test]
fn qualification_admit_missing_data_is_only_a_command_smoke_test() {
    let binary = env!("CARGO_BIN_EXE_pangopup-runtime-v2-qualify");
    let temp = tempfile::TempDir::new().expect("temporary root");
    let data = temp.path().join("missing-data");
    let error = command_error(
        binary,
        &[
            "admit".as_ref(),
            "--data-dir".as_ref(),
            data.as_os_str(),
            "--expected-snv".as_ref(),
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".as_ref(),
        ],
    );
    assert_eq!(error["code"], "ASSETS_MISSING");
    assert_eq!(error["message"], "installed runtime profile is missing");
}
