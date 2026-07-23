use flate2::{Compression, GzBuilder};
use pangopup_assets::{
    AssetErrorKind, DataPathInputs, LocalStatus, active_bundle, install_transport, local_status,
    pack_bundle, resolve_data_root,
};
use pangopup_build::build_bundle;
use std::{
    fs::{self, File},
    io::Write,
    os::{fd::AsRawFd, unix::fs::PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pangopup-local-install-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated scratch");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = make_writable(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn make_writable(path: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        for entry in fs::read_dir(path)? {
            make_writable(&entry?.path())?;
        }
    } else if metadata.is_file() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn fixture(root: &Path) -> (PathBuf, PathBuf) {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = root.join("source");
    fs::create_dir(&source).expect("source directory");
    for gene in ["ENSG00000000001", "ENSG00000000002"] {
        let input = fs::read(
            repository
                .join("tests/fixtures/full-build-source")
                .join(format!("{gene}.tsv")),
        )
        .expect("source fixture");
        let output = File::create(source.join(format!("{gene}.tsv.gz"))).expect("gzip output");
        let mut encoder = GzBuilder::new().mtime(0).write(output, Compression::best());
        encoder.write_all(&input).expect("gzip source");
        encoder.finish().expect("finish gzip");
    }
    let bundle = root.join("bundle");
    build_bundle(
        &source,
        &repository.join("tests/fixtures/full-build-reference.fa"),
        &bundle,
    )
    .expect("build fixture bundle");
    let transport = root.join("transport");
    pack_bundle(&bundle, &transport).expect("pack fixture transport");
    (bundle, transport)
}

#[test]
fn local_install_is_atomic_reusable_and_lock_safe() {
    let scratch = Scratch::new();
    let (_bundle, transport) = fixture(&scratch.0);
    let data = scratch.0.join("data");
    let data = data.canonicalize().unwrap_or(data);

    assert_eq!(
        local_status(&data).expect("missing status"),
        LocalStatus::Missing {
            data_dir: data.clone()
        }
    );
    let installed = install_transport(&transport, &data).expect("install");
    assert_eq!(installed.status, "installed");
    assert!(installed.path.is_absolute());
    assert_eq!(active_bundle(&data).expect("active").path, installed.path);
    assert!(matches!(
        local_status(&data).expect("ready"),
        LocalStatus::Ready {
            installing: false,
            ..
        }
    ));

    let staging = data.join(".staging");
    let stale_nonce = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let stale = staging.join(stale_nonce);
    fs::create_dir(&stale).expect("stale wrapper");
    fs::set_permissions(&stale, fs::Permissions::from_mode(0o700)).expect("stale mode");
    fs::create_dir(stale.join("payload")).expect("empty stale payload");
    fs::set_permissions(stale.join("payload"), fs::Permissions::from_mode(0o700))
        .expect("payload mode");
    let marker = serde_json::json!({
        "schema": "pangopup.install-stage.v1",
        "nonce": stale_nonce,
        "euid": unsafe { libc::geteuid() },
        "bundle_id": installed.bundle_id.clone(),
        "transport_id": installed.transport_id.clone(),
    });
    fs::write(
        stale.join("marker.json"),
        serde_jcs::to_vec(&marker).expect("canonical marker"),
    )
    .expect("stale marker");
    fs::set_permissions(stale.join("marker.json"), fs::Permissions::from_mode(0o400))
        .expect("marker mode");
    let empty_nonce = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    fs::create_dir(staging.join(empty_nonce)).expect("empty markerless wrapper");
    fs::set_permissions(staging.join(empty_nonce), fs::Permissions::from_mode(0o700))
        .expect("empty wrapper mode");

    let part = transport.join("payload.pgi.zst.part0000");
    fs::set_permissions(&part, fs::Permissions::from_mode(0o000)).expect("deny payload reads");
    let reused = install_transport(&transport, &data).expect("reuse without part open");
    assert_eq!(reused.status, "reused");
    assert_eq!(reused.path, installed.path);
    assert!(!stale.exists(), "valid stale stage was reconciled");
    assert!(
        !staging.join(empty_nonce).exists(),
        "empty markerless wrapper was reconciled"
    );
    fs::set_permissions(&part, fs::Permissions::from_mode(0o600)).expect("restore fixture");

    let lock = File::options()
        .read(true)
        .write(true)
        .open(data.join(".install.lock"))
        .expect("lock file");
    // SAFETY: flock acts on this live descriptor and retains no pointer.
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    assert_eq!(
        install_transport(&transport, &data)
            .expect_err("nonblocking lock")
            .kind(),
        AssetErrorKind::AssetLocked
    );
    assert!(matches!(
        local_status(&data).expect("installing status"),
        LocalStatus::Ready {
            installing: true,
            ..
        }
    ));
    // SAFETY: release the lock before exercising malformed reconciliation.
    assert_eq!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_UN) }, 0);

    let invalid_nonce = "cccccccccccccccccccccccccccccccc";
    let invalid = staging.join(invalid_nonce);
    fs::create_dir(&invalid).expect("invalid stage wrapper");
    fs::set_permissions(&invalid, fs::Permissions::from_mode(0o700)).expect("invalid mode");
    fs::write(invalid.join("marker.json"), b"not-json").expect("invalid marker");
    fs::set_permissions(
        invalid.join("marker.json"),
        fs::Permissions::from_mode(0o400),
    )
    .expect("invalid marker mode");
    assert_eq!(
        install_transport(&transport, &data)
            .expect_err("malformed staging")
            .kind(),
        AssetErrorKind::StagingInvalid
    );
    assert!(invalid.exists(), "malformed stage must remain untouched");
}

#[test]
fn path_and_store_shape_fail_closed() {
    let scratch = Scratch::new();
    assert_eq!(
        resolve_data_root(&DataPathInputs {
            explicit: Some("relative".into()),
            home: Some(scratch.0.clone().into_os_string()),
            ..DataPathInputs::default()
        })
        .expect_err("relative explicit path")
        .kind(),
        AssetErrorKind::PathInvalid
    );

    let root = scratch.0.join("unsafe-root");
    fs::create_dir(&root).expect("root");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o777)).expect("unsafe mode");
    assert_eq!(
        local_status(&root).expect_err("unsafe root").kind(),
        AssetErrorKind::AssetStateInvalid
    );

    let safe = scratch.0.join("safe-root");
    fs::create_dir(&safe).expect("safe root");
    fs::set_permissions(&safe, fs::Permissions::from_mode(0o700)).expect("safe mode");
    std::os::unix::fs::symlink("elsewhere", safe.join("active.json")).expect("active symlink");
    assert_eq!(
        local_status(&safe).expect_err("active symlink").kind(),
        AssetErrorKind::AssetStateInvalid
    );
}
