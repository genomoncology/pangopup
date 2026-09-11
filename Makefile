.PHONY: lint test spec gene-name-index

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

PORTABLE_QUALIFICATION := tests/build-directory-residue.sh tests/built-executable-currency.sh tests/changelog-release-coverage.sh tests/ci-platform-support.sh tests/ci-service-fixture-evidence.sh tests/ci-test-failure-evidence.sh tests/cli-spawn-cache-isolation.sh tests/harness-rule-coverage.sh tests/inherited-cache-variables.sh tests/model-cache-layout-history.sh tests/model-cache-limit-inheritance.sh tests/negative-assertion-strength.sh tests/pipeline-match-integrity.sh tests/published-claim-evidence.sh tests/qualification-runner-cache-isolation.sh tests/readme-budget-exactness.sh tests/recipe-cache-rule-coverage.sh tests/recipe-spawn-cache-isolation.sh tests/release-help-contract.sh tests/repository-sourcing.sh tests/route-disagreement-rate.sh tests/rust-literal-continuity.sh tests/shell-matching-determinism.sh tests/shell-spawn-cache-isolation.sh tests/spec-block-execution.sh tests/spec-cargo-filter-evidence.sh tests/spec-download-cache-durability.sh tests/spec-record-pin-completeness.sh tests/spec-refutation-evidence.sh tests/version-consistency-python39.sh tests/workflow-command-anchoring.sh tests/workflow-setting-anchoring.sh


lint:          ## static analysis: rustfmt + clippy + dependency policy
	python3 scripts/check-version-consistency.py
	cargo fmt --all --check
	cargo clippy --locked $(WORKSPACE_TESTS) --all-targets -- -D warnings
	cargo deny check advisories bans licenses sources --warn unmaintained

# The spawn helper drops the four inherited cache variables for every child it
# starts, which is every route that runs the built executable from a test. The
# unit tests inside `pangopup-cli` are not children: they parse a lookup in the
# test process itself, and `resolve_model_cache_options` reads
# `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` out of that process. Measured on this
# recipe: an operator who exported it at a value the product cannot parse saw
# six unit tests panic with `invalid model cache configuration`, so the suite
# could not be run at all. The drop belongs on the run that starts the test
# process, which is this line.
test:          ## inside-out unit and integration tests
	env -u PANGOPUP_MODEL_CACHE -u PANGOPUP_CACHE_DIR -u PANGOPUP_DATA_DIR -u PANGOPUP_MODEL_CACHE_MAX_ENTRIES cargo test --locked $(WORKSPACE_TESTS)
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

