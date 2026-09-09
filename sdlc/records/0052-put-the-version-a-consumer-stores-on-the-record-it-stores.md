---
base: 2c66777363cd9853680b929ac12111510e30a5e0
head: b2e069ae8cbebc982ffa34e935e657d505664ef1
---

# Put the version a consumer stores on the record it stores

`architecture/compatibility.md` told a consumer to store `data_set_version` beside every retained score. No score carried that value. Every score item carried `scoring_identity`, which hashes the effective CPU policy and therefore moves when a deployment restarts with a different `--model-threads`. A consumer that took the one identity an item offered and named its own column after the data-set version stored a value that a thread change moves.

`add_service_fields` in `crates/pangopup-cli/src/service.rs` now takes the whole `ScoringIdentities` instead of one identity and appends `data_set_version` after `scoring_identity`. That function is the single place a score item is assembled, and it has one caller, so precomputed, modeled, cached, ambiguous and rejected items all gained the field together. The value comes from the same `state.identities` field the status response reads, so an item and its own status response always agree. Request-level errors still return no item and carry neither field.

`architecture/compatibility.md` names the added field in the v0.5.0 response-shape inventory, tells a consumer to store the value the item carries, and states that reaching it takes no second request. It also states that the command-line tool prints neither field, because a `--bundle` lookup opens no installed runtime profile and no data-set version is computable on every command-line path. `spec/http-service.md` pins both statements verbatim.

Three gates hold the new field. `scripts/check-version-consistency.py` and the independent Python 3.9 copy in `tests/version-consistency-python39.sh` both require the inventory entry. `scripts/check-production-qualification.py` requires the field on every HTTP item, requires a well-formed digest, and requires it to equal the status value; `tests/production-release-qualification.sh` drives six new mutations through the checker and its stub now publishes two different digests, so a checker that compared an item against the wrong one fails there. The failing-mutation helper in that harness now prints what it expected and what the checker said, because a bare `grep -Fxq` exited non-zero and printed nothing.

The boundary held. The command-line tool prints the same bytes it printed before, so the JSONL oracles under `tests/fixtures/` and their pinned digests in `scripts/check-production-qualification.py` did not move. No score, position, status, rejection code or rejection reason changed, and `scoring_identity` keeps its value, its meaning and its place.

Verified against a real `pangopup serve` started on the repository's miniature runtime fixtures in a scratch data root. A precomputed, a modeled, a cached and two rejected items all carried `data_set_version` as the last key, immediately after `scoring_identity`, all equal to the status value and all distinct from the identity. A 100-item batch at the request limit carried one version on every item. A second service under `--model-threads 4` moved `scoring_identity` on the same variant, held `data_set_version` unchanged, and changed nothing else on the item except `effective_cpu_policy`. `pangopup lookup` printed neither field on the installed, bundle and explicit-asset paths, in both `jsonl` and `table` form.

Four findings stay open as drafts. 0054 asks what a retained command-line score pins. 0055 records five spec gates that pass on a cargo filter matching nothing. 0056 records two documents that still place the stored version only on the status route. 0057 records that the release checker accepts a deployment publishing one value under both identity names.
