//! Conservative removal of a direct Linux user installation.
//!
//! This module intentionally does not discover arbitrary paths. It admits the
//! running executable and the two roots selected by the runtime XDG contract,
//! then performs descriptor-relative, no-follow removal.

use pangopup_assets::{CachePathInputs, DataPathInputs, resolve_cache_root, resolve_data_root};
use rustix::fs::{self, AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, Stat};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    ffi::{CStr, OsString},
    io::{self, BufRead, IsTerminal, Write},
    os::fd::{AsRawFd, OwnedFd},
    os::unix::ffi::OsStrExt,
    path::{Component, Path, PathBuf},
};

const DATA_ENTRIES: &[&str] = &[
    ".install.lock",
    ".sync.lock",
    ".staging",
    "active.json",
    "bundles",
    "runtime",
];
const CACHE_ENTRIES: &[&str] = &[
    "model-results.sqlite3",
    "model-results.sqlite3-shm",
    "model-results.sqlite3-wal",
    "profiles",
];

#[derive(Debug)]
pub(crate) struct UninstallError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) exit: u8,
}

impl UninstallError {
    fn usage(message: impl Into<String>) -> Self {
        Self::new("CLI_USAGE", message, 2)
    }

    fn unsafe_path(message: impl Into<String>) -> Self {
        Self::new("UNINSTALL_UNSAFE", message, 1)
    }

    fn busy(message: impl Into<String>) -> Self {
        Self::new("UNINSTALL_BUSY", message, 1)
    }

    fn io(message: impl Into<String>) -> Self {
        Self::new("UNINSTALL_IO", message, 1)
    }

    fn noninteractive() -> Self {
        Self::new(
            "UNINSTALL_NONINTERACTIVE",
            "uninstall requires terminal stdin and stderr; use --yes for noninteractive removal",
            2,
        )
    }

    fn new(code: &'static str, message: impl Into<String>, exit: u8) -> Self {
        Self {
            code,
            message: message.into(),
            exit,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Choice {
    CodeOnly,
    Full,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Options {
    full: bool,
    yes: bool,
}

// Stat field widths differ by platform: Linux reports a 64-bit device and a
// 32-bit mode where macOS reports 32 and 16. Widen through these so the
// comparisons read the same on both, and so neither build carries a cast its
// own lint would call redundant.
#[cfg(target_os = "linux")]
fn raw_mode(mode: u32) -> rustix::fs::RawMode {
    mode
}

#[cfg(not(target_os = "linux"))]
fn raw_mode(mode: u32) -> rustix::fs::RawMode {
    mode as rustix::fs::RawMode
}

#[cfg(target_os = "linux")]
fn stat_device(stat: &fs::Stat) -> u64 {
    stat.st_dev
}

#[cfg(not(target_os = "linux"))]
fn stat_device(stat: &fs::Stat) -> u64 {
    u64::try_from(stat.st_dev).unwrap_or(u64::MAX)
}

#[cfg(target_os = "linux")]
fn stat_mode(stat: &fs::Stat) -> u32 {
    stat.st_mode
}

#[cfg(not(target_os = "linux"))]
fn stat_mode(stat: &fs::Stat) -> u32 {
    u32::from(stat.st_mode)
}

#[cfg(target_os = "linux")]
fn stat_nlink(stat: &fs::Stat) -> u64 {
    u64::from(stat.st_nlink)
}

#[cfg(not(target_os = "linux"))]
fn stat_nlink(stat: &fs::Stat) -> u64 {
    u64::from(stat.st_nlink)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
}

#[derive(Debug)]
struct RootPlan {
    path: PathBuf,
    identity: Option<Identity>,
    allowed: &'static [&'static str],
}

#[derive(Debug)]
struct Plan {
    executable: PathBuf,
    executable_identity: Identity,
    executable_parent: PathBuf,
    data: RootPlan,
    cache: RootPlan,
    uid: u32,
}

#[derive(Serialize)]
struct ResultLine<'a> {
    status: &'static str,
    scope: &'static str,
    executable: PathResult<'a>,
    data: PathResult<'a>,
    cache: PathResult<'a>,
}

#[derive(Serialize)]
struct PathResult<'a> {
    path: &'a str,
    state: &'static str,
}

pub(crate) fn run(raw: &[OsString]) -> Result<Vec<u8>, UninstallError> {
    let options = parse(raw)?;
    let executable = std::env::current_exe()
        .map_err(|error| UninstallError::io(format!("resolve current executable: {error}")))?;
    let data = resolve_data_root(&DataPathInputs::from_environment(None))
        .map_err(|error| UninstallError::unsafe_path(error.to_string()))?;
    let cache = resolve_cache_root(&CachePathInputs::from_environment(None))
        .map_err(|error| UninstallError::unsafe_path(error.to_string()))?
        .ok_or_else(|| UninstallError::unsafe_path("no Linux cache directory is available"))?;
    let terminals = io::stdin().is_terminal() && io::stderr().is_terminal();
    let mut input = io::stdin().lock();
    let mut error = io::stderr().lock();
    execute(
        options, executable, data, cache, terminals, &mut input, &mut error,
    )
}

fn parse(raw: &[OsString]) -> Result<Options, UninstallError> {
    let mut full = false;
    let mut yes = false;
    for value in raw {
        let value = value
            .to_str()
            .ok_or_else(|| UninstallError::usage("arguments must be UTF-8"))?;
        let slot = match value {
            "--full" => &mut full,
            "--yes" => &mut yes,
            _ => {
                return Err(UninstallError::usage(format!(
                    "unknown uninstall option {value}"
                )));
            }
        };
        if *slot {
            return Err(UninstallError::usage(format!(
                "{value} may be supplied once"
            )));
        }
        *slot = true;
    }
    Ok(Options { full, yes })
}

fn execute(
    options: Options,
    executable: PathBuf,
    data: PathBuf,
    cache: PathBuf,
    terminals: bool,
    input: &mut dyn BufRead,
    stderr: &mut dyn Write,
) -> Result<Vec<u8>, UninstallError> {
    let plan = inspect(executable, data, cache)?;
    render_plan(stderr, &plan)?;
    let choice = if options.yes {
        if options.full {
            Choice::Full
        } else {
            Choice::CodeOnly
        }
    } else {
        if !terminals {
            return Err(UninstallError::noninteractive());
        }
        prompt(options.full, input, stderr)?
    };

    if choice == Choice::Cancel {
        return result_json(&plan, "cancelled", "none", false, false);
    }

    if choice == Choice::Full {
        let authority = acquire_authorities(&plan)?;
        revalidate_roots(&plan, &authority)?;
        let data_blocker = match authority {
            DataAuthority::Present(locks) => {
                remove_root(&plan.data, plan.uid, Some(&locks.root), true, true)?
                    .expect("present data removal retains a blocker")
            }
            DataAuthority::Absent(blocker) => blocker,
        };
        test_hook(TestHookPoint::BeforeCacheRemoval);
        let cache_removal = remove_root(&plan.cache, plan.uid, None, false, false);
        match cache_removal {
            Ok(_) => finalize_public_blocker(data_blocker)?,
            Err(error) => {
                finalize_public_blocker(data_blocker)?;
                return Err(error);
            }
        }
    }
    unlink_executable(&plan)?;
    result_json(
        &plan,
        "removed",
        if choice == Choice::Full {
            "full"
        } else {
            "code_only"
        },
        true,
        choice == Choice::Full,
    )
}

fn render_plan(writer: &mut dyn Write, plan: &Plan) -> Result<(), UninstallError> {
    writeln!(writer, "Pangopup uninstall plan:")
        .and_then(|()| writeln!(writer, "  executable: {}", plan.executable.display()))
        .and_then(|()| {
            writeln!(
                writer,
                "  data: {} ({})",
                plan.data.path.display(),
                state(&plan.data)
            )
        })
        .and_then(|()| {
            writeln!(
                writer,
                "  cache: {} ({})",
                plan.cache.path.display(),
                state(&plan.cache)
            )
        })
        .map_err(|error| UninstallError::io(format!("write uninstall plan: {error}")))
}

fn state(root: &RootPlan) -> &'static str {
    if root.identity.is_some() {
        "present"
    } else {
        "absent"
    }
}

