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

# Spec files that only hold on Linux. mustmatch has no per-block skip, so the
# exclusion is whole files. Each one is listed with the reason it cannot run
# here. CI is Linux and still runs every spec file.
#
# cli.md              `pangopup uninstall` refuses on every non-Linux target.
#                     Direct uninstall is a Linux-only product feature.
# full-bundle.md      SNV bundle publication uses renameat2 RENAME_NOREPLACE.
# reference.md        Reference bundle publication uses the same syscall.
# snv-lookup.md       Needs a published SNV bundle.
# snv-transport.md    Needs a published SNV bundle.
# snv-release.md      Release packing refuses off Linux.
# runtime-release.md  Release packing refuses off Linux.
# runtime-transport.md Transport packing refuses off Linux.
# http-service.md     Startup asset resolution refuses off Linux first.
# local-assets.md     Asset installation is built on openat2 RESOLVE_BENEATH.
# remote-assets.md    Same asset installation path.
# runtime-install.md  Same asset installation path.
#
# The publication syscall has a working macOS spelling, renameatx_np with
# RENAME_EXCL, already used in crates/pangopup-build/src/runtime_profile.rs.
# Porting the other two sites would mean editing production.rs and
# reference_builder.rs. source_fingerprint.rs hashes both as published builder
# provenance, so an edit there changes what the builder claims to be.
SPEC_LINUX_ONLY := cli.md full-bundle.md http-service.md local-assets.md \
	reference.md remote-assets.md runtime-install.md runtime-release.md \
	runtime-transport.md snv-lookup.md snv-release.md snv-transport.md
SPEC_PATHS := spec/
ifneq ($(shell uname -s),Linux)
SPEC_PATHS := $(filter-out $(addprefix spec/,$(SPEC_LINUX_ONLY)),$(wildcard spec/*.md))
endif

spec:          ## outside-in CLI contracts
	cargo build --locked --quiet --package pangopup-cli --package pangopup-build
	rm -rf target/spec-cache
	install -d -m 700 target/spec-cache
	XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" PATH="$(CURDIR)/target/debug:$$PATH" mustmatch test $(SPEC_PATHS)
