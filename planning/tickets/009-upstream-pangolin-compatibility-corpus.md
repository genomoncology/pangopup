# 009 — Retain a pinned upstream Pangolin compatibility corpus

Status: ready
Superseded contract identity: `sha256:0fc618afd1073c7592f2aaa8d65eb5d37f719c8da37fe5b7745fe0390ecd2e5d`
Accepted revised contract identity: `sha256:c31886f84cbea4144d7bde4573fec6ab1c15ba107694299aacc07dea28c177fd`
Base revision: `7563f90b7bda4a018833ca89cb628a26aed76c88`

## Outcome

`pangopup-build compatibility inspect --corpus <DIR>` validates a small,
self-contained GRCh38 corpus. Its scored cases and eligible rejection
observations are captured from pinned upstream Pangolin 1.0.2 CPU execution;
its two unsafe chromosome-boundary rejections are retained as independently
replayable slice bounds. The inspector replays masking, ordering, maxima,
positions, public two-decimal output, and rejection rules without Python,
PyTorch, model checkpoints, a whole reference genome, or a gffutils database.

This establishes the acceptance oracle for later Rust model work. It does not
implement model inference or rebuild, rescan, verify, download, install, or
change the shipped SNV index.

## Current facts and provenance

- Pangopup `main` at the base revision ships exact precomputed-SNV lookup,
  deterministic transport, Linux/XDG installation, immutable GitHub release,
  and pinned resumable sync. Model assets, inference, routing, HTTP, and Docker
  remain future work.
- The upstream authority is `https://github.com/tkzeng/Pangolin` commit
  `5cf94b8db938c658391b4305cd7ce33297d44ff7`. `setup.py` declares version
  `1.0.2`, but upstream has no `1.0.2` tag, automated tests, or CI. Its retained
  output is four GRCh37 BRCA rows, so those rows document upstream packaging;
  they are not the GRCh38 acceptance oracle.
- The upstream CLI loads exactly twelve checkpoints, in nested order
  `i = [0,2,4,6]`, then `j = [1,2,3]`, named
  `final.<j>.<i>.3.v2`. The corpus must bind each exact filename, order, byte
  size, and SHA-256. The other checkpoint files in the package are outside this
  profile.
- Upstream `compute_score` returns one unmasked loss array and gain array per
  evaluated strand after one-hot encoding, strand reversal, three-checkpoint
  mean, four-tissue minimum/maximum, and indel length reconciliation. Upstream's
  reference context has `10,100 + len(REF)` bases at `d=50`; model cropping and
  reconciliation leave exactly `100 + len(REF)` values. SNVs and anchored
  insertions therefore have 101 values, while equal-length MNVs and deletions
  retain longer arrays and may report positions through `len(REF) + 49`.
- Upstream masking mutates the arrays while iterating genes. Its gffutils
  `FeatureDB.region()` query has no explicit `ORDER BY`, so same-strand overlap
  output depends on the exact SQLite bytes and observed gene iteration order.
  Opposite strands use separate score arrays.
- The model reference is the compressed NCBI RefSeq GRCh38.p14 assembly
  `GCF_000001405.40`, transformed deterministically to uppercase, chr-named
  primary sequences for upstream compatibility. The exact compressed source,
  assembly report/accession mapping, transformation recipe, derived FASTA, and
  retained contexts all require identities; the assembly accession alone is
  insufficient.
- The masking authority is upstream's exact GENCODE release 38
  `gencode.v38.annotation.db`, plus the official comprehensive chromosome GTF
  `gencode.v38.annotation.gtf.gz` and upstream `Ensembl_canonical` database
  construction behavior. GENCODE v38 is GRCh38.p13; the corpus must keep its
  annotation identity distinct from the RefSeq GRCh38.p14 sequence identity.
- Existing Pangopup precomputed-score excerpts supply four useful cross-route
  SNV observations. They may seed case selection, but the unknown software,
  checkpoint, FASTA, and annotation identities behind the published Zenodo
  score set mean those values are observations, not the model oracle.
- The pinned CPython 3.13.5/pyfastx 2.3.1 upstream CLI cannot safely receive
  the two deliberately out-of-bounds chrM context cases. A recorded capture
  attempt reached `R05`/`R06` and terminated in the pyfastx native extension
  before Python could emit a warning. Those two cases therefore exercise the
  same deterministic slice arithmetic in Rust and are excluded from both
  unmodified CLI invocations; this native crash is documentary upstream
  behavior, not a compatibility requirement.
- No other GenomOncology project is an input, dependency, provenance source, or
  documentation reference for this corpus. Pangopup remains standalone.
- This is new compatibility infrastructure. The protected invariant is that a
  later runtime cannot claim Pangolin compatibility from architectural
  resemblance or a few rounded maxima; it must be tested against independently
  captured upstream inputs and outputs plus the two explicit boundary-rule
  witnesses.

## Scope

### Corpus and exact coverage

Add one checked, versioned `pangopup-compat-v1` corpus with the exact 24 cases
below. Coordinates are GRCh38, positions are 1-based, every real accepted case
uses `d=50`, and gene lists are the observed order from the pinned upstream
SQLite. `both` means Pangolin computes separate `+` and `-` arrays.