fn prompt(
    full: bool,
    input: &mut dyn BufRead,
    writer: &mut dyn Write,
) -> Result<Choice, UninstallError> {
    if full {
        writeln!(writer, "Remove code and all managed data? Type yes or no.")
            .map_err(|error| UninstallError::io(format!("write uninstall prompt: {error}")))?;
    } else {
        writeln!(
            writer,
            "Choose: 1) code only  2) code and all managed data  3) cancel"
        )
        .map_err(|error| UninstallError::io(format!("write uninstall prompt: {error}")))?;
    }
    writer
        .flush()
        .map_err(|error| UninstallError::io(format!("flush uninstall prompt: {error}")))?;
    let mut answer = String::new();
    input
        .read_line(&mut answer)
        .map_err(|error| UninstallError::io(format!("read uninstall choice: {error}")))?;
    let answer = answer.trim_end_matches(['\r', '\n']);
    if full {
        match answer {
            "yes" => Ok(Choice::Full),
            "no" => Ok(Choice::Cancel),
            _ => Err(UninstallError::unsafe_path(
                "confirmation must be exactly yes or no",
            )),
        }
    } else {
        match answer {
            "1" => Ok(Choice::CodeOnly),
            "2" => Ok(Choice::Full),
            "3" => Ok(Choice::Cancel),
            _ => Err(UninstallError::unsafe_path(
                "choice must be exactly 1, 2, or 3",
            )),
        }
    }
}

#[cfg_attr(not(target_os = "linux"), allow(unused_variables, unreachable_code))]
fn inspect(executable: PathBuf, data: PathBuf, cache: PathBuf) -> Result<Plan, UninstallError> {
    #[cfg(not(target_os = "linux"))]
    return Err(UninstallError::unsafe_path(
        "direct uninstall is supported only on Linux",
    ));

    let uid = rustix::process::geteuid().as_raw();
    reject_lexical_alias(&executable, "executable")?;
    reject_lexical_alias(&data, "data root")?;
    reject_lexical_alias(&cache, "cache root")?;
    let executable = canonical_utf8(&executable, "executable")?;
    if executable.file_name().and_then(|name| name.to_str()) != Some("pangopup") {
        return Err(UninstallError::unsafe_path(
            "current executable is not named pangopup",
        ));
    }
    let executable_metadata = std::fs::symlink_metadata(&executable)
        .map_err(|error| UninstallError::io(format!("inspect executable: {error}")))?;
    use std::os::unix::fs::MetadataExt;
    if !executable_metadata.file_type().is_file()
        || executable_metadata.uid() != uid
        || executable_metadata.nlink() != 1
    {
        return Err(UninstallError::unsafe_path(
            "current executable must be a single-link regular file owned by the current user; remove Docker images and volumes on the host",
        ));
    }
    let executable_parent = executable
        .parent()
        .ok_or_else(|| UninstallError::unsafe_path("executable has no parent"))?;
    let executable_parent = canonical_utf8(executable_parent, "executable parent")?;
    let parent_metadata = std::fs::symlink_metadata(&executable_parent)
        .map_err(|error| UninstallError::io(format!("inspect executable parent: {error}")))?;
    if !parent_metadata.is_dir()
        || parent_metadata.uid() != uid
        || parent_metadata.mode() & 0o300 != 0o300
    {
        return Err(UninstallError::unsafe_path(
            "executable parent must be a removable real directory owned by the current user; remove Docker images and volumes on the host",
        ));
    }

    let data = inspect_root(data, DATA_ENTRIES, uid, "data")?;
    let cache = inspect_root(cache, CACHE_ENTRIES, uid, "cache")?;
    reject_relationships(&executable, &executable_parent, &data.path, &cache.path)?;
    reject_reserved(&data.path, &cache.path, &executable_parent)?;
    Ok(Plan {
        executable,
        executable_identity: Identity {
            device: executable_metadata.dev(),
            inode: executable_metadata.ino(),
            mode: executable_metadata.mode(),
            uid: executable_metadata.uid(),
        },
        executable_parent,
        data,
        cache,
        uid,
    })
}

