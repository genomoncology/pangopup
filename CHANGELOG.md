# Changelog

What changed between released versions of PangoPup, newest first. Each section
describes what a consumer sees. A version that must be acted on before upgrading
says so first. The tickets, records and `planning/` folder carry the internal
work; this file does not.

`snv-grch38-v1` and `runtime-grch38-v1` tag the separately versioned scoring
assets. They are not software releases and have no section here.

## 0.5.0 - unreleased

**Published output gains five fields, and one of them is inserted between two
existing keys.** A reader that compares whole payloads byte for byte, or that
refuses a property it does not know, breaks. A reader that takes fields by name
and ignores the rest keeps working.

- Every structured score record and every `source_reference_ambiguities[]`
  entry gains `gene_names`, on the command line and over HTTP. The field stands
  between `stable_gene` and `gain_score`. A gene the index cannot name carries
  no `gene_names` object at all.
- Every command-line score line gains `provenance.software_version`.
- The HTTP status root and every HTTP score item gain `data_set_version`.
- The HTTP status root gains `runtime_profile_id` and `scoring_semantics`.

[`architecture/compatibility.md`](architecture/compatibility.md) enumerates the
whole response shape and what each field is for.

**The model cache is emptied once.** The cache layout stamp moved from 1 to 2
and the software version joined the recorded setup, so the first run after the
upgrade discards the file the previous release wrote and prints
`discarded model cache <path>: an earlier layout wrote it` on standard error.
That run still returns its answer. There is no migration and no flag.

No published asset digest moved. A consumer holding assets installed by 0.4.1
re-syncs nothing.

`gene_names` comes from a gene-name index built into the executable. It is not
downloadable and not installable, and gene names refresh only by upgrading.
[`NOTICE`](NOTICE) records HGNC, NCBI Gene and GENCODE as the sources it was
built from. The container image gains that index and changes nothing else.

Store `data_set_version` beside a retained HTTP score. It hashes the software
version and the runtime profile identity. `scoring_identity` hashes the
effective CPU policy as well, so `--model-threads` moves `scoring_identity`
while the scores stay the same.

The command-line commands and flags, the four HTTP routes and their request
shape, the installer, and the GPL-3.0-only licence are unchanged.

## 0.4.1 - 2026-09-06

`pangopup uninstall --full --yes` completes removal of a normal read-only
managed installation. It removes the managed data and cache first and the
executable last, so a failed removal leaves the executable in place.

Release qualification stopped failing for reasons unrelated to the software it
qualifies.

The HTTP, JSON, command-line and scoring contracts do not change from 0.4.0,
except for the reported software version and the scoring identity derived from
it. Assets do not change, and an existing installation stays usable through
`pangopup sync --offline`.

## 0.4.0 - 2026-09-06

**Response shapes change from 0.3.0.** A strict reader must accept the whole
inventory before a producer emits it. The status root adds `scoring_identity`
and `request_contract`; the status `model` object adds `work_unit`,
`planning_millis_per_unit` and `full_capacity_planning_seconds`; every score
item adds `input` and `scoring_identity`; a score item may now be `rejected` and
carry `error` and `reason`; every structured score record adds `stable_gene`.
[`architecture/compatibility.md`](architecture/compatibility.md) enumerates it.

Every valid HTTP batch without an operational failure returns HTTP 200 with one
ordered outcome per submitted variant. One rejected item no longer discards its
valid neighbours. Every item echoes the string that was submitted.

`/v1/status` publishes the active scoring identity, the request contract and
model-work planning guidance. A request above the model-work limit returns
`MODEL_BATCH_TOO_LARGE`, and temporary queue saturation returns `Retry-After`.

The command line and the service accept exact unpadded insertions and deletions,
common mitochondrial contig spellings, and stable, versioned and `_PAR_Y`
Ensembl gene identifiers.

`stable_gene` is the stable Ensembl grouping and filter key. `gene` remains the
source-reported identity. Keep `gene` where exact version or PAR identity
matters.

The scoring assets do not change. The scoring identity moves because the running
software version is one of its inputs.

PangoPup published no 0.4.0 container image. The first published image carrying
this response shape is 0.4.1.

## 0.3.0 - 2026-08-05

`pangopup uninstall` shows the resolved executable, data and cache paths before
offering code-only removal, complete removal, or cancellation. `--full` selects
complete removal and `--yes` makes either scope noninteractive.

The README states exact download and installed sizes and measured Linux memory
and latency guidance.

Published as a Linux x86-64 executable and as a native Linux AMD64/ARM64
container image. Both forms identify the same source revision.

This is an application-code release. It does not rebuild or replace the
`snv-grch38-v1` or `runtime-grch38-v1` assets, and the score semantics, signed
loss values, lookup-first default and `--model-only` behaviour are unchanged
from 0.2.0.

## 0.2.0 - 2026-08-04

`pangopup serve` provides bounded foreground HTTP scoring on `/livez`,
`/readyz`, `/v1/status` and `/v1/score`.

`pangopup lookup --model-only` bypasses the SNV index and scores through the
model.

`pangopup sync --progress` reports phases and byte progress. Safe partial
downloads resume, transient failures retry, and `--quiet` keeps standard error
silent without changing the final JSON result.

Every runtime command and asset namespace has focused `-h` and `--help`.
`pangopup status` works against a read-only installed-data mount.

Automatic and explicit model-only results both reuse the persistent SQLite
cache.

The scoring contract is unchanged from 0.1.0, and the asset identities are the
ones 0.1.0 used.

## 0.1.0 - 2026-07-31

The first release. A Linux x86-64 executable that memory-maps an index of
published Pangolin scores for GRCh38 SNVs, runs Pangolin-compatible CPU
inference for a supported lookup miss or non-SNV, synchronizes the separately
published SNV and runtime assets with checksum verification, caches model
results in SQLite, and writes JSON Lines or tab-separated output.

It requires Linux x86-64/amd64 with GLIBC 2.39 or newer. The installer places
the executable under `$HOME/.local/bin` unless `PANGOPUP_INSTALL_DIR` says
otherwise, and downloads no runtime data. `pangopup sync` installs the pinned
assets separately.
