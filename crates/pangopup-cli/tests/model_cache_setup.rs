//! The model cache is a shortcut, not a store.
//!
//! A cached row answers a request only when the setup that filled the file is
//! the setup asking. The setup is the running software version together with
//! the model, reference and mask assets the run reaches. It is not the
//! effective CPU policy: ticket 0040 measured that no thread or worker setting
//! moves any score, position, status, reason or provenance field.
//!
//! Every test here drives the shipped executable and then reads the cache file
//! back, so nothing depends on an internal signature. `XDG_CACHE_HOME` and
//! `HOME` are redirected into a private temporary directory on every run, so
//! no test can reach an operator's real cache.

#![cfg(unix)]

use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// Two variants the miniature model route scores. A test fills the cache with
/// both and then asks for one, so a surviving row for the other is visible as
/// an entry count that did not fall.
const FIRST_VARIANT: &str = "GRCh38:chr1:5051:A:AC";
const SECOND_VARIANT: &str = "GRCh38:chr1:5051:A:C";

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// One private scratch directory. Its permissions satisfy the cache's own
/// private-directory rule, and it stands in for `HOME` so a run that resolves
/// the default cache cannot escape it.
fn private_temp() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("temp");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("private temp");
    temp
}

/// The three assets one modelled run reaches. Each one can be replaced by a
/// copy whose recorded identity differs and whose scored answer does not.
#[derive(Clone)]
struct Setup {
    model: PathBuf,
    reference: PathBuf,
    mask: PathBuf,
}

impl Setup {
    fn fixtures() -> Self {
        Self {
            model: repository_path("tests/fixtures/pangolin-model-kernel-mini/bundle"),
            reference: repository_path("tests/fixtures/reference-route-test/bundle"),
            mask: repository_path("tests/fixtures/route-mask/domains.pgm"),
        }
    }

    /// A copy of the model bundle that declares a different converter
    /// environment. The ONNX member is byte-identical, so the answer cannot
    /// move; only the model bundle identity does.
    fn with_other_model(&self, scratch: &Path) -> Self {
        let model = scratch.join("other-model");
        copy_tree(&self.model, &model);
        edit_manifest(&model.join("manifest.json"), |manifest| {
            manifest["conversion"]["environment"]["numpy"] = Value::String("0.0.0".to_owned());
        });
        Self {
            model,
            ..self.clone()
        }
    }

    /// A copy of the reference bundle that declares a different builder
    /// version. Every sequence member is byte-identical, so the answer cannot
    /// move; only the reference bundle identity does.
    fn with_other_reference(&self, scratch: &Path) -> Self {
        let reference = scratch.join("other-reference");
        copy_tree(&self.reference, &reference);
        edit_manifest(&reference.join("manifest.json"), |manifest| {
            manifest["builder"]["version"] = Value::String("0.0.0".to_owned());
        });
        Self {
            reference,
            ..self.clone()
        }
    }

    /// A different mask asset. The miniature GENCODE mask carries different
    /// bytes and a different digest from the route mask.
    fn with_other_mask(&self) -> Self {
        Self {
            mask: repository_path("tests/fixtures/gencode-mask-mini/domains.pgm"),
            ..self.clone()
        }
    }

    fn args(&self) -> Vec<String> {
        vec![
            "--model-bundle".to_owned(),
            self.model.display().to_string(),
            "--reference-bundle".to_owned(),
            self.reference.display().to_string(),
            "--mask".to_owned(),
            self.mask.display().to_string(),
        ]
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create copy");
    for entry in fs::read_dir(from).expect("read fixture") {
        let entry = entry.expect("fixture entry");
        fs::copy(entry.path(), to.join(entry.file_name())).expect("copy fixture member");
    }
}

fn edit_manifest(path: &Path, edit: impl FnOnce(&mut Value)) {
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(path).expect("read manifest")).expect("manifest JSON");
    edit(&mut manifest);
    fs::write(path, serde_json::to_vec(&manifest).expect("manifest bytes"))
        .expect("write manifest");
}

