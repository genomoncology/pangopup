.PHONY: lint test spec

# pangopup-build is the offline Linux builder. Its tests need Linux syscalls and
# a corpus fixture that is not in the checkout, and `source_fingerprint` hashes
# several of its files as the builder's published provenance -- so making them
# pass on another platform would mean changing what the builder claims to be.
# Exclude it off Linux. The Linux CI job still runs the whole workspace.
WORKSPACE_TESTS := --workspace
ifneq ($(shell uname -s),Linux)
WORKSPACE_TESTS := --workspace --exclude pangopup-build
endif

# The qualification harnesses below drive GNU sed (in-place `-i`, `1i`, `0,/re/`)
# and GNU find. macOS ships BSD versions that reject those forms outright, so
# they run only in the Linux CI job.
SHELL_QUALIFICATION := tests/readme-branding.sh tests/executable-delivery.sh tests/production-release-qualification.sh
ifneq ($(shell uname -s),Linux)
SHELL_QUALIFICATION :=
endif

PORTABLE_QUALIFICATION := tests/ci-platform-support.sh tests/ci-test-failure-evidence.sh tests/version-consistency-python39.sh


lint:          ## static analysis: rustfmt + clippy + dependency policy
	python3 scripts/check-version-consistency.py
	cargo fmt --all --check
	cargo clippy --locked $(WORKSPACE_TESTS) --all-targets -- -D warnings
	cargo deny check advisories bans licenses sources --warn unmaintained

test:          ## inside-out unit and integration tests
	cargo test --locked $(WORKSPACE_TESTS)
	@for script in $(PORTABLE_QUALIFICATION); do echo "bash $$script"; bash $$script || exit 1; done
	@for script in $(SHELL_QUALIFICATION); do echo "bash $$script"; bash $$script || exit 1; done

# Spec files that only hold on Linux. mustmatch has no per-block skip, so the
# exclusion is whole files. Each one is listed with the reason it cannot run
# here. The Linux CI job still runs every spec file.
#
# cli.md              `pangopup uninstall` refuses on every non-Linux target.
#                     Direct uninstall is a Linux-only product feature.
# full-bundle.md      Its source builder remains Linux-only.
# reference.md        Its source builder remains Linux-only.
# snv-lookup.md       Needs a published SNV bundle.
# snv-transport.md    Needs a published SNV bundle.
# snv-release.md      Release packing refuses off Linux.
SPEC_LINUX_ONLY := cli.md full-bundle.md reference.md snv-lookup.md \
	snv-release.md snv-transport.md
SPEC_PATHS := spec/
ifneq ($(shell uname -s),Linux)
SPEC_PATHS := $(filter-out $(addprefix spec/,$(SPEC_LINUX_ONLY)),$(wildcard spec/*.md))
endif

spec:          ## outside-in CLI contracts
	cargo build --locked --quiet --package pangopup-cli --package pangopup-build
	rm -rf target/spec-cache
	install -d -m 700 target/spec-cache
	XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" PATH="$(CURDIR)/target/debug:$$PATH" mustmatch test $(SPEC_PATHS)
