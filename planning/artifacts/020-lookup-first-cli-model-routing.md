# Ticket 020 lookup-first CLI model-routing evidence

Date: 2026-07-26

## Result

`pangopup-engine` now routes an owned literal GRCh38 request through the
precomputed provider first. A score record or source-reference ambiguity is
authoritative. When fallback is enabled, only a pure SNV miss or a supported
non-SNV reaches one identity-bound model scorer; all containing genes are
masked before an optional stable-gene filter is applied.

The CLI enables that route only when all three explicit local fallback paths
are present. With no fallback paths, the established SNV-only interface is
unchanged, including its honest `not_found` results. A batch opens the
descriptor-identified reference and mask plus the model at most once and only
when required. JSONL carries exact model/reference/mask provenance and ordered
warnings; table output retains the established columns.

## Checked synthetic route

The file-backed normal-test route is deliberately synthetic and cannot be
installed as production GRCh38:

- reference source FASTA: 11,136 bytes,
  SHA-256 `81a5af971ad9c72b3a679a678ca05f2b050a865a622e272abc77eb1be43c3eb8`;
- assembly report: 2,112 bytes,
  SHA-256 `ca184c4c4448bdb9af66899d1d84f7925ab993528f774fe843a45825015a9c5f`;
- reference NOTICE: 291 bytes,
  SHA-256 `57598dd8a7e6c8159d1e0e4af9660e2d567b4f383d72740ba9200f31bf7faa68`;
- `reference.pgr`: 6,648 bytes,
  SHA-256 `fcd1441d5ff6d703acd52f5766ca597c6202044d4e3b330726d3460707cad880`;
- reference manifest/bundle ID:
  `6773713ad79462b8bfb2bce7f194041e85a0804b38f68282c965adc5f43f9493`;
- logical sequence-set SHA-256:
  `afb720dad5979f65694dab6ae80a497ef56db434d7d346e79cdcb0e7da97e0b3`;
- route-mask semantic oracle: 412 bytes,
  SHA-256 `c897027053cda82f232544c902151ebcecd61f818b57f3744d336577255aa165`;
- `domains.pgm`: 260 bytes,
  SHA-256 `004f9f95be50b92fd5c67ca44a785e950c20e5455a903ad9350b68c91566f827`.

The reference contains all 25 required accessions in canonical order: chr1 is
exactly 10,101 `A` bases and the other 24 contigs contain one `A` each. The
route-mask oracle contains exactly plus-strand `ENSG00000000001.1`, rank zero,
with effective domain `(1,10101]` and no annotated boundaries. The checked
request at chr1:5,051 therefore forces the complete 10,101-base context and
the `no_annotated_sites` warning. A fixture-only literal encoder reproduces the
260 checked mask bytes; no general mask writer was restored.

Adding the distinct `pangopup-reference-route-test-v1` profile changed the
future reference-builder source identity to
`4bc0e93b83b28e235a7d0f498976bfe1e97b39d13e4f8c940d4c03cfd3d641bf`.
The corresponding current miniature migration manifest is
`8617204d0678ea23aa00e288e94bbf2622cf3884cf26562f65fb85eda5b18bd2`.
Existing miniature source/member bytes and all production bytes and identities
remain unchanged.

## Exactness and failure controls

Focused tests prove:

- authoritative records and ambiguities never request model completion;
- filtered and unfiltered pure misses request it, while non-SNVs skip lookup;
- mask queries receive all genes before stable-gene filtering;
- the explicit fallback path hashes the complete bounded `reference.pgr`,
  verifies its manifest size and SHA-256, then mmaps and retains that same
  authenticated descriptor;
- same-size reference corruption, a reference symlink, mutation or pathname
  replacement during hashing, and post-open pathname substitution are covered;
  the last test proves queries still read the authenticated mmap;
- the same retained regular single-link descriptor supplies both observed mask
  identity and mmap queries;
- mask symlinks, mutation during hashing, and pathname replacement during
  hashing are rejected;
- the ordinary installed reference open remains the cheap structural path;
  the complete reference hash is paid only when explicit model fallback is
  actually required;
- hit-only CLI batches do not inspect fallback paths;
- a non-SNV without fallback flags returns `MODEL_ASSETS_REQUIRED` before an
  invalid or missing SNV bundle can be observed, with no partial stdout;
- mixed/model batches open reference, mask, then model exactly once;
- component failures are stable and redact supplied paths;
- expected rejection, operational scoring failure, and late-batch failure emit
  no partial stdout;
- the non-SNV `GRCh38:chr1:5051:A:AC` and flagged SNV miss
  `GRCh38:chr1:5051:A:C` both traverse the real synthetic PGRREF01, domains
  mmap, ONNX Runtime, scorer, router, and renderer; and
