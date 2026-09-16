---
flow: build
priority: 5
---
# The repository-sourcing check runs on macOS and Linux

Mac `make test` now passes the qualification cache check and stops in `tests/repository-sourcing.sh`: BSD `sed` rejects a GNU-style in-place fixture edit, and the mutation changes nothing. The same check causes one of the three native Mac `make spec` failures through `spec/repository-motivation.md`. Syntax failure hides whether the project's source and date claims are actually verified.

The sourcing check must run on both supported operating systems. Its clean fixture and source-preserving copy edits must still pass. Every altered source, date, licence, service quotation, and explanation fixture must still make a real change and be refused for the stated missing or unsupported claim.

Done, observably:

- `bash tests/repository-sourcing.sh` exits 0 on macOS and Linux. All in-place fixture edits change their copied files before the relevant refusal is tested.
- Its positive rewording cases still pass without pinning the exact prose. Its negative cases still name the missing source, date, or attribution rather than a command syntax error.
- Mac `make test` advances beyond this check. The repository-motivation spec block also passes on Mac.
- Run `make lint`, `make test`, and `make spec` before commit; record separate remaining failures honestly.

Boundary: do not change the published source claims, URLs, dates, licence statements, or claim rules. Do not change the container-image or model-kernel spec blocks. Record Mac and Linux evidence in `sdlc/records/`; no public score contract changes.
