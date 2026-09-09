//! The shared spawn helper redirects, so routing through it means something.
//!
//! `tests/cli-spawn-cache-isolation.sh` holds every test to spawning the
//! shipped executable through one helper. That check reads the calling side of
//! the boundary: it proves the spawns go through the helper and nothing else.
//! Should the helper stop redirecting, every one of those spawns would reach
//! the model cache of whoever runs the suite and the static check would still
//! be green, because the spawns would still be going through the helper.
//!
//! This reads the other side. Together they are the rule.

mod support;

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

/// The default model cache a run resolves from one environment, spelled the way
/// `resolve_model_cache_options` spells it: `XDG_CACHE_HOME` when it is set,
/// otherwise `HOME/.cache`.
fn default_model_cache(cache_home: Option<&OsStr>, home: Option<&OsStr>) -> PathBuf {
    let root = match (cache_home, home) {
        (Some(root), _) => PathBuf::from(root),
        (None, Some(home)) => Path::new(home).join(".cache"),
        (None, None) => {
            panic!("neither XDG_CACHE_HOME nor HOME is set, so no default cache exists")
        }
    };
    root.join("pangopup/model-results.sqlite3")
}

/// A command the helper built must not resolve the suite's own model cache.
///
/// The model cache is on by default and its directory comes from the
/// environment, so a command that inherits `XDG_CACHE_HOME` or `HOME` writes
/// rows scored from miniature fixtures into the operator's own cache file --
/// and since a cache whose recorded setup no longer matches is discarded
/// rather than ignored, it destroys that file rather than only growing it.
#[test]
fn a_helper_spawn_resolves_a_cache_of_its_own() {
    let spawn = support::pangopup();

    let mut cache_home = None;
    let mut home = None;
    for (key, value) in spawn.command.get_envs() {
        if key == OsStr::new("XDG_CACHE_HOME") {
            cache_home = value;
        } else if key == OsStr::new("HOME") {
            home = value;
        }
    }

    let cache_home = cache_home.expect(
        "the helper must set XDG_CACHE_HOME, or a spawn reads the cache directory of whoever is \
         running the suite",
    );
    let home = home.expect(
        "the helper must set HOME, or a spawn whose XDG_CACHE_HOME is later cleared falls back to \
         the home directory of whoever is running the suite",
    );
    assert!(
        Path::new(cache_home).is_dir(),
        "the helper's XDG_CACHE_HOME must be a directory that exists, and {} is not",
        Path::new(cache_home).display()
    );

    // The two variables together decide one path, so compare the path rather
    // than the variables: that is what a run actually opens.
    let private = default_model_cache(Some(cache_home), Some(home));
    let ambient = default_model_cache(
        std::env::var_os("XDG_CACHE_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    );
    assert_ne!(
        private,
        ambient,
        "a command the helper built resolves {}, the model cache of whoever is running the suite",
        ambient.display()
    );
}
