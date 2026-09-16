use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    println!(
        "cargo:rustc-env=PANGOPUP_TARGET={}",
        env::var("TARGET").expect("target triple")
    );
    println!(
        "cargo:rustc-env=PANGOPUP_BUILD_PROFILE={}",
        env::var("PROFILE").expect("build profile")
    );
    let rustc = env::var_os("RUSTC").expect("rustc path");
    let rustc_version = Command::new(rustc)
        .arg("--version")
        .output()
        .expect("run rustc --version");
    assert!(rustc_version.status.success(), "rustc --version failed");
    let rustc_version = String::from_utf8(rustc_version.stdout).expect("rustc version UTF-8");
    println!(
        "cargo:rustc-env=PANGOPUP_RUSTC_VERSION={}",
        rustc_version.trim()
    );
    let crate_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let workspace = crate_dir.join("../..");
    let git_commit = git_stdout(&workspace, &["rev-parse", "HEAD"]);
    let git_clean = git_stdout(
        &workspace,
        &["status", "--porcelain", "--untracked-files=all"],
    )
    .map(|status| status.is_empty());
    println!(
        "cargo:rustc-env=PANGOPUP_GIT_COMMIT={}",
        git_commit.as_deref().unwrap_or("unavailable")
    );
    println!(
        "cargo:rustc-env=PANGOPUP_GIT_CLEAN={}",
        match git_clean {
            Some(true) => "true",
            Some(false) => "false",
            None => "unavailable",
        }
    );
    if git_commit.is_some() {
        if let Some(git_dir) = git_stdout(&workspace, &["rev-parse", "--absolute-git-dir"]) {
            println!(
                "cargo:rerun-if-changed={}",
                resolve_watch_path(&workspace, &git_dir)
                    .join("HEAD")
                    .display()
            );
        }
        if let Some(reference) = git_stdout(&workspace, &["symbolic-ref", "-q", "HEAD"])
            && let Some(reference_path) =
                git_stdout(&workspace, &["rev-parse", "--git-path", &reference])
        {
            println!(
                "cargo:rerun-if-changed={}",
                resolve_watch_path(&workspace, &reference_path).display()
            );
        }
        if let Some(packed_refs) =
            git_stdout(&workspace, &["rev-parse", "--git-path", "packed-refs"])
        {
            println!(
                "cargo:rerun-if-changed={}",
                resolve_watch_path(&workspace, &packed_refs).display()
            );
        }
    }
    let mut paths = vec![
        PathBuf::from("Cargo.toml"),
        PathBuf::from("Cargo.lock"),
        PathBuf::from("NOTICE"),
    ];
    for name in [
        "pangopup-core",
        "pangopup-index",
        "pangopup-assets",
        "pangopup-build",
    ] {
        let root = PathBuf::from(format!("crates/{name}"));
        paths.push(root.join("Cargo.toml"));
        collect_rs(&workspace.join(&root), &workspace, &mut paths);
    }
    paths.sort();
    let mut hash = Sha256::new();
    for relative in paths {
        let name = relative.to_str().expect("UTF-8 source path").as_bytes();
        let bytes = fs::read(workspace.join(&relative)).expect("read builder source");
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(&bytes);
        println!(
            "cargo:rerun-if-changed={}",
            workspace.join(relative).display()
        );
    }
    println!(
        "cargo:rustc-env=PANGOPUP_BUILDER_SOURCE_SHA256={:x}",
        hash.finalize()
    );
}

fn git_stdout(workspace: &Path, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(workspace)
        .args(arguments)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
}

fn resolve_watch_path(workspace: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        workspace.join(path)
    }
}

fn collect_rs(directory: &Path, workspace: &Path, paths: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .expect("read source directory")
        .map(|entry| entry.expect("source entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rs(&path, workspace, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(
                path.strip_prefix(workspace)
                    .expect("workspace source")
                    .to_owned(),
            );
        }
    }
}
