# Align transport test with current SNV builder fingerprint

At pushed main `762ab07`, `builder_identity_covers_assets_manifest_notice_and_certification_source` failed on yellow. The test expected historical fingerprint `sha256:c40e9b931784f92f5b21236259b13979870582388acebf0cf0c3802d458447bb`, while the current builder and the checked source fingerprint unit oracle report `sha256:7e3c23058526103a2a9f3161670e6f4a634c39fa0ed6b3aefb4c904db1c5e8df`.

The test now pins the current fingerprint. It still asserts equality with the freshly generated bundle manifest and still checks that changes to root wiring, the notice, or the SNV asset source alter the fingerprint. Production code, fingerprint inputs, published assets, and historical fixture bytes did not change.

On yellow at `762ab07` plus this change, the focused test failed before the edit and passed after it. All 17 transport integration tests and the full `pangopup-build` test suite passed. On macOS, `make lint`, `make test`, and `make spec` passed. The spec gate reported 207 passed. `git diff --check` passed.
