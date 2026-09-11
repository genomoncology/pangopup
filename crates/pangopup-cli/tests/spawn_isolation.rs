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

/// A path in the repository, spelled the way `model_routing.rs` spells it.
fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// The miniature modelled lookup, which is the run that opens the model cache.
/// A precomputed lookup never opens it, so a bad cache path set on such a run
/// is never read and proves nothing about whether the value arrived.
fn model_only_args() -> Vec<String> {
    let mut args = vec![
        "lookup".to_owned(),
        "--model-only".to_owned(),
        "--variant".to_owned(),
        "GRCh38:chr1:5051:A:C".to_owned(),
    ];
    for (flag, fixture) in [
        (
            "--model-bundle",
            "tests/fixtures/pangolin-model-kernel-mini/bundle",
        ),
        (
            "--reference-bundle",
            "tests/fixtures/reference-route-test/bundle",
        ),
        ("--mask", "tests/fixtures/route-mask/domains.pgm"),
    ] {
        args.push(flag.to_owned());
        args.push(repository_path(fixture).display().to_string());
    }
    args
}

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

/// The three variables that name a cache location outright, in the order the
/// product reads them. `PANGOPUP_MODEL_CACHE` names the model cache file
/// itself and is read *ahead* of `XDG_CACHE_HOME` and `HOME`, so redirecting
/// those two leaves a spawn reaching the file this variable names.
/// `PANGOPUP_CACHE_DIR` and `PANGOPUP_DATA_DIR` name the download cache and
/// the installed bundle directory the same way.
const CACHE_VARIABLES: [&str; 3] = [
    "PANGOPUP_MODEL_CACHE",
    "PANGOPUP_CACHE_DIR",
    "PANGOPUP_DATA_DIR",
];

/// What the helper-built command does with `name`: `None` when it leaves the
/// inherited value alone, `Some(None)` when it removes it, `Some(Some(value))`
/// when it sets one.
fn disposition<'a>(spawn: &'a support::Spawn, name: &str) -> Option<Option<&'a OsStr>> {
    let wanted = OsStr::new(name);
    spawn
        .command
        .get_envs()
        .find(|(key, _)| *key == wanted)
        .map(|(_, value)| value)
}

/// A command the helper built must not resolve a cache the suite's own
/// environment names.
///
/// Redirecting `XDG_CACHE_HOME` and `HOME` is not enough. Each variable here
/// names a cache location outright and is read ahead of both, so an operator
/// who exports one runs the whole suite against the file it names -- and since
/// a cache whose recorded setup no longer matches is discarded rather than
/// ignored, the run destroys that file rather than only growing it. The helper
/// has to drop them, not overwrite them: an empty or placeholder value is a
/// value the product still reads.
#[test]
fn a_helper_spawn_forgets_the_cache_variables_it_inherited() {
    let spawn = support::pangopup();
    for name in CACHE_VARIABLES {
        match disposition(&spawn, name) {
            Some(None) => {}
            Some(Some(value)) => panic!(
                "the helper sets {name} to {:?} instead of removing it, so a spawn still reads a \
                 cache location from a variable the suite inherited",
                value
            ),
            None => panic!(
                "the helper leaves {name} inherited, so a spawn reaches the cache location \
                 whoever is running the suite exported and can destroy that file"
            ),
        }
    }
}

/// Clearing the inherited value must not take the variable away from a caller
/// that owns a directory of its own.
///
/// `model_routing.rs` sets `PANGOPUP_MODEL_CACHE` on a command to prove the
/// product rejects a relative path, and the product itself keeps honouring all
/// three for an operator running it. What the helper drops is the inherited
/// value; what a caller sets after it still reaches the child.
#[test]
fn a_caller_may_still_name_a_cache_location_of_its_own() {
    for name in CACHE_VARIABLES {
        let spawn = support::pangopup().env(name, "/nowhere/a-caller-owns");
        assert_eq!(
            disposition(&spawn, name),
            Some(Some(OsStr::new("/nowhere/a-caller-owns"))),
            "a caller that sets {name} after the helper must reach the child with it, or the \
             product's own variables become untestable"
        );
    }

    // Reading the command proves the helper asked for the value to go through.
    // Running proves it arrived: a helper that dropped the three on the way out
    // instead of on the way in would satisfy every assertion above and take the
    // product's own variables away all the same. A modelled lookup opens the
    // model cache, so a relative path has to come back refused by name.
    let refused = support::pangopup()
        .args(model_only_args())
        .env("PANGOPUP_MODEL_CACHE", "relative/is/invalid")
        .output()
        .expect("run pangopup");
    let reported = format!(
        "{}{}",
        String::from_utf8_lossy(&refused.stdout),
        String::from_utf8_lossy(&refused.stderr)
    );
    assert!(
        !refused.status.success(),
        "a modelled lookup whose PANGOPUP_MODEL_CACHE names a relative path must be refused, and \
         this run succeeded: {reported}"
    );
    assert!(
        reported.contains("model cache path must be an absolute file path"),
        "the product never read the PANGOPUP_MODEL_CACHE this caller set, so the helper takes the \
         variable away from a caller that owns a location of its own: {reported}"
    );
}