| ID | Input and origin | Evaluated strands and ordered genes | Array length | Required coverage |
|---|---|---|---:|---|
| `M01-snv-cd4-precomputed` | real `chr12:6801301 G>A` | `+:[ENSG00000010610.10]` | 101 | SNV, plus, lookup observation, low/zero |
| `M02-snv-wrap53-tp53-precomputed` | real `chr17:7686079 A>T` | `+:[ENSG00000141499.17]`; `-:[ENSG00000141510.18]` | 101 per strand | SNV, both strands, opposite-strand overlap, lookup observation, nonzero |
| `M03-snv-afap1l2-precomputed` | real `chr10:114306065 A>T` | `-:[ENSG00000169129.15]` | 101 | SNV, minus, lookup observation |
| `M04-snv-grk1-precomputed` | real `chr13:113723021 C>G` | `+:[ENSG00000185974.7]` | 101 | SNV, plus, lookup observation |
| `M05-snv-same-strand-overlap` | real `chr3:29000000 T>C` | `+:[ENSG00000283563.1,ENSG00000144642.22]` | 101 | SNV, same-strand overlap/order |
| `M06-snv-gene-start-plus-one` | real `chr12:6786859 A>G` | `+:[ENSG00000010610.10]` | 101 | SNV, gene-start-plus-one boundary |
| `M07-mnv-plus` | real `chr12:6801303 GG>AC` | `+:[ENSG00000010610.10]` | 102 | equal MNV, plus |
| `M08-mnv-both-strands` | real `chr17:7687421 GCCC>ATTA` | `+:[ENSG00000141499.17]`; `-:[ENSG00000141510.18]` | 104 per strand | equal MNV, both strands |
| `M09-insertion-short-plus` | real `chr12:6801303 G>GA` | `+:[ENSG00000010610.10]` | 101 | anchored insertion, short, plus |
| `M10-insertion-short-both` | real `chr17:7687421 G>GACG` | `+:[ENSG00000141499.17]`; `-:[ENSG00000141510.18]` | 101 per strand | anchored insertion, both strands |
| `M11-insertion-long-overlap` | real `chr3:29000000 T>TACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTAC` | `+:[ENSG00000283563.1,ENSG00000144642.22]` | 101 | anchored insertion, 50 inserted bases, same-strand overlap |
| `M12-deletion-short-plus` | real `chr12:6801303 GG>G` | `+:[ENSG00000010610.10]` | 102 | anchored deletion, short, plus |
| `M13-deletion-short-both` | real `chr17:7687421 GCCC>G` | `+:[ENSG00000141499.17]`; `-:[ENSG00000141510.18]` | 104 per strand | anchored deletion, both strands |
| `M14-deletion-ref100-overlap` | real `chr3:29000000 <REF100>>T` | `+:[ENSG00000283563.1,ENSG00000144642.22]` | 200 | anchored deletion, REF length 100 accepted, same-strand overlap |
| `R01-complex-replacement` | real `chr12:6801303 GG>AAA` | none; first operation `variant_shape_guard` | none | unequal complex rejection |
| `R02-deletion-ref101` | real `chr3:29000000 <REF101>>T` | none; first operation `deletion_length_guard` | none | REF length 101 rejection |
| `R03-reference-mismatch` | real `chr13:113723021 A>G`, retained true base `C` | none; first operation `reference_anchor_compare` | none | REF mismatch |
| `R04-no-containing-gene` | real `chr12:6691000 T>C` | empty observed query; first operation `get_genes_empty` | none | no-gene observation |
| `R05-left-context` | real `chrM:600 A>G`, contig length 16,569 | none; first operation `reference_slice` | none | insufficient left context |
| `R06-right-context` | real `chrM:16000 G>A`, contig length 16,569 | none; first operation `reference_slice` | none | insufficient right context |
| `P01-same-strand-order` | controlled vector `order-v1` | `+:[GENE_A boundary 99,GENE_B boundary 101]` at position 100, `d=2` | 5 | in-place order mutation |
| `P02-empty-boundaries` | controlled vector `empty-v1` | `+:[GENE_EMPTY boundaries []]` at position 100, `d=2` | 5 | empty-boundary masking/warning |
| `P03-first-extremum` | controlled vector `tie-v1` | unmasked at position 100, `d=2` | 5 | first-index extrema |
| `P04-rounding-signed-zero` | controlled scalar vector `round-v1` | formatting only | 6 scalars | two-decimal and signed-zero formatting |

`REF100` is exactly:

```text
TTTTTTGCACCTAAATTTAGGATTATATTCAAATAGCAAATGCCTTGAAGTGCTCTGATACTGAGCTTCCCAGTTTTTGTTGAGCTAGTGACATATTTGT
```

`REF101` is `REF100` followed by `T`.

Rejection categories are closed: `R01 = unsupported_variant_shape`,
`R02 = deletion_too_large`, `R03 = reference_mismatch`,
`R04 = not_in_gene`, and `R05`/`R06 = insufficient_reference_context` with
side `left`/`right`. Witnesses retain allele lengths 2/3 for `R01`, REF/ALT
lengths 101/1 and `2*d=100` for `R02`, true anchor base `C` for `R03`, the
bracketing annotation rows specified below for `R04`, required start `-4450`
against first coordinate 1 for `R05`, and required end 21,050 against chrM
length 16,569 for `R06`.

The four non-normative precomputed observations are fixed to these checked
Pangopup source-excerpt rows; the model corpus never silently substitutes a
different lookup record:

| Case | Checked source member | Exact record(s) |
|---|---|---|
| `M01` | `tests/fixtures/pangolin-precompute/ENSG00000010610.tsv.gz` | `chr12 6801301 G A 0.0 -50 -0.0 -50` |
| `M02` | `ENSG00000141499.tsv.gz`; `ENSG00000141510.tsv.gz` in that directory | `chr17 7686079 A T 0.21 18 -0.0 -50`; `chr17 7686079 A T 0.0 -50 -0.0 -50` |
| `M03` | `tests/fixtures/pangolin-precompute/ENSG00000169129.tsv.gz` | `chr10 114306065 A T 0.06 12 0.0 -50` |
| `M04` | `tests/fixtures/pangolin-precompute/ENSG00000185974.tsv.gz` | `chr13 113723021 C G 0.03 0 -0.0 -50` |

The manifest's ordered coverage list has exactly 28 cells:

```text
shape.snv, shape.mnv_equal, shape.insertion_anchored,
shape.deletion_anchored, strand.plus, strand.minus,
overlap.same_strand, overlap.opposite_strand, mask.masked, mask.unmasked,
boundary.gene_start_plus_one, indel.insertion_short, indel.insertion_long,
indel.deletion_short, indel.deletion_ref_100, lookup.precomputed_observation,
effect.zero_or_low, effect.nonzero, reject.complex_unequal,
reject.deletion_ref_101, reject.ref_mismatch, reject.no_gene,
reject.left_context, reject.right_context, postprocess.same_strand_order,
postprocess.empty_boundaries, postprocess.first_extremum,
postprocess.rounding_signed_zero
```

