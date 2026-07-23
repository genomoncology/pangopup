use flate2::{Compression, write::GzEncoder};
use pangopup_assets::{
    ChildPreExecBarrierPhase, LeaseBreakTimeTest, PayloadOperation, PayloadTestFaults,
    ReleasePreparationContract, ReleaseUploadChildBarrier, ReleaseUploadTestBarrier,
    ReleaseUploadTestContract, ReleaseUploadTestHooks, inspect_transport, pack_bundle,
    prepare_release, prepare_release_with_contract, test_reset_input_opens, test_take_input_opens,
    unpack_transport, upload_release_asset_with_contract, verify_transport,
};
use pangopup_build::{build_bundle, verify_bundle};
use pangopup_index::{
    BundleManifest, IndexError, MAX_MANIFEST_BYTES, canonical_manifest_bytes,
    parse_bundle_manifest_bytes,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    mem::MaybeUninit,
    os::fd::{AsRawFd, FromRawFd},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pangopup-transport-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create temporary directory");
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove temporary directory");
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn build_fixture(temp: &Temp) -> PathBuf {
    let source = temp.path().join("source");
    fs::create_dir(&source).expect("source directory");
    for gene in ["ENSG00000000001", "ENSG00000000002"] {
        let bytes =
            fs::read(fixture(&format!("full-build-source/{gene}.tsv"))).expect("source fixture");
        let file = File::create(source.join(format!("{gene}.tsv.gz"))).expect("gzip file");
        let mut gzip = GzEncoder::new(file, Compression::default());
        gzip.write_all(&bytes).expect("gzip input");
        gzip.finish().expect("finish gzip");
    }
    let reference = temp.path().join("reference.fa");
    fs::copy(fixture("full-build-reference.fa"), &reference).expect("reference fixture");
    let bundle = temp.path().join("bundle");
    build_bundle(&source, &reference, &bundle).expect("build fixture bundle");
    bundle
}

fn exact_members(path: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(path)
        .expect("directory")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("UTF-8")
        })
        .collect();
    names.sort();
    names
}

fn copy_directory(source: &Path, destination: &Path) {
    fs::create_dir(destination).expect("copy destination");
    for member in exact_members(source) {
        fs::copy(source.join(&member), destination.join(member)).expect("copy member");
    }
}

fn invocation_stages(parent: &Path, output_name: &str) -> Vec<String> {
    let prefix = format!(".{output_name}.pangopup-stage-");
    exact_members(parent)
        .into_iter()
        .filter(|name| name.starts_with(&prefix))
        .collect()
}

fn encode_pinned(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 9).expect("encoder");
    encoder.include_checksum(true).expect("checksum");
    encoder.include_contentsize(true).expect("content size");
    encoder.include_dictid(false).expect("dictionary ID");
    encoder.long_distance_matching(false).expect("LDM");
    encoder
        .set_pledged_src_size(Some(bytes.len() as u64))
        .expect("pledged size");
    encoder.write_all(bytes).expect("compress");
    encoder.finish().expect("finish")
}

fn hash(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn collect_rs(directory: &Path, workspace: &Path, paths: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .expect("source directory")
        .map(|entry| entry.expect("source entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rs(&path, workspace, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(
                path.strip_prefix(workspace)
                    .expect("relative source")
                    .to_owned(),
            );
        }
    }
}

fn builder_digest(mutation: Option<&Path>) -> String {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
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
        let name = relative.to_str().expect("UTF-8 path").as_bytes();
        let mut bytes = fs::read(workspace.join(&relative)).expect("builder source");
        if mutation.is_some_and(|target| relative == target) {
            bytes.push(0);
        }
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    format!("sha256:{:x}", hash.finalize())
}

fn resign(transport: &Path, compressed: &[u8], scores: Option<&[u8]>) {
    let part = transport.join("payload.pgi.zst.part0000");
    fs::write(&part, compressed).expect("write re-signed payload");
    let manifest_path = transport.join("transport.json");
    let mut outer: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("transport manifest"))
            .expect("transport JSON");
    outer["payload"]["compressed_size"] = Value::from(compressed.len() as u64);
    outer["payload"]["compressed_sha256"] = Value::String(hash(compressed));
    outer["payload"]["parts"][0]["size"] = Value::from(compressed.len() as u64);
    outer["payload"]["parts"][0]["sha256"] = Value::String(hash(compressed));
    if let Some(scores) = scores {
        let inner_path = transport.join("bundle-manifest.json");
        let mut inner: BundleManifest =
            serde_json::from_slice(&fs::read(&inner_path).expect("inner manifest"))
                .expect("inner JSON");
        let member = inner
            .members
            .iter_mut()
            .find(|member| member.path == "scores.pgi")
            .expect("scores member");
        member.size = scores.len() as u64;
        member.sha256 = hash(scores);
        let bytes = canonical_manifest_bytes(&inner).expect("canonical inner manifest");
        fs::write(&inner_path, &bytes).expect("write inner manifest");
        let identity = hash(&bytes);
        outer["bundle"]["bundle_id"] = Value::String(identity.clone());
        outer["bundle"]["manifest"]["size"] = Value::from(bytes.len() as u64);
        outer["bundle"]["manifest"]["sha256"] = Value::String(identity);
        outer["bundle"]["scores"]["size"] = Value::from(scores.len() as u64);
        outer["bundle"]["scores"]["sha256"] = Value::String(hash(scores));
    }
    outer
        .as_object_mut()
        .expect("outer object")
        .remove("transport_id");
    let unsigned = serde_jcs::to_vec(&outer).expect("canonical unsigned transport");
    outer["transport_id"] = Value::String(hash(&unsigned));
    fs::write(
        manifest_path,
        serde_jcs::to_vec(&outer).expect("canonical transport"),
    )
    .expect("write transport manifest");
}

fn rewrite_outer(transport: &Path, mutate: impl FnOnce(&mut Value)) {
    let manifest_path = transport.join("transport.json");
    let mut outer: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("transport manifest"))
            .expect("transport JSON");
    mutate(&mut outer);
    outer
        .as_object_mut()
        .expect("outer object")
        .remove("transport_id");
    let unsigned = serde_jcs::to_vec(&outer).expect("canonical unsigned transport");
    outer["transport_id"] = Value::String(hash(&unsigned));
    fs::write(
        manifest_path,
        serde_jcs::to_vec(&outer).expect("canonical transport"),
    )
    .expect("write transport manifest");
}

fn miniature_release_contract(transport: &Path) -> (Vec<u8>, Vec<u8>) {
    let inspected = inspect_transport(transport).expect("inspect miniature transport");
    let inner: BundleManifest = serde_json::from_slice(&inspected.bundle_manifest_bytes)
        .expect("miniature bundle manifest");
    let notice = inner
        .members
        .iter()
        .find(|member| member.path == "NOTICE")
        .expect("notice member");
    let scores = inner
        .members
        .iter()
        .find(|member| member.path == "scores.pgi")
        .expect("scores member");
    let parts: Vec<_> = inspected
        .parts
        .iter()
        .map(|part| {
            serde_json::json!({
                "ordinal": part.ordinal,
                "path": part.path,
                "size": part.size,
                "sha256": part.sha256,
            })
        })
        .collect();
    let receipt_value = serde_json::json!({
        "schema": "pangopup.proof-receipt.v1",
        "source": {
            "archive_name": inner.source.archive_name,
            "archive_size": inner.source.published_archive_size,
            "archive_md5": inner.source.published_archive_md5,
            "observed_member_count": inner.source.observed_member_count,
            "observed_members_sha256": inner.source.observed_members_sha256,
        },
        "reference": {
            "assembly_accession": inner.reference.assembly_accession,
            "input_size": inner.reference.input_size,
            "input_sha256": inner.reference.input_sha256,
            "sequence_set_sha256": inner.reference.sequence_set_sha256,
        },
        "bundle": {
            "bundle_id": inspected.bundle_id,
            "builder_version": inner.builder.version,
            "builder_source_sha256": inner.builder.source_sha256,
            "manifest": {
                "size": inspected.bundle_manifest_size,
                "sha256": inspected.bundle_manifest_sha256,
            },
            "members": [
                {"path": "NOTICE", "size": notice.size, "sha256": notice.sha256},
                {"path": "scores.pgi", "size": scores.size, "sha256": scores.sha256},
            ],
        },
        "transport": {
            "transport_id": inspected.transport_id,
            "manifest": {
                "size": inspected.transport_bytes.len(),
                "sha256": inspected.transport_sha256,
            },
            "compressed": {
                "size": inspected.compressed_size,
                "sha256": inspected.compressed_sha256,
            },
            "parts": parts,
        },
        "tool": {
            "implementation_commit": "1111111111111111111111111111111111111111",
            "encoder_crate": inspected.compression.encoder_crate,
            "libzstd_version": inspected.compression.libzstd_version,
        },
        "verify": {
            "bundle": ["pangopup-build", "verify", "bundles/miniature"],
            "transport": ["pangopup-build", "transport", "verify", "--transport", "transports/miniature"],
        },
    });
    let mut receipt = serde_jcs::to_vec(&receipt_value).expect("canonical miniature receipt");
    receipt.push(b'\n');
    let receipt_sha256 = hash(&receipt);
    let tag = "miniature-snv-v1";
    let repository = "example/pangopup";
    let prefix = format!("https://github.com/{repository}/releases/download/{tag}/");
    let mut members = vec![
        (
            "transport.json".to_owned(),
            inspected.transport_bytes.len() as u64,
            inspected.transport_sha256,
        ),
        (
            "bundle-manifest.json".to_owned(),
            inspected.bundle_manifest_size,
            inspected.bundle_manifest_sha256,
        ),
        (
            "NOTICE".to_owned(),
            inspected.notice_size,
            inspected.notice_sha256,
        ),
    ];
    members.extend(
        inspected
            .parts
            .into_iter()
            .map(|part| (part.path, part.size, part.sha256)),
    );
    let profile_members: Vec<_> = members
        .into_iter()
        .map(|(name, size, sha256)| {
            serde_json::json!({
                "logical_path": name,
                "asset_name": name,
                "size": size,
                "sha256": sha256,
                "url": format!("{prefix}{name}"),
            })
        })
        .collect();
    let profile_value = serde_json::json!({
        "schema": "pangopup.release-profile.v1",
        "profile": tag,
        "repository": repository,
        "release": {
            "tag": tag,
            "title": "Miniature Pangopup SNV scores",
            "target_commit": "2222222222222222222222222222222222222222",
            "page_url": format!("https://github.com/{repository}/releases/tag/{tag}"),
        },
        "source": {
            "title": inner.source.title,
            "creators": inner.source.creators,
            "doi": inner.source.doi,
            "license": "CC-BY-4.0",
            "archive": {
                "name": inner.source.archive_name,
                "size": inner.source.published_archive_size,
                "md5": inner.source.published_archive_md5,
            },
            "assembly": "GRCh38",
            "masked": true,
            "window": 50,
        },
        "reference_compatibility": {
            "assembly": inner.reference.assembly,
            "assembly_accession": inner.reference.assembly_accession,
            "input_size": inner.reference.input_size,
            "input_sha256": inner.reference.input_sha256,
            "sequence_set_sha256": inner.reference.sequence_set_sha256,
            "ordinary_ref_mismatches": 0,
            "preserved_ref_n_loci": 0,
        },
        "bundle": {
            "schema": inner.schema,
            "index_format": inner.index_format,
            "bundle_id": inspected.bundle_id,
        },
        "transport": {
            "schema": "pangopup.snv-transport.v1",
            "transport_id": inspected.transport_id,
            "members": profile_members,
        },
        "proof": {
            "schema": "pangopup.proof-receipt.v1",
            "asset_name": "proof-receipt.json",
            "size": receipt.len(),
            "sha256": receipt_sha256,
        },
    });
    let profile = serde_jcs::to_vec(&profile_value).expect("canonical miniature profile");
    (receipt, profile)
}

