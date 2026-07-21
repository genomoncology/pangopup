# Pangopup

Pangopup is a standalone GPL-3.0 Rust service for high-performance,
Pangolin-compatible splice scoring on GRCh38 genomic variants.

It answers each request through one of two paths:

1. **SNV lookup:** return an exact precomputed Pangolin result from a compact,
   memory-mapped index.
2. **Model fallback:** when no lookup record exists, run the bundled Pangolin
   model against a local GRCh38 sequence window and splice-site annotation.

An SNV is a single-nucleotide variant: one reference base replaced by one
alternate base. The published Zenodo dataset already contains masked Pangolin
scores for every SNV it covers, so recomputing those values with the neural
network would be slower and could introduce small numeric differences.

## How one request works

Pangopup accepts an explicit GRCh38 genomic variant:

```json
{
  "assembly": "GRCh38",
  "contig": "17",
  "position": 43106534,
  "ref": "C",
  "alt": "A"
}
```

The service routes it as follows:

```text
GRCh38 chromosome + position + REF + ALT
                    |
                    v
             validate the variant
                    |
          +---------+----------+
          |                    |
      SNV index hit       no index record
          |               or a non-SNV
          v                    |
 exact published score         v
                       supported by model?
                            |       |
                           yes      no
                            |       |
                            v       v
                       model score  typed no-score result
```

The response identifies whether its values came from `precomputed` lookup or
`model` inference. Because genes can overlap and Pangolin masking is
gene-specific, one genomic variant can return several source-gene score records.
A caller may provide an optional Ensembl gene filter; Pangopup never guesses a
single best gene.

Pangopup deliberately does not implement HGVS, transcript/protein projection,
clinical interpretation, or general gene annotation. Callers must identify one
concrete GRCh38 genomic variant before asking for a splice score.

## Why lookup comes first

The Zenodo score source contains 4,099,255,665 SNV rows across 19,913
protein-coding genes. Pangopup compiles those text files into a purpose-built
binary index that exploits their genomic ordering, repeated defaults, and
three-alternates-per-locus structure.

Logically this behaves like a key-value store:

```text
(GRCh38 contig, position, REF, ALT) -> one or more gene-specific score records
```

Physically it is not a generic hash table or embedded database. Private v1 uses
an immutable 11-byte record per ordinary locus, contiguous gene segments, and a
balanced per-contig overlap tree. That shape removes text parsing and
decompression from the request path and avoids loading billions of ordinary key
objects.

The complete-corpus fixed payload projects to about 14.0 GiB before small
directories and provenance. This is deliberately larger than the 1.589 GiB
hierarchical sparse candidate: the real-corpus benchmark found fixed lookup
consistently faster on the equal candidate harness after direct was corrected
to use ranked zero-copy mmap lookup, and query speed is the first accepted
priority. The
installed file is memory-mapped, so Pangopup reads only the directory and
record pages needed by a query rather than copying the file into heap.

## Runtime assets

A full Pangopup installation uses four versioned data assets:

| Asset | Used for | Original source | Installed form |
|---|---|---|---|
| SNV score index | Fast path | Zenodo precomputed scores | Fixed 11-byte mmap file |
| Model weights | Fallback | Upstream Pangolin checkpoints | Verified Rust-runtime representation |
| GRCh38 sequence | Fallback sequence window and REF validation | NCBI RefSeq GRCh38.p14 FASTA | Compact indexed mmap file |
| Splice mask | Gene strand, spans, and exon boundaries | GENCODE release 38 annotation | Compact interval/boundary mmap file |

NCBI supplies the reference genome sequence; it does not supply the Pangolin
model. Pangopup publishes a pinned copy or verified conversion of the upstream
model as its own release asset.

The original NCBI reference is downloaded as FASTA when the reference asset is
built. A normal Pangopup installation downloads the compiled reference member,
not the raw FASTA. The service therefore performs bounded indexed sequence
reads rather than parsing FASTA during a request. The same principle applies to
GENCODE: GTF/gffutils is build input, not a runtime database.

## Automatic asset installation

Each Pangopup binary pins a compatible release manifest containing asset URLs,
sizes, SHA-256 digests, format versions, source identities, and licenses.

At service startup Pangopup:

1. resolves its platform data directory and requested asset profile;
2. takes an installation lock so concurrent processes cannot publish a partial
   bundle;
3. reuses a complete compatible bundle when one is already installed;
4. otherwise downloads missing transport archives to a temporary cache path;
5. verifies archive size and SHA-256 before extraction;
6. extracts and validates every member in a staging directory;
7. atomically publishes the immutable bundle;
8. memory-maps the installed members, initializes the selected model provider,
   and only then reports the service ready.

