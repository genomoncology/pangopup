//! The one place a test may name the shipped executable.
//!
//! `tests/cli-spawn-cache-isolation.sh` refuses any other Rust source under
//! this repository that names `CARGO_BIN_EXE_pangopup`, so every spawn in the
//! suite is built here. That is what makes it worth redirecting here.
//!
//! The model cache is on by default and its directory comes from
//! `XDG_CACHE_HOME`, or from `HOME` when that is unset. A spawn that inherits
//! either reaches the cache file of whoever is running the suite: it fills that
//! file with rows scored from miniature fixtures, and since a cache whose
//! recorded setup no longer matches is discarded rather than ignored, it
//! destroys that file rather than only growing it. So every command built here
//! gets a private cache home of its own, and that home stays alive until the
//! process it belongs to has exited.
//!
//! `spawn_isolation.rs` proves that from the other side.

#![allow(dead_code)]

use std::{
    ffi::OsStr,
    fs, io,
    ops::{Deref, DerefMut},
    os::unix::fs::PermissionsExt,
    process::{Child, Command, Output, Stdio},
};

use tempfile::TempDir;

/// A command against the shipped executable, together with the private cache
/// home it runs against.
pub struct Spawn {
    pub command: Command,
    home: TempDir,
}

/// A running child, holding its cache home open until the child has exited.
pub struct Running {
    child: Child,
    _home: TempDir,
}

/// Build a command against the shipped executable, redirected at a cache home
/// of its own.
///
/// A fresh home per command also keeps one test from filling, evicting or
/// discarding the cache another test is reading.
pub fn pangopup() -> Spawn {
    let home = tempfile::tempdir().expect("private cache home");
    fs::set_permissions(home.path(), fs::Permissions::from_mode(0o700)).expect("private home");
    let mut command = Command::new(env!("CARGO_BIN_EXE_pangopup"));
    command
        .env("XDG_CACHE_HOME", home.path())
        .env("HOME", home.path());
    Spawn { command, home }
}

impl Spawn {
    #[must_use]
    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.command.arg(arg);
        self
    }

    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command.args(args);
        self
    }

    /// Set one variable on the command. A caller that owns a directory of its
    /// own may point `XDG_CACHE_HOME` or `HOME` at it here; what it may not do
    /// is leave them inherited.
    #[must_use]
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.command.env(key, value);
        self
    }

    #[must_use]
    pub fn stdout(mut self, configuration: impl Into<Stdio>) -> Self {
        self.command.stdout(configuration);
        self
    }

    #[must_use]
    pub fn stderr(mut self, configuration: impl Into<Stdio>) -> Self {
        self.command.stderr(configuration);
        self
    }

    /// Run to completion. The cache home outlives the run and is removed after.
    pub fn output(mut self) -> io::Result<Output> {
        self.command.output()
    }

    /// Start a child that outlives this call, keeping its cache home open.
    pub fn spawn(mut self) -> io::Result<Running> {
        Ok(Running {
            child: self.command.spawn()?,
            _home: self.home,
        })
    }
}

/// The version the shipped executable reports through `--version`.
///
/// Ticket 0054 put that version on every command-line result line, so a test
/// that pins printed bytes has to know it. Reading it from the executable
/// keeps the pin alive across a release: a literal would have to be rewritten
/// every time the version moves, and a stale literal would pass for the wrong
/// reason.
pub fn software_version() -> String {
    let output = pangopup()
        .arg("--version")
        .output()
        .expect("run pangopup --version");
    assert!(output.status.success(), "--version failed");
    let line = String::from_utf8(output.stdout).expect("UTF-8 version line");
    line.trim()
        .strip_prefix("pangopup ")
        .expect("the version line names the tool")
        .to_owned()
}

/// The exact bytes ticket 0054 adds to a result line, for the version the
/// shipped executable reports.
pub fn software_version_field() -> String {
    format!(",\"software_version\":\"{}\"", software_version())
}

/// Remove every occurrence of `needle` from `bytes` and report how many were
/// removed. `needle` is an exact byte string, so nothing wider than it can be
/// taken out and a comparison that follows still sees every other difference.
pub fn strip_exact(bytes: &[u8], needle: &[u8]) -> (Vec<u8>, usize) {
    assert!(!needle.is_empty(), "an exact strip needs a needle");
    let mut kept = Vec::with_capacity(bytes.len());
    let mut removed = 0;
    let mut rest = bytes;
    while let Some(at) = rest
        .windows(needle.len())
        .position(|window| window == needle)
    {
        kept.extend_from_slice(&rest[..at]);
        rest = &rest[at + needle.len()..];
        removed += 1;
    }
    kept.extend_from_slice(rest);
    (kept, removed)
}

impl Deref for Running {
    type Target = Child;

    fn deref(&self) -> &Child {
        &self.child
    }
}

impl DerefMut for Running {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.child
    }
}