struct FakeGh {
    path: PathBuf,
    control: PathBuf,
    response: PathBuf,
    argv: PathBuf,
    environment: PathBuf,
    input: PathBuf,
    count: PathBuf,
    pids: PathBuf,
    signal_state: PathBuf,
    size: u64,
    sha256: String,
}

fn c_literal(path: &Path) -> String {
    path.to_str()
        .expect("UTF-8 fake-child path")
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn compile_fake_gh(temp: &Temp, label: &str) -> FakeGh {
    let root = temp.path().join(format!("fake-gh-{label}"));
    fs::create_dir(&root).expect("fake-gh capture directory");
    let source = root.join("fake-gh.c");
    let path = root.join("gh");
    let control = root.join("control");
    let response = root.join("response");
    let argv = root.join("argv");
    let environment = root.join("environment");
    let input = root.join("stdin");
    let count = root.join("count");
    let pids = root.join("pids");
    let signal_state = root.join("signal-state");
    let code = format!(
        r#"#include <dirent.h>
#include <limits.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <unistd.h>
extern char **environ;

static void dump_lines(const char *path, char **values) {{
    FILE *file = fopen(path, "wb");
    if (!file) exit(91);
    for (size_t index = 0; values[index]; index++) {{
        fputs(values[index], file);
        fputc('\n', file);
    }}
    if (fclose(file)) exit(92);
}}

int main(int argc, char **argv) {{
    (void)argc;
    sigset_t mask;
    if (sigprocmask(SIG_SETMASK, NULL, &mask)) return 89;
    int leaked_signalfd = 0;
    DIR *fds = opendir("/proc/self/fd");
    if (!fds) return 88;
    struct dirent *entry;
    while ((entry = readdir(fds))) {{
        if (entry->d_name[0] == '.') continue;
        char fd_path[PATH_MAX];
        char target[PATH_MAX];
        snprintf(fd_path, sizeof(fd_path), "/proc/self/fd/%s", entry->d_name);
        ssize_t length = readlink(fd_path, target, sizeof(target) - 1);
        if (length > 0) {{
            target[length] = '\0';
            if (strstr(target, "signalfd")) leaked_signalfd = 1;
        }}
    }}
    closedir(fds);
    FILE *signal_state = fopen("{signal_state}", "wb");
    if (!signal_state) return 87;
    fprintf(signal_state, "%d %d %d %d\n",
            sigismember(&mask, SIGINT), sigismember(&mask, SIGTERM),
            sigismember(&mask, SIGIO), leaked_signalfd);
    fclose(signal_state);
    FILE *calls = fopen("{count}", "ab");
    if (!calls) return 93;
    fputc('x', calls);
    fclose(calls);
    dump_lines("{argv}", argv);
    dump_lines("{environment}", environ);

    FILE *captured = fopen("{input}", "wb");
    if (!captured) return 94;
    unsigned char buffer[8192];
    size_t got;
    while ((got = fread(buffer, 1, sizeof(buffer), stdin)) != 0) {{
        if (fwrite(buffer, 1, got, captured) != got) return 95;
    }}
    fclose(captured);

    char mode[64] = {{0}};
    FILE *control = fopen("{control}", "rb");
    if (control) {{
        size_t mode_len = fread(mode, 1, sizeof(mode) - 1, control);
        mode[mode_len] = '\0';
        fclose(control);
    }}
    if (!strcmp(mode, "nonzero")) {{
        fputs("sensitive fake failure detail", stderr);
        return 17;
    }}
    if (!strcmp(mode, "stdout-overflow")) {{
        for (size_t index = 0; index < 70000; index++) fputc('x', stdout);
        return 0;
    }}
    if (!strcmp(mode, "stderr-overflow")) {{
        for (size_t index = 0; index < 70000; index++) fputc('y', stderr);
        return 0;
    }}
    if (!strcmp(mode, "silent")) {{
        sleep(60);
        return 0;
    }}
    if (!strcmp(mode, "group-hang")) {{
        pid_t grandchild = fork();
        if (grandchild < 0) return 98;
        if (grandchild == 0) for (;;) pause();
        FILE *pids = fopen("{pids}", "wb");
        if (!pids) return 99;
        fprintf(pids, "%ld %ld\n", (long)getpid(), (long)grandchild);
        fclose(pids);
        return 0;
    }}

    FILE *response = fopen("{response}", "rb");
    if (!response) return 96;
    while ((got = fread(buffer, 1, sizeof(buffer), response)) != 0) {{
        if (fwrite(buffer, 1, got, stdout) != got) return 97;
    }}
    fclose(response);
    return 0;
}}
"#,
        count = c_literal(&count),
        argv = c_literal(&argv),
        environment = c_literal(&environment),
        input = c_literal(&input),
        control = c_literal(&control),
        response = c_literal(&response),
        pids = c_literal(&pids),
        signal_state = c_literal(&signal_state),
    );
    fs::write(&source, code).expect("write fake-gh source");
    let status = Command::new("cc")
        .args(["-D_GNU_SOURCE", "-std=c11", "-O2", "-o"])
        .arg(&path)
        .arg(&source)
        .status()
        .expect("compile fake gh");
    assert!(status.success(), "compile fake gh child");
    let bytes = fs::read(&path).expect("fake gh bytes");
    FakeGh {
        path,
        control,
        response,
        argv,
        environment,
        input,
        count,
        pids,
        signal_state,
        size: bytes.len() as u64,
        sha256: hash(&bytes),
    }
}

fn prepare_upload_fixture(temp: &Temp) -> (PathBuf, PathBuf, Vec<u8>, Vec<u8>) {
    let bundle = build_fixture(temp);
    let transport = temp.path().join("upload.transport");
    pack_bundle(&bundle, &transport).expect("pack upload transport");
    let (receipt, profile) = miniature_release_contract(&transport);
    let receipt_path = temp.path().join("upload-receipt.json");
    fs::write(&receipt_path, &receipt).expect("write upload receipt");
    let prepared = temp.path().join("upload.prepared");
    let receipt_sha256 = hash(&receipt);
    prepare_release_with_contract(
        &transport,
        &receipt_path,
        &prepared,
        ReleasePreparationContract {
            receipt_bytes: &receipt,
            receipt_sha256: &receipt_sha256,
            profile_bytes: &profile,
        },
    )
    .expect("prepare upload fixture");
    (transport, prepared, receipt, profile)
}

fn call_fake_upload(
    transport: &Path,
    prepared: &Path,
    fake: &FakeGh,
    receipt: &[u8],
    profile: &[u8],
    asset: &str,
    hooks: ReleaseUploadTestHooks<'_>,
) -> Result<pangopup_assets::UploadAssetOutcome, pangopup_assets::AssetError> {
    call_fake_upload_at(
        transport, prepared, fake, &fake.path, receipt, profile, asset, hooks,
    )
}

#[allow(clippy::too_many_arguments)]
fn call_fake_upload_at(
    transport: &Path,
    prepared: &Path,
    fake: &FakeGh,
    gh_path: &Path,
    receipt: &[u8],
    profile: &[u8],
    asset: &str,
    hooks: ReleaseUploadTestHooks<'_>,
) -> Result<pangopup_assets::UploadAssetOutcome, pangopup_assets::AssetError> {
    upload_release_asset_with_contract(
        transport,
        prepared,
        gh_path,
        12345,
        asset,
        ReleaseUploadTestContract {
            receipt_bytes: receipt,
            receipt_sha256: &hash(receipt),
            profile_bytes: profile,
            gh_size: fake.size,
            gh_sha256: &fake.sha256,
        },
        hooks,
    )
}

fn pipe_pair() -> (File, File) {
    let mut fds = [-1; 2];
    assert_eq!(
        unsafe { libc::pipe(fds.as_mut_ptr()) },
        0,
        "create test pipe"
    );
    unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) }
}

fn upload_helper_command(
    transport: &Path,
    prepared: &Path,
    fake: &FakeGh,
    asset: &str,
    result: &Path,
) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("integration test executable"));
    command
        .args(["--exact", "release_upload_subprocess_helper", "--nocapture"])
        .env("PANGOPUP_UPLOAD_HELPER", "1")
        .env("PANGOPUP_UPLOAD_TRANSPORT", transport)
        .env("PANGOPUP_UPLOAD_PREPARED", prepared)
        .env("PANGOPUP_UPLOAD_GH", &fake.path)
        .env("PANGOPUP_UPLOAD_GH_SIZE", fake.size.to_string())
        .env("PANGOPUP_UPLOAD_GH_SHA256", &fake.sha256)
        .env("PANGOPUP_UPLOAD_ASSET", asset)
        .env("PANGOPUP_UPLOAD_RESULT", result)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