/// Score variants through the shipped executable against the default cache
/// under `cache_home`. The default cache is the one this ticket is about: it
/// is on by default and it survives an upgrade in place.
fn score(setup: &Setup, cache_home: &Path, variants: &[&str]) -> Output {
    let mut args = vec!["lookup".to_owned(), "--model-only".to_owned()];
    for variant in variants {
        args.push("--variant".to_owned());
        args.push((*variant).to_owned());
    }
    args.extend(setup.args());
    Command::new(env!("CARGO_BIN_EXE_pangopup"))
        .args(&args)
        .env("XDG_CACHE_HOME", cache_home)
        .env("HOME", cache_home)
        .output()
        .expect("run pangopup")
}

fn default_cache_path(cache_home: &Path) -> PathBuf {
    cache_home.join("pangopup/model-results.sqlite3")
}

fn succeeded(output: &Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).expect("UTF-8 stdout")
}

fn provenance(output: &Output) -> Value {
    let line = succeeded(output);
    let record: Value = serde_json::from_str(line.lines().next().expect("one score line"))
        .expect("score line is JSON");
    record["provenance"].clone()
}

fn open_cache(path: &Path) -> Option<rusqlite::Connection> {
    path.is_file()
        .then(|| rusqlite::Connection::open(path).expect("open cache database"))
}

/// How many cached rows the file holds. A file that was discarded and refilled
/// holds only the rows the refilling run wrote. A file that is gone holds none.
/// Any other failure to read the row table is raised rather than reported as
/// zero, so no assertion about a discard can pass on an unreadable file.
fn entry_count(path: &Path) -> u64 {
    let Some(connection) = open_cache(path) else {
        return 0;
    };
    let present: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='entries'",
            [],
            |row| row.get(0),
        )
        .expect("read cache schema");
    if present == 0 {
        return 0;
    }
    let count: i64 = connection
        .query_row("SELECT count(*) FROM entries", [], |row| row.get(0))
        .expect("count cached rows");
    u64::try_from(count).expect("non-negative count")
}

/// The next write sequence the cache will hand out. Every fill takes one. A run
/// that found its answer already stored takes none, so this number holding
/// still is how a test tells a hit from a recomputed row written over the top
/// of the one it was supposed to find.
fn next_write_sequence(path: &Path) -> i64 {
    open_cache(path)
        .expect("cache database")
        .query_row("SELECT next_write_sequence FROM metadata", [], |row| {
            row.get(0)
        })
        .expect("read write sequence")
}