fn inspect_root(
    path: PathBuf,
    allowed: &'static [&'static str],
    uid: u32,
    label: &str,
) -> Result<RootPlan, UninstallError> {
    if path.file_name().and_then(|name| name.to_str()) != Some("pangopup") {
        return Err(UninstallError::unsafe_path(format!(
            "{label} root must be named pangopup"
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| UninstallError::unsafe_path(format!("{label} root has no parent")))?;
    let parent = resolve_existing_prefix(parent, &format!("{label} parent"))?;
    let resolved = parent.join("pangopup");
    match std::fs::symlink_metadata(&resolved) {
        Ok(metadata) => {
            use std::os::unix::fs::MetadataExt;
            if metadata.file_type().is_symlink() || !metadata.is_dir() || metadata.uid() != uid {
                return Err(UninstallError::unsafe_path(format!(
                    "{label} root must be a real user-owned directory"
                )));
            }
            let identity = Identity {
                device: metadata.dev(),
                inode: metadata.ino(),
                mode: metadata.mode(),
                uid: metadata.uid(),
            };
            inspect_tree(&resolved, identity, uid, allowed, true)?;
            Ok(RootPlan {
                path: resolved,
                identity: Some(identity),
                allowed,
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(RootPlan {
            path: resolved,
            identity: None,
            allowed,
        }),
        Err(error) => Err(UninstallError::io(format!("inspect {label} root: {error}"))),
    }
}

fn reject_lexical_alias(path: &Path, label: &str) -> Result<(), UninstallError> {
    if !path.is_absolute() || path.to_str().is_none() {
        return Err(UninstallError::unsafe_path(format!(
            "{label} must be an absolute UTF-8 path"
        )));
    }
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(UninstallError::unsafe_path(format!(
            "{label} must not contain .."
        )));
    }
    Ok(())
}

fn canonical_utf8(path: &Path, label: &str) -> Result<PathBuf, UninstallError> {
    let path = std::fs::canonicalize(path)
        .map_err(|error| UninstallError::io(format!("resolve {label}: {error}")))?;
    if path.to_str().is_none() {
        return Err(UninstallError::unsafe_path(format!(
            "{label} must resolve to UTF-8"
        )));
    }
    Ok(path)
}

fn resolve_existing_prefix(path: &Path, label: &str) -> Result<PathBuf, UninstallError> {
    let mut missing = Vec::new();
    let mut cursor = path;
    while !cursor.exists() {
        let name = cursor.file_name().ok_or_else(|| {
            UninstallError::unsafe_path(format!("{label} has no existing ancestor"))
        })?;
        missing.push(name.to_owned());
        cursor = cursor.parent().ok_or_else(|| {
            UninstallError::unsafe_path(format!("{label} has no existing ancestor"))
        })?;
    }
    let mut resolved = canonical_utf8(cursor, label)?;
    for name in missing.iter().rev() {
        resolved.push(name);
    }
    if resolved.to_str().is_none() {
        return Err(UninstallError::unsafe_path(format!(
            "{label} must resolve to UTF-8"
        )));
    }
    Ok(resolved)
}

fn reject_relationships(
    executable: &Path,
    executable_parent: &Path,
    data: &Path,
    cache: &Path,
) -> Result<(), UninstallError> {
    if data == cache || data.starts_with(cache) || cache.starts_with(data) {
        return Err(UninstallError::unsafe_path(
            "data and cache roots must be distinct and non-nested",
        ));
    }
    for (label, root) in [("data", data), ("cache", cache)] {
        if root == Path::new("/")
            || root == executable_parent
            || executable.starts_with(root)
            || root.starts_with(executable)
        {
            return Err(UninstallError::unsafe_path(format!(
                "{label} root overlaps the executable"
            )));
        }
    }
    Ok(())
}

fn reject_reserved(
    data: &Path,
    cache: &Path,
    executable_parent: &Path,
) -> Result<(), UninstallError> {
    let mut reserved = BTreeSet::new();
    reserved.insert(PathBuf::from("/"));
    if let Some(home) = std::env::var_os("HOME")
        && let Ok(path) = resolve_existing_prefix(&PathBuf::from(home), "HOME")
    {
        reserved.insert(path);
    }
    for name in ["XDG_DATA_HOME", "XDG_CACHE_HOME"] {
        if let Some(value) = std::env::var_os(name)
            && let Ok(path) = resolve_existing_prefix(&PathBuf::from(value), name)
        {
            reserved.insert(path);
        }
    }
    reserved.insert(executable_parent.to_owned());
    if reserved.contains(data) || reserved.contains(cache) {
        return Err(UninstallError::unsafe_path(
            "a removal root is a protected home, XDG, or executable directory",
        ));
    }
    Ok(())
}

fn inspect_tree(
    path: &Path,
    identity: Identity,
    uid: u32,
    allowed: &[&str],
    top: bool,
) -> Result<(), UninstallError> {
    let fd = fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| UninstallError::unsafe_path(format!("open managed root: {error}")))?;
    let stat = fs::fstat(&fd)
        .map_err(|error| UninstallError::io(format!("inspect managed root: {error}")))?;
    if stat_device(&stat) != identity.device || stat.st_ino != identity.inode {
        return Err(UninstallError::unsafe_path(
            "managed root changed during inspection",
        ));
    }
    inspect_dir(&fd, stat_device(&stat), uid, allowed, top)
}

fn inspect_dir(
    fd: &OwnedFd,
    device: u64,
    uid: u32,
    allowed: &[&str],
    top: bool,
) -> Result<(), UninstallError> {
    let mut dir = Dir::read_from(fd)
        .map_err(|error| UninstallError::io(format!("read managed directory: {error}")))?;
    while let Some(entry) = dir.read() {
        let entry =
            entry.map_err(|error| UninstallError::io(format!("read managed entry: {error}")))?;
        let name = entry.file_name();
        if name.to_bytes() == b"." || name.to_bytes() == b".." {
            continue;
        }
        let text = name
            .to_str()
            .map_err(|_| UninstallError::unsafe_path("managed entry name must be UTF-8"))?;
        if top && !allowed.contains(&text) {
            return Err(UninstallError::unsafe_path(format!(
                "managed root contains unknown top-level entry {text}"
            )));
        }
        let stat = fs::statat(fd, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            UninstallError::io(format!("inspect managed entry {text}: {error}"))
        })?;
        validate_descendant(&stat, device, uid, text)?;
        if FileType::from_raw_mode(stat.st_mode).is_dir() {
            let child = fs::openat(
                fd,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| {
                UninstallError::unsafe_path(format!("open managed entry {text}: {error}"))
            })?;
            let opened = fs::fstat(&child).map_err(|error| {
                UninstallError::io(format!("revalidate managed entry {text}: {error}"))
            })?;
            same_identity(&stat, &opened, text)?;
            inspect_dir(&child, device, uid, allowed, false)?;
        }
    }
    Ok(())
}

fn validate_descendant(
    stat: &Stat,
    device: u64,
    uid: u32,
    name: &str,
) -> Result<(), UninstallError> {
    let kind = FileType::from_raw_mode(stat.st_mode);
    if stat_device(stat) != device {
        return Err(UninstallError::unsafe_path(format!(
            "managed entry {name} crosses a filesystem boundary"
        )));
    }
    if stat.st_uid != uid {
        return Err(UninstallError::unsafe_path(format!(
            "managed entry {name} has a foreign owner"
        )));
    }
    if kind.is_file() && stat_nlink(stat) != 1 {
        return Err(UninstallError::unsafe_path(format!(
            "managed entry {name} is unexpectedly hard-linked"
        )));
    }
    if !(kind.is_file() || kind.is_dir() || kind.is_symlink()) {
        return Err(UninstallError::unsafe_path(format!(
            "managed entry {name} has an unsupported special type"
        )));
    }
    Ok(())
}

fn same_identity(before: &Stat, after: &Stat, name: &str) -> Result<(), UninstallError> {
    if before.st_dev != after.st_dev
        || before.st_ino != after.st_ino
        || before.st_mode != after.st_mode
    {
        return Err(UninstallError::unsafe_path(format!(
            "managed entry {name} changed during inspection"
        )));
    }
    Ok(())
}

struct Authorities {
    root: OwnedFd,
    _sync: OwnedFd,
    _install: OwnedFd,
}

enum DataAuthority {
    Present(Authorities),
    Absent(PublicBlocker),
}

fn acquire_authorities(plan: &Plan) -> Result<DataAuthority, UninstallError> {
    if plan.data.identity.is_none() {
        return create_absent_data_blocker(&plan.data.path, plan.uid).map(DataAuthority::Absent);
    }
    let root = fs::open(
        &plan.data.path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| UninstallError::unsafe_path(format!("open data root for locking: {error}")))?;
    test_hook(TestHookPoint::AuthorityRootOpened);
    let stat = fs::fstat(&root).map_err(|error| {
        UninstallError::io(format!("revalidate data root for locking: {error}"))
    })?;
    let identity = plan.data.identity.expect("present data root has identity");
    validate_root_stat(&stat, identity, plan.uid, "data root for locking")?;
    let sync = acquire_lock(&root, c".sync.lock", "synchronization")?;
    let install = acquire_lock(&root, c".install.lock", "installation")?;
    Ok(DataAuthority::Present(Authorities {
        root,
        _sync: sync,
        _install: install,
    }))
}

fn acquire_lock(root: &OwnedFd, name: &CStr, label: &str) -> Result<OwnedFd, UninstallError> {
    let file = fs::openat(
        root,
        name,
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|error| UninstallError::unsafe_path(format!("open {label} authority: {error}")))?;
    // SAFETY: flock operates on this live descriptor and stores no pointer.
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        Ok(file)
    } else if io::Error::last_os_error().kind() == io::ErrorKind::WouldBlock {
        Err(UninstallError::busy(format!(
            "another asset {label} operation is active; stop it and retry"
        )))
    } else {
        Err(UninstallError::io(format!("lock {label} authority")))
    }
}

fn revalidate_roots(plan: &Plan, authority: &DataAuthority) -> Result<(), UninstallError> {
    for root in [&plan.data, &plan.cache] {
        match root.identity {
            Some(identity) => inspect_tree(&root.path, identity, plan.uid, root.allowed, true)?,
            None if std::ptr::eq(root, &plan.data) => match authority {
                DataAuthority::Absent(blocker) => blocker.revalidate()?,
                DataAuthority::Present(_) => unreachable!("present authority for absent data"),
            },
            None if root.path.exists() => {
                return Err(UninstallError::unsafe_path(
                    "an absent removal root appeared after confirmation",
                ));
            }
            None => {}
        }
    }
    Ok(())
}

fn remove_root(
    root: &RootPlan,
    uid: u32,
    held_root: Option<&OwnedFd>,
    retain_authorities: bool,
    retain_public_blocker: bool,
) -> Result<Option<PublicBlocker>, UninstallError> {
    let Some(identity) = root.identity else {
        if root.path.exists() {
            return Err(UninstallError::unsafe_path(
                "an absent removal root appeared during uninstall",
            ));
        }
        return Ok(None);
    };
    let parent = root.path.parent().expect("admitted root has parent");
    let name = root.path.file_name().expect("admitted root has name");
    let parent_fd = fs::open(
        parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| UninstallError::io(format!("open removal parent: {error}")))?;
    let stat = fs::statat(&parent_fd, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| UninstallError::io(format!("revalidate removal root: {error}")))?;
    validate_root_stat(&stat, identity, uid, "removal root")?;
    test_hook(TestHookPoint::PublicRootStated);
    let opened;
    let root_fd = if let Some(held) = held_root {
        held
    } else {
        opened = fs::openat(
            &parent_fd,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| UninstallError::unsafe_path(format!("open removal root: {error}")))?;
        &opened
    };
    let opened_stat = fs::fstat(root_fd)
        .map_err(|error| UninstallError::io(format!("inspect opened removal root: {error}")))?;
    validate_root_stat(&opened_stat, identity, uid, "opened removal root")?;
    same_identity(&stat, &opened_stat, "removal root")?;

    if fail_root_detach() {
        return Err(UninstallError::io("injected later-phase removal failure"));
    }
    let tombstone = create_tombstone(&parent_fd, uid)?;
    if let Err(error) = fs::renameat_with(
        &parent_fd,
        name,
        &parent_fd,
        tombstone.name.as_c_str(),
        RenameFlags::EXCHANGE,
    ) {
        let _ = fs::unlinkat(&parent_fd, tombstone.name.as_c_str(), AtFlags::empty());
        return Err(UninstallError::io(format!(
            "atomically detach managed root: {error}"
        )));
    }
    let detached = fs::statat(
        &parent_fd,
        tombstone.name.as_c_str(),
        AtFlags::SYMLINK_NOFOLLOW,
    )
    .map_err(|error| UninstallError::io(format!("inspect detached managed root: {error}")))?;
    let public_tombstone =
        fs::statat(&parent_fd, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            UninstallError::io(format!("inspect public uninstall tombstone: {error}"))
        })?;
    if let Err(error) = validate_root_stat(&detached, identity, uid, "detached managed root")
        .and_then(|()| same_identity(&opened_stat, &detached, "detached managed root"))
        .and_then(|()| same_identity(&tombstone.stat, &public_tombstone, "uninstall tombstone"))
    {
        rollback_exchange(
            &parent_fd,
            name,
            tombstone.name.as_c_str(),
            &opened_stat,
            &tombstone.stat,
        );
        return Err(error);
    }
    test_hook(TestHookPoint::RootDetached);

    let removal = (|| {
        remove_contents(
            root_fd,
            identity.device,
            uid,
            retain_authorities,
            Some(root.allowed),
        )?;
        if retain_authorities {
            remove_authority(root_fd, c".install.lock", identity.device, uid)?;
            remove_authority(root_fd, c".sync.lock", identity.device, uid)?;
        }
        fs::unlinkat(&parent_fd, tombstone.name.as_c_str(), AtFlags::REMOVEDIR)
            .map_err(|error| UninstallError::io(format!("remove detached managed root: {error}")))
    })();
    if removal.is_err() {
        let detached_exists = fs::statat(
            &parent_fd,
            tombstone.name.as_c_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .is_ok();
        if detached_exists {
            rollback_exchange(
                &parent_fd,
                name,
                tombstone.name.as_c_str(),
                &opened_stat,
                &tombstone.stat,
            );
        }
    }
    removal?;
    let public_name = std::ffi::CString::new(name.as_bytes())
        .map_err(|_| UninstallError::unsafe_path("public root name contains NUL"))?;
    let blocker = PublicBlocker {
        parent: parent_fd,
        name: public_name,
        _file: tombstone._file,
        stat: tombstone.stat,
    };
    blocker.revalidate()?;
    if retain_public_blocker {
        Ok(Some(blocker))
    } else {
        finalize_public_blocker(blocker)?;
        Ok(None)
    }
}

fn validate_root_stat(
    stat: &Stat,
    identity: Identity,
    uid: u32,
    label: &str,
) -> Result<(), UninstallError> {
    if stat_device(stat) != identity.device
        || stat.st_ino != identity.inode
        || stat_mode(stat) != identity.mode
        || stat.st_uid != identity.uid
        || identity.uid != uid
        || !FileType::from_raw_mode(raw_mode(identity.mode)).is_dir()
    {
        return Err(UninstallError::unsafe_path(format!(
            "{label} changed after admission"
        )));
    }
    Ok(())
}

struct Tombstone {
    name: std::ffi::CString,
    _file: OwnedFd,
    stat: Stat,
}

struct PublicBlocker {
    parent: OwnedFd,
    name: std::ffi::CString,
    _file: OwnedFd,
    stat: Stat,
}

impl PublicBlocker {
    fn revalidate(&self) -> Result<(), UninstallError> {
        let held = fs::fstat(&self._file)
            .map_err(|error| UninstallError::io(format!("inspect held data blocker: {error}")))?;
        same_identity(&self.stat, &held, "held data blocker")?;
        let named = fs::statat(
            &self.parent,
            self.name.as_c_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|error| {
            UninstallError::unsafe_path(format!("revalidate public data blocker: {error}"))
        })?;
        same_identity(&self.stat, &named, "public data blocker")?;
        if !FileType::from_raw_mode(named.st_mode).is_file() || named.st_nlink != 1 {
            return Err(UninstallError::unsafe_path(
                "public data blocker has an unsafe shape",
            ));
        }
        Ok(())
    }

    /// Remove the public name only while it still names the regular file held
    /// by this object. This is deliberately safe to call from error cleanup:
    /// a missing or replaced name is left alone.
    fn unlink_if_owned(&self) -> Result<(), UninstallError> {
        let held = fs::fstat(&self._file)
            .map_err(|error| UninstallError::io(format!("inspect held data blocker: {error}")))?;
        same_identity(&self.stat, &held, "held data blocker")?;
        let named = match fs::statat(
            &self.parent,
            self.name.as_c_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(named) => named,
            Err(rustix::io::Errno::NOENT) => return Ok(()),
            Err(error) => {
                return Err(UninstallError::unsafe_path(format!(
                    "inspect public data blocker for cleanup: {error}"
                )));
            }
        };
        if same_identity(&self.stat, &named, "public data blocker cleanup").is_err()
            || !FileType::from_raw_mode(named.st_mode).is_file()
            || named.st_nlink != 1
        {
            return Ok(());
        }
        fs::unlinkat(&self.parent, self.name.as_c_str(), AtFlags::empty())
            .map_err(|error| UninstallError::io(format!("remove public data blocker: {error}")))
    }
}

impl Drop for PublicBlocker {
    fn drop(&mut self) {
        let _ = self.unlink_if_owned();
    }
}

fn create_absent_data_blocker(path: &Path, uid: u32) -> Result<PublicBlocker, UninstallError> {
    let parent = path
        .parent()
        .ok_or_else(|| UninstallError::unsafe_path("data root has no parent"))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| UninstallError::io(format!("create data blocker parent: {error}")))?;
    let resolved = canonical_utf8(parent, "data blocker parent")?;
    if resolved != parent {
        return Err(UninstallError::unsafe_path(
            "data blocker parent changed after admission",
        ));
    }
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::symlink_metadata(parent)
        .map_err(|error| UninstallError::io(format!("inspect data blocker parent: {error}")))?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o022 != 0 {
        return Err(UninstallError::unsafe_path(
            "data blocker parent must be a private user-owned directory",
        ));
    }
    let parent_fd = fs::open(
        parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| UninstallError::unsafe_path(format!("open data blocker parent: {error}")))?;
    let name = std::ffi::CString::new(
        path.file_name()
            .expect("admitted data root has name")
            .as_bytes(),
    )
    .expect("admitted UTF-8 root has no NUL");
    let file = fs::openat(
        &parent_fd,
        name.as_c_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|error| {
        if error == rustix::io::Errno::EXIST {
            UninstallError::busy("data root appeared while uninstall was acquiring authority")
        } else {
            UninstallError::io(format!("create public data blocker: {error}"))
        }
    })?;
    let stat = fs::fstat(&file)
        .map_err(|error| UninstallError::io(format!("inspect public data blocker: {error}")))?;
    let blocker = PublicBlocker {
        parent: parent_fd,
        name,
        _file: file,
        stat,
    };
    blocker.revalidate()?;
    Ok(blocker)
}

fn finalize_public_blocker(blocker: PublicBlocker) -> Result<(), UninstallError> {
    test_hook(TestHookPoint::BlockerFinalizing);
    blocker.revalidate()?;
    blocker.unlink_if_owned()
}

fn create_tombstone(parent: &OwnedFd, uid: u32) -> Result<Tombstone, UninstallError> {
    for nonce in 0..128_u32 {
        let name = std::ffi::CString::new(format!(
            ".pangopup-uninstall-{}-{nonce}",
            rustix::process::getpid().as_raw_nonzero()
        ))
        .expect("fixed tombstone name has no NUL");
        match fs::openat(
            parent,
            name.as_c_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        ) {
            Ok(file) => {
                let stat = fs::fstat(&file).map_err(|error| {
                    UninstallError::io(format!("inspect uninstall tombstone: {error}"))
                })?;
                if stat.st_uid != uid
                    || stat_nlink(&stat) != 1
                    || !FileType::from_raw_mode(stat.st_mode).is_file()
                {
                    let _ = fs::unlinkat(parent, name.as_c_str(), AtFlags::empty());
                    return Err(UninstallError::unsafe_path(
                        "uninstall tombstone has an unsafe identity",
                    ));
                }
                return Ok(Tombstone {
                    name,
                    _file: file,
                    stat,
                });
            }
            Err(rustix::io::Errno::EXIST) => {}
            Err(error) => {
                return Err(UninstallError::io(format!(
                    "create uninstall tombstone: {error}"
                )));
            }
        }
    }
    Err(UninstallError::io(
        "cannot allocate a unique uninstall tombstone",
    ))
}

fn rollback_exchange(
    parent: &OwnedFd,
    public: &std::ffi::OsStr,
    tombstone: &CStr,
    detached_identity: &Stat,
    blocker_identity: &Stat,
) {
    let Ok(detached) = fs::statat(parent, tombstone, AtFlags::SYMLINK_NOFOLLOW) else {
        return;
    };
    let Ok(blocker) = fs::statat(parent, public, AtFlags::SYMLINK_NOFOLLOW) else {
        return;
    };
    if same_identity(detached_identity, &detached, "rollback root").is_err()
        || same_identity(blocker_identity, &blocker, "rollback blocker").is_err()
    {
        return;
    }
    if fs::renameat_with(parent, public, parent, tombstone, RenameFlags::EXCHANGE).is_err() {
        return;
    }
    let Ok(named_blocker) = fs::statat(parent, tombstone, AtFlags::SYMLINK_NOFOLLOW) else {
        return;
    };
    if same_identity(blocker_identity, &named_blocker, "rollback blocker").is_ok() {
        let _ = fs::unlinkat(parent, tombstone, AtFlags::empty());
    }
}

fn remove_authority(
    fd: &OwnedFd,
    name: &CStr,
    device: u64,
    uid: u32,
) -> Result<(), UninstallError> {
    match fs::statat(fd, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            validate_descendant(&stat, device, uid, &name.to_string_lossy())?;
            if !FileType::from_raw_mode(stat.st_mode).is_file() {
                return Err(UninstallError::unsafe_path(
                    "lock authority changed during removal",
                ));
            }
            fs::unlinkat(fd, name, AtFlags::empty())
                .map_err(|error| UninstallError::io(format!("remove lock authority: {error}")))
        }
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Err(error) => Err(UninstallError::io(format!(
            "inspect lock authority: {error}"
        ))),
    }
}