#[test]
fn release_upload_subprocess_helper() {
    if std::env::var_os("PANGOPUP_UPLOAD_HELPER").is_none() {
        return;
    }
    let transport =
        PathBuf::from(std::env::var_os("PANGOPUP_UPLOAD_TRANSPORT").expect("helper transport"));
    let prepared =
        PathBuf::from(std::env::var_os("PANGOPUP_UPLOAD_PREPARED").expect("helper prepared"));
    let gh = PathBuf::from(std::env::var_os("PANGOPUP_UPLOAD_GH").expect("helper gh"));
    let asset = std::env::var("PANGOPUP_UPLOAD_ASSET").expect("helper asset");
    let result_path =
        PathBuf::from(std::env::var_os("PANGOPUP_UPLOAD_RESULT").expect("helper result"));
    let receipt = fs::read(prepared.join("proof-receipt.json")).expect("helper receipt");
    let profile = fs::read(prepared.join("release-profile.json")).expect("helper profile");
    let receipt_sha256 = hash(&receipt);
    let gh_size = std::env::var("PANGOPUP_UPLOAD_GH_SIZE")
        .expect("helper gh size")
        .parse::<u64>()
        .expect("decimal helper gh size");
    let gh_sha256 = std::env::var("PANGOPUP_UPLOAD_GH_SHA256").expect("helper gh digest");
    let supervision_barrier = match (
        std::env::var("PANGOPUP_UPLOAD_SUPERVISION_READY_FD").ok(),
        std::env::var("PANGOPUP_UPLOAD_SUPERVISION_RELEASE_FD").ok(),
    ) {
        (Some(ready), Some(release)) => Some(ReleaseUploadTestBarrier {
            ready_fd: ready.parse().expect("helper supervision ready fd"),
            release_fd: release.parse().expect("helper supervision release fd"),
        }),
        (None, None) => None,
        _ => panic!("incomplete helper supervision barrier"),
    };
    let child_pre_exec_barrier = match (
        std::env::var("PANGOPUP_UPLOAD_CHILD_BARRIER_PHASE").ok(),
        std::env::var("PANGOPUP_UPLOAD_CHILD_READY_FD").ok(),
        std::env::var("PANGOPUP_UPLOAD_CHILD_RELEASE_FD").ok(),
    ) {
        (Some(phase), Some(ready), Some(release)) => Some(ReleaseUploadChildBarrier {
            phase: match phase.as_str() {
                "before" => ChildPreExecBarrierPhase::BeforeParentDeathSignal,
                "after" => ChildPreExecBarrierPhase::AfterParentDeathSignal,
                _ => panic!("unknown helper child barrier phase"),
            },
            ready_fd: ready.parse().expect("helper child ready fd"),
            release_fd: release.parse().expect("helper child release fd"),
        }),
        (None, None, None) => None,
        _ => panic!("incomplete helper child barrier"),
    };
    let before_mask = supervised_signal_mask();
    let upload = upload_release_asset_with_contract(
        &transport,
        &prepared,
        &gh,
        12345,
        &asset,
        ReleaseUploadTestContract {
            receipt_bytes: &receipt,
            receipt_sha256: &receipt_sha256,
            profile_bytes: &profile,
            gh_size,
            gh_sha256: &gh_sha256,
        },
        ReleaseUploadTestHooks {
            child_deadline: Some(Duration::from_secs(10)),
            supervision_barrier,
            child_pre_exec_barrier,
            ..ReleaseUploadTestHooks::default()
        },
    );
    let after_mask = supervised_signal_mask();
    let summary = format!(
        "before={before_mask:?}\nafter={after_mask:?}\nresult={}\n",
        upload
            .as_ref()
            .map(|_| "uploaded".to_owned())
            .unwrap_or_else(|error| error.to_string())
    );
    fs::write(result_path, summary).expect("write helper result");
    std::process::exit(if upload.is_ok() { 0 } else { 23 });
}

#[test]
fn release_preparation_is_deterministic_atomic_and_never_opens_a_part() {
    use std::os::unix::fs::PermissionsExt;

    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("release.transport");
    pack_bundle(&bundle, &transport).expect("pack miniature release transport");
    let (receipt, profile) = miniature_release_contract(&transport);
    let receipt_path = temp.path().join("proof-receipt.json");
    fs::write(&receipt_path, &receipt).expect("miniature receipt");
    let receipt_sha256 = hash(&receipt);
    let contract = ReleasePreparationContract {
        receipt_bytes: &receipt,
        receipt_sha256: &receipt_sha256,
        profile_bytes: &profile,
    };
    test_reset_input_opens();
    let first = temp.path().join("prepared-a");
    let outcome = prepare_release_with_contract(&transport, &receipt_path, &first, contract)
        .expect("prepare miniature release");
    assert_eq!(outcome.status, "prepared");
    assert_eq!(outcome.asset_count, 7);
    assert_eq!(
        fs::metadata(&first)
            .expect("prepared output metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let opened = test_take_input_opens();
    assert_eq!(opened.len(), 4);
    assert!(opened.iter().all(|path| {
        [
            "proof-receipt.json",
            "transport.json",
            "bundle-manifest.json",
            "NOTICE",
        ]
        .iter()
        .any(|name| path.ends_with(name))
    }));
    assert!(
        opened
            .iter()
            .all(|path| !path.contains("payload.pgi.zst.part"))
    );
    assert_eq!(
        exact_members(&first),
        [
            "SHA256SUMS",
            "proof-receipt.json",
            "release-notes.md",
            "release-profile.json"
        ]
    );
    assert_eq!(
        fs::read(first.join("proof-receipt.json")).expect("proof copy"),
        receipt
    );
    assert_eq!(
        fs::read(first.join("release-profile.json")).expect("profile"),
        profile
    );
    let sums = fs::read_to_string(first.join("SHA256SUMS")).expect("SHA list");
    assert!(sums.ends_with('\n'));
    assert_eq!(sums.lines().count(), 6);
    assert_eq!(
        sums.lines()
            .map(|line| line.split_once("  ").expect("digest separator").1)
            .collect::<Vec<_>>(),
        [
            "transport.json",
            "bundle-manifest.json",
            "NOTICE",
            "payload.pgi.zst.part0000",
            "proof-receipt.json",
            "release-profile.json",
        ]
    );
    let notes = fs::read_to_string(first.join("release-notes.md")).expect("release notes");
    for required in [
        "Nils Wagner",
        "Aleksandr Neverov",
        "10.5281/zenodo.15649338",
        "CC BY 4.0",
        "does not name an exact FASTA/patch release or GENCODE release",
        "RefSeq GRCh38.p14",
        "per-gene TSV rows",
        "model weights",
        "remote sync",
        "pangopup assets install --transport \"$transport_dir\"",
        "downloads exactly the 4 transport members",
    ] {
        assert!(notes.contains(required), "release notes omit {required}");
    }
    assert_eq!(notes.matches("curl --fail --location --output").count(), 4);
    for member in [
        "transport.json",
        "bundle-manifest.json",
        "NOTICE",
        "payload.pgi.zst.part0000",
    ] {
        assert!(notes.contains(&format!("$transport_dir/{member}")));
        assert!(notes.contains(&format!(
            "https://github.com/example/pangopup/releases/download/miniature-snv-v1/{member}"
        )));
    }

    let second = temp.path().join("prepared-b");
    prepare_release_with_contract(&transport, &receipt_path, &second, contract)
        .expect("repeat miniature release preparation");
    for member in exact_members(&first) {
        assert_eq!(
            fs::read(first.join(&member)).expect("first output"),
            fs::read(second.join(&member)).expect("second output")
        );
    }
    assert_eq!(
        prepare_release_with_contract(&transport, &receipt_path, &first, contract)
            .expect_err("output conflict")
            .kind(),
        pangopup_assets::AssetErrorKind::OutputConflict
    );
    assert!(invocation_stages(temp.path(), "prepared-a").is_empty());
}

#[test]
fn release_upload_uses_exact_held_child_request_environment_and_stdin() {
    let temp = Temp::new();
    let (transport, prepared, receipt, profile) = prepare_upload_fixture(&temp);
    let fake = compile_fake_gh(&temp, "request");
    let selected = fs::read(transport.join("transport.json")).expect("selected bytes");
    let selected_digest = hash(&selected);
    fs::write(
        &fake.response,
        format!(
            r#"{{"name":"transport.json","size":{},"state":"uploaded","digest":"{}"}}"#,
            selected.len(),
            selected_digest
        ),
    )
    .expect("fake response");

    let outcome = call_fake_upload(
        &transport,
        &prepared,
        &fake,
        &receipt,
        &profile,
        "transport.json",
        ReleaseUploadTestHooks::default(),
    )
    .expect("fake upload succeeds");
    assert_eq!(outcome.status, "uploaded");
    assert_eq!(outcome.asset, "transport.json");
    assert_eq!(outcome.size, selected.len() as u64);
    assert_eq!(outcome.digest.as_deref(), Some(selected_digest.as_str()));
    assert_eq!(fs::read(&fake.input).expect("captured stdin"), selected);
    assert_eq!(fs::read(&fake.count).expect("call count"), b"x");
    assert_eq!(
        fs::read_to_string(&fake.signal_state).expect("child signal state"),
        "0 0 0 0\n",
        "sealed child restores the original mask and inherits no signalfd"
    );

    let argv = fs::read_to_string(&fake.argv)
        .expect("captured argv")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(
        argv,
        [
            "gh",
            "api",
            "https://uploads.github.com/repos/genomoncology/pangopup/releases/12345/assets?name=transport.json",
            "--method",
            "POST",
            "--header",
            "Accept:application/vnd.github+json",
            "--header",
            "X-GitHub-Api-Version:2022-11-28",
            "--header",
            "Content-Type:application/octet-stream",
            "--header",
            &format!("Content-Length:{}", selected.len()),
            "--input",
            "-",
            "--jq",
            r#"{"name":.name,"size":.size,"state":.state,"digest":.digest}"#,
        ]
    );
    let temp_path = temp.path().to_str().expect("UTF-8 temporary path");
    assert!(argv.iter().all(|value| !value.contains(temp_path)));

    let environment = fs::read_to_string(&fake.environment).expect("captured environment");
    let allowed = [
        "HOME",
        "XDG_CONFIG_HOME",
        "GH_CONFIG_DIR",
        "GH_TOKEN",
        "GITHUB_TOKEN",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "HTTPS_PROXY",
        "NO_PROXY",
        "LANG",
        "LC_ALL",
        "GH_PROMPT_DISABLED",
        "GH_PAGER",
        "PAGER",
        "NO_COLOR",
    ];
    let names = environment
        .lines()
        .map(|line| line.split_once('=').expect("environment assignment").0)
        .collect::<Vec<_>>();
    assert!(names.iter().all(|name| allowed.contains(name)));
    for forced in [
        "GH_PROMPT_DISABLED=1",
        "GH_PAGER=cat",
        "PAGER=cat",
        "NO_COLOR=1",
    ] {
        assert!(environment.lines().any(|line| line == forced));
    }
    assert!(!names.contains(&"PATH"));
}

#[test]
fn release_upload_holds_validated_executable_and_selected_assets_across_swaps() {
    use std::os::unix::fs::symlink;

    let gh_temp = Temp::new();
    let (transport, prepared, receipt, profile) = prepare_upload_fixture(&gh_temp);
    let fake = compile_fake_gh(&gh_temp, "held-executable");
    let selected = fs::read(transport.join("transport.json")).expect("selected bytes");
    fs::write(
        &fake.response,
        format!(
            r#"{{"name":"transport.json","size":{},"state":"uploaded","digest":null}}"#,
            selected.len()
        ),
    )
    .expect("fake response");
    let held_gh = gh_temp.path().join("held-original-gh");
    let swap_gh = || {
        fs::rename(&fake.path, &held_gh).expect("move validated gh");
        symlink("/bin/false", &fake.path).expect("replace gh path");
    };
    let outcome = call_fake_upload(
        &transport,
        &prepared,
        &fake,
        &receipt,
        &profile,
        "transport.json",
        ReleaseUploadTestHooks {
            after_gh_validation: Some(&swap_gh),
            ..ReleaseUploadTestHooks::default()
        },
    )
    .expect("held executable runs after pathname swap");
    assert_eq!(outcome.digest, None);
    assert_eq!(fs::read(&fake.count).expect("held gh call count"), b"x");

    for asset in [
        "transport.json",
        "release-profile.json",
        "payload.pgi.zst.part0000",
    ] {
        let temp = Temp::new();
        let (transport, prepared, receipt, profile) = prepare_upload_fixture(&temp);
        let fake = compile_fake_gh(&temp, asset);
        let source = if asset == "release-profile.json" {
            &prepared
        } else {
            &transport
        };
        let selected_path = source.join(asset);
        let original = fs::read(&selected_path).expect("original selected bytes");
        fs::write(
            &fake.response,
            format!(
                r#"{{"name":"{asset}","size":{},"state":"uploaded","digest":"{}"}}"#,
                original.len(),
                hash(&original)
            ),
        )
        .expect("fake response");
        let held_asset = temp.path().join(format!("held-{asset}"));
        let wrong_target = temp.path().join(format!("wrong-{asset}"));
        fs::write(&wrong_target, vec![b'w'; original.len()]).expect("wrong target");
        let after_open = || {
            fs::rename(&selected_path, &held_asset).expect("move selected asset");
            symlink(&wrong_target, &selected_path).expect("swap selected asset to symlink");
        };
        let after_validation = || {
            fs::remove_file(&selected_path).expect("remove swapped symlink");
            fs::write(&selected_path, vec![b'z'; original.len()])
                .expect("replace selected after validation");
        };
        call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            asset,
            ReleaseUploadTestHooks {
                after_asset_open: Some(&after_open),
                after_contract_validation: Some(&after_validation),
                ..ReleaseUploadTestHooks::default()
            },
        )
        .expect("held selected asset survives pathname swaps");
        assert_eq!(
            fs::read(&fake.input).expect("captured held stdin"),
            original
        );
        assert_eq!(fs::read(&fake.count).expect("one child"), b"x");
    }
}

#[test]
fn release_upload_sealed_snapshots_survive_same_inode_overwrite_and_truncate() {
    let gh_temp = Temp::new();
    let (transport, prepared, receipt, profile) = prepare_upload_fixture(&gh_temp);
    let fake = compile_fake_gh(&gh_temp, "sealed-gh-same-inode");
    let selected = fs::read(transport.join("transport.json")).expect("selected bytes");
    fs::write(
        &fake.response,
        format!(
            r#"{{"name":"transport.json","size":{},"state":"uploaded","digest":null}}"#,
            selected.len()
        ),
    )
    .expect("fake response");
    let overwrite_gh = || {
        fs::write(&fake.path, vec![0_u8; fake.size as usize])
            .expect("overwrite gh source inode after snapshot");
    };
    call_fake_upload(
        &transport,
        &prepared,
        &fake,
        &receipt,
        &profile,
        "transport.json",
        ReleaseUploadTestHooks {
            after_gh_validation: Some(&overwrite_gh),
            ..ReleaseUploadTestHooks::default()
        },
    )
    .expect("sealed gh snapshot survives source overwrite");
    assert_eq!(fs::read(&fake.count).expect("sealed gh call"), b"x");

    for (index, asset) in [
        "transport.json",
        "bundle-manifest.json",
        "NOTICE",
        "proof-receipt.json",
        "release-profile.json",
        "SHA256SUMS",
    ]
    .into_iter()
    .enumerate()
    {
        let temp = Temp::new();
        let (transport, prepared, receipt, profile) = prepare_upload_fixture(&temp);
        let fake = compile_fake_gh(&temp, &format!("sealed-small-{index}"));
        let source = if matches!(
            asset,
            "proof-receipt.json" | "release-profile.json" | "SHA256SUMS"
        ) {
            &prepared
        } else {
            &transport
        };
        let selected_path = source.join(asset);
        let original = fs::read(&selected_path).expect("small selected bytes");
        fs::write(
            &fake.response,
            format!(
                r#"{{"name":"{asset}","size":{},"state":"uploaded","digest":"{}"}}"#,
                original.len(),
                hash(&original)
            ),
        )
        .expect("fake response");
        let mutate = || {
            if index % 2 == 0 {
                fs::write(&selected_path, vec![b'm'; original.len()])
                    .expect("overwrite selected source inode");
            } else {
                File::options()
                    .write(true)
                    .open(&selected_path)
                    .expect("open selected source inode")
                    .set_len(0)
                    .expect("truncate selected source inode");
            }
        };
        call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            asset,
            ReleaseUploadTestHooks {
                after_asset_open: Some(&mutate),
                ..ReleaseUploadTestHooks::default()
            },
        )
        .expect("sealed small snapshot survives source mutation");
        assert_eq!(fs::read(&fake.input).expect("sealed stdin"), original);
    }
}

