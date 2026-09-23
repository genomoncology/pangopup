# PangoPup v0.5.0 release notes

## Scoring assets and values

The separately versioned [`snv-grch38-v2`](https://github.com/genomoncology/pangopup/releases/tag/snv-grch38-v2) and [`runtime-grch38-v2`](https://github.com/genomoncology/pangopup/releases/tag/runtime-grch38-v2) releases provide the smaller sparse SNV index and its matching runtime profile. Run `pangopup sync` after upgrading. Sync stages and verifies the complete pair before activating it. The fixed v1 pair remains available for an explicit rollback.

Scores still have two decimal places. The published SNV source provides hundredths, and PangoPup rounds model values to hundredths. A third decimal place is not available from both routes. Preserve `provenance.kind` with a retained score: precomputed and modeled results can differ, even for the same SNV. No whole-genome throughput or non-SNV latency improvement is claimed for this release.

## Consumer compatibility

Every structured score record and source-reference ambiguity gains `gene_names` where the built-in gene-name index has a name. `stable_gene` remains the grouping key. The command-line score line gains `provenance.software_version`. The HTTP status response gains `data_set_version`, `runtime_profile_id`, and `scoring_semantics`; every HTTP score item gains `data_set_version`. Store the item's `data_set_version` beside a retained HTTP score. `scoring_identity` also includes the effective CPU policy and can change when thread settings change without changing scores.

Readers that ignore unknown JSON fields remain compatible. Readers that reject unknown fields or compare complete payloads must accept the [v0.5.0 response-shape inventory](../../architecture/compatibility.md#v050-response-shape-inventory) before upgrading. The HTTP routes, request shape, command-line commands and flags, installer behavior, and licence do not change.

The model cache discards its previous layout once on first use after the upgrade. The request still completes; later model calls refill the cache. No cache migration is required.

## Install

The Linux x86-64 installer is:

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.5.0/install.sh \
  | bash -s -- --version 0.5.0
```

The native Linux AMD64 and ARM64 container image is:

```bash
docker pull ghcr.io/genomoncology/pangopup:0.5.0
```

The executable and container remain thin. Neither embeds the scoring assets. Run `pangopup sync` to install the separately released v2 assets before scoring.