/// Every table in the cache file, row table included, rendered as text.
fn whole_file(path: &Path) -> String {
    table_text(path, TableScope::Every)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum TableScope {
    ExceptRows,
    Every,
}

/// Every table in the cache file except the row table, rendered as text. This
/// is what the cache records about itself rather than about any one variant, so
/// it is where a reader looks to see which setup filled the file.
fn recorded_setup(path: &Path) -> String {
    table_text(path, TableScope::ExceptRows)
}

fn table_text(path: &Path, scope: TableScope) -> String {
    let Some(connection) = open_cache(path) else {
        return String::new();
    };
    let filter = match scope {
        TableScope::ExceptRows => "AND name<>'entries'",
        TableScope::Every => "",
    };
    let mut names = connection
        .prepare(&format!(
            "SELECT name FROM sqlite_master
             WHERE type='table' {filter} AND name NOT LIKE 'sqlite_%'
             ORDER BY name"
        ))
        .expect("list tables");
    let names: Vec<String> = names
        .query_map([], |row| row.get::<_, String>(0))
        .expect("table names")
        .map(|name| name.expect("table name"))
        .collect();
    let mut recorded = String::new();
    for name in names {
        let mut statement = connection
            .prepare(&format!("SELECT * FROM \"{name}\""))
            .expect("read table");
        let columns = statement.column_count();
        let mut rows = statement.query([]).expect("table rows");
        while let Some(row) = rows.next().expect("table row") {
            for index in 0..columns {
                let value = row.get_ref(index).expect("column value");
                let text = match value {
                    rusqlite::types::ValueRef::Text(bytes)
                    | rusqlite::types::ValueRef::Blob(bytes) => {
                        String::from_utf8_lossy(bytes).into_owned()
                    }
                    rusqlite::types::ValueRef::Integer(number) => number.to_string(),
                    rusqlite::types::ValueRef::Real(number) => number.to_string(),
                    rusqlite::types::ValueRef::Null => String::new(),
                };
                recorded.push_str(&text);
                recorded.push('\n');
            }
        }
    }
    recorded
}

/// Rewrite one value inside what the cache records about its own setup, leaving
/// the cached rows untouched. This is how a test reaches a setup input the
/// running executable cannot be asked to change, such as its own version.
///
/// The rewrite reaches the recorded setup itself. A cache that decided a match
/// from a digest it also stored would not see this change, so the design
/// records the setup as the text a reader can see and compares that text.
fn rewrite_recorded_setup(path: &Path, from: &str, to: &str) {
    assert!(
        recorded_setup(path).contains(from),
        "the cache file records no {from}, so nothing in it identifies the setup that filled it"
    );
    let connection = open_cache(path).expect("cache database");
    let tables: Vec<String> = {
        let mut statement = connection
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type='table' AND name<>'entries' AND name NOT LIKE 'sqlite_%'",
            )
            .expect("list tables");
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("table names")
            .map(|name| name.expect("table name"))
            .collect()
    };
    for table in tables {
        let columns: Vec<(String, String)> = {
            let mut statement = connection
                .prepare(&format!("PRAGMA table_info(\"{table}\")"))
                .expect("table info");
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                })
                .expect("column info")
                .map(|column| column.expect("column"))
                .collect()
        };
        for (column, declared) in columns {
            let replaced = match declared.to_ascii_uppercase().as_str() {
                "TEXT" => format!("replace(\"{column}\", ?1, ?2)"),
                "BLOB" => format!("CAST(replace(CAST(\"{column}\" AS TEXT), ?1, ?2) AS BLOB)"),
                _ => continue,
            };
            connection
                .execute(
                    &format!(
                        "UPDATE \"{table}\" SET \"{column}\"={replaced}
                         WHERE instr(CAST(\"{column}\" AS TEXT), ?1)>0"
                    ),
                    rusqlite::params![from, to],
                )
                .expect("rewrite recorded setup");
        }
    }
    drop(connection);
    let recorded = recorded_setup(path);
    assert!(
        recorded.contains(to) && !recorded.contains(from),
        "the rewrite left the recorded setup unchanged"
    );
}

#[test]
fn the_cache_file_records_the_setup_that_filled_it() {
    let temp = private_temp();
    let setup = Setup::fixtures();
    let filled = score(&setup, temp.path(), &[FIRST_VARIANT]);
    let provenance = provenance(&filled);

    let recorded = recorded_setup(&default_cache_path(temp.path()));
    for (what, expected) in [
        ("the running software version", env!("CARGO_PKG_VERSION")),
        (
            "the model bundle it scored with",
            provenance["model_bundle_id"].as_str().expect("model id"),
        ),
        (
            "the reference bundle it read",
            provenance["reference_bundle_id"]
                .as_str()
                .expect("reference id"),
        ),
        (
            "the mask it applied",
            provenance["mask_sha256"].as_str().expect("mask digest"),
        ),
    ] {
        assert!(
            recorded.contains(expected),
            "the cache file does not record {what} ({expected}), so a reader cannot tell which \
             setup filled it"
        );
    }
    let policy = provenance["effective_cpu_policy"]
        .as_str()
        .expect("cpu policy");
    assert!(
        !whole_file(&default_cache_path(temp.path())).contains(policy),
        "nothing in the cache file may record the effective CPU policy ({policy}): a thread \
         setting moves no answer, so keying or stamping on one throws paid-for rows away and \
         still fails to notice a software change"
    );
}