- all 1,000 frozen legacy requests and all seven unchanged CLI batches still
  match their exact precomputed oracle, including six pure misses.

No normal test uses production assets, Python, PyTorch, network access, SQLite,
or another repository.

## Non-gating routed-hit benchmark

Command:

```text
cargo bench --locked -p pangopup-cli --bench snv_regression
```

The benchmark derives 994 authoritative requests from the frozen 1,000-case
corpus in original order. Only the 1,000-row routed sample appends the first
six authoritative rows again. Every sample first compares the selected and
repeated JSONL bytes with the correspondingly selected and repeated frozen
oracle. The original regression remains a separate unchanged 1,000-request
lookup-only corpus with six misses.

| Mode | Requests | Results | p50 µs | p95 µs | p99 µs | Allocations/sample | Bytes/sample | Minor faults/sample | Major faults/sample | RSS delta KiB | Output bytes |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| fresh open | 0 | 0 | 102 | 134 | 147 | 1,598 | 76,949 | 1 | 0 | 0 | 0 |
| legacy fresh process | 1 | 1 | 4,523 | 5,369 | 5,369 | 20 | 1,542 | 0 | 0 | 0 | 491 |
| warm router + JSONL | 1 | 1 | 2 | 2 | 2 | 23 | 1,664 | 0 | 0 | 0 | 491 |
| warm router + JSONL | 10 | 10 | 14 | 14 | 21 | 152 | 22,776 | 0 | 0 | 0 | 4,919 |
| warm router + JSONL | 100 | 100 | 138 | 170 | 186 | 1,389 | 194,856 | 0 | 0 | 0 | 49,116 |
| warm router + JSONL | 1,000 | 1,012 | 1,911 | 2,156 | 2,490 | 13,753 | 1,687,840 | 217 | 0 | 0 | 492,283 |

Ticket 006's provider/render baseline reported warm p50/p95 of `2/2`,
`17/18`, `193/202`, and `1639/1771` microseconds for 1/10/100/1,000 requests.
The last corpus is not identical because the routed benchmark excludes six
misses then repeats six hits. The additional owned route value is visible in
allocated bytes, and the legacy fresh-process diagnostic increased from
1,909 to 4,523 microseconds after linking the model-capable executable. These
are same-host observations, not hardware gates or cold-I/O claims.

## Pre-review retained production qualification

The coordinator ran exactly one bounded M09 CLI request after the focused
synthetic tests and before code review:

```text
pangopup lookup \
  --bundle <accepted-snv-bundle> \
  --model-bundle <accepted-model-bundle> \
  --reference-bundle <accepted-reference-bundle> \
  --mask <accepted-mask-member> \
  --variant GRCh38:chr12:6801303:G:GA \
  --format jsonl
```

It exited zero and returned exactly one ordered
`ENSG00000010610.10` record: gain `0.00` at `0`, loss `0.00` at `-50`,
and no warnings. This matches frozen compatibility case
`M09-insertion-short-plus`.

The emitted provenance reported:

- model
  `sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`,
  profile `pangolin-1.0.2-5cf94b8-onnx-cpu-v1`;
- reference
  `sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`,
  profile `refseq-grch38p14-primary-v1`, sequence set
  `sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4`;
  and
- the 6,703,320-byte mask member
  `sha256:714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`.

The command only opened the four preserved runtime inputs. It did not rebuild,
convert, download, or scan an upstream source corpus.

This run preceded the descriptor-authenticated reference remediation described
above. It is retained to show what the first code review evaluated, but it is
not the final production qualification.

## Post-remediation retained production qualification

After the same-descriptor reference authentication was implemented and its
focused synthetic corruption/race tests passed, the coordinator reran only the
same bounded M09 request. The explicit fallback open authenticated the complete
manifest-declared `reference.pgr` member before inference. The command exited
zero and returned the same exact ordered record and provenance listed above:
`ENSG00000010610.10`, gain `0.00` at `0`, loss `0.00` at `-50`, no warnings,
and the accepted model/reference/mask identities.

The second run is the final production qualification for the reviewed
reference-open implementation. Across both review stages the coordinator ran
two single-case M09 requests, not the 14-case corpus or a full-source
verification. Neither run rebuilt, converted, downloaded, or published an
asset.

## Final gate

After independent code re-review accepted all remediations:

```text
make lint
  passed
make test
  passed (workspace exit 0)
make spec
  167 passed, 2 skipped
git diff --check
  passed
```

## Limits

This outcome does not select a production CPU policy, batch model contexts,
install or publish a coherent four-asset profile, cache modeled results, or add
HTTP, Docker, systemd, process supervision, HGVS, or normalization.
