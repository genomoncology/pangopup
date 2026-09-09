---
base: 431b17017f4d3441b23d0c45d892fc796f2b9bac
head: d296bc7db48ce31049aac5b010fd00a2e77c5806
---

# A spec gate cannot pass on a cargo filter that matches nothing

`cargo test` exits 0 when a name filter selects no test. Nineteen gates under `spec/` ran `cargo test` with a filter, and most of them read only the exit code, so each one passed on nothing the moment the test it named was renamed or deleted. Ticket 0050 closed that shape for CI's one feature-gated step and its boundary forbade reaching into these.

`scripts/spec-cargo-test.sh` is one counting form for every such gate. It runs cargo unchanged, passes cargo's own non-zero status straight through, sums the `passed` count across every `test result:` line cargo printed, and refuses a run below the floor the gate names. The floor is the number of tests the gate covered when it was written, so coverage may grow and may not shrink. A refusal names the filter, the spec file carrying the gate, the count and the floor, so an operator finds the gate without searching for it. Summing rather than reading one line matters: a filter with no target selector prints one summary per cargo target, and eight empty targets must not mask the one real pass.

All nineteen gates now call the helper. The old form piped cargo into `rg -F '1 passed; 0 failed'`, which threw cargo's exit status away, so the new form is stronger in the other direction too: a compile failure or a panicking test now fails the gate on cargo's status rather than on absent text.

`tests/spec-cargo-filter-evidence.sh` holds both halves shut and runs in `make test`. It reads the gates out of the spec files rather than pinning today's list, refuses any block under `spec/` that runs `cargo test` directly, and requires each gate to name a floor of at least one and to end in a test-name filter. It then proves the helper's arithmetic against a fake cargo. It counts what it inspected and refuses when it inspected nothing, both when `spec/` holds no file and when the files hold no gate.

The boundary held. No test was renamed, no product source under `crates/` changed, no gate's assertion about product behavior moved, and `scripts/run-service-fixture-tests.sh` and `tests/ci-service-fixture-evidence.sh` were not touched. Every fenced block and every `mustmatch like` sentinel in the three changed spec files survives unchanged in number. No CI step was added or removed; the one Makefile line adds the new check to the portable qualification list.

Verified by breaking real gates, not fixtures, with `HOME`, `XDG_CACHE_HOME` and `TMPDIR` redirected away from the operator's own store. Marking `a_halfway_model_value_rounds_to_the_even_hundredth` as `#[ignore]` made raw cargo print `0 passed; 1 ignored` and exit 0; the gate refused, naming the filter and `score-value.md`, and `mustmatch test spec/score-value.md` reported `5 passed, 1 failed`. Renaming `rebuilding_from_identical_source_bytes_produces_identical_index_bytes` emptied its filter the same silent way; the gate refused and `spec/gene-naming.md` went to `10 passed, 1 failed`. Deleting the whole `gene_name_index` test target made the gate exit 101 and pass cargo's own list of available targets through. Adding a raw `cargo test` block to a spec file was refused with its file and line, and so was a block that called the helper and then also ran cargo directly. A floor of 0, a missing floor, a gate ending in a flag and a gate with no arguments after its floor were each refused by name. With `spec/` emptied the check refused for holding no file, and with a file holding no gate it refused for inspecting nothing.

`make lint`, `make test`, `make spec`, `scripts/run-service-fixture-tests.sh` and `sdlc/scripts/lint` all pass. `make spec` reports 302 passed, 7 skipped, the same as before the change. No draft was filed: every `cargo test` outside `spec/` names a package or a target and carries no name filter, so none has this shape.