# Cargo keeps its registry under `$$HOME/.cargo` and rustup the toolchains under
# `$$HOME/.rustup`, and spec blocks run `cargo test` through
# `scripts/spec-cargo-test.sh`. Moving `HOME` without pinning these makes a
# rustup shim install the toolchain from the network into `target/spec-cache`,
# paid on every run because the recipe removes that directory first. Each is
# pinned to what it resolved to before the move, the way
# `tests/support/private-cache-home.sh` pins them.
#
# `ORT_CACHE_DIR` is the same accident one layer down. `ort-sys` builds with
# `download-binaries` and keeps the 87 MB ONNX Runtime static library under the
# cache directory it resolves, which on Linux is `XDG_CACHE_HOME`. Left
# unnamed, that library would land in `target/spec-cache` and be removed before
# every run, so any run that rebuilt `ort-sys` would fetch it again over the
# network.
#
# It is pointed at `$$HOME/.cache/ort.pyke.io`: the directory `ort-sys` itself
# resolves on Linux when neither `ORT_CACHE_DIR` nor `XDG_CACHE_HOME` names one,
# which is where `make lint` and `make test` already keep that library. One copy
# on the machine therefore serves every gate, and the recipe writes into it
# rather than beside it. An operator who exports `XDG_CACHE_HOME` moves the
# copy those two gates fill and keeps this one where it is, and pays a second
# copy for as long as that export stands.
#
# A cache of its own under the checkout is the alternative. It costs a second
# 87 MB copy and a first `make spec` that pays the download again. This recipe
# already pins `CARGO_HOME` and `RUSTUP_HOME` to the operator's own for the same
# reason: a downloaded build input is not what the private cache home isolates.
# Nothing here removes that directory, and the model cache home below is still
# emptied on every run.
#
# `target/ort-cache` is what it named before. `cargo clean` collects all of
# `target/`, so the run after a routine clean paid the 87 MB again.
# `tests/spec-download-cache-durability.sh` holds both halves: outside every
# directory this recipe removes, and outside the build directory. Measured on
# 2026-09-11: a `cargo clean` that emptied `target/`, followed by `make spec`,
# wrote 0 bytes into `$$HOME/.cache/ort.pyke.io` and left all eleven entries
# there byte-identical, and `ort-sys` logged no download.
#
# On macOS `ort-sys` resolves `$$HOME/Library/Caches/ort.pyke.io` instead, so
# `make spec` there keeps a second copy under `$$HOME/.cache/ort.pyke.io`. That
# is the cost rejected just above, paid on a Mac rather than on every machine.
# It is paid once, and afterwards the copy is durable and shared between
# `make spec` runs. The gates that need a real production release are
# Linux-only, and naming one directory keeps the recipe readable by the rule
# that holds it.
#
# It is named on both lines that build, not only on the line that runs the spec
# suite. The suite builds through `scripts/spec-cargo-test.sh` and the first
# line builds on its own, and a name on one of them leaves the other resolving
# the cache of whoever ran make. Measured: with the name on the suite line
# alone, a `spec` run that rebuilt `ort-sys` under a transient
# `XDG_CACHE_HOME` wrote 90647244 bytes there from the first line, which is the
# download this recipe exists to stop paying.
spec:          ## outside-in CLI contracts
	env ORT_CACHE_DIR="$$HOME/.cache/ort.pyke.io" cargo build --locked --quiet --package pangopup-cli --package pangopup-build
	rm -rf target/spec-cache
	install -d -m 700 target/spec-cache
	env -u PANGOPUP_MODEL_CACHE -u PANGOPUP_CACHE_DIR -u PANGOPUP_DATA_DIR -u PANGOPUP_MODEL_CACHE_MAX_ENTRIES CARGO_HOME="$${CARGO_HOME:-$$HOME/.cargo}" RUSTUP_HOME="$${RUSTUP_HOME:-$$HOME/.rustup}" ORT_CACHE_DIR="$$HOME/.cache/ort.pyke.io" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" PATH="$(CURDIR)/target/debug:$$PATH" mustmatch test $(SPEC_PATHS)

# Maintainer-run. This target reaches the network. No gate invokes it. It is
# defined after the gates so that no gate recipe can name it.
#
# HGNC publishes an immutable dated monthly TSV. NCBI replaces
# Homo_sapiens.gene_info.gz at a fixed URL and keeps no dated archive, so a
# later run reads different NCBI bytes. The committed index and the record
# beside it are the durable artifacts.
HGNC_RELEASE := 2026-09-04
HGNC_TSV_URL := https://storage.googleapis.com/public-download-files/hgnc/archive/archive/monthly/tsv/hgnc_complete_set_$(HGNC_RELEASE).tsv
NCBI_GENE_INFO_URL := https://ftp.ncbi.nlm.nih.gov/gene/DATA/GENE_INFO/Mammalia/Homo_sapiens.gene_info.gz
NAMING_SOURCES := target/naming-sources

gene-name-index:  ## maintainer-run: download both naming sources and rebuild the committed index
	install -d -m 755 $(NAMING_SOURCES)
	curl --fail --location --output $(NAMING_SOURCES)/hgnc_complete_set_$(HGNC_RELEASE).tsv $(HGNC_TSV_URL)
	curl --fail --location --output $(NAMING_SOURCES)/Homo_sapiens.gene_info.gz $(NCBI_GENE_INFO_URL)
	cargo run --locked --quiet --package pangopup-build -- naming build \
	  --hgnc $(NAMING_SOURCES)/hgnc_complete_set_$(HGNC_RELEASE).tsv \
	  --ncbi $(NAMING_SOURCES)/Homo_sapiens.gene_info.gz \
	  --output assets/gene-names/gene-names.pgn
