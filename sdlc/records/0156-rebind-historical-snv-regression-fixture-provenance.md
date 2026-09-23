# Rebind historical SNV regression fixture provenance

Linux CI run `35856423930` failed `checked_regression_fixture_regenerates_byte_exactly`. On yellow, the generated manifest's builder source fingerprint was `sha256:7e3c23058526103a2a9f3161670e6f4a634c39fa0ed6b3aefb4c904db1c5e8df`. The checked historical fixture carries `sha256:c40e9b931784f92f5b21236259b13979870582388acebf0cf0c3802d458447bb`. The test already translated the current builder version and bundle identity to their historical values, but did not translate the fingerprint.

The test now asserts the generated fingerprint equals the current source fingerprint unit oracle, then translates that field for byte comparison with the historical manifest. Every other generated file and manifest field retains its byte comparison. The production builder and checked fixture did not change.

On yellow at pushed base `6379272`, the focused regression test failed before the change and passed after it. After updating the test worktree to `268ef36`, `cargo test --locked -p pangopup-build` passed the corrected regression test and then failed an unrelated `sparse_candidate` test at `canonical_manifest_bytes` with `Corrupt("manifest format provenance")`. The package suite is therefore not green. The separate failure needs its own ticket.

On macOS at `268ef36` plus this change, `make lint`, `make test`, and `make spec` passed. The spec gate reported 207 passed. `git diff --check` passed.
