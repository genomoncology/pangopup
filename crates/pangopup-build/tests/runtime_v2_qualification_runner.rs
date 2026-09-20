#![cfg(feature = "runtime-v2-qualification")]

use std::process::Command;

#[test]
fn qualification_runner_is_separate_and_requires_closed_explicit_inputs() {
    let binary = env!("CARGO_BIN_EXE_pangopup-runtime-v2-qualify");
    let help = Command::new(binary).arg("--help").output().expect("help");
    assert!(help.status.success());
    assert_eq!(
        String::from_utf8(help.stdout).expect("UTF-8 help"),
        "Usage: pangopup-runtime-v2-qualify prepare --v1-transport <ABSOLUTE_DIR> --sparse-bundle <ABSOLUTE_DIR> --scratch <ABSENT_ABSOLUTE_DIR> --output <ABSENT_ABSOLUTE_DIR>\n"
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
}