Every accepted model case carries `mask.masked` and `mask.unmasked`; the other
cells map exactly to the corresponding labels in the table. The inspector
requires all 28 cells and rejects an extra, missing, reordered, or unsupported
cell.

Controlled `f32` values are stored as exact big-endian display hex for their
32-bit IEEE-754 bit patterns:

- `order-v1`: gain
  `[3dcccccd,3f4ccccd,3e4ccccd,3f333333,3e99999a]`, loss
  `[bdcccccd,bf19999a,be4ccccd,bf000000,becccccd]`. With gene order A then B,
  masked A reports gain `0.7` at `+1` and loss `-0.6` at `-1`; masked B sees
  A's mutations and reports gain `0.3` at `+2` and loss `0.0` at `-2`.
- `empty-v1`: gain
  `[3dcccccd,3e4ccccd,3e99999a,3e4ccccd,3dcccccd]`, loss
  `[becccccd,be99999a,be4ccccd,bdcccccd,bf000000]`. Masking preserves the
  gain maximum `0.3` at `0`, clamps all loss to zero with first position `-2`,
  and observes `NoAnnotatedSitesToMaskForThisGene` as documentary prose.
- `tie-v1`: gain
  `[3dcccccd,3f4ccccd,3f4ccccd,3e4ccccd,00000000]`, loss
  `[bf000000,bdcccccd,bf000000,00000000,00000000]`; unmasked output chooses
  gain position `-1` and loss position `-2`.
- `round-v1`: bit strings
  `[00000000,80000000,3ba3d70a,bba3d70a,3f80a3d7,bf80a3d7]` format at two
  decimals as `[0.0,-0.0,0.0,-0.0,1.0,-1.0]` in the pinned NumPy environment.

For every accepted real case retain the exact forward input context. Upstream
slice coordinates are `context_start_1based = POS - 5,050`, zero-based anchor
offset `5,050`, and context length `10,100 + len(REF)`. For every evaluated
strand retain the two `100 + len(REF)` unmasked arrays returned after upstream
reconciliation. Every finite `f32` is encoded as exactly eight lowercase
hexadecimal digits representing the numeric 32 bits in network/display order;
Rust parses the digits to a `u32` and then uses `f32::from_bits` without byte
reinterpretation.

Also retain ordered containing genes and ordered absolute exon start/end
boundaries, exact masked and unmasked per-gene maxima/positions, and exact
unmodified-CLI output for `M01`–`M14`. `R01`, `R02`, and `R03` retain exact
documentary CLI warnings plus sufficient facts for independent rule replay.
`R04` retains the exact empty upstream query plus preceding rowid 244405
`ENSG00000126746.18:6666477-6689572(-)` and following rowid 244419
`ENSG00000139200.14:6693791-6700815(-)`, database identity, and CLI observation.
The inspector authenticates it as an observed rejection and does not claim a
24-case excerpt independently proves absence from the whole annotation.
`R05` and `R06` retain the exact context-bound witness and a closed documentary
reason that they were excluded from the unsafe upstream CLI path; Rust
independently replays their slice arithmetic.

### Fixed capture identities

The profile pins these twelve 2,877,321-byte checkpoints in load order:

| # | Filename | SHA-256 |
|---:|---|---|
| 1 | `final.1.0.3.v2` | `f0478fab173b75f7f7e9fe96688bad6c50fa4a46d70557f423b110caaf565501` |
| 2 | `final.2.0.3.v2` | `c4c6bb4880fa6fb28b14182ae3ea0600edb07056158f55325b5e6e6e48fc9f26` |
| 3 | `final.3.0.3.v2` | `ec685a6e7105a4486c1f89a005458a13deb3fe7171f13d434f4877e386d10676` |
| 4 | `final.1.2.3.v2` | `559c05de3e1ce65c2515ca3e92ef85edb0ec2e47686ca58060e25891ce06eb3a` |
| 5 | `final.2.2.3.v2` | `48758ba8b95eee9aa9feea52672ef06ca1b34111299c27f8a710f734d8b9aae5` |
| 6 | `final.3.2.3.v2` | `7cb576c2b24db4fdd6970c4ca4fb7c20ae1b1d8ae80645ebbe689848b5743129` |
| 7 | `final.1.4.3.v2` | `c50b12e0c0af776d5674ca5e346493f8265783494d4df383364de9c1136657f6` |
| 8 | `final.2.4.3.v2` | `e03303bed4fd6f135ec0f6c1b192cce954ea42d0646f44d17b4a6fbb2b1f610e` |
| 9 | `final.3.4.3.v2` | `9476d2e25520d7ff15bece0cd5d3b657e3b1dd3cc5fcab1d9c3b62bea7a0c5b6` |
| 10 | `final.1.6.3.v2` | `2aae563fa18a8a9b6699c6c96e0d32b8ec7543f8f805fb3bc9de77302cc9f66e` |
| 11 | `final.2.6.3.v2` | `7d3c0b1b2a60067b940dec315567874fbc8bcd322f1b7c76bf969f51f0f53f7f` |
| 12 | `final.3.6.3.v2` | `756e7721a382cace24e9bfea5b543af5623f2487d9a3efe7385e9c76367005fd` |

Other fixed identities:

- NCBI `GCF_000001405.40_GRCh38.p14_genomic.fna.gz`: 972,898,531 bytes,
  SHA-256 `11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3`;
  assembly report: 80,454 bytes, SHA-256
  `64318ddff470b69b261a667d813210044f60d4ce654253a547db80ff73638d38`.
- Capture-only six-contig FASTA (`chr3`, `chr10`, `chr12`, `chr13`, `chr17`,
  `chrM`, uppercase, original 80-base wrapping): 671,294,255 bytes, SHA-256
  `81645a227efbbd196ae337f743f31a5b1c32979d6d7bb5713e0322402a70fafb`.
