# Production GRCh38 Reference

Pangopup's model path needs exact DNA around a genomic variant. It does not
need a general sequence database, transcript projection, HGVS, or gene facts.
The reference subsystem therefore exposes one narrow operation: copy an exact
one-based window from one of the 25 GRCh38 primary/non-nuclear sequences into a
buffer supplied by the caller.

## Bundle

The immutable bundle contains only `NOTICE`, canonical `manifest.json`, and
`reference.pgr`. Production profile `refseq-grch38p14-primary-v1` binds the
exact NCBI RefSeq GRCh38.p14 gzip, assembly report, ordered accessions and
lengths, 3,088,286,401-base logical digest, 680 ignored records, attribution,
and both member hashes. `pangopup-reference-mini-v1` is a registered synthetic
25-contig profile for normal tests; it cannot identify itself as GRCh38.

`reference.pgr` uses production magic `PGRREF01`. It packs A/C/G/T into two
bits and records exact IUPAC ambiguity runs in one bounded table. Its header,
25 fixed-order directory entries, padding, dense closure, and ambiguity-table
closure are explicitly little-endian and checked. Ticket 010's `PGRBEN01`
containers are incompatible benchmark evidence, not runtime assets.

## Runtime

`ReferenceBundleOpen` validates the exact member set, canonical closed
manifest, fixed profile, sizes, header/directory, padding, and all ambiguity
runs. It then retains one read-only mmap and the small ambiguity table.
Opening does not hash or scan dense sequence bytes. A successful window copies
only the packed bytes it needs, overlays intersecting ambiguity runs, performs
no heap allocation, and returns uppercase IUPAC.

The provider rejects empty or out-of-range windows without changing the
destination. chrM never wraps. Alias parsing is an adapter concern; the core
trait receives a typed contig and position. The mapped regular file must remain
immutable and untruncated for the reader lifetime.

## Build and integrity

The maintenance builder accepts one explicit registered profile and opens both
inputs once with no-follow semantics. It authenticates the held assembly report
and the compressed/plain FASTA bytes before any decompression or FASTA parse,
then rechecks the same descriptors after use. The decoded FASTA is streamed
once. Registered profiles cap decoded bytes, records, accession length, line
length, and ambiguity runs; a same-size hostile gzip therefore cannot turn an
authenticated build into unbounded decompression, allocation, or accession
collection. The report is the authority for required accession lengths. Dense
bytes go directly to precomputed file offsets; only bounded parser and
ambiguity state is retained.

Before atomic no-replace publication, private certification hashes both
members, checks dense padding and ambiguity placeholders, decodes all sequences
through the production provider, reproduces the independent logical digest,
and checks four synthetic or fourteen frozen Pangolin contexts. There is no
public exhaustive verification command and normal gates never read the NCBI
source.

The one-time qualification path is separate from normal CLI and tests. An
opt-in builder environment variable writes a canonical Rust outstanding-heap
report without changing the maintenance command's JSON contract. The retained
full build is immutable: the feature-gated v2 qualification binary accepts a
canonical reuse input, a retained v1 root, the frozen corpus, and an absent
output root. It opens every path component without following symlinks, retains
the exact file descriptors it authenticated, constructs its reader from the
held manifest bytes and reference descriptor, and re-hashes those same files
after measurement. The compiled executable binds the complete builder source
inventory, including the reader, evaluator, lockfile, and harness.
Every bounded small evidence file owns one authenticated byte buffer; all JSON
and text checks reuse that buffer rather than rereading the descriptor. The
descriptor remains held for the required final metadata and full-hash mutation
check.

Qualification measures warm latency, open heap, dense bytes read during open,
allocations, logical pages, RSS/faults, installed size, and the exact pinned
level-9 Rust-Zstandard encoding. It never copies or rebuilds the 772 MB member.
Small success or failure evidence is fsynced in a private stage and published
atomically without replacement; a failure remains separate and cannot be
mistaken for the successful contract root. It fails rather than emits a
passing report when any Ticket 011 threshold is exceeded. RSS is reported
separately because mapped file-backed pages are not Rust heap.

After the private stage exists, every preflight, measurement, report,
validation, or success-publication error enters one failure repair/seal/publish
path. That path reconstructs the exact closed failure root from held
authoritative inputs. If failure publication itself cannot complete, the
private stage remains in place and the process identifies its exact local path.

A same-size two-bit substitution is a valid encoding and deliberately is not
detected by cheap open. Build certification detects it through member and
logical hashes. Future transport/install work must authenticate the
manifest-bound member hash before activating a downloaded bundle; adding
per-page checksums would tax the highest-priority query path and requires new
measurement.