#[test]
fn a_software_version_change_discards_every_earlier_row() {
    let temp = private_temp();
    let setup = Setup::fixtures();
    let cache = default_cache_path(temp.path());
    succeeded(&score(
        &setup,
        temp.path(),
        &[FIRST_VARIANT, SECOND_VARIANT],
    ));
    assert_eq!(entry_count(&cache), 2, "both variants must be cached first");

    rewrite_recorded_setup(&cache, env!("CARGO_PKG_VERSION"), "0.0.0");

    succeeded(&score(&setup, temp.path(), &[FIRST_VARIANT]));
    assert_eq!(
        entry_count(&cache),
        1,
        "a cache filled by another software version must be discarded whole, leaving only the row \
         the running version wrote"
    );
}

#[test]
fn a_model_change_discards_every_earlier_row() {
    let temp = private_temp();
    let setup = Setup::fixtures();
    let cache = default_cache_path(temp.path());
    let other = setup.with_other_model(temp.path());
    assert_ne!(
        provenance(&score(&setup, temp.path(), &[FIRST_VARIANT]))["model_bundle_id"],
        provenance(&score(&other, temp.path(), &[FIRST_VARIANT]))["model_bundle_id"],
        "this test must change the model bundle identity"
    );

    fs::remove_file(&cache).expect("start from a cold cache");
    succeeded(&score(
        &setup,
        temp.path(),
        &[FIRST_VARIANT, SECOND_VARIANT],
    ));
    assert_eq!(entry_count(&cache), 2, "both variants must be cached first");

    succeeded(&score(&other, temp.path(), &[FIRST_VARIANT]));
    assert_eq!(
        entry_count(&cache),
        1,
        "a changed model bundle must discard the file whole, leaving only the row the running \
         model wrote"
    );
}

#[test]
fn a_reference_change_discards_every_earlier_row() {
    let temp = private_temp();
    let setup = Setup::fixtures();
    let cache = default_cache_path(temp.path());
    let other = setup.with_other_reference(temp.path());
    assert_ne!(
        provenance(&score(&setup, temp.path(), &[FIRST_VARIANT]))["reference_bundle_id"],
        provenance(&score(&other, temp.path(), &[FIRST_VARIANT]))["reference_bundle_id"],
        "this test must change the reference bundle identity"
    );

    fs::remove_file(&cache).expect("start from a cold cache");
    succeeded(&score(
        &setup,
        temp.path(),
        &[FIRST_VARIANT, SECOND_VARIANT],
    ));
    assert_eq!(entry_count(&cache), 2, "both variants must be cached first");

    succeeded(&score(&other, temp.path(), &[FIRST_VARIANT]));
    assert_eq!(
        entry_count(&cache),
        1,
        "a changed reference bundle must discard the file whole, leaving only the row the running \
         reference wrote"
    );
}

#[test]
fn a_mask_change_discards_every_earlier_row() {
    let temp = private_temp();
    let setup = Setup::fixtures();
    let cache = default_cache_path(temp.path());
    succeeded(&score(
        &setup,
        temp.path(),
        &[FIRST_VARIANT, SECOND_VARIANT],
    ));
    assert_eq!(entry_count(&cache), 2, "both variants must be cached first");

    // The miniature GENCODE mask holds no domain over this variant, so this run
    // reaches the cache and then declines to score. The setup it opened the
    // cache under is still another setup, and the file it opened was still
    // filled by the route mask.
    let rejected = score(&setup.with_other_mask(), temp.path(), &[FIRST_VARIANT]);
    assert!(
        !rejected.status.success(),
        "the miniature GENCODE mask covers no domain here"
    );
    assert_eq!(
        entry_count(&cache),
        0,
        "a changed mask must discard the file whole, leaving no row the route mask filled"
    );
}