- Upstream Dropbox `gencode.v38.annotation.db`: 380,366,848 bytes, SHA-256
  `221a61eec1f6934ae426d80599989c7b2ee4d9577b52e8a0e4bf02ccd73ca4a6`.
- Official EBI `gencode.v38.annotation.gtf.gz`: 46,556,621 bytes, SHA-256
  `22020df0d3356e965868f4b193e89fa13e838b950a574349f7fcd461ac01c050`,
  official MD5 `16fcae8ca8e488cd8056cf317d963407`.
- Capture environment: CPython 3.13.5, PyTorch 2.7.1+cpu, NumPy 2.5.1,
  pandas 3.0.3, pyfastx 2.3.1, gffutils 0.14, PyVCF3 1.0.4, Linux x86_64;
  CUDA disabled and both PyTorch thread counts forced to one.

The manifest and notices retain these literal authoritative URLs:

```text
https://github.com/tkzeng/Pangolin/tree/5cf94b8db938c658391b4305cd7ce33297d44ff7
https://www.dropbox.com/sh/6zo0aegoalvgd9f/AADOhGYJo8tbUhpscp3wSFj6a/gencode.v38.annotation.db?dl=1
https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_genomic.fna.gz
https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/000/001/405/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_assembly_report.txt
https://ftp.ebi.ac.uk/pub/databases/gencode/Gencode_human/release_38/gencode.v38.annotation.gtf.gz
https://ftp.ebi.ac.uk/pub/databases/gencode/Gencode_human/release_38/MD5SUMS
https://www.ncbi.nlm.nih.gov/home/about/policies/
https://www.gencodegenes.org/human/release_38.html
https://www.gencodegenes.org/pages/data_access.html
https://www.gencodegenes.org/pages/citing_gencode.html
```

The corpus-local `NOTICE` and root `NOTICE` name those URLs, the Pangolin
GPL-3.0 source/copyright and Genome Biology citation, NCBI source plus its
acknowledgment/disclaimer link, GENCODE source/citation/access terms, and state
that Pangopup transformed reference names/case contexts and annotation facts.

### Reproducible capture boundary

Add Rust orchestration in `pangopup-build`:

```text
pangopup-build compatibility capture \
  --upstream <PINNED_CHECKOUT> --python <ABSOLUTE_INTERPRETER> \
  --reference-source <PINNED_FNA_GZ> --assembly-report <PINNED_REPORT> \
  --reference <PINNED_SIX_CONTIG_FASTA> \
  --annotation-db <PINNED_DB> --annotation-gtf <PINNED_GTF_GZ> \
  --output <ABSENT_DIR>
```

The Rust command owns input hashing, exact case-plan selection, bounded child
execution/parsing, reference-context extraction, controlled vectors, canonical
serialization, member hashing, and atomic absent-directory publication. It
refuses any mismatched source revision, size, digest, package version, runtime
version, or checkpoint profile before inference.

A minimal source-controlled GPL Python helper, clearly marked as a Pangolin
wrapper/modification, imports the pinned upstream `compute_score` path only to
emit raw post-ensemble arrays and observed genes/boundaries. Rust invokes it
with CUDA disabled and one compute/interop thread. Separately, Rust invokes the
**unmodified** pinned upstream module as a CLI twice (`-m False` and `-m True`)
over `M01`–`M14` plus `R01`–`R04`. `R05` and `R06` are never passed to that
native slice path. Capture fails unless extrema/output derived from the helper
arrays agree with the independent unmodified CLI files for every scored case,
and unless the four eligible rejection observations agree. The helper does not
serialize the final corpus or supply the expected CLI strings.

Capture never downloads an input and never runs in normal gates. Its selected
input plan and controlled vectors are checked source; the 671 MB capture FASTA,
whole GTF/SQLite, checkpoints, temporary helper output, and unmodified CLI
files remain outside Git.

### Closed corpus storage

The checked corpus directory is flat and contains exactly three no-follow
regular files: `manifest.json`, `cases.jsonl`, and `NOTICE`. Extras, missing
members, directories, symlinks, and other nonregular entries fail before member
deserialization.

- `manifest.json` is at most 128 KiB. It is a closed `serde` struct with
  `deny_unknown_fields`, compact UTF-8 JSON in declared struct-field order and
  one terminal LF. It contains schema/profile strings, every fixed identity
  above, the ordered 28-cell coverage list, capture environment/helper hash,
  exactly 24 ordered case IDs, and size/SHA-256 records for `cases.jsonl` and
  `NOTICE`. Its own SHA-256 is the corpus identity but is not self-embedded.
- `cases.jsonl` is at most 3,800,000 bytes, has exactly 24 compact UTF-8 JSON
  lines in the table order, each at most 256 KiB, and ends in LF. Each line is a
  closed tagged enum (`model`, `rejection`, or `postprocess`) with common
  `id`, `coverage`, and provenance fields. Model records contain the exact input,
  context contract, per-strand raw arrays, ordered genes/boundaries, masked and
  unmasked expected outputs, unmodified CLI strings, and optional exact
  precomputed observations. Rejections contain first operation, sufficient
  witness, normalized category, and the closed tagged documentary upstream
  evidence defined below. Post-processing records contain the exact vectors
  and expectations above.
- `NOTICE` is at most 64 KiB, UTF-8 with LF line endings, terminal LF, and the
  minimum pinned source/license/citation/transformation statements above.
- Aggregate member bytes are at most 4 MiB. Before allocating a member buffer,
  the inspector opens the directory no-follow, proves exactly three entries,
  requires each declared file to be a regular single-link file owned through
  that opened directory, and checks its individual metadata bound. Strings are
  at most 8 KiB; IDs/hex tokens are at most 128 bytes; a model context is at
  most 10,200 bases; a score array is at most 200 values; at most four genes per
  strand and 512 boundaries per gene are accepted.

The closed field layouts are:

