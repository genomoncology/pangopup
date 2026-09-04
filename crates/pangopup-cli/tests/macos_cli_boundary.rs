#![cfg(not(target_os = "linux"))]

use serde_json::Value;
use std::{
    ffi::OsString,
    path::Path,
    process::{Command, Output},
};

fn run(arguments: impl IntoIterator<Item = OsString>) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pangopup"))
        .args(arguments)
        .output()
        .expect("run pangopup")
}

fn assert_unsupported(output: &Output, message: &str) {
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("one JSON error");
    assert_eq!(error["code"], "UNSUPPORTED_PLATFORM");
    assert_eq!(error["message"], message);
    assert_eq!(
        output.stderr.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
}

fn text(path: &Path) -> OsString {
    path.as_os_str().to_owned()
}

#[test]
fn status_refuses_before_inspecting_a_native_asset_root() {
    let temp = tempfile::tempdir().expect("temp");
    let data = temp.path().join("status-data");
    let output = run([
        OsString::from("status"),
        OsString::from("--data-dir"),
        text(&data),
    ]);
    assert_unsupported(&output, "local asset installation requires Linux");
    assert!(!data.exists());
}

#[test]
fn asset_installs_refuse_before_opening_inputs_or_creating_a_root() {
    let temp = tempfile::tempdir().expect("temp");
    let data = temp.path().join("install-data");
    let output = run([
        OsString::from("assets"),
        OsString::from("install"),
        OsString::from("--transport"),
        text(&temp.path().join("missing-transport")),
        OsString::from("--data-dir"),
        text(&data),
    ]);
    assert_unsupported(&output, "local asset installation requires Linux");
    assert!(!data.exists());

    let runtime_data = temp.path().join("runtime-install-data");
    let output = run([
        OsString::from("assets"),
        OsString::from("runtime"),
        OsString::from("install"),
        OsString::from("--profile"),
        text(&temp.path().join("missing-profile.json")),
        OsString::from("--model-bundle"),
        text(&temp.path().join("missing-model")),
        OsString::from("--reference-bundle"),
        text(&temp.path().join("missing-reference")),
        OsString::from("--mask"),
        text(&temp.path().join("missing-mask")),
        OsString::from("--data-dir"),
        text(&runtime_data),
    ]);
    assert_unsupported(&output, "local asset installation requires Linux");
    assert!(!runtime_data.exists());
}

#[test]
fn sync_refuses_before_creating_native_asset_or_cache_roots() {
    let temp = tempfile::tempdir().expect("temp");
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
    assert_unsupported(&output, "asset sync is supported only on Linux");
    assert!(!data.exists());
    assert!(!cache.exists());
}

#[test]
fn serve_refuses_before_inspecting_a_native_asset_root() {
    let temp = tempfile::tempdir().expect("temp");
    let data = temp.path().join("serve-data");
    let output = run([
        OsString::from("serve"),
        OsString::from("--data-dir"),
        text(&data),
    ]);
    assert_unsupported(&output, "local asset installation requires Linux");
    assert!(!data.exists());
}

#[test]
fn uninstall_keeps_its_existing_non_linux_refusal() {
    let temp = tempfile::tempdir().expect("temp");
    let output = Command::new(env!("CARGO_BIN_EXE_pangopup"))
        .args(["uninstall", "--yes"])
        .env("XDG_DATA_HOME", temp.path().join("data"))
        .env("XDG_CACHE_HOME", temp.path().join("cache"))
        .output()
        .expect("run pangopup uninstall");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("one JSON error");
    assert_eq!(error["code"], "UNINSTALL_UNSAFE");
    assert_eq!(
        error["message"],
        "direct uninstall is supported only on Linux"
    );
}