The first start is therefore also a provisioning operation and should expose
download and verification progress. Later starts use the already installed
bundle without contacting the network. A failed download or checksum never
replaces an older complete bundle and never starts with partial data.

Full hashes are checked during installation and by an explicit verification
command. Ordinary startup performs cheap manifest, size, version, and structural
checks rather than rereading several gigabytes and defeating fast startup.

On Linux, durable assets live under:

```text
${XDG_DATA_HOME:-$HOME/.local/share}/pangopup/bundles/<bundle-id>/
```

Temporary downloads may use:

```text
${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/
```

The data directory is authoritative and must not be treated as disposable
cache. macOS and Windows builds use their standard application-data locations.
`PANGOPUP_DATA_DIR` can override discovery.

Automatic provisioning is the normal service experience. Air-gapped and
container deployments can preinstall the same assets with `pangopup assets
install`; an offline mode refuses network access and reports exactly which
pinned asset is missing.

## Performance priorities

After correctness, Pangopup optimizes in this order:

1. query latency and throughput;
2. resident memory and pages touched;
3. compressed download size.

The raw, warm SNV lookup should operate on the microsecond scale. A long-lived
local HTTP request should remain sub-millisecond when it is a lookup hit.
Cold-page faults can take longer, and model inference is orders of magnitude
more expensive than lookup. These are design targets, not performance claims;
the release gate will report warm and cold p50/p95/p99 latency, throughput,
allocations, resident memory, page faults, and bytes touched.

Memory mapping does not mean that the index uses literally no RAM. It means the
operating system loads file pages only as they are touched and can reclaim them
under pressure. The process may show a large virtual address mapping while its
resident working set remains much smaller. Model weights and active inference
tensors consume ordinary resident memory and are measured separately.

The release archive may use strong compression because download encoding is not
the runtime encoding. Installation expands it once into the fixed mmap form.
This deliberately spends disk space to avoid per-query decompression.

## Current state

Implemented today:

- the four-crate Rust workspace and strict lint/test/spec gates;
- CLI help/version behavior with two executable smoke specs;
- GPL-3.0 source licensing, upstream Pangolin attribution, and CC BY 4.0
  dataset attribution;
- a retained Rust analyzer that scanned the complete downloaded score corpus;
- complete-corpus entropy, sparsity, and candidate-format measurements;
- six deterministic, attributed excerpts of the real score source, including
  overlapping genes, both published `REF=N` shapes, and every coordinate gap;
- exact GRCh38 SNV, Ensembl gene, centi-score, and relative-position Rust types;
- bounded-memory gzip/TSV validation plus an observable source-inspection
  command, `pangopup-build inspect <SOURCE_DIR>`;
- measured fixed/direct/Zstd/LZ4/Tabix comparison on a deterministic real lab
  corpus, selecting and hardening the fixed 11-byte private v1 format;
- deterministic miniature fixed-index writing, structurally checked mmap open,
  exact lookup/exception round trips, and `pangopup-build prototype-roundtrip`;
- the standalone API, runtime-data, delivery, and performance decisions.

Not implemented yet: the complete-corpus streaming/certified index build, a
stable public provider trait or lookup result, real CLI lookup, automatic asset
manager, model runtime, HTTP service, or result cache.

The development order is:

1. checked source fixture and executable source contract (complete);
2. measured miniature index writer/reader (complete);
3. full streaming builder, complete index certification, and CLI;
4. release packaging and automatic asset installation;
5. compatible model fallback and compact reference/mask assets;
6. unified HTTP service and end-to-end performance proof.

See [`planning/frontier.md`](planning/frontier.md) for the current boundary and
[`architecture/README.md`](architecture/README.md) for the durable design.

## Workspace

- `pangopup-core` — public typed vocabulary, routing, and provider capabilities;
- `pangopup-index` — private format codec and validated mmap reader;
- `pangopup-build` — offline source validation and deterministic artifact
  builders;
- `pangopup-cli` — command-line and asset-management adapter;
- future `pangopup-assets` — shared download, verification, and installation;
- future `pangopup-model` — model execution behind the core provider contract;
- future `pangopup-http` — long-lived HTTP adapter over the same core.

## Development

```bash skip
make lint
make test
make spec
```

Pangopup source is licensed under GPL-3.0-only. Pangolin model/source notices
and the score dataset's separate CC BY 4.0 attribution are recorded in
[`NOTICE`](NOTICE) and must travel with applicable release assets.
