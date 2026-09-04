#![cfg(target_os = "macos")]

use serde_json::Value;
use std::{
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn run(arguments: impl IntoIterator<Item = OsString>) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pangopup"))
        .args(arguments)
        .output()
        .expect("run pangopup")
}

fn error(output: &Output) -> Value {
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    serde_json::from_slice(&output.stderr).expect("one JSON error")
}

fn text(path: &Path) -> OsString {
    path.as_os_str().to_owned()
}

#[test]
fn status_reports_missing_without_creating_the_native_asset_root() {
    let temp = tempfile::tempdir().expect("temp");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("private temp");
    let data = temp.path().join("status-data");
    let output = run([
        OsString::from("status"),
        OsString::from("--data-dir"),
        text(&data),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status: Value = serde_json::from_slice(&output.stdout).expect("status JSON");
    assert_eq!(status["status"], "missing");
    assert_eq!(
        status["data_dir"].as_str(),
        Some(data.to_str().expect("UTF-8"))
    );
    assert_eq!(status["snv"]["status"], "missing");
    assert_eq!(status["runtime"]["status"], "missing");
    assert!(!data.exists());
}

#[test]
fn caller_supplied_snv_transport_installs_on_macos() {
    let temp = tempfile::tempdir().expect("temp");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("private temp");
    let transport = temp.path().join("transport");
    pangopup_assets::pack_bundle(
        &repository_path("tests/fixtures/snv-regression/bundle"),
        &transport,
    )
    .expect("pack portable transport");
    let data = temp.path().join("install-data");
    let output = run([
        OsString::from("assets"),
        OsString::from("install"),
        OsString::from("--transport"),
        text(&transport),
        OsString::from("--data-dir"),
        text(&data),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let installed: Value = serde_json::from_slice(&output.stdout).expect("install JSON");
    assert_eq!(installed["status"], "installed");
    assert!(data.join("active.json").is_file());
}

#[test]
fn runtime_install_reaches_asset_validation_on_macos() {
    let temp = tempfile::tempdir().expect("temp");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("private temp");
    let data = temp.path().join("runtime-install-data");
    let output = run([
        OsString::from("assets"),
        OsString::from("runtime"),
        OsString::from("install"),
        OsString::from("--profile"),
        text(&repository_path(
            "planning/artifacts/024-four-asset-runtime-profile.json",
        )),
        OsString::from("--model-bundle"),
        text(&temp.path().join("not-opened-model")),
        OsString::from("--reference-bundle"),
        text(&temp.path().join("not-opened-reference")),
        OsString::from("--mask"),
        text(&temp.path().join("not-opened-mask")),
        OsString::from("--data-dir"),
        text(&data),
    ]);
    let failure = error(&output);
    assert_eq!(failure["code"], "ASSETS_MISSING");
    assert!(!data.join("runtime/active.json").exists());
    assert!(!data.join("runtime/components").exists());
}

#[test]
fn offline_sync_reaches_cache_validation_on_macos() {
    let temp = tempfile::tempdir().expect("temp");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("private temp");
    let data = temp.path().join("sync-data");
    let cache = temp.path().join("sync-cache");
    let output = run([
        OsString::from("sync"),
        OsString::from("--offline"),
        OsString::from("--data-dir"),
        text(&data),
        OsString::from("--cache-dir"),
        text(&cache),
    ]);
    let failure = error(&output);
    assert_eq!(failure["code"], "ASSET_SYNC_INCOMPLETE");
    assert_eq!(failure["details"]["snv"]["code"], "ASSETS_MISSING");
}

#[test]
fn serve_reaches_asset_validation_on_macos() {
    let temp = tempfile::tempdir().expect("temp");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("private temp");
    let data = temp.path().join("serve-data");
    let output = run([
        OsString::from("serve"),
        OsString::from("--data-dir"),
        text(&data),
    ]);
    let failure = error(&output);
    assert_eq!(failure["code"], "ASSETS_MISSING");
    assert!(!data.exists());
}

#[test]
fn uninstall_keeps_its_existing_macos_refusal() {
    let temp = tempfile::tempdir().expect("temp");
    let output = Command::new(env!("CARGO_BIN_EXE_pangopup"))
        .args(["uninstall", "--yes"])
        .env("XDG_DATA_HOME", temp.path().join("data"))
        .env("XDG_CACHE_HOME", temp.path().join("cache"))
        .output()
        .expect("run pangopup uninstall");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let failure: Value = serde_json::from_slice(&output.stderr).expect("one JSON error");
    assert_eq!(failure["code"], "UNINSTALL_UNSAFE");
    assert_eq!(
        failure["message"],
        "direct uninstall is supported only on Linux"
    );
}