#[test]
fn release_upload_payload_lease_blocks_same_inode_mutation_and_proves_zero_preread() {
    for truncate in [false, true] {
        let temp = Temp::new();
        let (transport, prepared, receipt, profile) = prepare_upload_fixture(&temp);
        let fake = compile_fake_gh(
            &temp,
            if truncate {
                "lease-truncate"
            } else {
                "lease-write"
            },
        );
        let asset = "payload.pgi.zst.part0000";
        let selected_path = transport.join(asset);
        let original = fs::read(&selected_path).expect("payload fixture bytes");
        fs::write(
            &fake.response,
            format!(
                r#"{{"name":"{asset}","size":{},"state":"uploaded","digest":null}}"#,
                original.len()
            ),
        )
        .expect("fake response");
        fs::write(&fake.control, "silent").expect("silent fake child");

        let (started_send, started_receive) = mpsc::channel();
        let (done_send, done_receive) = mpsc::channel();
        let writer = Arc::new(Mutex::new(None));
        let writer_slot = Arc::clone(&writer);
        let before_spawn = |operations: &[PayloadOperation], offset: i64| {
            assert_eq!(offset, 0);
            assert_eq!(
                operations,
                [
                    PayloadOperation::BlockUploadSignals,
                    PayloadOperation::CreateSignalFd,
                    PayloadOperation::ReadLeaseBreakTime,
                    PayloadOperation::OpenNoFollow,
                    PayloadOperation::AcquireReadLease,
                    PayloadOperation::SetOwnerThread,
                    PayloadOperation::GetOwnerThread,
                    PayloadOperation::QueryLeaseAfterOwner,
                    PayloadOperation::Fstat,
                    PayloadOperation::DuplicateChildStdin,
                    PayloadOperation::QueryOffsetBeforeSpawn,
                    PayloadOperation::DrainSignalsBeforeSpawn,
                ]
            );
            let path = selected_path.clone();
            let started = started_send.clone();
            let done = done_send.clone();
            let handle = thread::spawn(move || {
                started.send(()).expect("writer started signal");
                if truncate {
                    File::options()
                        .write(true)
                        .truncate(true)
                        .open(path)
                        .expect("lease-blocked truncate");
                } else {
                    let mut file = File::options()
                        .write(true)
                        .open(path)
                        .expect("lease-blocked overwrite");
                    file.write_all(b"X").expect("overwrite after lease release");
                }
                done.send(()).expect("writer completion signal");
            });
            *writer_slot.lock().expect("writer slot") = Some(handle);
            started_receive
                .recv_timeout(Duration::from_secs(1))
                .expect("writer reached open attempt");
            thread::sleep(Duration::from_millis(20));
            assert!(
                done_receive.try_recv().is_err(),
                "writer must remain lease-blocked"
            );
        };

        let error = call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            asset,
            ReleaseUploadTestHooks {
                before_child_spawn: Some(&before_spawn),
                child_deadline: Some(Duration::from_secs(2)),
                ..ReleaseUploadTestHooks::default()
            },
        )
        .expect_err("lease break must cancel upload");
        assert!(error.to_string().contains("lease break"));
        done_receive
            .recv_timeout(Duration::from_secs(1))
            .expect("writer unblocks after child reap and lease release");
        writer
            .lock()
            .expect("writer slot")
            .take()
            .expect("writer handle")
            .join()
            .expect("writer thread");
        assert!(
            !fake.count.exists() || fs::read(&fake.count).expect("bounded child count") == b"x",
            "at most one child may reach the fake executable before cancellation"
        );
    }

    let temp = Temp::new();
    let (transport, prepared, receipt, profile) = prepare_upload_fixture(&temp);
    let fake = compile_fake_gh(&temp, "preexisting-writer");
    let writer = File::options()
        .write(true)
        .open(transport.join("payload.pgi.zst.part0000"))
        .expect("preexisting writer");
    assert!(
        call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            "payload.pgi.zst.part0000",
            ReleaseUploadTestHooks::default(),
        )
        .is_err()
    );
    drop(writer);
    assert!(
        !fake.count.exists(),
        "lease acquisition failure starts no child"
    );
}

#[test]
fn release_upload_payload_source_boundary_has_no_content_access_escape() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let caller = fs::read_to_string(workspace.join("crates/pangopup-assets/src/release.rs"))
        .expect("release source");
    let boundary =
        fs::read_to_string(workspace.join("crates/pangopup-assets/src/release_upload_linux.rs"))
            .expect("payload boundary source");
    for forbidden in [
        "payload.as_raw_fd",
        "impl Read for LeasedPayload",
        "impl Seek for LeasedPayload",
        "impl AsRawFd for LeasedPayload",
        "libc::pread",
        "libc::pread64",
        "libc::readv",
        "libc::mmap",
        "libc::sendfile",
        "libc::splice",
        "libc::copy_file_range",
        "io_uring",
    ] {
        assert!(!caller.contains(forbidden), "caller contains {forbidden}");
        assert!(
            !boundary.contains(forbidden),
            "boundary contains {forbidden}"
        );
    }
    assert_eq!(boundary.matches("libc::read(").count(), 1);
    assert!(boundary.contains("fn drain_signal_fd"));
    assert!(boundary.contains("no content-access operation"));
}

#[test]
fn release_upload_payload_injected_lease_failures_all_fail_closed() {
    let temp = Temp::new();
    let (transport, prepared, receipt, profile) = prepare_upload_fixture(&temp);
    let fake = compile_fake_gh(&temp, "lease-injected-failures");
    let asset = "payload.pgi.zst.part0000";
    let selected = fs::read(transport.join(asset)).expect("payload fixture");
    fs::write(
        &fake.response,
        format!(
            r#"{{"name":"{asset}","size":{},"state":"uploaded","digest":null}}"#,
            selected.len()
        ),
    )
    .expect("fake response");

    for faults in [
        PayloadTestFaults {
            set_owner_error: true,
            ..PayloadTestFaults::default()
        },
        PayloadTestFaults {
            get_owner_error: true,
            ..PayloadTestFaults::default()
        },
        PayloadTestFaults {
            owner_mismatch: true,
            ..PayloadTestFaults::default()
        },
        PayloadTestFaults {
            post_owner_query_error: true,
            ..PayloadTestFaults::default()
        },
        PayloadTestFaults {
            post_owner_lease_lost: true,
            ..PayloadTestFaults::default()
        },
        PayloadTestFaults {
            lease_break_time: LeaseBreakTimeTest::Unavailable,
            ..PayloadTestFaults::default()
        },
        PayloadTestFaults {
            lease_break_time: LeaseBreakTimeTest::Malformed,
            ..PayloadTestFaults::default()
        },
        PayloadTestFaults {
            lease_break_time: LeaseBreakTimeTest::Seconds(9),
            ..PayloadTestFaults::default()
        },
    ] {
        assert!(
            call_fake_upload(
                &transport,
                &prepared,
                &fake,
                &receipt,
                &profile,
                asset,
                ReleaseUploadTestHooks {
                    payload_faults: faults,
                    ..ReleaseUploadTestHooks::default()
                },
            )
            .is_err()
        );
        assert!(
            !fake.count.exists(),
            "pre-spawn injected failure started child"
        );
    }

    let final_loss = call_fake_upload(
        &transport,
        &prepared,
        &fake,
        &receipt,
        &profile,
        asset,
        ReleaseUploadTestHooks {
            payload_faults: PayloadTestFaults {
                final_lease_lost: true,
                ..PayloadTestFaults::default()
            },
            ..ReleaseUploadTestHooks::default()
        },
    )
    .expect_err("silent final lease loss rejected");
    assert!(final_loss.to_string().contains("not stable"));
    assert_eq!(fs::read(&fake.count).expect("final-loss child"), b"x");

    fs::write(&fake.control, "silent").expect("silent child mode");
    let cleanup = call_fake_upload(
        &transport,
        &prepared,
        &fake,
        &receipt,
        &profile,
        asset,
        ReleaseUploadTestHooks {
            child_deadline: Some(Duration::from_millis(50)),
            payload_faults: PayloadTestFaults {
                cleanup_deadline_exhausted: true,
                ..PayloadTestFaults::default()
            },
            ..ReleaseUploadTestHooks::default()
        },
    )
    .expect_err("injected cleanup deadline rejected");
    assert!(
        cleanup
            .to_string()
            .contains("cleanup exceeded five seconds")
    );
    assert_eq!(fs::read(&fake.count).expect("two children"), b"xx");
}

