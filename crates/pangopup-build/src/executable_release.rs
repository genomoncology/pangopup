//! Deterministic, network-free preparation of the Linux executable release set.

use crate::CommandError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};

const BINARY: &str = "pangopup-linux-x86_64";
const SBOM: &str = "pangopup-linux-x86_64.cdx.json";

#[derive(Debug, Serialize)]
pub struct PrepareOutcome {
    command: &'static str,
    status: &'static str,
    version: String,
    target_commit: String,
    output: PathBuf,
}

#[derive(Serialize)]
struct ReleaseManifest {
    schema: &'static str,
    version: String,
    target_commit: String,
    target: &'static str,
    rust_toolchain: &'static str,
    binary_size: u64,
    maximum_glibc_version: String,
    dynamic_dependencies: Vec<String>,
    members: Vec<Member>,
}

#[derive(Serialize)]
struct Member {
    name: String,
    size: u64,
    sha256: String,
}

pub fn prepare_executable_release(
    executable: &Path,
    sbom: &Path,
    version: &str,
    target_commit: &str,
    repository: &Path,
    output: &Path,
) -> Result<PrepareOutcome, CommandError> {
    validate_version(version)?;
    if target_commit.len() != 40
        || !target_commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(error(
            "RELEASE_INPUT",
            "target commit must be 40 lowercase hexadecimal characters",
        ));
    }
    if version != env!("CARGO_PKG_VERSION") || workspace_version(repository)? != version {
        return Err(error(
            "RELEASE_VERSION",
            "supplied version does not match the workspace version",
        ));
    }
    require_checkout(repository, target_commit)?;
    if output.exists() {
        return Err(error("RELEASE_OUTPUT", "output directory must be absent"));
    }
    let sbom_bytes = fs::read(sbom).map_err(|cause| io_error("read SBOM", cause))?;
    serde_json::from_slice::<serde_json::Value>(&sbom_bytes)
        .map_err(|cause| error("RELEASE_SBOM", format!("SBOM is not valid JSON: {cause}")))?;

    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let name = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            error(
                "RELEASE_OUTPUT",
                "output directory must have a UTF-8 final component",
            )
        })?;
    let stage = parent.join(format!(".{name}.tmp-{}", std::process::id()));
    if stage.exists() {
        return Err(error(
            "RELEASE_OUTPUT",
            "private staging directory already exists",
        ));
    }
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&stage)
        .map_err(|cause| io_error("create private staging directory", cause))?;

    let result = prepare_stage(
        executable,
        &sbom_bytes,
        version,
        target_commit,
        repository,
        &stage,
    )
    .and_then(|()| publish_noreplace(&stage, output));
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result?;
    Ok(PrepareOutcome {
        command: "executable-release.prepare",
        status: "ok",
        version: version.to_owned(),
        target_commit: target_commit.to_owned(),
        output: output.to_path_buf(),
    })
}

