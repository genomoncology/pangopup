.PHONY: lint test spec

lint:          ## static analysis: rustfmt + clippy
	cargo fmt --all --check
	cargo clippy --locked --workspace --all-targets -- -D warnings

test:          ## inside-out unit and integration tests
	cargo test --locked --workspace

spec:          ## outside-in CLI contracts
	cargo build --locked --quiet --package pangopup-cli --package pangopup-build
	cargo build --locked --quiet --package pangopup-build --features mask-qualification --bin pangopup-mask-candidates
	PATH="$(CURDIR)/target/debug:$$PATH" mustmatch test spec/