fn process_is_absent_or_zombie(pid: i32) -> bool {
    let stat = match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat,
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                || matches!(error.raw_os_error(), Some(libc::ENOENT) | Some(libc::ESRCH)) =>
        {
            return true;
        }
        Err(error) => panic!("read child process state: {error}"),
    };
    stat.rsplit_once(") ")
        .and_then(|(_, suffix)| suffix.chars().next())
        .is_some_and(|state| state == 'Z')
}

fn supervised_signal_mask() -> [bool; 3] {
    let mut mask = MaybeUninit::<libc::sigset_t>::uninit();
    let result =
        unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, std::ptr::null(), mask.as_mut_ptr()) };
    assert_eq!(result, 0, "inspect test-thread signal mask");
    let mask = unsafe { mask.assume_init() };
    [libc::SIGINT, libc::SIGTERM, libc::SIGIO].map(|signal| {
        let member = unsafe { libc::sigismember(&mask, signal) };
        assert!(member == 0 || member == 1);
        member == 1
    })
}

fn wait_for_path(path: &Path) {
    let started = Instant::now();
    while !path.exists() && started.elapsed() < Duration::from_secs(3) {
        thread::sleep(Duration::from_millis(2));
    }
    assert!(path.exists(), "timed out waiting for {}", path.display());
}

#[test]
fn release_upload_drains_pending_interrupt_before_spawn_and_restores_mask() {
    for signal in [libc::SIGINT, libc::SIGTERM] {
        let temp = Temp::new();
        let (transport, prepared, receipt, profile) = prepare_upload_fixture(&temp);
        let fake = compile_fake_gh(&temp, "pending-interrupt");
        let original_mask = supervised_signal_mask();
        let raise_interrupt = || {
            assert_eq!(unsafe { libc::raise(signal) }, 0);
        };
        let error = call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            "transport.json",
            ReleaseUploadTestHooks {
                after_contract_validation: Some(&raise_interrupt),
                ..ReleaseUploadTestHooks::default()
            },
        )
        .expect_err("pending interrupt must prevent child spawn");
        assert!(error.to_string().contains("upload interrupted"));
        assert_eq!(supervised_signal_mask(), original_mask);
        assert!(!fake.count.exists(), "pending interrupt started child");
    }
}

#[test]
fn release_upload_orderly_interrupt_kills_group_reaps_child_and_releases_writer() {
    for signal in [libc::SIGINT, libc::SIGTERM] {
        let temp = Temp::new();
        let (transport, prepared, _receipt, _profile) = prepare_upload_fixture(&temp);
        let fake = compile_fake_gh(&temp, "orderly-interrupt");
        let asset = "payload.pgi.zst.part0000";
        fs::write(&fake.control, "group-hang").expect("group-hang fake mode");
        let result = temp.path().join(format!("interrupt-result-{signal}"));
        let (mut ready_read, ready_write) = pipe_pair();
        let (release_read, mut release_write) = pipe_pair();
        let mut command = upload_helper_command(&transport, &prepared, &fake, asset, &result);
        command
            .env(
                "PANGOPUP_UPLOAD_SUPERVISION_READY_FD",
                ready_write.as_raw_fd().to_string(),
            )
            .env(
                "PANGOPUP_UPLOAD_SUPERVISION_RELEASE_FD",
                release_read.as_raw_fd().to_string(),
            );
        let mut coordinator = command.spawn().expect("spawn upload coordinator helper");
        let mut ready = [0_u8; std::mem::size_of::<libc::pid_t>()];
        ready_read
            .read_exact(&mut ready)
            .expect("coordinator reached supervision barrier");
        let supervisor_tid = libc::pid_t::from_ne_bytes(ready);
        wait_for_path(&fake.pids);

        let selected = transport.join(asset);
        let (writer_started_send, writer_started_receive) = mpsc::channel();
        let (writer_done_send, writer_done_receive) = mpsc::channel();
        let writer = thread::spawn(move || {
            writer_started_send.send(()).expect("writer started");
            let mut file = File::options()
                .write(true)
                .open(selected)
                .expect("writer opens after lease release");
            file.write_all(b"X").expect("writer mutates after release");
            writer_done_send.send(()).expect("writer done");
        });
        writer_started_receive
            .recv_timeout(Duration::from_secs(1))
            .expect("writer reached lease-blocked open");
        thread::sleep(Duration::from_millis(20));
        assert!(
            writer_done_receive.try_recv().is_err(),
            "writer must remain blocked while upload is supervised"
        );

        assert_eq!(
            unsafe {
                libc::syscall(
                    libc::SYS_tgkill,
                    coordinator.id() as libc::pid_t,
                    supervisor_tid,
                    signal,
                )
            },
            0,
            "send orderly coordinator interrupt"
        );
        release_write
            .write_all(&[1])
            .expect("release coordinator supervision barrier");
        let status = coordinator
            .wait()
            .expect("wait for interrupted coordinator");
        assert!(
            !status.success(),
            "interrupted coordinator returned success"
        );
        assert_eq!(status.code(), Some(23));
        writer_done_receive
            .recv_timeout(Duration::from_secs(2))
            .expect("writer released after interrupt cleanup");
        writer.join().expect("writer thread");

        let helper_result = fs::read_to_string(&result).expect("interrupted helper result");
        assert!(
            helper_result.contains("GitHub CLI upload interrupted"),
            "unexpected helper result: {helper_result}"
        );
        let mut result_lines = helper_result.lines();
        let before_mask = result_lines
            .next()
            .and_then(|line| line.strip_prefix("before="))
            .expect("before mask result");
        let after_mask = result_lines
            .next()
            .and_then(|line| line.strip_prefix("after="))
            .expect("after mask result");
        assert_eq!(before_mask, after_mask, "signal mask restored");
        let pids = fs::read_to_string(&fake.pids).expect("fake process IDs");
        let pids = pids
            .split_whitespace()
            .map(|value| value.parse::<i32>().expect("decimal PID"))
            .collect::<Vec<_>>();
        assert_eq!(pids.len(), 2);
        let wait_started = Instant::now();
        while !process_is_absent_or_zombie(pids[1])
            && wait_started.elapsed() < Duration::from_secs(2)
        {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            fs::metadata(format!("/proc/{}", pids[0])).is_err(),
            "direct upload child must be reaped"
        );
        assert!(
            process_is_absent_or_zombie(pids[1]),
            "descriptor-holding descendant must be group-killed"
        );
    }
}

#[test]
fn release_upload_abrupt_parent_death_closes_both_pdeathsig_race_windows() {
    for (phase_name, phase) in [
        ("before", ChildPreExecBarrierPhase::BeforeParentDeathSignal),
        ("after", ChildPreExecBarrierPhase::AfterParentDeathSignal),
    ] {
        let temp = Temp::new();
        let (transport, prepared, _receipt, _profile) = prepare_upload_fixture(&temp);
        let fake = compile_fake_gh(&temp, "abrupt-parent-death");
        let result = temp.path().join(format!("abrupt-result-{phase_name}"));
        let (mut ready_read, ready_write) = pipe_pair();
        let (release_read, mut release_write) = pipe_pair();
        let mut command =
            upload_helper_command(&transport, &prepared, &fake, "transport.json", &result);
        command
            .env("PANGOPUP_UPLOAD_CHILD_BARRIER_PHASE", phase_name)
            .env(
                "PANGOPUP_UPLOAD_CHILD_READY_FD",
                ready_write.as_raw_fd().to_string(),
            )
            .env(
                "PANGOPUP_UPLOAD_CHILD_RELEASE_FD",
                release_read.as_raw_fd().to_string(),
            );
        let mut coordinator = command.spawn().expect("spawn abrupt-death coordinator");
        let mut ready = [0_u8; std::mem::size_of::<libc::pid_t>()];
        ready_read
            .read_exact(&mut ready)
            .expect("child reached requested pre-exec barrier");
        let upload_child = libc::pid_t::from_ne_bytes(ready);
        assert_eq!(
            unsafe { libc::kill(coordinator.id() as libc::pid_t, libc::SIGKILL) },
            0,
            "kill coordinator abruptly"
        );
        let status = coordinator.wait().expect("reap abrupt-death coordinator");
        assert!(!status.success());
        if phase == ChildPreExecBarrierPhase::BeforeParentDeathSignal {
            release_write
                .write_all(&[1])
                .expect("release pre-protection child after parent death");
        }
        let wait_started = Instant::now();
        while !process_is_absent_or_zombie(upload_child)
            && wait_started.elapsed() < Duration::from_secs(2)
        {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            process_is_absent_or_zombie(upload_child),
            "direct child survived abrupt parent death in {phase_name} window"
        );
        assert!(
            !fake.count.exists(),
            "fake gh executed after abrupt parent death"
        );
        assert!(
            !result.exists(),
            "killed coordinator reported an upload result"
        );
    }
}

#[test]
fn release_upload_deadline_kills_process_group_and_reaps_direct_child() {
    let temp = Temp::new();
    let (transport, prepared, receipt, profile) = prepare_upload_fixture(&temp);
    let fake = compile_fake_gh(&temp, "deadline-process-group");
    let selected = fs::read(transport.join("transport.json")).expect("selected bytes");
    fs::write(
        &fake.response,
        format!(
            r#"{{"name":"transport.json","size":{},"state":"uploaded","digest":null}}"#,
            selected.len()
        ),
    )
    .expect("unused fake response");
    fs::write(&fake.control, "group-hang").expect("group-hang mode");
    let error = call_fake_upload(
        &transport,
        &prepared,
        &fake,
        &receipt,
        &profile,
        "transport.json",
        ReleaseUploadTestHooks {
            child_deadline: Some(Duration::from_millis(100)),
            ..ReleaseUploadTestHooks::default()
        },
    )
    .expect_err("silent process group must hit deadline");
    assert!(error.to_string().contains("deadline exceeded"));
    let pids = fs::read_to_string(&fake.pids).expect("fake process IDs");
    let pids = pids
        .split_whitespace()
        .map(|value| value.parse::<i32>().expect("decimal PID"))
        .collect::<Vec<_>>();
    assert_eq!(pids.len(), 2);
    let wait_started = Instant::now();
    while !process_is_absent_or_zombie(pids[1]) && wait_started.elapsed() < Duration::from_secs(2) {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        fs::metadata(format!("/proc/{}", pids[0])).is_err(),
        "direct child must be reaped"
    );
    assert!(
        process_is_absent_or_zombie(pids[1]),
        "grandchild must be killed by process-group SIGKILL"
    );
}

