# Build a valid sparse input for the format refusal test

At pushed main `6eaf858`, the focused Linux test `sparse_source_is_rejected_without_output_or_scratch` failed before invoking the candidate builder. Its synthetic sparse manifest lacked `sparse_provenance`, and `canonical_manifest_bytes` correctly returned `Corrupt("manifest format provenance")`.

The test now derives the fixed authority bundle ID and builder identity from its source bundle, supplies a syntactically valid synthetic candidate commit, and verifies the constructed sparse bundle. The candidate builder then returns the intended `SPARSE_INPUT_FORMAT` code. The test still checks that scratch, candidate, report, and sidecar files are absent. Production code, strict manifest validation, and checked fixtures did not change.

On yellow at `6eaf858` plus this change, the focused test failed before the change and passed after it. The full `pangopup-build` test run passed all seven `sparse_candidate` tests, then failed the separate `builder_identity_covers_assets_manifest_notice_and_certification_source` test in `transport.rs`. That test still expects the historical SNV builder fingerprint `c40e9b…` while production reports the current checked fingerprint `7e3c2305…`. This leaves the full Linux builder suite red and requires a separate ticket.

On macOS, `make lint`, `make test`, and `make spec` passed. The spec gate reported 207 passed. `git diff --check` passed.
