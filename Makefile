.PHONY: lint test spec

# pangopup-build is the offline Linux builder. Its tests need Linux syscalls and
# a corpus fixture that is not in the checkout, and `source_fingerprint` hashes
# several of its files as the builder's published provenance -- so making them
# pass on another platform would mean changing what the builder claims to be.
# Exclude it off Linux instead; CI is Linux and still runs the whole workspace.
WORKSPACE_TESTS := --workspace
ifneq ($(shell uname -s),Linux)
WORKSPACE_TESTS := --workspace --exclude pangopup-build
endif

# The qualification harnesses below drive GNU sed (in-place `-i`, `1i`, `0,/re/`)
# and GNU find. macOS ships BSD versions that reject those forms outright, so
# they run on Linux only. CI is Linux and still runs all three.
SHELL_QUALIFICATION := tests/readme-branding.sh tests/executable-delivery.sh tests/production-release-qualification.sh
ifneq ($(shell uname -s),Linux)
SHELL_QUALIFICATION :=
endif


lint:          ## static analysis: rustfmt + clippy + dependency policy
	cargo fmt --all --check
	cargo clippy --locked $(WORKSPACE_TESTS) --all-targets -- -D warnings
	cargo deny check advisories bans licenses sources --warn unmaintained

test:          ## inside-out unit and integration tests
	cargo test --locked $(WORKSPACE_TESTS)
	@for script in $(SHELL_QUALIFICATION); do echo "bash $$script"; bash $$script || exit 1; done

spec:          ## outside-in CLI contracts
	cargo build --locked --quiet --package pangopup-cli --package pangopup-build
	rm -rf target/spec-cache
	install -d -m 700 target/spec-cache
	XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" PATH="$(CURDIR)/target/debug:$$PATH" mustmatch test spec/
