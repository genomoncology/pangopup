---
base: 91917fd8d81fd887c905067e5b81026d2f89c76f
head: 7dfcd3ba780e4d82832b15ce2b816993087b2708
---
# The repository has no changelog

`CHANGELOG.md` stands at the root with six sections, newest first: one for each
of the five software release tags, carrying the date its tag was made, and one
for 0.5.0, marked `unreleased` because no tag names that version.
`tests/changelog-release-coverage.sh` holds the file to the tree, and the README
points a reader at it from the upgrade instructions.

## What the tags are

`git tag` carries seven tags. Five are software releases: `v0.1.0` 2026-07-31,
`v0.2.0` 2026-08-04, `v0.3.0` 2026-08-05, `v0.4.0` 2026-09-06 and `v0.4.1`
2026-09-06, each date read with `git log -1 --format=%cd --date=format:%Y-%m-%d`.
`snv-grch38-v1` and `runtime-grch38-v1` tag the separately versioned scoring
assets. `architecture/delivery.md` describes each as its own immutable release
and `NOTICE` names the runtime one as separately versioned, so neither gets a
section. The gate sets them aside by name rather than dropping them silently.

## What was read for each claim

Every 0.5.0 claim was measured against `git diff v0.4.1..HEAD` or the tree.

| claim | read from |
| --- | --- |
| `gene_names` stands between `stable_gene` and `gain_score` | the `JsonRecord` and `JsonModelRecord` field order in `crates/pangopup-cli/src/lib.rs`, where `gene_names` is serialized between them and skipped when absent |
| the five added fields | the `## v0.5.0 response-shape inventory` section of `architecture/compatibility.md` |
| no published asset digest moved | `git diff v0.4.1..HEAD -- release-profiles/` is empty, and the release tags did not move |
| the cache is emptied once | `USER_VERSION` moves 1 to 2 in `crates/pangopup-cache/src/lib.rs`, `software_version` joins the recorded setup, and the stderr line is the `discarded model cache ...: an earlier layout wrote it` branch in `crates/pangopup-cli/src/main.rs` |
| the command line is unchanged | every `long = "..."` and `#[command(name = "...")]` in `crates/pangopup-cli/src/main.rs` is identical at `v0.4.1` and `HEAD` |
| the four routes are unchanged | the route literals in `crates/pangopup-cli/src/service.rs` are the same four at both revisions, and the `spec/http-service.md` diff carries no change to the request envelope, status codes or headers |
| the container gains the index and nothing else | the `Dockerfile` diff is one `COPY assets/gene-names`, and `.dockerignore` gains the two lines that admit it |
| the sources of the gene-name index | the `NOTICE` diff adds 59 lines covering HGNC, NCBI Gene and GENCODE |
| the licence is unchanged | `license = "GPL-3.0-only"` at both revisions |
| `data_set_version` against `scoring_identity` | the `## What a consumer pins` section of `architecture/compatibility.md` |

Entries for 0.1.0 through 0.4.1 were written from the release notes each tag
carries. Those are `planning/artifacts/038`, `050`, `054`, `057` and
`059-release-notes.md`, and `git ls-tree` confirmed each one present in its own
tag's tree. Each entry was then checked against the tagged source. `crates/pangopup-cli/src/service.rs` does not exist at `v0.1.0`, so
`pangopup serve` is new in 0.2.0. `uninstall` appears in no `v0.2.0` source file,
so it is new in 0.3.0. `--model-only` appears in no `v0.1.0` source file.
`MODEL_BATCH_TOO_LARGE` and the retry-after header are both in the `v0.4.0`
service source.

Two claims were dropped for want of evidence. Nothing in the checkout says which
release first published a container image: the v0.2.0 notes say no registry image
was part of that release, and `planning/artifacts/051-public-container.md` records
public `0.2.0` container tags published afterwards. The changelog says only what
`architecture/compatibility.md` states, that no 0.4.0 image was published. This
repository also holds no compose file, so the changelog claims no compose change.

## The gate

`tests/changelog-release-coverage.sh` reads the workspace version out of
`Cargo.toml`, the sections out of `CHANGELOG.md` and the tags out of git. It
requires the newest section to name the version `Cargo.toml` states, every
released tag to have a section carrying its tag date, the list to run newest
first, and a section naming no tag to be the stated version marked `unreleased`.
Its floors are the measured counts: five software release tags and two asset
tags. It refuses a count of zero on sections, on tags and on the version, and
names in every refusal the version it could not find.

Nine refusals are proved inside the gate against fixture copies before the tree
is read. Seven more were proved by hand on the real tree, each file copied aside
and copied back afterwards.

| by-hand mutation | the gate said |
| --- | --- |
| `Cargo.toml` bumped to 0.6.0, changelog untouched | the newest section names 0.5.0, and Cargo.toml states 0.6.0, so no section describes 0.6.0 |
| `## 0.9.9 - 2026-09-09` appended | the section for 0.9.9 stands below the section for 0.1.0, and the list is newest first |
| `## 0.9.9 - unreleased` appended | the same refusal |
| `## 0.4.2 - 2026-09-07` inserted in order | the section for 0.4.2 names a version no tag released and Cargo.toml does not state |
| the 0.4.1 section removed | no section names 0.4.1, which this repository released |
| the 0.4.0 section marked `unreleased` | the section for 0.4.0 says unreleased, and this repository tagged that version on 2026-09-06 |
| the 0.5.0 section dated 2026-09-11 | the section for 0.5.0 gives 2026-09-11, and no tag names that version, so it must say unreleased |
| the 0.3.0 date moved one day | the section for 0.3.0 gives 2026-08-06, and the tag for that version was made on 2026-08-05 |
| the README link removed | README.md carries no link to CHANGELOG.md |

## The README trade

The link is paid for out of the guide. Seven words were added to the upgrade
instructions and seven removed from the storage paragraph, whose sentence about
Linux and macOS reading and reclaiming file pages repeated what the performance
overview above it already says. No line moved. `README.md` measures 271 lines and
1,829 words before and after, which is what `spec/readme-first-use.md` pins and
`tests/readme-budget-exactness.sh` requires as equality. The presentation section
was not touched.

## The ladder

`make lint` 35.28 s, exit 0. `make test` 187.23 s, exit 0, 582 unit tests passed
and 0 failed, 35 shell gates run. `make spec` 24.62 s, `316 passed`, 0 failed;
this change adds no spec block and edits no spec file.
`scripts/run-service-fixture-tests.sh` 1.72 s, 20 tests matched, 0 failed.
`bash sdlc/scripts/lint` exit 0. All 32 `PORTABLE_QUALIFICATION` and 3
`SHELL_QUALIFICATION` gates were run one at a time: 35 of 35 exit 0.

The new gate costs 0.08 s in that per-gate run and 0.10 to 0.14 s over five
timed runs, so it is the whole of this change's cost to `make test` wall time.

Run with `CARGO_HOME` and `RUSTUP_HOME` pinned to the operator's own before
`HOME` was moved to a private root, the four `PANGOPUP_*` names unset, and
`ORT_CACHE_DIR` never exported. `~/.cache/pangopup/model-results.sqlite3` is
byte-identical afterwards: md5 `0ce402e99f91b4d16ed3aaddc88160fc`, 303,104
bytes, mtime unchanged. `~/.cache/ort.pyke.io` holds 288,252,072 bytes, the
figure it held beforehand. No `pangopup` or `pangopup-build` process is left
running.

Verify filed no draft.
