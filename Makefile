.PHONY: lint test spec

lint:          ## static analysis: rustfmt + clippy + dependency policy
	cargo fmt --all --check
	cargo clippy --locked --workspace --all-targets -- -D warnings
	cargo deny check advisories bans licenses sources --warn unmaintained

test:          ## inside-out unit and integration tests
	cargo test --locked --workspace
	bash tests/executable-delivery.sh

spec:          ## outside-in CLI contracts
	cargo build --locked --quiet --package pangopup-cli --package pangopup-build
	rm -rf target/spec-cache
	install -d -m 700 target/spec-cache
	XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" PATH="$(CURDIR)/target/debug:$$PATH" mustmatch test spec/
