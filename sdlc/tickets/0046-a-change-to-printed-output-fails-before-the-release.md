---
flow: build
priority: 2
---
# A change to printed output fails before the release, not during it

`scripts/check-production-qualification.py` compares what a released executable prints against oracles committed here. Two callers run it. A maintainer runs it against the real Linux release during the production qualification runbook in `sdlc/planning/artifacts/050-public-linux-release.md`. `tests/production-release-qualification.sh` runs it inside `make test` against a bash script at `$root/bin/pangopup`.

That bash script is not the executable. Its `lookup` branch prints the bytes of `tests/fixtures/snv-regression/expected/$group.jsonl`, and a second script inserts a `gene_names` object after every `"stable_gene"` field. Its `serve` branch is a Python HTTP server that replays the same oracles and attaches the same object. Nothing the shipped renderer prints reaches the checker in any gate.

Ticket 0045 fell into the gap. v0.5.0 added a `gene_names` object to every named record. The checker compared release output byte for byte against oracles that carry no such object. The real production qualification would have failed on the seven SNV group comparisons, on both model oracles and on the three HTTP score comparisons. `make test` stayed green through all of it. A human reading the checker found it, and the code review repaired the checker and taught the replay script to insert the object by hand.

The repair closed that instance. The shape survives it. Two hand-maintained descriptions of a released record now agree by construction rather than by evidence, and only one of them has a gate. Any further change to what a record carries can pass every gate and fail at qualification time.

Done, observably:

- A field added to, removed from, or renamed in what the tool prints for a scored record fails `make test`, and the failure names `scripts/check-production-qualification.py` as the file that must change. A test proves this by making such a change and observing the failure, in the style of the negative cases already in `tests/production-release-qualification.sh`.
- That holds for the precomputed route, for the model route, and for the HTTP score item.
- Every scored record the checker compares in a gate came from the shipped renderer. No gate presents the checker a scored record written by hand.
- No gate reaches the network, the published SNV dataset, or the published model. Repository fixtures are the only inputs.
- The maintainer's release use is unchanged. The same script takes the same arguments, prints the same summary lines, and still rejects everything it rejects today. Every negative case in `tests/production-release-qualification.sh` still passes: the four unsafe installed-bundle layouts, the pre-existing XDG directories, the reused first online sync, the formatting drift, the decreased progress counter, and output that names no gene.
- `make lint`, `make test` and `make spec` pass without reducing specification coverage.

Boundary: this ticket changes what a gate observes. It must not change what the executable prints. No field is added, removed or renamed by this work, and no score, position, provenance value or error code moves.

It must not change the oracles at `tests/fixtures/snv-regression/expected/*.jsonl` and `tests/fixtures/executable-release/*.jsonl`, their pinned digests in the checker, or `pangopup-regression-fixture`. Those oracles never call the CLI renderer, the bundle opener or the gene-name index. The comparison is worth running because of that independence.

It must not change the release runbooks under `sdlc/planning/artifacts/`, publish a release, or move a published artifact pin.

Keep the replay stub for what repository fixtures cannot produce. The installed-bundle layouts, the sync progress lines and the safety refusals are the harness's real value and they stay proved.

`scripts/run-linux-tests-with-public-failure.sh` hardcodes `file=Makefile,line=30` and ticket 0045 added `tests/ci-test-failure-evidence.sh` to hold it. A sweep for other couplings of that class is out of scope here. A finding becomes a numbered flowless draft.