#[test]
fn release_upload_rejects_unsafe_paths_unreviewed_inputs_and_bad_children() {
    use std::os::unix::fs::symlink;

    let temp = Temp::new();
    let (transport, prepared, receipt, profile) = prepare_upload_fixture(&temp);
    let fake = compile_fake_gh(&temp, "failures");
    let selected = fs::read(transport.join("transport.json")).expect("selected bytes");
    let good_response = format!(
        r#"{{"name":"transport.json","size":{},"state":"uploaded","digest":null}}"#,
        selected.len()
    );
    fs::write(&fake.response, &good_response).expect("good fake response");

    let real_gh = temp.path().join("real-gh");
    fs::rename(&fake.path, &real_gh).expect("move fake gh");
    symlink(&real_gh, &fake.path).expect("symlink fake gh");
    assert_eq!(
        call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            "transport.json",
            ReleaseUploadTestHooks::default(),
        )
        .expect_err("symlinked gh rejected")
        .kind(),
        pangopup_assets::AssetErrorKind::ReleaseUpload
    );
    fs::remove_file(&fake.path).expect("remove gh symlink");
    fs::rename(&real_gh, &fake.path).expect("restore fake gh");

    let gh_component = temp.path().join("symlinked-gh-component");
    symlink(fake.path.parent().expect("fake gh parent"), &gh_component)
        .expect("symlink gh component");
    assert!(
        call_fake_upload_at(
            &transport,
            &prepared,
            &fake,
            &gh_component.join("gh"),
            &receipt,
            &profile,
            "transport.json",
            ReleaseUploadTestHooks::default(),
        )
        .is_err()
    );

    let real_transport = temp.path().join("real-upload.transport");
    fs::rename(&transport, &real_transport).expect("move transport root");
    symlink(&real_transport, &transport).expect("symlink transport root");
    assert!(
        call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            "transport.json",
            ReleaseUploadTestHooks::default(),
        )
        .is_err()
    );
    fs::remove_file(&transport).expect("remove transport symlink");
    fs::rename(&real_transport, &transport).expect("restore transport root");

    let selected_path = transport.join("transport.json");
    let selected_original = fs::read(&selected_path).expect("selected original");
    let selected_real = temp.path().join("real-transport.json");
    fs::rename(&selected_path, &selected_real).expect("move selected member");
    symlink(&selected_real, &selected_path).expect("symlink selected member");
    assert!(
        call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            "transport.json",
            ReleaseUploadTestHooks::default(),
        )
        .is_err()
    );
    fs::remove_file(&selected_path).expect("remove selected symlink");
    fs::rename(&selected_real, &selected_path).expect("restore selected member");

    fs::rename(&selected_path, &selected_real).expect("move selected for directory case");
    fs::create_dir(&selected_path).expect("nonregular selected member");
    assert!(
        call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            "transport.json",
            ReleaseUploadTestHooks::default(),
        )
        .is_err()
    );
    fs::remove_dir(&selected_path).expect("remove nonregular selected member");
    fs::rename(&selected_real, &selected_path).expect("restore selected after directory case");

    File::options()
        .write(true)
        .open(&selected_path)
        .expect("open selected for truncation")
        .set_len(selected_original.len() as u64 - 1)
        .expect("truncate selected member");
    assert!(
        call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            "transport.json",
            ReleaseUploadTestHooks::default(),
        )
        .is_err()
    );
    fs::write(&selected_path, &selected_original).expect("restore selected bytes");

    assert!(
        call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            "not-reviewed.bin",
            ReleaseUploadTestHooks::default(),
        )
        .is_err()
    );
    assert!(!fake.count.exists(), "rejections must not start a child");

    fs::write(prepared.join("unexpected"), b"unexpected").expect("extra prepared member");
    assert!(
        call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            "transport.json",
            ReleaseUploadTestHooks::default(),
        )
        .is_err()
    );
    assert!(
        !fake.count.exists(),
        "invalid closed set must not start child"
    );
    fs::remove_file(prepared.join("unexpected")).expect("remove extra member");

    let notes_path = prepared.join("release-notes.md");
    let notes = fs::read(&notes_path).expect("prepared notes");
    let mut malformed_notes = notes.clone();
    malformed_notes[0] ^= 1;
    fs::write(&notes_path, malformed_notes).expect("malformed prepared notes");
    assert!(
        call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            "transport.json",
            ReleaseUploadTestHooks::default(),
        )
        .is_err()
    );
    fs::write(&notes_path, notes).expect("restore prepared notes");
    assert!(
        !fake.count.exists(),
        "malformed prepared bytes must not start child"
    );

    fs::write(&fake.control, "nonzero").expect("nonzero mode");
    let error = call_fake_upload(
        &transport,
        &prepared,
        &fake,
        &receipt,
        &profile,
        "transport.json",
        ReleaseUploadTestHooks::default(),
    )
    .expect_err("nonzero child rejected");
    assert!(!error.to_string().contains("sensitive fake failure detail"));
    fs::write(&fake.control, "stdout-overflow").expect("overflow mode");
    assert!(
        call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            "transport.json",
            ReleaseUploadTestHooks::default(),
        )
        .expect_err("overflow rejected")
        .to_string()
        .contains("exceeded 64 KiB")
    );
    fs::write(&fake.control, "stderr-overflow").expect("stderr overflow mode");
    assert!(
        call_fake_upload(
            &transport,
            &prepared,
            &fake,
            &receipt,
            &profile,
            "transport.json",
            ReleaseUploadTestHooks::default(),
        )
        .expect_err("stderr overflow rejected")
        .to_string()
        .contains("exceeded 64 KiB")
    );

    fs::write(&fake.control, "").expect("normal mode");
    let wrong_digest = format!(
        r#"{{"name":"transport.json","size":{},"state":"uploaded","digest":"sha256:{}"}}"#,
        selected.len(),
        "0".repeat(64)
    );
    for response in [
        r#"{"name":"wrong","size":1,"state":"uploaded","digest":null}"#.to_owned(),
        wrong_digest,
        r#"{"name":"transport.json","name":"transport.json","size":1,"state":"uploaded","digest":null}"#.to_owned(),
        r#"{"name":"transport.json","size":1,"state":"uploaded","digest":null,"extra":true}"#.to_owned(),
        r#"{"name":"transport.json","size":1,"state":"uploaded","digest":null} {}"#.to_owned(),
    ] {
        fs::write(&fake.response, response).expect("bad response");
        assert!(
            call_fake_upload(
                &transport,
                &prepared,
                &fake,
                &receipt,
                &profile,
                "transport.json",
                ReleaseUploadTestHooks::default(),
            )
            .is_err(),
            "bad child response must fail"
        );
    }
    assert_eq!(
        fs::read(&fake.count)
            .expect("one child per attempted request")
            .len(),
        8
    );
}

#[test]
fn public_release_contract_rejects_miniature_and_metadata_mismatch() {
    use std::os::unix::fs::symlink;

    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("release.transport");
    pack_bundle(&bundle, &transport).expect("pack miniature release transport");
    let (mut receipt, profile) = miniature_release_contract(&transport);
    let receipt_path = temp.path().join("proof-receipt.json");
    fs::write(&receipt_path, &receipt).expect("miniature receipt");
    assert_eq!(
        prepare_release(
            &transport,
            &receipt_path,
            &temp.path().join("public-rejected")
        )
        .expect_err("public contract must reject miniature")
        .kind(),
        pangopup_assets::AssetErrorKind::ReleaseInvalid
    );

    receipt[100] ^= 1;
    fs::write(&receipt_path, &receipt).expect("mutated receipt");
    let receipt_sha256 = hash(&receipt);
    let contract = ReleasePreparationContract {
        receipt_bytes: &receipt,
        receipt_sha256: &receipt_sha256,
        profile_bytes: &profile,
    };
    assert_eq!(
        prepare_release_with_contract(
            &transport,
            &receipt_path,
            &temp.path().join("malformed-rejected"),
            contract,
        )
        .expect_err("malformed receipt")
        .kind(),
        pangopup_assets::AssetErrorKind::ReleaseInvalid
    );
    assert!(invocation_stages(temp.path(), "malformed-rejected").is_empty());

    let (valid_receipt, valid_profile) = miniature_release_contract(&transport);
    let mut value: Value = serde_json::from_slice(&valid_receipt).expect("valid receipt value");
    value["transport"]["compressed"]["sha256"] =
        Value::String(format!("sha256:{}", "0".repeat(64)));
    let mut mismatched = serde_jcs::to_vec(&value).expect("canonical mismatched receipt");
    mismatched.push(b'\n');
    fs::write(&receipt_path, &mismatched).expect("mismatched receipt");
    let mismatched_hash = hash(&mismatched);
    let mismatched_contract = ReleasePreparationContract {
        receipt_bytes: &mismatched,
        receipt_sha256: &mismatched_hash,
        profile_bytes: &valid_profile,
    };
    assert_eq!(
        prepare_release_with_contract(
            &transport,
            &receipt_path,
            &temp.path().join("metadata-rejected"),
            mismatched_contract,
        )
        .expect_err("receipt metadata mismatch")
        .kind(),
        pangopup_assets::AssetErrorKind::ReleaseInvalid
    );
    assert!(invocation_stages(temp.path(), "metadata-rejected").is_empty());

    let symlinked_receipt = temp.path().join("symlinked-receipt.json");
    symlink(&receipt_path, &symlinked_receipt).expect("symlink receipt");
    assert_eq!(
        prepare_release(
            &transport,
            &symlinked_receipt,
            &temp.path().join("symlink-rejected"),
        )
        .expect_err("symlinked receipt")
        .kind(),
        pangopup_assets::AssetErrorKind::ReleaseInvalid
    );
}

#[test]
fn bounded_transport_inspection_rejects_part_shape_and_size_without_opening_it() {
    use std::os::unix::fs::symlink;

    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let baseline = temp.path().join("inspection.transport");
    pack_bundle(&bundle, &baseline).expect("pack inspection fixture");
    let part_name = exact_members(&baseline)
        .into_iter()
        .find(|name| name.starts_with("payload.pgi.zst.part"))
        .expect("payload part");
    let expected_size = fs::metadata(baseline.join(&part_name))
        .expect("part metadata")
        .len();

    let wrong_size = temp.path().join("wrong-size.transport");
    copy_directory(&baseline, &wrong_size);
    File::options()
        .write(true)
        .open(wrong_size.join(&part_name))
        .expect("open part for fixture mutation")
        .set_len(expected_size - 1)
        .expect("truncate fixture part");
    test_reset_input_opens();
    assert_eq!(
        inspect_transport(&wrong_size)
            .expect_err("wrong part size")
            .kind(),
        pangopup_assets::AssetErrorKind::PartSetInvalid
    );
    assert!(
        test_take_input_opens()
            .iter()
            .all(|path| !path.contains("payload.pgi.zst.part"))
    );

    let symlinked = temp.path().join("symlinked-part.transport");
    copy_directory(&baseline, &symlinked);
    fs::remove_file(symlinked.join(&part_name)).expect("remove copied part");
    symlink(baseline.join(&part_name), symlinked.join(&part_name)).expect("symlink part");
    assert_eq!(
        inspect_transport(&symlinked)
            .expect_err("symlinked part")
            .kind(),
        pangopup_assets::AssetErrorKind::PartSetInvalid
    );
}

