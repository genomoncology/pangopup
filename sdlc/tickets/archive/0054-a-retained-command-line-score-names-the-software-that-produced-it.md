---
flow: build
priority: 2
---
# A retained command-line score names the software that produced it

A system that keeps `pangopup lookup` output can name every asset that answered, and cannot name the software that answered. `architecture/compatibility.md:66` publishes that a PangoPup version change can move an answer. Nothing in the output records which version ran.

Measured in this checkout on 2026-09-09. `render_jsonl` (`crates/pangopup-cli/src/lib.rs:102`) emits `bundle_id`, `source_doi`, `source_archive_md5`, `masked` and `window` on a precomputed line, and the model asset identifiers on a modeled line. Neither carries a PangoPup version. `grep -n "software_version\|CARGO_PKG_VERSION" crates/pangopup-cli/src/lib.rs` returns nothing. So a retained line pins the assets exactly and the software not at all, and two runs that differ only by an upgrade are indistinguishable in the file.

Settled: the answer is the running PangoPup version, as a plain version string. It is not a digest.

That choice is deliberate. The HTTP service publishes `data_set_version`, a digest over the software version and the admitted runtime profile. The command-line tool cannot compute that value: `--bundle` excludes `--data-dir` (`crates/pangopup-cli/src/main.rs:1685`), a `--bundle` run returns before reaching any runtime opener (`:779-819`), and `ScoringDataSetVersionPreimage::new` requires a `runtime_profile_id` that such a run never holds. Publishing a second, differently-computed digest under a similar name would give a consumer two version vocabularies that disagree for the same setup. A plain version string beside the asset identifiers the output already carries lets a consumer reconstruct the setup without inventing that second vocabulary.

Settled: every invocation shape reports it — installed data dir, explicit bundle, explicit model assets, and `--model-only`. The version does not depend on which assets were reached, so no shape has a reason to omit it.

Done, observably:

- Every `pangopup lookup` result line names the PangoPup version that produced it, on both routes and on every invocation shape.
- The value matches the version the same build reports elsewhere, so a consumer comparing a retained line against a running deployment gets a true answer.
- The `table` output form is unaffected, or changes deliberately and says so.
- `architecture/compatibility.md` carries the addition in its response-shape inventory, and states what a retained command-line score pins now that it pins something. The sentence ticket 0052 pinned saying this is an open question with its own ticket is replaced by what was decided, and every gate reading that sentence moves with it.
- The release checker judges the added field the way it judges the other published fields.
- `make lint`, `make test` and `make spec` pass. `make test` wall time does not grow by more than two seconds.

Boundary: this ticket adds one field to command-line result output and states what it means. It must not change any score value, position, status, reason, gene name, or existing provenance field on either route.

It must not add a digest, reuse the name `data_set_version` or `scoring_identity` for a differently computed value, or make the command-line tool open a runtime profile it does not open today.

It must not change what an HTTP score item carries. Ticket 0052 settled that and it stays as it is.

It must not change the model cache. What the cache is stamped with is ticket 0058, and the two must not be made to share a value without a decision that says so.
