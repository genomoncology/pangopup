# Align reference miniature oracles with the current source fingerprint

GitHub CI for `e75a266` failed `source_fingerprint_reference_members_and_legacy_reader_are_invariant` in the Linux `builder_provenance` integration suite. The generated miniature manifest reported `sha256:9d19e7cc7dda6d6b7475c80cdb60f2601b2c7ff0ce2ab9274b5d8f078cc5b8cd`; the test expected `sha256:09cd44449b77592e4b9948cc0756e736b01ecf5220b3d5312c52b12b6b6e9c65`.

The mismatch predates ticket 0153. Commit `b485048` added a macOS-only unused-variable statement to fingerprinted `reference_builder.rs`, changing its source bytes. Commit `8eb8917` updated the source fingerprint unit oracle to `9d19e7cc…` but left three miniature integration expectations at the old value. Production code at `14bb9e7` already reports `9d19e7cc…`.

The two reference integration tests now expect the existing checked source fingerprint at those three positions. Their byte hashes, historical v1 rebound, and comparison with the checked route bundle remain in place. No production source or authority changed.

On Linux arm64 in an ephemeral Debian trixie container with Rust 1.93.1, `cargo test --locked -p pangopup-build --test builder_provenance` passed all four tests. The affected `v2_miniature_preserves_the_independent_current_v1_byte_oracle` and `route_reference_rebuilds_byte_identically_and_covers_the_full_model_context` reference tests each passed separately. A broad container run passed the other 18 reference tests but failed `reference_cli_grammar_errors_are_canonical_empty_stderr_and_redacted`: ONNX Runtime printed a CPU vendor warning to standard error under the container's emulated CPU. The test assertion remains unchanged for CI's native runner.

On macOS, `make lint`, `make test`, and `make spec` passed. The spec gate reported 207 passed. `git diff --check` passed. The pushed commit's Linux CI remains the final full workspace confirmation.