fn prepare_stage(
    executable: &Path,
    sbom_bytes: &[u8],
    version: &str,
    target_commit: &str,
    repository: &Path,
    stage: &Path,
) -> Result<(), CommandError> {
    let binary = stage.join(BINARY);
    fs::copy(executable, &binary).map_err(|cause| io_error("copy executable", cause))?;
    let strip = Command::new("strip")
        .arg("--strip-all")
        .arg(&binary)
        .status()
        .map_err(|cause| io_error("run strip", cause))?;
    if !strip.success() {
        return Err(error("RELEASE_BINARY", "strip rejected the executable"));
    }
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))
        .map_err(|cause| io_error("set executable mode", cause))?;
    let observed = Command::new(&binary)
        .arg("--version")
        .output()
        .map_err(|cause| io_error("execute release binary", cause))?;
    if !observed.status.success()
        || !observed.stderr.is_empty()
        || observed.stdout != format!("pangopup {version}\n").as_bytes()
    {
        return Err(error(
            "RELEASE_VERSION",
            "executable --version output does not match the requested version",
        ));
    }

    fs::write(stage.join(SBOM), sbom_bytes).map_err(|cause| io_error("copy SBOM", cause))?;
    fs::copy(repository.join("LICENSE"), stage.join("LICENSE"))
        .map_err(|cause| io_error("copy LICENSE", cause))?;
    fs::copy(repository.join("NOTICE"), stage.join("NOTICE"))
        .map_err(|cause| io_error("copy NOTICE", cause))?;

    let (dependencies, maximum_glibc_version) = inspect_elf(&binary)?;
    let binary_digest = digest_file(&binary)?;
    fs::write(
        stage.join(format!("{BINARY}.sha256")),
        format!("{binary_digest}  {BINARY}\n"),
    )
    .map_err(|cause| io_error("write checksum", cause))?;

    let names = [
        BINARY,
        &format!("{BINARY}.sha256"),
        SBOM,
        "LICENSE",
        "NOTICE",
    ];
    let mut members = Vec::with_capacity(names.len());
    for name in names {
        let path = stage.join(name);
        members.push(Member {
            name: name.to_owned(),
            size: fs::metadata(&path)
                .map_err(|cause| io_error("stat release member", cause))?
                .len(),
            sha256: digest_file(&path)?,
        });
    }
    members.sort_by(|left, right| left.name.cmp(&right.name));
    let manifest = ReleaseManifest {
        schema: "pangopup-executable-release-v1",
        version: version.to_owned(),
        target_commit: target_commit.to_owned(),
        target: "x86_64-unknown-linux-gnu",
        rust_toolchain: "1.93.1",
        binary_size: fs::metadata(&binary)
            .map_err(|cause| io_error("stat binary", cause))?
            .len(),
        maximum_glibc_version,
        dynamic_dependencies: dependencies,
        members,
    };
    let bytes = serde_jcs::to_vec(&manifest)
        .map_err(|cause| error("RELEASE_MANIFEST", format!("serialize manifest: {cause}")))?;
    fs::write(stage.join("release-manifest.json"), bytes)
        .map_err(|cause| io_error("write release manifest", cause))?;
    Ok(())
}

fn require_checkout(repository: &Path, target_commit: &str) -> Result<(), CommandError> {
    let head = git(repository, &["rev-parse", "HEAD"])?;
    if head.trim() != target_commit {
        return Err(error(
            "RELEASE_COMMIT",
            "repository HEAD does not match target commit",
        ));
    }
    for arguments in [
        ["diff", "--quiet", "--"].as_slice(),
        ["diff", "--cached", "--quiet", "--"].as_slice(),
    ] {
        let status = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(arguments)
            .status()
            .map_err(|cause| io_error("inspect repository cleanliness", cause))?;
        if !status.success() {
            return Err(error(
                "RELEASE_DIRTY",
                "repository tracked tree and index must be clean",
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn publish_noreplace(stage: &Path, output: &Path) -> Result<(), CommandError> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        stage,
        rustix::fs::CWD,
        output,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)
    .map_err(|cause| {
        if matches!(
            cause.kind(),
            io::ErrorKind::AlreadyExists | io::ErrorKind::DirectoryNotEmpty
        ) {
            error(
                "RELEASE_OUTPUT",
                "output directory appeared during preparation",
            )
        } else {
            io_error("publish release directory", cause)
        }
    })
}

#[cfg(not(target_os = "linux"))]
fn publish_noreplace(_stage: &Path, _output: &Path) -> Result<(), CommandError> {
    Err(error(
        "RELEASE_OUTPUT",
        "atomic no-replace executable release publication requires Linux",
    ))
}

fn workspace_version(repository: &Path) -> Result<String, CommandError> {
    let manifest = fs::read_to_string(repository.join("Cargo.toml"))
        .map_err(|cause| io_error("read workspace manifest", cause))?;
    let mut in_workspace_package = false;
    let mut version = None;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_workspace_package = line == "[workspace.package]";
            continue;
        }
        if in_workspace_package && line.starts_with("version") {
            let value = line
                .strip_prefix("version")
                .and_then(|rest| rest.trim_start().strip_prefix('='))
                .map(str::trim)
                .and_then(|value| value.strip_prefix('"'))
                .and_then(|value| value.strip_suffix('"'))
                .ok_or_else(|| error("RELEASE_VERSION", "workspace version is malformed"))?;
            if version.replace(value.to_owned()).is_some() {
                return Err(error("RELEASE_VERSION", "workspace version is duplicated"));
            }
        }
    }
    version.ok_or_else(|| error("RELEASE_VERSION", "workspace version is missing"))
}

fn git(repository: &Path, arguments: &[&str]) -> Result<String, CommandError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .map_err(|cause| io_error("run git", cause))?;
    if !output.status.success() {
        return Err(error("RELEASE_REPOSITORY", "git rejected the repository"));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| error("RELEASE_REPOSITORY", "git output is not UTF-8"))
}