- Manifest, in order: `schema: string`, `profile: string`, `upstream: object`
  (`url`, `commit`, `declared_version`, `license`, `helper_sha256`),
  `checkpoints: array` (`ordinal: u8`, `filename`, `bytes: u64`, `sha256`),
  `reference: object` (`source_url`, `source_bytes`, `source_sha256`,
  `assembly_report_url`, `assembly_report_bytes`, `assembly_report_sha256`,
  `transform`, `derived_bytes`, `derived_sha256`, `contigs: array`),
  `annotation: object` (`database_url`, `database_bytes`, `database_sha256`,
  `gtf_url`, `gtf_bytes`, `gtf_md5`, `gtf_sha256`, `filter`,
  `logical_sha256`), `environment: object` (the named versions and enforced
  CPU/thread settings), `coverage: [string;28]`, `case_ids: [string;24]`, and
  `members: array` of exactly `cases.jsonl` then `NOTICE`, each with `filename`,
  `bytes`, and `sha256`.
- Every case begins, in order, with `id: string`, `kind: string`, and
  `coverage: [string]`. A model case then has `input` (`assembly`, `contig`,
  `position: u32`, `ref`, `alt`, `distance: u16`, `allele_shape`), `context`
  (`start_1based: u32`, `anchor_offset: u16`, `bases`, `sha256`), and `strands`
  in `+` then `-` order when present. Each strand has `strand`, `loss_bits`,
  `gain_bits`, ordered `genes` (`id`, `boundaries: [u32]`), and `expected`
  (`unmasked`, `masked`, `cli_unmasked`, `cli_masked`); each gene expectation
  contains gain/loss bit string and relative `i32` position. The optional
  `precomputed` array contains exact source member, gene, score bits, and
  positions and is present only on `M01`–`M04`.
- A rejection case has `input`, `first_operation`, `normalized_category`,
  closed tagged `witness`, and closed tagged `upstream_evidence`. The evidence
  is `{"kind":"cli","warning":<STRING>}` for `R01`–`R04` and
  `{"kind":"rule_replay","reason":"excluded_from_cli_native_reference_slice_crash"}`
  for `R05`/`R06`; all upstream evidence is documentary. A postprocess case has
  `position`, `distance`, exact bit-string
  vectors or scalars, ordered genes/boundaries where applicable, and exact
  expectations. Optional fields and JSON `null` are forbidden; each tagged
  variant has only its named fields.

The fixture contains no checkpoint, whole FASTA, GTF, SQLite database,
generated model, or precomputed production-index member.

### Rust inspector

Add this maintainer-only CLI:

```text
pangopup-build compatibility inspect --corpus <DIR>
```

It emits exactly this compact JSON object plus LF on success:

```json
{"status":"valid","schema":"pangopup-compat-v1","profile":"pangolin-1.0.2-5cf94b8-grch38-v1","cases":24,"scored_cases":14,"rejection_cases":6,"postprocess_cases":4,"coverage_cells":28}
```

The inspector belongs to `pangopup-build`; it is not exposed by the end-user
`pangopup` executable. It must:

- enforce the exact flat three-member, no-follow, per-member, aggregate,
  per-line, string, context, array, gene, and boundary bounds before unbounded
  allocation;
- require the exact twelve-checkpoint profile and all reference, annotation,
  capture-environment, license, coverage, and member identities;
- validate DNA alphabet, context length, anchor/REF agreement, allele-shape
  category, strand, exact `100 + len(REF)` array length, relative range
  `-50..=len(REF)+49`, exact finite `f32` bits, gene order, and absolute exon
  boundaries;
- independently replay upstream masking on separate per-strand copies,
  including same-strand in-place mutation in the recorded SQLite order;
- independently reproduce NumPy first-index argmin/argmax selection, relative
  positions, and the observed Python/NumPy two-decimal output from raw bits;
- independently validate the five replayable normalized rejections from their
  stored witnesses, authenticate `R04` as an upstream DB/CLI observation with
  bracketing evidence, and never treat exact warning prose, CSV/VCF writing, or
  process exit style as a normative Pangopup API; and
- return `COMPATIBILITY_INVALID` on schema, provenance, coverage, bounds, or
  semantic disagreement, without network access or model execution.

Success writes nothing to stderr and exits zero. Invalid corpus content writes
one sanitized compact `CommandError` JSON plus LF to stderr, nothing to stdout,
and exits one with code `COMPATIBILITY_INVALID`; messages may name only a
member basename and case ID, never the caller's path. Invalid command grammar
uses existing `CLI_USAGE` JSON and exit two. Ordinary bounded open/read errors
use the existing sanitized `IO` code and exit one.

### Documentation and durable evidence

- Add a durable architecture decision for the strict
  `pangolin-1.0.2-5cf94b8-grch38-v1` corpus profile. Its retained overlap cases
  preserve exact observed upstream SQLite order and in-place masking. The later
  mask-asset ticket decides how ordering is represented over the complete
  annotation. Any runtime claiming this profile must pass the retained cases
  and may not silently use independent per-gene masking; a corrected behavior
  requires a separately named future profile.
- Update `README.md`, `NOTICE`, `AGENTS.md`, `architecture/README.md`,
  `architecture/runtime-data.md`, `planning/frontier.md`, `planning/faq.md`,
  and add executable compatibility documentation under `spec/`.
- Record exact capture inputs, hashes, environment, case/coverage summary,
  command evidence, limitations, and focused runtime/size evidence in
  `planning/artifacts/009-upstream-pangolin-compatibility-corpus.md`.

### Explicit exclusions

- no production SNV index build, verification, scan, download, installation,
  transport, or release change;
- no Python, PyTorch, gffutils, FASTA, GTF, SQLite, or checkpoint access in
  normal lint/test/spec gates;
- no committed or published model/reference/mask runtime asset;
- no Rust model architecture, checkpoint conversion, tensor runtime,
  quantization, accelerator, or numeric-tolerance decision;
- no lookup-first routing, model-result cache, HTTP server, process supervisor,
  Docker image, systemd unit, or release publication; and
- no dependency on or reference to another GenomOncology software project.

## Decisions

### 1. The exact upstream commit, not a missing tag, is authoritative

The compatibility profile is Pangolin source commit
`5cf94b8db938c658391b4305cd7ce33297d44ff7`, declared package version `1.0.2`,
plus the twelve exact checkpoint identities. The older `v1.0.1` tag and a
mutable branch name are not substitutes.