#[test]
fn deterministic_pack_verify_unpack_and_conflict() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let first = temp.path().join("first.transport");
    let second = temp.path().join("second.transport");
    let packed = pack_bundle(&bundle, &first).expect("pack");
    let repeated = pack_bundle(&bundle, &second).expect("repeat pack");
    assert_eq!(packed.transport_id, repeated.transport_id);
    assert_eq!(exact_members(&first), exact_members(&second));
    for member in exact_members(&first) {
        assert_eq!(
            fs::read(first.join(&member)).expect("first member"),
            fs::read(second.join(&member)).expect("second member")
        );
    }
    let verified = verify_transport(&first).expect("verify transport");
    assert_eq!(verified.transport_id, packed.transport_id);
    let unpacked = temp.path().join("unpacked");
    let outcome = unpack_transport(&first, &unpacked).expect("unpack");
    assert_eq!(outcome.bundle_id, packed.bundle_id);
    verify_bundle(&unpacked).expect("shared bundle verification");
    for member in ["NOTICE", "manifest.json", "scores.pgi"] {
        assert_eq!(
            fs::read(bundle.join(member)).expect("bundle member"),
            fs::read(unpacked.join(member)).expect("unpacked member")
        );
    }
    assert_eq!(
        unpack_transport(&first, &unpacked)
            .expect_err("destination conflict")
            .kind()
            .code(),
        "OUTPUT_CONFLICT"
    );
}

#[test]
fn builder_identity_covers_assets_manifest_notice_and_certification_source() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let manifest: BundleManifest =
        serde_json::from_slice(&fs::read(bundle.join("manifest.json")).expect("manifest"))
            .expect("manifest JSON");
    let actual = builder_digest(None);
    assert_eq!(manifest.builder.source_sha256, actual);
    assert_ne!(
        actual,
        builder_digest(Some(Path::new("crates/pangopup-assets/src/lib.rs")))
    );
    assert_ne!(actual, builder_digest(Some(Path::new("NOTICE"))));
}

#[test]
fn shared_inner_manifest_parser_is_bounded_duplicate_aware_and_canonical() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let bytes = fs::read(bundle.join("manifest.json")).expect("manifest");
    parse_bundle_manifest_bytes(&bytes).expect("canonical manifest");

    let text = String::from_utf8(bytes.clone()).expect("UTF-8 manifest");
    let duplicate = text.replacen(
        "\"path\":\"NOTICE\"",
        "\"path\":\"NOTICE\",\"path\":\"NOTICE\"",
        1,
    );
    assert!(matches!(
        parse_bundle_manifest_bytes(duplicate.as_bytes()),
        Err(IndexError::Corrupt("manifest JSON"))
    ));

    let oversized = vec![b' '; MAX_MANIFEST_BYTES as usize + 1];
    assert!(matches!(
        parse_bundle_manifest_bytes(&oversized),
        Err(IndexError::Corrupt("manifest size"))
    ));

    let mut noncanonical = bytes;
    noncanonical.push(b'\n');
    assert!(matches!(
        parse_bundle_manifest_bytes(&noncanonical),
        Err(IndexError::Corrupt("manifest is not canonical"))
    ));
}

#[test]
fn corrupt_part_and_member_set_fail_closed_without_publication() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("transport");
    pack_bundle(&bundle, &transport).expect("pack");
    let part = exact_members(&transport)
        .into_iter()
        .find(|name| name.starts_with("payload."))
        .expect("part");
    let path = transport.join(part);
    let mut bytes = fs::read(&path).expect("part bytes");
    bytes[0] ^= 1;
    fs::write(&path, bytes).expect("mutate part");
    assert_eq!(
        verify_transport(&transport)
            .expect_err("part hash")
            .kind()
            .code(),
        "TRANSPORT_HASH_MISMATCH"
    );
    let output = temp.path().join("must-not-exist");
    assert!(unpack_transport(&transport, &output).is_err());
    assert!(!output.exists());
    assert!(invocation_stages(temp.path(), "must-not-exist").is_empty());

    fs::write(transport.join("extra"), b"x").expect("extra member");
    assert_eq!(
        verify_transport(&transport)
            .expect_err("exact set")
            .kind()
            .code(),
        "PART_SET_INVALID"
    );
}

#[test]
fn independent_transport_layers_fail_at_their_declared_boundaries() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let baseline = temp.path().join("baseline");
    pack_bundle(&bundle, &baseline).expect("pack baseline");

    let copied_manifest = temp.path().join("copied-manifest");
    copy_directory(&baseline, &copied_manifest);
    let path = copied_manifest.join("bundle-manifest.json");
    let mut bytes = fs::read(&path).expect("bundle manifest");
    bytes[10] ^= 1;
    fs::write(path, bytes).expect("corrupt copied manifest");
    assert_eq!(
        verify_transport(&copied_manifest)
            .expect_err("copied manifest identity")
            .kind()
            .code(),
        "TRANSPORT_HASH_MISMATCH"
    );

    let notice = temp.path().join("notice");
    copy_directory(&baseline, &notice);
    let path = notice.join("NOTICE");
    let mut bytes = fs::read(&path).expect("notice");
    bytes[0] ^= 1;
    fs::write(path, bytes).expect("corrupt notice");
    assert_eq!(
        verify_transport(&notice)
            .expect_err("notice identity")
            .kind()
            .code(),
        "TRANSPORT_HASH_MISMATCH"
    );

    for (label, mutation) in [
        ("missing", "missing"),
        ("renamed", "renamed"),
        ("sized", "sized"),
    ] {
        let case = temp.path().join(label);
        copy_directory(&baseline, &case);
        let part = case.join("payload.pgi.zst.part0000");
        match mutation {
            "missing" => fs::remove_file(part).expect("remove part"),
            "renamed" => {
                fs::rename(part, case.join("payload.pgi.zst.part0001")).expect("rename part")
            }
            "sized" => {
                let file = fs::OpenOptions::new()
                    .write(true)
                    .open(part)
                    .expect("open part");
                file.set_len(1).expect("truncate part");
            }
            _ => unreachable!(),
        }
        assert_eq!(
            verify_transport(&case)
                .expect_err("invalid part set")
                .kind()
                .code(),
            "PART_SET_INVALID"
        );
    }

    let whole_hash = temp.path().join("whole-hash");
    copy_directory(&baseline, &whole_hash);
    rewrite_outer(&whole_hash, |outer| {
        outer["payload"]["compressed_sha256"] = Value::String(format!("sha256:{}", "0".repeat(64)));
    });
    assert_eq!(
        verify_transport(&whole_hash)
            .expect_err("whole stream identity")
            .kind()
            .code(),
        "TRANSPORT_HASH_MISMATCH"
    );

    for (label, truncate) in [("checksum", false), ("truncated", true)] {
        let case = temp.path().join(label);
        copy_directory(&baseline, &case);
        let part = case.join("payload.pgi.zst.part0000");
        let mut compressed = fs::read(part).expect("compressed payload");
        if truncate {
            compressed.pop();
        } else {
            let last = compressed.last_mut().expect("checksum byte");
            *last ^= 1;
        }
        resign(&case, &compressed, None);
        assert_eq!(
            verify_transport(&case)
                .expect_err("invalid compressed stream")
                .kind()
                .code(),
            "COMPRESSION_INVALID"
        );
    }

    let decoded_hash = temp.path().join("decoded-hash");
    copy_directory(&baseline, &decoded_hash);
    let part = decoded_hash.join("payload.pgi.zst.part0000");
    let mut scores =
        zstd::stream::decode_all(fs::read(part).expect("compressed payload").as_slice())
            .expect("decode payload");
    let last = scores.last_mut().expect("score byte");
    *last ^= 1;
    let compressed = encode_pinned(&scores);
    resign(&decoded_hash, &compressed, None);
    assert_eq!(
        verify_transport(&decoded_hash)
            .expect_err("decoded score hash")
            .kind()
            .code(),
        "TRANSPORT_HASH_MISMATCH"
    );
}

#[test]
fn self_consistent_semantic_corruption_passes_integrity_but_not_unpack() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("transport");
    pack_bundle(&bundle, &transport).expect("pack");
    let part = transport.join("payload.pgi.zst.part0000");
    let mut scores =
        zstd::stream::decode_all(fs::read(&part).expect("compressed payload").as_slice())
            .expect("decode");
    scores[312] |= 0x80;
    let compressed = encode_pinned(&scores);
    resign(&transport, &compressed, Some(&scores));
    verify_transport(&transport).expect("integrity-only verification");
    let output = temp.path().join("invalid-bundle");
    assert_eq!(
        unpack_transport(&transport, &output)
            .expect_err("semantic certification")
            .kind()
            .code(),
        "BUNDLE_INVALID"
    );
    assert!(!output.exists());
    assert!(invocation_stages(temp.path(), "invalid-bundle").is_empty());
}

#[test]
fn hash_consistent_trailing_and_second_frames_are_compression_errors() {
    for second_frame in [false, true] {
        let temp = Temp::new();
        let bundle = build_fixture(&temp);
        let transport = temp.path().join("transport");
        pack_bundle(&bundle, &transport).expect("pack");
        let part = transport.join("payload.pgi.zst.part0000");
        let mut compressed = fs::read(&part).expect("payload");
        if second_frame {
            compressed.extend_from_within(..);
        } else {
            compressed.push(0);
        }
        resign(&transport, &compressed, None);
        assert_eq!(
            verify_transport(&transport)
                .expect_err("compression structure")
                .kind()
                .code(),
            "COMPRESSION_INVALID"
        );
    }

    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("invalid-magic");
    pack_bundle(&bundle, &transport).expect("pack");
    let part = transport.join("payload.pgi.zst.part0000");
    let mut compressed = fs::read(part).expect("payload");
    compressed[0] ^= 1;
    resign(&transport, &compressed, None);
    assert_eq!(
        verify_transport(&transport)
            .expect_err("hash-consistent invalid magic")
            .kind()
            .code(),
        "COMPRESSION_INVALID"
    );
}

#[test]
fn concurrent_unpack_has_one_atomic_winner() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("transport");
    pack_bundle(&bundle, &transport).expect("pack");
    let output = temp.path().join("race-output");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let mut threads = Vec::new();
    for _ in 0..2 {
        let transport = transport.clone();
        let output = output.clone();
        let barrier = barrier.clone();
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            unpack_transport(&transport, &output)
        }));
    }
    barrier.wait();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().expect("thread"))
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .next()
            .expect("loser")
            .kind()
            .code(),
        "OUTPUT_CONFLICT"
    );
    verify_bundle(&output).expect("winner is complete");
}

