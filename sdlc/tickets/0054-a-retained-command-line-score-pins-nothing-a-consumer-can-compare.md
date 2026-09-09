---
flow: build
priority: 5
---
# A retained command-line score pins nothing a consumer can compare

The HTTP service publishes a version a consumer stores beside a retained score. The command-line tool publishes none. `render_lookup_requests` (`crates/pangopup-cli/src/main.rs:1095`) hands the shared JSONL renderer in `crates/pangopup-cli/src/lib.rs` a record and its provenance and nothing else. A system that keeps `pangopup lookup` output has no field to compare a later run against.

Ticket 0052 puts `data_set_version` on every HTTP score item and states in `architecture/compatibility.md` that the command-line tool prints neither that field nor `scoring_identity`, that a `--bundle` lookup opens no installed runtime profile, and that what a retained command-line score pins is a separate open question with its own ticket. `spec/http-service.md` pins that sentence verbatim, so the repository publishes the claim that this ticket exists. This is that ticket.

The obstacle is real and measured in this checkout. `--bundle` and `--data-dir` are mutually exclusive (`crates/pangopup-cli/src/main.rs:1685`) and `--bundle` cannot be combined with `--model-only` (`:1689`). With an explicit bundle and no explicit model assets, `run_lookup_with_runtime_opener` answers from the bundle and returns before reaching any runtime opener (`:779-819`). `admit_installed_model_fallback` (`:1181`) is the only caller of `open_installed_runtime_profile` and `open_installed_runtime_profile_for_model`, and it takes a data root. So a `--bundle` run holds no admitted runtime profile, has no `runtime_profile_id`, and cannot compute `data_set_version`. `ScoringDataSetVersionPreimage::new` requires that identity.

So the answer is not "print `data_set_version`". A successor has to decide what a command-line score pins when the runtime profile identity is unavailable, and whether the four invocation shapes — installed data dir, explicit bundle, explicit model assets, and `--model-only` — pin the same kind of value or say plainly that they pin nothing. Whatever it decides, the guidance sentence ticket 0052 pinned has to stay true or move with it.

The published dataset bundle already carries `bundle_id`, `source_doi` and `source_archive_md5` on a precomputed item's provenance, and a modeled item's provenance carries `model_bundle_id`, `reference_bundle_id` and `mask_sha256`. A consumer can already reconstruct something from those. Whether one concise field is worth adding beside them is the decision, not a foregone conclusion.

Ticket 0052's boundary forbade answering this. Nothing here is a defect in what 0052 shipped.