### 2. Raw post-ensemble arrays are the normative numeric observation

Public two-decimal maxima alone can conceal model or indel-reconciliation
drift. Retain exact post-ensemble unmasked loss/gain `f32` bits and independently
derive masks, maxima, positions, and public output. Per-checkpoint intermediate
tensors are not needed for the first CPU acceptance boundary and remain out of
scope. The later CPU-runtime ticket sets comparison tolerances; this corpus
preserves the exact observed bits without prematurely declaring bitwise Rust
parity.

### 3. Strict compatibility preserves observed masking order

The corpus records the exact upstream SQLite bytes and observed gene iteration
order for its retained cases and treats same-strand in-place mutation as
normative there. It does not yet define a general all-gene ordering algorithm
or compact mask layout; the later mask-asset ticket owns that decision. A
runtime claiming this profile must pass these order-sensitive cases and may not
silently substitute independent per-gene masking. A corrected policy is a
later, separately named option.

### 4. Scoring semantics are normative; adapter quirks are documentary

Raw arrays, masking, extrema, relative positions, and public rounded score
strings define compatibility. Exact warning prose, permissive input bugs,
CSV/VCF serialization, and process exit behavior are retained only where useful
to explain an observed rejection and do not become a future Pangopup API.

### 5. Normal gates validate semantics without rerunning the model

The expensive upstream capture is a deliberate one-time maintainer operation.
The checked Rust inspector must prove that the frozen raw arrays imply the
frozen gene-specific outputs; hashes bind provenance but are not a substitute
for semantic replay. Routine CI stays small, deterministic, offline, and fast.

## Red or discriminating control

Before the corpus is accepted, add tests that fail against independently
mutated copies of the checked fixture when any one of these changes:

- one raw `f32` bit that changes a maximum;
- one expected masked score or relative position;
- same-strand gene order or one exon boundary;
- a context base at the REF anchor;
- one checkpoint identity;
- a required coverage cell; or
- one normalized rejection category.

The positive control is the frozen compatibility corpus. Expected raw
arrays come through the minimal upstream Python extraction helper, while public
masked/unmasked strings for `M01`–`M14` and documentary observations for
`R01`–`R04` come through separate unmodified upstream CLI runs. `R05`/`R06`
come only from independently replayable bounds, because sending them through
the pinned pyfastx native slice path can terminate the process. The Rust
capture orchestrator rejects disagreement across every eligible comparison.
No normative expectation comes from the Rust inspector or a future Rust model
candidate.

## Acceptance

- The checked corpus contains exactly 24 cases in the closed distribution and
  exact matrix above, covers exactly the ordered 28 cells, obeys every member
  bound, and regenerates byte-for-byte into a new absent directory when the
  explicit pinned upstream inputs are supplied.
- The inspector returns the exact deterministic compact JSON summary for the
  valid corpus and rejects malformed, oversized, noncanonical, or semantically
  inconsistent inputs with sanitized `COMPATIBILITY_INVALID` failures.
- Inside-out tests cover extra/missing/symlink/nonregular/nested members,
  excessive entry/member/line/string/context/array/gene/boundary sizes, unknown
  fields, duplicate/missing case and checkpoint IDs, invalid DNA/ref
  anchor/strand, wrong formula-derived context and array lengths, nonfinite or
  malformed bits, missing provenance/license/coverage, changed gene ordering,
  and semantic score/position/masking mutations.
- Tests prove masked and unmasked replay on both strands, same-strand mutation
  order, opposite-strand independence, indel shapes and reconciliation
  observations through the full `-50..=len(REF)+49` relative range, replayable
  versus authenticated-observation rejection behavior, first-index ties, and
  rounded output.
- Executable spec covers exact command grammar, valid summary, missing corpus,
  and a semantically corrupted miniature copy. It does not run the capture
  program or contact a network.
- Focused compatibility tests run against the checked corpus without Python,
  PyTorch, checkpoints, production data, or external services. The artifact
  records corpus bytes, case counts, allocation/bounds evidence, and focused
  elapsed time; it does not add a flaky wall-clock assertion.
- `make lint`, `make test`, and `make spec` pass offline.

Focused commands expected during implementation:

```text
cargo test --locked -p pangopup-build compatibility
cargo test --locked -p pangopup-build --test compatibility
mustmatch test spec/upstream-compatibility.md
```

The exact test target names may follow the owning crate's existing Rust test
layout, but the retained evidence must identify the commands actually run.

## Dependencies

None. The exact upstream source/checkpoints, RefSeq source/report and derived
six-contig reference, and GENCODE DB/GTF are locally preserved with the fixed
identities in this contract. They are maintainer-only capture inputs; no
product asset publication or normal-gate network dependency is permitted.

## Work ownership

- Pre-existing user changes: none; the base worktree is clean.
- Coordinator/ticket author: primary agent `/root`.
- Independent design reviewer: Codex sub-agent `ticket_009_design_review`;
  must not be the preliminary scope reviewer, implementer, or code reviewer.
- Implementer: Codex sub-agent `ticket_009_developer`; distinct from
  coordinator and both reviewers.
- Adversarial code reviewer: Codex sub-agent `ticket_009_code_review`;
  distinct from coordinator, design reviewer, and implementer.
- Generated capture inputs and whole upstream assets stay outside Git. Only
  the bounded checked corpus and ticket evidence are ticket-owned generated
  artifacts.
- Concurrent unrelated work: none observed at ticket creation.

## Long-running jobs

Design-evidence fetch planned 2026-07-23:

- Purpose: preserve and identify the two missing upstream annotation inputs
  required to make the ticket reviewable; this is not model capture or a
  runtime asset publication.
- Candidate: upstream Dropbox `gencode.v38.annotation.db` reached from the URL
  in Pangolin commit `5cf94b8`; official EBI GENCODE release 38
  `gencode.v38.annotation.gtf.gz`.