#[test]
fn oversized_manifest_is_rejected_before_allocation() {
    let temp = Temp::new();
    let transport = temp.path().join("oversized");
    fs::create_dir(&transport).expect("transport directory");
    let file = File::create(transport.join("transport.json")).expect("manifest");
    file.set_len(1024 * 1024 + 1).expect("oversized manifest");
    assert_eq!(
        verify_transport(&transport)
            .expect_err("manifest cap")
            .kind()
            .code(),
        "MANIFEST_INVALID"
    );
}

#[test]
fn bundle_certification_caps_members_before_hashing_or_mapping() {
    let temp = Temp::new();
    let original = build_fixture(&temp);

    let oversized_notice = temp.path().join("oversized-notice");
    copy_directory(&original, &oversized_notice);
    File::create(oversized_notice.join("NOTICE"))
        .expect("notice")
        .set_len(64 * 1024 + 1)
        .expect("sparse oversized notice");
    assert_eq!(
        verify_bundle(&oversized_notice)
            .expect_err("notice cap")
            .code,
        "BUNDLE_NOTICE"
    );

    let oversized_scores = temp.path().join("oversized-scores");
    copy_directory(&original, &oversized_scores);
    File::create(oversized_scores.join("scores.pgi"))
        .expect("scores")
        .set_len(17_179_869_184 + 1)
        .expect("sparse oversized scores");
    assert_eq!(
        verify_bundle(&oversized_scores)
            .expect_err("score cap")
            .code,
        "BUNDLE_INDEX"
    );

    let extra = temp.path().join("extra-member");
    copy_directory(&original, &extra);
    fs::write(extra.join("fourth"), b"").expect("fourth member");
    assert_eq!(
        verify_bundle(&extra).expect_err("bounded exact set").code,
        "BUNDLE_INVALID"
    );
}

#[test]
fn typed_asset_error_mapping_covers_io_and_future_versions() {
    let temp = Temp::new();
    let binary = env!("CARGO_BIN_EXE_pangopup-build");
    let missing_path = temp.path().join("missing");
    assert_eq!(
        verify_bundle(&missing_path)
            .expect_err("legacy missing bundle")
            .code,
        "BUNDLE_INVALID"
    );
    assert_eq!(
        verify_transport(&missing_path)
            .expect_err("missing transport")
            .kind()
            .code(),
        "INPUT_IO"
    );
    let missing_cli = Command::new(binary)
        .args(["transport", "verify", "--transport"])
        .arg(&missing_path)
        .output()
        .expect("missing CLI");
    assert_eq!(missing_cli.status.code(), Some(1));
    assert!(
        String::from_utf8(missing_cli.stderr)
            .expect("UTF-8")
            .starts_with("{\"status\":\"error\",\"code\":\"INPUT_IO\"")
    );

    let bundle = build_fixture(&temp);
    let blocked_parent = temp.path().join("blocked-parent");
    fs::write(&blocked_parent, b"not a directory").expect("blocking file");
    assert_eq!(
        pack_bundle(&bundle, &blocked_parent.join("transport"))
            .expect_err("output parent")
            .kind()
            .code(),
        "OUTPUT_IO"
    );
    let output_cli = Command::new(binary)
        .args(["transport", "pack", "--bundle"])
        .arg(&bundle)
        .arg("--output")
        .arg(blocked_parent.join("transport-cli"))
        .output()
        .expect("output CLI");
    assert_eq!(output_cli.status.code(), Some(1));
    assert!(
        String::from_utf8(output_cli.stderr)
            .expect("UTF-8")
            .starts_with("{\"status\":\"error\",\"code\":\"OUTPUT_IO\"")
    );

    let transport = temp.path().join("transport");
    pack_bundle(&bundle, &transport).expect("pack");
    rewrite_outer(&transport, |outer| {
        outer["schema"] = Value::String("pangopup.snv-transport.v2".to_owned());
        outer["future"] = Value::Bool(true);
    });
    assert_eq!(
        verify_transport(&transport)
            .expect_err("future transport")
            .kind()
            .code(),
        "TRANSPORT_INCOMPATIBLE"
    );
    let future_cli = Command::new(binary)
        .args(["transport", "verify", "--transport"])
        .arg(&transport)
        .output()
        .expect("future CLI");
    assert_eq!(future_cli.status.code(), Some(1));
    assert!(
        String::from_utf8(future_cli.stderr)
            .expect("UTF-8")
            .starts_with("{\"status\":\"error\",\"code\":\"TRANSPORT_INCOMPATIBLE\"")
    );
}

#[cfg(unix)]
#[test]
fn read_only_inputs_work_and_symlinked_parts_are_rejected() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    for member in ["NOTICE", "manifest.json", "scores.pgi"] {
        fs::set_permissions(bundle.join(member), fs::Permissions::from_mode(0o444))
            .expect("read-only bundle member");
    }
    fs::set_permissions(&bundle, fs::Permissions::from_mode(0o555))
        .expect("read-only bundle directory");
    let transport = temp.path().join("transport");
    pack_bundle(&bundle, &transport).expect("pack read-only bundle");
    for member in exact_members(&transport) {
        fs::set_permissions(transport.join(member), fs::Permissions::from_mode(0o444))
            .expect("read-only transport member");
    }
    fs::set_permissions(&transport, fs::Permissions::from_mode(0o555))
        .expect("read-only transport directory");
    verify_transport(&transport).expect("verify read-only transport");
    unpack_transport(&transport, &temp.path().join("unpacked")).expect("unpack read-only");

    fs::set_permissions(&transport, fs::Permissions::from_mode(0o755))
        .expect("restore transport directory");
    let part_name = exact_members(&transport)
        .into_iter()
        .find(|name| name.starts_with("payload."))
        .expect("part name");
    let part = transport.join(&part_name);
    fs::set_permissions(&part, fs::Permissions::from_mode(0o644)).expect("restore part");
    let replacement = temp.path().join("replacement-part");
    fs::copy(&part, &replacement).expect("replacement bytes");
    fs::remove_file(&part).expect("remove part");
    symlink(&replacement, &part).expect("symlink part");
    assert_eq!(
        verify_transport(&transport)
            .expect_err("symlink rejection")
            .kind()
            .code(),
        "PART_SET_INVALID"
    );
    fs::set_permissions(&bundle, fs::Permissions::from_mode(0o755))
        .expect("restore bundle directory");
}

#[cfg(unix)]
#[test]
fn sigkill_never_leaves_a_partial_final_directory() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("transport");
    pack_bundle(&bundle, &transport).expect("pack");
    let output = temp.path().join("killed-output");
    let mut child = Command::new(env!("CARGO_BIN_EXE_pangopup-build"))
        .arg("transport")
        .arg("unpack")
        .arg("--transport")
        .arg(&transport)
        .arg("--output")
        .arg(&output)
        .spawn()
        .expect("spawn unpack");
    let mut observed_stage = false;
    while child.try_wait().expect("poll unpack").is_none() {
        if !invocation_stages(temp.path(), "killed-output").is_empty() {
            observed_stage = true;
            child.kill().expect("kill staged unpack");
            break;
        }
        std::thread::yield_now();
    }
    child.wait().expect("wait for killed unpack");
    assert!(observed_stage, "the test must kill after staging exists");
    if output.exists() {
        assert_eq!(
            exact_members(&output),
            ["NOTICE", "manifest.json", "scores.pgi"]
        );
        verify_bundle(&output).expect("post-rename output must already be complete");
    } else {
        assert!(!invocation_stages(temp.path(), "killed-output").is_empty());
    }
}

#[test]
fn maintenance_cli_pins_grammar_json_and_streams() {
    let temp = Temp::new();
    let bundle = build_fixture(&temp);
    let transport = temp.path().join("transport");
    let binary = env!("CARGO_BIN_EXE_pangopup-build");
    let pack = Command::new(binary)
        .args(["transport", "pack", "--output"])
        .arg(&transport)
        .arg("--bundle")
        .arg(&bundle)
        .output()
        .expect("pack CLI");
    assert!(pack.status.success());
    assert!(pack.stderr.is_empty());
    let stdout = String::from_utf8(pack.stdout).expect("UTF-8 JSON");
    let packed: Value = serde_json::from_str(stdout.trim_end()).expect("pack JSON");
    let transport_id = packed["transport_id"].as_str().expect("transport ID");
    let bundle_id = packed["bundle_id"].as_str().expect("bundle ID");
    let part_count = packed["part_count"].as_u64().expect("part count");
    let compressed = packed["compressed_bytes"]
        .as_u64()
        .expect("compressed bytes");
    assert_eq!(
        stdout,
        format!(
            "{{\"status\":\"packed\",\"transport_id\":\"{transport_id}\",\"bundle_id\":\"{bundle_id}\",\"part_count\":{part_count},\"compressed_bytes\":{compressed}}}\n"
        )
    );

    let verify = Command::new(binary)
        .args(["transport", "verify", "--transport"])
        .arg(&transport)
        .output()
        .expect("verify CLI");
    assert!(verify.status.success());
    assert!(verify.stderr.is_empty());
    assert_eq!(
        String::from_utf8(verify.stdout).expect("UTF-8 JSON"),
        format!(
            "{{\"status\":\"verified\",\"transport_id\":\"{transport_id}\",\"bundle_id\":\"{bundle_id}\",\"part_count\":{part_count},\"compressed_bytes\":{compressed}}}\n"
        )
    );

    let unpacked = temp.path().join("cli-unpacked");
    let unpack = Command::new(binary)
        .args(["transport", "unpack", "--output"])
        .arg(&unpacked)
        .arg("--transport")
        .arg(&transport)
        .output()
        .expect("unpack CLI");
    assert!(unpack.status.success());
    assert!(unpack.stderr.is_empty());
    assert_eq!(
        String::from_utf8(unpack.stdout).expect("UTF-8 JSON"),
        format!(
            "{{\"status\":\"unpacked\",\"transport_id\":\"{transport_id}\",\"bundle_id\":\"{bundle_id}\"}}\n"
        )
    );

    let missing = Command::new(binary)
        .args(["transport", "verify", "--transport"])
        .arg(temp.path().join("missing-transport"))
        .output()
        .expect("missing input CLI");
    assert_eq!(missing.status.code(), Some(1));
    assert!(missing.stdout.is_empty());
    assert!(
        String::from_utf8(missing.stderr)
            .expect("UTF-8 error")
            .starts_with("{\"status\":\"error\",\"code\":\"INPUT_IO\"")
    );

    for arguments in [
        vec!["transport", "pack"],
        vec!["transport", "verify", "--transport", "--unknown"],
        vec![
            "transport",
            "verify",
            "--transport",
            "a",
            "--transport",
            "b",
        ],
        vec!["transport", "unpack", "x", "--output", "y"],
    ] {
        let output = Command::new(binary)
            .args(arguments)
            .output()
            .expect("usage CLI");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .expect("UTF-8 error")
                .starts_with("{\"status\":\"error\",\"code\":\"CLI_USAGE\"")
        );
    }
}