fn remove_contents(
    fd: &OwnedFd,
    device: u64,
    uid: u32,
    retain_authorities: bool,
    top_level_allowed: Option<&[&str]>,
) -> Result<(), UninstallError> {
    let mut names = Vec::new();
    let mut dir = Dir::read_from(fd)
        .map_err(|error| UninstallError::io(format!("read removal directory: {error}")))?;
    while let Some(entry) = dir.read() {
        let entry =
            entry.map_err(|error| UninstallError::io(format!("read removal entry: {error}")))?;
        let name = entry.file_name();
        if name.to_bytes() != b"." && name.to_bytes() != b".." {
            if retain_authorities
                && (name.to_bytes() == b".sync.lock" || name.to_bytes() == b".install.lock")
            {
                continue;
            }
            if let Some(allowed) = top_level_allowed {
                let text = name
                    .to_str()
                    .map_err(|_| UninstallError::unsafe_path("managed entry name must be UTF-8"))?;
                if !allowed.contains(&text) {
                    return Err(UninstallError::unsafe_path(format!(
                        "managed root contains unknown top-level entry {text}"
                    )));
                }
            }
            names.push(name.to_owned());
        }
    }
    for name in names {
        let text = name.to_string_lossy();
        let stat = fs::statat(fd, &name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            UninstallError::io(format!("revalidate removal entry {text}: {error}"))
        })?;
        validate_descendant(&stat, device, uid, &text)?;
        if FileType::from_raw_mode(stat.st_mode).is_dir() {
            let child = fs::openat(
                fd,
                &name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| {
                UninstallError::unsafe_path(format!("open removal entry {text}: {error}"))
            })?;
            let opened = fs::fstat(&child).map_err(|error| {
                UninstallError::io(format!("inspect removal entry {text}: {error}"))
            })?;
            same_identity(&stat, &opened, &text)?;
            remove_contents(&child, device, uid, false, None)?;
            let final_stat = fs::statat(fd, &name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
                UninstallError::io(format!("finalize removal entry {text}: {error}"))
            })?;
            same_identity(&opened, &final_stat, &text)?;
            fs::unlinkat(fd, &name, AtFlags::REMOVEDIR)
                .map_err(|error| UninstallError::io(format!("remove directory {text}: {error}")))?;
        } else {
            fs::unlinkat(fd, &name, AtFlags::empty())
                .map_err(|error| UninstallError::io(format!("remove entry {text}: {error}")))?;
        }
    }
    Ok(())
}