- Command: resumable `curl -fL --continue-at -` downloads, sequentially, into
  `/home/ian/workspace/data/pangopup-compat-inputs/`, followed by local size,
  SHA-256, SQLite, gzip, and official MD5 checks.
- Process/session: unified exec session `69374`, completed successfully.
- Expected duration: unknown. Progress is destination byte length against
  380,366,848 SQLite bytes and 46,556,621 GTF bytes.
- Success: both exact regular files are complete, parse as SQLite/gzip, and the
  official GTF MD5 is `16fcae8ca8e488cd8056cf317d963407`.
- Failure: curl error, wrong declared length, invalid file type, or digest/check
  failure. Output remains a resumable non-repository data input, never corpus
  evidence until all checks pass.
- Cancellation: send `TERM` to the recorded curl process/session; preserve the
  bounded partial for exact resume.

Fetch completion evidence:

- `gencode.v38.annotation.db`: 380,366,848 bytes, SHA-256
  `221a61eec1f6934ae426d80599989c7b2ee4d9577b52e8a0e4bf02ccd73ca4a6`,
  valid SQLite with `PRAGMA integrity_check = ok`.
- `gencode.v38.annotation.gtf.gz`: 46,556,621 bytes, SHA-256
  `22020df0d3356e965868f4b193e89fa13e838b950a574349f7fcd461ac01c050`,
  valid gzip, official MD5 `16fcae8ca8e488cd8056cf317d963407`.

Capture-reference transformation planned 2026-07-23:

- Purpose: deterministically produce the chr-named uppercase six-contig FASTA
  needed by the named corpus cases and unmodified upstream CLI comparison.
- Candidate: RefSeq compressed source SHA-256
  `11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3`
  plus assembly report SHA-256
  `64318ddff470b69b261a667d813210044f60d4ce654253a547db80ff73638d38`;
  exact accessions `NC_000003.12`, `NC_000010.11`, `NC_000012.12`,
  `NC_000013.11`, `NC_000017.11`, and `NC_012920.1`.
- Command: one sequential gzip stream; select those six accessions, rewrite
  headers to `chr3`, `chr10`, `chr12`, `chr13`, `chr17`, `chrM`, uppercase
  bases, preserve 80-base wrapping, sync an absent `.partial`, and rename to
  `/home/ian/workspace/data/pangopup-compat-inputs/refseq-grch38p14-compat-six-contigs.fa`.
- Process/session: unified exec session `7341`, completed successfully.
- Expected duration: unknown. Progress is output byte length against the sum of
  the six assembly-report lengths plus headers/newlines.
- Success: exactly six headers in the declared order, sequence lengths equal
  the assembly report, no lowercase or non-IUPAC sequence bytes, synced regular
  output, and a recorded SHA-256.
- Failure: decompression/write error, unexpected/missing/duplicate accession,
  length/alphabet mismatch, or digest failure. The final path remains absent.
- Cancellation: send `TERM` to the recorded stream process/session and remove
  only its named `.partial` before a fresh start.

Transformation completion evidence: 671,294,255-byte regular FASTA, SHA-256
`81645a227efbbd196ae337f743f31a5b1c32979d6d7bb5713e0322402a70fafb`;
exactly the six declared headers in order; sequence lengths 198,295,559,
133,797,422, 133,275,309, 114,364,328, 83,257,441, and 16,569; no lowercase or
non-IUPAC sequence lines.

One-time upstream CPU capture launched 2026-07-23:

- Purpose: produce the exact reviewed 24-case compatibility corpus once, using
  the Rust capture orchestrator, GPL helper for raw arrays, and separate
  unmodified upstream masked/unmasked CLI executions.
- Candidate: reviewed-ready repository `9a35765a16ed3aa61d3cf4a9b9ad2fcfac8d87a7`;
  implementation diff SHA-256
  `5bf1a9936521389e57e49a73eb3b378be3d94afbec395e63035b8ee3f7f67315`;
  `target/debug/pangopup-build` SHA-256
  `48cd3a26ea1d2229ac0463ab849c3a24c58a8e893a3ca016d472318f15f3ccac`;
  helper SHA-256
  `4ba9096e943d47d17242dd748ba6eb7384e28ceacb207be9f57851f48b2497f5`;
  upstream/source/checkpoint/reference/annotation/runtime identities are the
  exact fixed values in this contract and passed pre-launch inspection.
- Command: from `/home/ian/workspace/repos/pangopup`,
  `target/debug/pangopup-build compatibility capture --upstream /home/ian/foss/Pangolin --python /home/ian/.local/share/uv/tools/pangolin/bin/python --reference-source /home/ian/foss/uta/ncbi-data/genomes/refseq/vertebrate_mammalian/Homo_sapiens/all_assembly_versions/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_genomic.fna.gz --assembly-report /home/ian/foss/uta/ncbi-data/genomes/refseq/vertebrate_mammalian/Homo_sapiens/all_assembly_versions/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_assembly_report.txt --reference /home/ian/workspace/data/pangopup-compat-inputs/refseq-grch38p14-compat-six-contigs.fa --annotation-db /home/ian/workspace/data/pangopup-compat-inputs/gencode.v38.annotation.db --annotation-gtf /home/ian/workspace/data/pangopup-compat-inputs/gencode.v38.annotation.gtf.gz --output tests/fixtures/pangolin-compat-v1`.
- Process/session: paused launch shell PID `949202`, unified exec session
  `62902`, started `2026-07-23T18:26:34-04:00`; capture starts only after this
  checkpoint is written and the session receives `GO`.
- Expected duration: unknown. Progress is child process state plus creation and
  bounded growth of the uniquely named sibling staging directory; poll with
  `ps` and `du`, never by rerunning capture.
- Success: exit zero; atomic final directory contains only `manifest.json`,
  `cases.jsonl`, and `NOTICE`; immediate Rust inspection returns the exact
  24/14/6/4/28 summary. Failure is nonzero exit, identity disagreement,
  helper/CLI disagreement, invalid corpus, or a leftover partial staging tree.
- Output: `tests/fixtures/pangolin-compat-v1`; session stdout/stderr is the job
  log. The final path was absent before launch.
