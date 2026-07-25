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

    // Ticket 012 candidates bind only the causal mask implementation. This is
    // deliberately independent of release, sync, SNV, and reference code.
    let mask_paths = [
        "Cargo.lock",
        "crates/pangopup-core/Cargo.toml",
        "crates/pangopup-core/src/lib.rs",
        "crates/pangopup-index/Cargo.toml",
        "crates/pangopup-index/src/mask_candidates.rs",
        "crates/pangopup-build/Cargo.toml",
        "crates/pangopup-build/build.rs",
        "crates/pangopup-build/src/compatibility.rs",
        "crates/pangopup-build/src/mask.rs",
        "crates/pangopup-build/src/bin/pangopup-mask-candidates.rs",
    ];
    let mut mask_hash = Sha256::new();
    for relative in mask_paths {
        let bytes = fs::read(workspace.join(relative)).expect("read mask builder source");
        mask_hash.update((relative.len() as u64).to_le_bytes());
        mask_hash.update(relative.as_bytes());
        mask_hash.update((bytes.len() as u64).to_le_bytes());
        mask_hash.update(bytes);
        println!(
            "cargo:rerun-if-changed={}",
            workspace.join(relative).display()
        );
    }
    println!(
        "cargo:rustc-env=PANGOPUP_MASK_BUILDER_SOURCE_SHA256={:x}",
        mask_hash.finalize()
    );
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