#[test]
fn a_matching_setup_keeps_every_earlier_row() {
    let temp = private_temp();
    let setup = Setup::fixtures();
    let cache = default_cache_path(temp.path());
    succeeded(&score(
        &setup,
        temp.path(),
        &[FIRST_VARIANT, SECOND_VARIANT],
    ));
    assert_eq!(entry_count(&cache), 2, "both variants must be cached first");

    let sequence = next_write_sequence(&cache);

    succeeded(&score(&setup, temp.path(), &[FIRST_VARIANT]));
    assert_eq!(
        entry_count(&cache),
        2,
        "an unchanged setup must keep every paid-for row"
    );
    assert_eq!(
        next_write_sequence(&cache),
        sequence,
        "the second run must find the row rather than write one over the top of it"
    );
}

#[test]
fn an_explicitly_named_cache_is_discarded_on_a_setup_change_too() {
    let temp = private_temp();
    let setup = Setup::fixtures();
    let cache = temp.path().join("chosen.sqlite3");
    let run = |setup: &Setup, variants: &[&str]| {
        let mut args = vec!["lookup".to_owned(), "--model-only".to_owned()];
        for variant in variants {
            args.push("--variant".to_owned());
            args.push((*variant).to_owned());
        }
        args.extend(setup.args());
        args.extend(["--model-cache".to_owned(), cache.display().to_string()]);
        Command::new(env!("CARGO_BIN_EXE_pangopup"))
            .args(&args)
            .env("XDG_CACHE_HOME", temp.path())
            .env("HOME", temp.path())
            .output()
            .expect("run pangopup")
    };
    succeeded(&run(&setup, &[FIRST_VARIANT, SECOND_VARIANT]));
    assert_eq!(entry_count(&cache), 2, "both variants must be cached first");

    let other = setup.with_other_reference(temp.path());
    let discarded = run(&other, &[FIRST_VARIANT]);
    succeeded(&discarded);
    assert_eq!(
        entry_count(&cache),
        1,
        "a chosen cache path is still a shortcut: a setup change discards it whole"
    );
    // A chosen file is never replaced silently. The rule the repository already
    // publishes is satisfied by saying so, not by keeping rows another setup
    // wrote.
    let report = String::from_utf8(discarded.stderr).expect("UTF-8 report");
    assert!(
        report.contains(&cache.display().to_string()),
        "discarding a file the caller chose must name it, and this run said: {report:?}"
    );
}

#[test]
fn a_discarded_cache_is_reported_and_a_kept_one_is_silent() {
    let temp = private_temp();
    let setup = Setup::fixtures();
    let cache = default_cache_path(temp.path());

    let cold = score(&setup, temp.path(), &[FIRST_VARIANT]);
    succeeded(&cold);
    assert!(
        cold.stderr.is_empty(),
        "filling a cold cache says nothing: {}",
        String::from_utf8_lossy(&cold.stderr)
    );

    let warm = score(&setup, temp.path(), &[FIRST_VARIANT]);
    succeeded(&warm);
    assert!(
        warm.stderr.is_empty(),
        "reading a matching cache says nothing: {}",
        String::from_utf8_lossy(&warm.stderr)
    );

    let discarded = score(
        &setup.with_other_reference(temp.path()),
        temp.path(),
        &[FIRST_VARIANT],
    );
    let scored = succeeded(&discarded);
    assert!(
        scored.contains("\"kind\":\"model\""),
        "the discard must not cost the caller an answer"
    );
    let report = String::from_utf8(discarded.stderr.clone()).expect("UTF-8 report");
    assert!(
        report.to_ascii_lowercase().contains("cache"),
        "a discarded cache must be reported, so an operator can tell a cold cache from a broken \
         one, and this run said: {report:?}"
    );
    assert!(
        report.contains(&cache.display().to_string()),
        "the report must name the file it discarded, and this run said: {report:?}"
    );
}
