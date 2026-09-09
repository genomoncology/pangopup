---
base: 31896c8cf51a925670eb8095395da611d791d4e5
head: b6389be49696795e30f3c4d5b87f0934c2a315fa
---

# A change to printed output fails before the release, not during it

`scripts/check-production-qualification.py` describes what a released scored record carries. Nothing proved that description against the shipped renderer, because the only gate that ran the checker fed it a bash stub replaying the committed oracles. Ticket 0045's naming leaf passed every gate on that path and would have failed the real qualification.

One file changed, `tests/production-release-qualification.sh`. The stub now delegates the seven SNV groups to the built executable against the committed fixture bundle, so the checker compares the release's own bytes, its own ambiguity leaves included. A shape gate renders a model-route record from the committed mini model kernel, the route reference bundle and the route mask, and requires the renderer and each pinned model oracle to carry the same keys, in the same order, with the same leaf types. Every checker call goes through a wrapper that names `scripts/check-production-qualification.py` on rejection, because the oracles never move and the checker is the description that has to.

Two limits stand, both deliberate. The model route's scores stay replayed: both oracles are the published model's answers and the ticket forbids reaching it, so only the record's shape is gated there. The three HTTP score items have no gate of their own. `service.rs` builds every completed score item with `render_result_raw`, which calls the same `render_jsonl` the CLI calls, so a scored-record field cannot reach the HTTP surface without also reaching a gated route. Verified by renaming the naming leaf on `JsonRecord` and watching a real `pangopup serve` emit `gene_labels` inside `records[0]`.

Probes on the final tree, each reverted: a field added to `JsonModelRecord`, a field removed from `JsonRecord`, a field added to `JsonAmbiguity`, and the naming leaf renamed on `JsonRecord`. All four fail `make test` and name the checker. The base harness passes on every one of them.

The maintainer's release use is unchanged. `scripts/check-production-qualification.py` and `scripts/run-production-qualification.sh` are byte-identical to the base commit, and the checker's eight summary lines are byte-identical to the ones the base harness produced. No oracle, no pinned digest, no runbook and no line of Rust moved.