fn unlink_executable(plan: &Plan) -> Result<(), UninstallError> {
    let parent = fs::open(
        &plan.executable_parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| UninstallError::io(format!("open executable parent: {error}")))?;
    let name = plan
        .executable
        .file_name()
        .expect("admitted executable has name");
    let stat = fs::statat(&parent, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| UninstallError::io(format!("revalidate executable: {error}")))?;
    if stat_device(&stat) != plan.executable_identity.device
        || stat.st_ino != plan.executable_identity.inode
        || stat.st_uid != plan.uid
        || stat_nlink(&stat) != 1
        || !FileType::from_raw_mode(stat.st_mode).is_file()
    {
        return Err(UninstallError::unsafe_path(
            "executable changed after confirmation",
        ));
    }
    fs::unlinkat(&parent, name, AtFlags::empty())
        .map_err(|error| UninstallError::io(format!("remove executable: {error}")))
}

fn result_json(
    plan: &Plan,
    status: &'static str,
    scope: &'static str,
    executable_removed: bool,
    full: bool,
) -> Result<Vec<u8>, UninstallError> {
    let executable = plan.executable.to_str().expect("preflight UTF-8");
    let data = plan.data.path.to_str().expect("preflight UTF-8");
    let cache = plan.cache.path.to_str().expect("preflight UTF-8");
    let state_for = |root: &RootPlan| {
        if root.identity.is_none() {
            "absent"
        } else if full {
            "removed"
        } else {
            "preserved"
        }
    };
    let value = ResultLine {
        status,
        scope,
        executable: PathResult {
            path: executable,
            state: if executable_removed {
                "removed"
            } else {
                "preserved"
            },
        },
        data: PathResult {
            path: data,
            state: state_for(&plan.data),
        },
        cache: PathResult {
            path: cache,
            state: state_for(&plan.cache),
        },
    };
    let mut bytes = serde_json::to_vec(&value)
        .map_err(|error| UninstallError::io(format!("serialize uninstall result: {error}")))?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestHookPoint {
    AuthorityRootOpened,
    PublicRootStated,
    RootDetached,
    BeforeCacheRemoval,
    BlockerFinalizing,
}

#[cfg(test)]
type TestHook = Option<(TestHookPoint, Box<dyn FnOnce()>)>;

#[cfg(test)]
thread_local! {
    static TEST_HOOK: std::cell::RefCell<TestHook> =
        std::cell::RefCell::new(None);
    static FAIL_ROOT_DETACH_CALL: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn test_hook(point: TestHookPoint) {
    TEST_HOOK.with(|slot| {
        let matches = slot
            .borrow()
            .as_ref()
            .is_some_and(|(expected, _)| *expected == point);
        if matches {
            let (_, hook) = slot.borrow_mut().take().expect("matching hook exists");
            hook();
        }
    });
}

#[cfg(not(test))]
fn test_hook(_point: TestHookPoint) {}

#[cfg(test)]
fn fail_root_detach() -> bool {
    FAIL_ROOT_DETACH_CALL.with(|slot| {
        let remaining = slot.get();
        if remaining == 0 {
            false
        } else if remaining == 1 {
            slot.set(0);
            true
        } else {
            slot.set(remaining - 1);
            false
        }
    })
}

#[cfg(not(test))]
fn fail_root_detach() -> bool {
    false
}

#[cfg(test)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code, unused_imports))]
mod tests {
    use super::*;
    use std::{
        fs as stdfs,
        io::Cursor,
        os::unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, symlink},
        },
    };
    use tempfile::TempDir;

    struct Fixture {
        _temp: TempDir,
        executable: PathBuf,
        data: PathBuf,
        cache: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = TempDir::new().expect("temp");
            let bin = temp.path().join("bin");
            let data = temp.path().join("data/pangopup");
            let cache = temp.path().join("cache/pangopup");
            stdfs::create_dir_all(&bin).expect("bin");
            stdfs::create_dir_all(&data).expect("data");
            stdfs::create_dir_all(&cache).expect("cache");
            use std::os::unix::fs::PermissionsExt;
            stdfs::set_permissions(
                data.parent().expect("data parent"),
                stdfs::Permissions::from_mode(0o700),
            )
            .expect("private data parent");
            stdfs::set_permissions(
                cache.parent().expect("cache parent"),
                stdfs::Permissions::from_mode(0o700),
            )
            .expect("private cache parent");
            let executable = bin.join("pangopup");
            stdfs::write(&executable, b"fixture executable").expect("executable");
            Self {
                _temp: temp,
                executable,
                data,
                cache,
            }
        }

        fn execute(
            &self,
            options: Options,
            answer: &str,
            terminal: bool,
        ) -> Result<(Vec<u8>, String), UninstallError> {
            let mut input = Cursor::new(answer.as_bytes());
            let mut error = Vec::new();
            let output = execute(
                options,
                self.executable.clone(),
                self.data.clone(),
                self.cache.clone(),
                terminal,
                &mut input,
                &mut error,
            )?;
            Ok((output, String::from_utf8(error).expect("stderr")))
        }
    }

    fn install_hook(point: TestHookPoint, hook: impl FnOnce() + 'static) {
        TEST_HOOK.with(|slot| {
            assert!(slot.borrow().is_none(), "test hook already installed");
            *slot.borrow_mut() = Some((point, Box::new(hook)));
        });
    }

    #[test]
    fn option_grammar_is_exact() {
        assert_eq!(
            parse(&[]).expect("none"),
            Options {
                full: false,
                yes: false
            }
        );
        assert_eq!(
            parse(&["--full".into(), "--yes".into()]).expect("both"),
            Options {
                full: true,
                yes: true
            }
        );
        assert_eq!(
            parse(&["--yes".into(), "--full".into()]).expect("reverse"),
            Options {
                full: true,
                yes: true
            }
        );
        for raw in [
            ["--full", "--full"].as_slice(),
            ["--yes", "--yes"].as_slice(),
            ["--force"].as_slice(),
            ["--full=yes"].as_slice(),
        ] {
            assert_eq!(
                parse(&raw.iter().map(OsString::from).collect::<Vec<_>>())
                    .expect_err("usage")
                    .code,
                "CLI_USAGE"
            );
        }
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn code_only_unlinks_executable_last_and_preserves_roots() {
        let fixture = Fixture::new();
        stdfs::write(fixture.data.join("active.json"), b"x").expect("data");
        let (output, stderr) = fixture
            .execute(
                Options {
                    full: false,
                    yes: true,
                },
                "",
                false,
            )
            .expect("remove");
        assert!(!fixture.executable.exists());
        assert!(fixture.data.join("active.json").exists());
        assert!(fixture.cache.exists());
        assert!(stderr.contains("Pangopup uninstall plan:"));
        let text = String::from_utf8(output).expect("json");
        assert!(text.contains("\"scope\":\"code_only\""));
        assert!(text.contains("\"state\":\"preserved\""));
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn full_yes_removes_roots_then_executable_and_reports_absence() {
        let fixture = Fixture::new();
        stdfs::create_dir_all(fixture.data.join("bundles/x/bundle")).expect("tree");
        stdfs::write(fixture.data.join("bundles/x/bundle/scores.pgi"), b"x").expect("file");
        stdfs::remove_dir(&fixture.cache).expect("missing cache");
        let (output, _) = fixture
            .execute(
                Options {
                    full: true,
                    yes: true,
                },
                "",
                false,
            )
            .expect("remove");
        assert!(!fixture.executable.exists());
        assert!(!fixture.data.exists());
        assert!(!fixture.cache.exists());
        let text = String::from_utf8(output).expect("json");
        assert!(text.contains("\"scope\":\"full\""));
        assert!(text.contains(&format!(
            "\"cache\":{{\"path\":\"{}\",\"state\":\"absent\"}}",
            fixture.cache.display()
        )));
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn both_absent_roots_are_reported_without_being_created() {
        let fixture = Fixture::new();
        stdfs::remove_dir(&fixture.data).expect("missing data");
        stdfs::remove_dir(&fixture.cache).expect("missing cache");
        let (output, _) = fixture
            .execute(
                Options {
                    full: true,
                    yes: true,
                },
                "",
                false,
            )
            .expect("remove code");
        assert!(!fixture.data.exists());
        assert!(!fixture.cache.exists());
        assert_eq!(
            String::from_utf8(output)
                .expect("json")
                .matches("\"state\":\"absent\"")
                .count(),
            2
        );
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn interactive_choices_and_full_confirmation_are_exact() {
        for (options, answer, expected_scope) in [
            (
                Options {
                    full: false,
                    yes: false,
                },
                "1\n",
                "code_only",
            ),
            (
                Options {
                    full: false,
                    yes: false,
                },
                "2\n",
                "full",
            ),
            (
                Options {
                    full: true,
                    yes: false,
                },
                "yes\n",
                "full",
            ),
        ] {
            let fixture = Fixture::new();
            let (output, _) = fixture.execute(options, answer, true).expect("choice");
            assert!(
                String::from_utf8(output)
                    .expect("json")
                    .contains(&format!("\"scope\":\"{expected_scope}\""))
            );
        }
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn cancellation_is_successful_and_does_not_create_locks_or_change_root() {
        use std::os::unix::fs::MetadataExt;
        let fixture = Fixture::new();
        let before = stdfs::metadata(&fixture.data).expect("metadata");
        let (output, _) = fixture
            .execute(
                Options {
                    full: false,
                    yes: false,
                },
                "3\n",
                true,
            )
            .expect("cancel");
        let after = stdfs::metadata(&fixture.data).expect("metadata");
        assert!(fixture.executable.exists());
        assert_eq!(stdfs::read_dir(&fixture.data).expect("entries").count(), 0);
        assert_eq!(
            (before.ino(), before.mtime(), before.mtime_nsec()),
            (after.ino(), after.mtime(), after.mtime_nsec())
        );
        assert!(
            String::from_utf8(output)
                .expect("json")
                .contains("\"status\":\"cancelled\"")
        );

        let full = Fixture::new();
        let (output, _) = full
            .execute(
                Options {
                    full: true,
                    yes: false,
                },
                "no\n",
                true,
            )
            .expect("cancel full");
        assert!(full.executable.exists());
        assert!(
            String::from_utf8(output)
                .expect("json")
                .contains("\"scope\":\"none\"")
        );
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn nonterminal_and_invalid_or_eof_input_never_mutate() {
        for (terminal, answer, code) in [
            (false, "1\n", "UNINSTALL_NONINTERACTIVE"),
            (true, "", "UNINSTALL_UNSAFE"),
            (true, " 1\n", "UNINSTALL_UNSAFE"),
            (true, "4\n", "UNINSTALL_UNSAFE"),
        ] {
            let fixture = Fixture::new();
            let error = fixture
                .execute(
                    Options {
                        full: false,
                        yes: false,
                    },
                    answer,
                    terminal,
                )
                .expect_err("reject");
            assert_eq!(error.code, code);
            assert!(fixture.executable.exists());
        }
    }

    #[test]
    fn unknown_top_level_root_and_hard_link_are_rejected() {
        let fixture = Fixture::new();
        stdfs::write(fixture.data.join("foreign"), b"x").expect("foreign");
        assert_eq!(
            fixture
                .execute(
                    Options {
                        full: true,
                        yes: true
                    },
                    "",
                    false
                )
                .expect_err("unknown")
                .code,
            "UNINSTALL_UNSAFE"
        );
        assert!(fixture.executable.exists());

        let hard = Fixture::new();
        stdfs::write(hard.data.join("active.json"), b"x").expect("file");
        stdfs::hard_link(
            hard.data.join("active.json"),
            hard._temp.path().join("outside-link"),
        )
        .expect("link");
        assert_eq!(
            hard.execute(
                Options {
                    full: true,
                    yes: true
                },
                "",
                false
            )
            .expect_err("hard link")
            .code,
            "UNINSTALL_UNSAFE"
        );
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn root_and_nested_symlinks_never_delete_outside_sentinels() {
        let root_link = Fixture::new();
        let outside = root_link._temp.path().join("outside");
        stdfs::create_dir(&outside).expect("outside");
        stdfs::write(outside.join("sentinel"), b"safe").expect("sentinel");
        stdfs::remove_dir(&root_link.data).expect("remove root");
        symlink(&outside, &root_link.data).expect("root link");
        assert_eq!(
            root_link
                .execute(
                    Options {
                        full: true,
                        yes: true
                    },
                    "",
                    false
                )
                .expect_err("root link")
                .code,
            "UNINSTALL_UNSAFE"
        );
        assert!(outside.join("sentinel").exists());

        let nested = Fixture::new();
        let outside = nested._temp.path().join("outside");
        stdfs::create_dir(&outside).expect("outside");
        stdfs::write(outside.join("sentinel"), b"safe").expect("sentinel");
        stdfs::create_dir(nested.data.join("bundles")).expect("bundles");
        symlink(&outside, nested.data.join("bundles/outside-link")).expect("link");
        nested
            .execute(
                Options {
                    full: true,
                    yes: true,
                },
                "",
                false,
            )
            .expect("unlink link");
        assert!(outside.join("sentinel").exists());
    }

    #[test]
    fn special_entry_is_rejected() {
        let fixture = Fixture::new();
        let fifo = fixture.cache.join("model-results.sqlite3");
        let path = std::ffi::CString::new(fifo.as_os_str().as_bytes()).expect("path");
        // SAFETY: path is a valid NUL-terminated path and mode is conventional.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        assert_eq!(
            fixture
                .execute(
                    Options {
                        full: true,
                        yes: true
                    },
                    "",
                    false
                )
                .expect_err("fifo")
                .code,
            "UNINSTALL_UNSAFE"
        );
        assert!(fixture.executable.exists());
    }

    #[test]
    fn equal_nested_and_executable_ancestry_roots_are_rejected() {
        let fixture = Fixture::new();
        assert!(
            reject_relationships(
                &fixture.executable,
                fixture.executable.parent().expect("parent"),
                &fixture.data,
                &fixture.data
            )
            .is_err()
        );
        assert!(
            reject_relationships(
                &fixture.executable,
                fixture.executable.parent().expect("parent"),
                fixture.executable.parent().expect("parent"),
                &fixture.cache
            )
            .is_err()
        );
        assert!(
            reject_relationships(
                &fixture.executable,
                fixture.executable.parent().expect("parent"),
                &fixture.data,
                &fixture.data.join("nested")
            )
            .is_err()
        );
    }

    #[test]
    fn parent_aliases_and_protected_roots_are_rejected() {
        assert_eq!(
            reject_lexical_alias(Path::new("/tmp/parent/../pangopup"), "root")
                .expect_err("parent alias")
                .code,
            "UNINSTALL_UNSAFE"
        );
        assert_eq!(
            reject_lexical_alias(Path::new("relative/pangopup"), "root")
                .expect_err("relative")
                .code,
            "UNINSTALL_UNSAFE"
        );
        assert!(
            reject_reserved(
                Path::new("/"),
                Path::new("/safe/pangopup"),
                Path::new("/bin")
            )
            .is_err()
        );
        assert!(
            reject_reserved(
                Path::new("/safe/pangopup"),
                Path::new("/bin"),
                Path::new("/bin")
            )
            .is_err()
        );
    }

    #[test]
    fn nonremovable_executable_parent_is_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = Fixture::new();
        let parent = fixture.executable.parent().expect("parent");
        stdfs::set_permissions(parent, stdfs::Permissions::from_mode(0o500)).expect("restrict");
        let error = inspect(
            fixture.executable.clone(),
            fixture.data.clone(),
            fixture.cache.clone(),
        )
        .expect_err("parent permissions");
        assert_eq!(error.code, "UNINSTALL_UNSAFE");
        stdfs::set_permissions(parent, stdfs::Permissions::from_mode(0o700)).expect("restore");
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn held_install_or_sync_authority_blocks_full_removal() {
        for lock in [".sync.lock", ".install.lock"] {
            let fixture = Fixture::new();
            let path = fixture.data.join(lock);
            let owner = stdfs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)
                .expect("lock file");
            // SAFETY: flock operates on the live fixture descriptor.
            assert_eq!(
                unsafe { libc::flock(owner.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
                0
            );
            let error = fixture
                .execute(
                    Options {
                        full: true,
                        yes: true,
                    },
                    "",
                    false,
                )
                .expect_err("busy");
            assert_eq!(error.code, "UNINSTALL_BUSY");
            assert!(fixture.executable.exists());
        }
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn admitted_root_swap_is_detected_before_replacement_mutation() {
        let fixture = Fixture::new();
        let admitted = fixture.data.clone();
        let moved = fixture._temp.path().join("admitted-data");
        let replacement = admitted.clone();
        install_hook(TestHookPoint::AuthorityRootOpened, move || {
            stdfs::rename(&admitted, &moved).expect("move admitted root");
            stdfs::create_dir(&replacement).expect("replacement root");
            stdfs::write(replacement.join("sentinel"), b"replacement").expect("sentinel");
        });
        let error = fixture
            .execute(
                Options {
                    full: true,
                    yes: true,
                },
                "",
                false,
            )
            .expect_err("swapped root");
        assert_eq!(error.code, "UNINSTALL_UNSAFE");
        assert_eq!(
            stdfs::read(fixture.data.join("sentinel")).expect("replacement survives"),
            b"replacement"
        );
        assert!(!fixture.data.join(".sync.lock").exists());
        assert!(!fixture.data.join(".install.lock").exists());
        assert!(fixture.executable.exists());
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn root_swap_between_named_stat_and_open_is_rejected() {
        let fixture = Fixture::new();
        stdfs::remove_dir(&fixture.data).expect("data absent avoids first removal");
        let admitted = fixture.cache.clone();
        let moved = fixture._temp.path().join("admitted-cache");
        let replacement = admitted.clone();
        install_hook(TestHookPoint::PublicRootStated, move || {
            stdfs::rename(&admitted, &moved).expect("move admitted root");
            stdfs::create_dir(&replacement).expect("replacement root");
            stdfs::write(replacement.join("sentinel"), b"replacement").expect("sentinel");
        });
        let error = fixture
            .execute(
                Options {
                    full: true,
                    yes: true,
                },
                "",
                false,
            )
            .expect_err("swapped cache");
        assert_eq!(error.code, "UNINSTALL_UNSAFE");
        assert_eq!(
            stdfs::read(fixture.cache.join("sentinel")).expect("replacement survives"),
            b"replacement"
        );
        assert!(fixture.executable.exists());
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn public_root_stays_blocked_until_destructive_traversal_finishes() {
        use std::sync::{Arc, Barrier, Mutex};
        let fixture = Fixture::new();
        let start = Arc::new(Barrier::new(2));
        let finish = Arc::new(Barrier::new(2));
        let acquired = Arc::new(Mutex::new(None));
        let worker_path = fixture.data.join(".sync.lock");
        let worker_start = Arc::clone(&start);
        let worker_finish = Arc::clone(&finish);
        let worker_acquired = Arc::clone(&acquired);
        let worker = std::thread::spawn(move || {
            worker_start.wait();
            let opened = stdfs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(worker_path)
                .is_ok();
            *worker_acquired.lock().expect("result") = Some(opened);
            worker_finish.wait();
        });
        install_hook(TestHookPoint::RootDetached, move || {
            start.wait();
            finish.wait();
        });
        fixture
            .execute(
                Options {
                    full: true,
                    yes: true,
                },
                "",
                false,
            )
            .expect("remove");
        worker.join().expect("worker");
        assert_eq!(*acquired.lock().expect("result"), Some(false));
        assert!(!fixture.data.exists());
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn present_data_blocker_prevents_fresh_authority_during_cache_removal() {
        let fixture = Fixture::new();
        let attempted = std::sync::Arc::new(std::sync::Mutex::new(None));
        let result = std::sync::Arc::clone(&attempted);
        let lock = fixture.data.join(".sync.lock");
        install_hook(TestHookPoint::BeforeCacheRemoval, move || {
            let acquired = stdfs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(lock)
                .is_ok();
            *result.lock().expect("attempt") = Some(acquired);
        });
        fixture
            .execute(
                Options {
                    full: true,
                    yes: true,
                },
                "",
                false,
            )
            .expect("remove");
        assert_eq!(*attempted.lock().expect("attempt"), Some(false));
        assert!(!fixture.data.exists());
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn absent_data_blocker_prevents_root_creation_during_cache_removal() {
        let fixture = Fixture::new();
        stdfs::remove_dir(&fixture.data).expect("data absent");
        let attempted = std::sync::Arc::new(std::sync::Mutex::new(None));
        let result = std::sync::Arc::clone(&attempted);
        let data = fixture.data.clone();
        install_hook(TestHookPoint::BeforeCacheRemoval, move || {
            let created = stdfs::create_dir(&data).is_ok();
            *result.lock().expect("attempt") = Some(created);
        });
        fixture
            .execute(
                Options {
                    full: true,
                    yes: true,
                },
                "",
                false,
            )
            .expect("remove");
        assert_eq!(*attempted.lock().expect("attempt"), Some(false));
        assert!(!fixture.data.exists());
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn absent_data_blocker_is_cleaned_when_cache_is_replaced_after_confirmation() {
        let fixture = Fixture::new();
        stdfs::remove_dir(&fixture.data).expect("data absent");
        let admitted_cache = fixture.cache.clone();
        let moved_cache = fixture._temp.path().join("admitted-cache-after-blocker");
        let replacement_cache = fixture.cache.clone();
        install_hook(TestHookPoint::BeforeCacheRemoval, move || {
            stdfs::rename(&admitted_cache, &moved_cache).expect("move admitted cache");
            stdfs::create_dir(&replacement_cache).expect("replacement cache");
            stdfs::write(replacement_cache.join("sentinel"), b"replacement")
                .expect("replacement sentinel");
        });

        let error = fixture
            .execute(
                Options {
                    full: true,
                    yes: true,
                },
                "",
                false,
            )
            .expect_err("cache replacement rejected");

        assert_eq!(error.code, "UNINSTALL_UNSAFE");
        assert!(
            !fixture.data.exists(),
            "owned absent-data blocker is cleaned"
        );
        assert_eq!(
            stdfs::read(fixture.cache.join("sentinel")).expect("replacement survives"),
            b"replacement"
        );
        assert!(fixture.executable.exists(), "executable remains on failure");
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn replaced_public_blocker_is_never_blindly_unlinked() {
        let fixture = Fixture::new();
        let blocker = fixture.data.clone();
        let moved = fixture._temp.path().join("held-blocker");
        let replacement = blocker.clone();
        install_hook(TestHookPoint::BlockerFinalizing, move || {
            stdfs::rename(&blocker, &moved).expect("move blocker");
            stdfs::write(&replacement, b"replacement").expect("replacement");
        });
        let error = fixture
            .execute(
                Options {
                    full: true,
                    yes: true,
                },
                "",
                false,
            )
            .expect_err("replacement rejected");
        assert_eq!(error.code, "UNINSTALL_UNSAFE");
        assert_eq!(
            stdfs::read(&fixture.data).expect("replacement survives"),
            b"replacement"
        );
        assert!(fixture.executable.exists());
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn injected_later_phase_failure_preserves_executable() {
        let fixture = Fixture::new();
        stdfs::write(fixture.cache.join("model-results.sqlite3"), b"cache").expect("cache");
        FAIL_ROOT_DETACH_CALL.with(|slot| slot.set(2));
        let error = fixture
            .execute(
                Options {
                    full: true,
                    yes: true,
                },
                "",
                false,
            )
            .expect_err("injected cache failure");
        assert_eq!(error.code, "UNINSTALL_IO");
        assert!(
            !fixture.data.exists(),
            "earlier phase may already be removed"
        );
        assert!(fixture.cache.exists(), "failed phase remains available");
        assert!(fixture.executable.exists(), "executable is always last");
    }

    // Exercises Linux-only asset installation; other platforms get the
    // documented refusal instead.
    #[cfg(target_os = "linux")]
    #[test]
    fn sparse_payload_preflight_is_metadata_only() {
        let fixture = Fixture::new();
        stdfs::create_dir(fixture.data.join("bundles")).expect("bundles");
        let file = stdfs::File::create(fixture.data.join("bundles/large")).expect("large");
        file.set_len(32 * 1024 * 1024 * 1024).expect("sparse");
        let plan = inspect(
            fixture.executable.clone(),
            fixture.data.clone(),
            fixture.cache.clone(),
        )
        .expect("metadata inspection");
        assert!(plan.data.identity.is_some());
        assert_eq!(
            stdfs::metadata(fixture.data.join("bundles/large"))
                .expect("metadata")
                .blocks(),
            0
        );
    }
}