- Cancellation: send `TERM` to process group/PID `949202` or Ctrl-C to session
  `62902`; remove only the named unpublished sibling staging directory after
  confirming the process exited. Preserve all fixed source inputs.

The first launch exited in preflight before hashing large inputs, starting a
model child, or creating staging: the supplied virtual-environment interpreter
is a symlink to the expected regular executable, while candidate `48cd3a…`
incorrectly rejected the path itself. No partial corpus exists. That harness
candidate is obsolete and its evidence is not combined with capture results.

Replacement capture candidate: implementation diff SHA-256
`b028a139d1814988ddc4794b5a7312456bbd092154621e741ddea4acb0ccd332`;
binary SHA-256
`5f48ce8df4019d9ee07ef9dcc1841ad42f83060d6f26b991c0e66b834ba161b0`;
unchanged helper SHA-256 `4ba9096…`. It resolves and validates the explicit
interpreter executable, with every scientific input and the exact command,
working directory, progress, success/failure, output, and cancellation
contract otherwise unchanged. Paused replacement shell PID `959561`, unified
exec session `58238`, started `2026-07-23T18:28:43-04:00`; it receives `GO`
only after this replacement identity is durable. Cancel with `TERM` to PID
`959561` or Ctrl-C to session `58238`.

The replacement received `GO`, passed every fixed input/source/runtime
identity, and completed the raw-array helper. Its first independent unmodified
CLI invocation (`-m False`) then reached the deliberately out-of-bounds chrM
cases and terminated in native code. The kernel recorded at
`2026-07-23T18:34:17-04:00` that Python PID `993239` segfaulted in
`pyfastx.cpython-313-x86_64-linux-gnu.so`. Python exception handling therefore
cannot make the accepted contract's CLI observation requirement safe for
`R05`/`R06`. The Rust parent exited nonzero, removed its uniquely named staging
directory, and did not publish the final corpus. No capture process or partial
output remains. This scientific-input-compatible candidate is superseded; it
must not be rerun unchanged or treated as corpus evidence.

That discovered upstream behavior returned this ticket to `proposed`. The
revised contract excludes only `R05`/`R06` from unmodified CLI execution,
retains their exact independently replayable bounds, and keeps the independent
CLI comparison for `M01`–`M14` and `R01`–`R04`. No new capture may start until
this revision is independently accepted and the implementation is changed to
match it.

## Independent design review

Reviewer: Codex sub-agent `ticket_009_design_review` (independent, read-only).

Verdict on proposed contract
`ea997784e28e70e70d7326624f62a7a31ebbab5565ed47dc0753c08c86f56d3c`:
**REJECTED**.

Material findings accepted for coordinator remediation:

1. Replace the false universal 101-value rule with upstream's exact
   `100 + len(REF)` post-reconciliation length and relative-position behavior.
2. Name the complete 24-row genomic/controlled case matrix and ordered coverage
   cells so the implementer does not choose the oracle.
3. Resolve and pin the missing GENCODE DB/GTF, all checkpoint identities, and
   local reference identities before declaring dependencies closed.
4. Define Rust orchestration plus a minimal GPL Python extraction helper and
   separate unmodified upstream CLI comparison.
5. Close the corpus member/schema, canonical encoding, identity graph, bounds,
   path safety, success output, and error contracts.
6. Retain sufficient rejection witnesses and avoid claiming independent replay
   where only an authenticated upstream observation is possible.
7. Narrow SQLite-order policy to retained compatibility cases; defer the
   general compact mask ordering rule.
8. Make unmodified CLI output an expectation independent of the raw-array
   helper, make directory bounds inside-out, and make license sources
   executable.

The same reviewer must accept the materially revised ticket before status may
become `ready`.

Re-review verdict on contract
`0fc618afd1073c7592f2aaa8d65eb5d37f719c8da37fe5b7745fe0390ecd2e5d`:
**ACCEPTED AS READY**.

The reviewer confirmed the formula-derived lengths/ranges, all 24 concrete
inputs and observed gene orders, 28 coverage cells, local fixed source/model
identities, Rust/helper/unmodified-CLI separation, storage/bounds contract,
rejection witnesses, narrowed order policy, and notice sources. One code-review
vigilance item remains within the accepted wording: capture must prove that the
module actually imported from the supplied pinned checkout and that its tracked
source bytes match the pinned commit, not merely accept `git rev-parse HEAD`.

That acceptance is superseded by the native pyfastx crash documented above.
The same reviewer rejected revised hashes `3b2c994…` and `aef43ff…` because
generic summary wording still implied CLI evidence for all 24 cases. Those
findings were accepted and corrected.

Final re-review verdict on exact revised contract
`c31886f84cbea4144d7bde4573fec6ab1c15ba107694299aacc07dea28c177fd`:
**ACCEPTED AS READY**. The reviewer confirmed that every execution, schema,
test, and evidence clause keeps `M01`–`M14`/`R01`–`R04` on the independent CLI
path while `R05`/`R06` use only exact independently replayable bounds and never
enter the unsafe native slice path.

## Adversarial code review

Pending.

## Acceptance trace

| Acceptance clause | Command or evidence | Result |
|---|---|---|
| Accepted contract identity and independent design review | Contract `c31886…`; `ticket_009_design_review` final re-review | Pass |
| Exact 24-case compatibility corpus and provenance | Pending | Pending |
| Rust semantic inspector and corruption controls | Pending | Pending |
| Deterministic capture regeneration | Pending | Pending |
| Focused offline tests and resource evidence | Pending | Pending |
| `make lint` | Pending | Pending |
| `make test` | Pending | Pending |
| `make spec` | Pending | Pending |
| Independent adversarial code review | Pending | Pending |

## Evidence and artifacts

- Base revision: `7563f90b7bda4a018833ca89cb628a26aed76c88`.
- Checked ticket contract identity, accepted reviewer, implementation author,
  code reviewer, diff identity, capture identities, commands, and limitations:
  pending.
- Durable implementation evidence:
  `planning/artifacts/009-upstream-pangolin-compatibility-corpus.md` (pending).
