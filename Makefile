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

PORTABLE_QUALIFICATION := tests/built-executable-currency.sh tests/ci-platform-support.sh tests/ci-service-fixture-evidence.sh tests/ci-test-failure-evidence.sh tests/cli-spawn-cache-isolation.sh tests/model-cache-layout-history.sh tests/release-help-contract.sh tests/route-disagreement-rate.sh tests/spec-cargo-filter-evidence.sh tests/spec-refutation-evidence.sh tests/version-consistency-python39.sh


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
