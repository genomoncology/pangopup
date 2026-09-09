//! The one place a test may name the shipped executable.
//!
//! `tests/cli-spawn-cache-isolation.sh` refuses any other Rust source under
//! this repository that names `CARGO_BIN_EXE_pangopup`, so every spawn in the
//! suite is built here. That is what makes it worth redirecting here.
//!
//! This is a seam, not the behaviour. It spawns the executable and redirects
//! nothing, so `spawn_isolation.rs` fails against it. Giving each spawn a cache
//! home of its own is the work.

#![allow(dead_code)]

use std::process::Command;

/// A command against the shipped executable, together with whatever that
/// command needs kept alive until the process it starts has exited.
pub struct Spawn {
    pub command: Command,
}

/// Build a command against the shipped executable.
pub fn pangopup() -> Spawn {
    Spawn {
        command: Command::new(env!("CARGO_BIN_EXE_pangopup")),
    }
}