fn inspect_elf(binary: &Path) -> Result<(Vec<String>, String), CommandError> {
    let dynamic = readelf(binary, &["-d"])?;
    let mut dependencies = Vec::new();
    for line in dynamic.lines().filter(|line| line.contains("(NEEDED)")) {
        let start = line
            .find('[')
            .ok_or_else(|| error("RELEASE_ELF", "malformed DT_NEEDED record"))?
            + 1;
        let end = line[start..]
            .find(']')
            .ok_or_else(|| error("RELEASE_ELF", "malformed DT_NEEDED record"))?
            + start;
        dependencies.push(line[start..end].to_owned());
    }
    let versions = readelf(binary, &["--version-info"])?;
    let mut maximum = None;
    for token in versions.split(|character: char| {
        character.is_whitespace() || matches!(character, '(' | ')' | '[' | ']')
    }) {
        if let Some(value) = token.strip_prefix("GLIBC_")
            && let Some((major, minor)) = parse_glibc(value)
        {
            maximum = maximum.max(Some((major, minor)));
        }
    }
    let (major, minor) =
        maximum.ok_or_else(|| error("RELEASE_ELF", "executable imports no GLIBC version"))?;
    Ok((dependencies, format!("{major}.{minor}")))
}

fn readelf(binary: &Path, arguments: &[&str]) -> Result<String, CommandError> {
    let output = Command::new("readelf")
        .args(arguments)
        .arg(binary)
        .output()
        .map_err(|cause| io_error("run readelf", cause))?;
    if !output.status.success() {
        return Err(error("RELEASE_ELF", "readelf rejected the executable"));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| error("RELEASE_ELF", "readelf output is not UTF-8"))
}

fn parse_glibc(value: &str) -> Option<(u32, u32)> {
    let value =
        value.trim_matches(|character: char| !character.is_ascii_digit() && character != '.');
    let (major, minor) = value.split_once('.')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

fn digest_file(path: &Path) -> Result<String, CommandError> {
    let mut file = fs::File::open(path).map_err(|cause| io_error("open release member", cause))?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest)
        .map_err(|cause| io_error("hash release member", cause))?;
    Ok(format!("{:x}", digest.finalize()))
}

fn validate_version(version: &str) -> Result<(), CommandError> {
    let pieces: Vec<_> = version.split('.').collect();
    if pieces.len() != 3
        || pieces.iter().any(|piece| {
            piece.is_empty()
                || !piece.bytes().all(|byte| byte.is_ascii_digit())
                || (piece.len() > 1 && piece.starts_with('0'))
        })
    {
        return Err(error(
            "RELEASE_VERSION",
            "version must be MAJOR.MINOR.PATCH",
        ));
    }
    Ok(())
}

fn error(code: &'static str, message: impl Into<String>) -> CommandError {
    CommandError::new(code, message)
}

fn io_error(operation: &str, cause: std::io::Error) -> CommandError {
    error("RELEASE_IO", format!("{operation}: {cause}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn strict_versions() {
        assert!(validate_version("0.1.0").is_ok());
        for invalid in ["v0.1.0", "1", "1.2", "1.2.3.4", "01.2.3", "1.2.x", ""] {
            assert!(validate_version(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn numeric_glibc_ordering() {
        assert_eq!(parse_glibc("2.9"), Some((2, 9)));
        assert_eq!(parse_glibc("2.35"), Some((2, 35)));
        assert!(Some((2, 35)) > Some((2, 9)));
    }

    #[test]
    fn private_stage_creation_and_late_output_are_fail_closed() {
        let root = tempfile::tempdir().expect("temporary root");
        let stage = root.path().join("stage");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&stage)
            .expect("private stage");
        assert_eq!(
            fs::metadata(&stage)
                .expect("stage metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        fs::write(stage.join("member"), b"prepared").expect("stage member");
        let output = root.path().join("output");
        fs::create_dir(&output).expect("late destination");
        fs::write(output.join("owner"), b"other").expect("late owner");
        let failure = publish_noreplace(&stage, &output).expect_err("late destination rejected");
        assert_eq!(failure.code, "RELEASE_OUTPUT");
        assert_eq!(
            fs::read(output.join("owner")).expect("late owner preserved"),
            b"other"
        );
        assert_eq!(
            fs::read(stage.join("member")).expect("stage retained"),
            b"prepared"
        );
    }
}
