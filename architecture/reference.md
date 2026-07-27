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
and both member hashes. `pangopup-reference-mini-v1` is a registered small
synthetic 25-contig profile. The distinct
`pangopup-reference-route-test-v1` fixture has a 10,101-base all-A chr1 and 24
one-base contigs so normal tests exercise one complete scorer context through
the production reader. Neither synthetic profile can identify itself as
GRCh38 or be installed as production evidence.

`reference.pgr` uses production magic `PGRREF01`. It packs A/C/G/T into two
bits and records exact IUPAC ambiguity runs in one bounded table. Its header,
25 fixed-order directory entries, padding, dense closure, and ambiguity-table
closure are explicitly little-endian and checked. Ticket 010's incompatible
`PGRBEN01` candidates have been removed; retained reports and decisions remain
historical selection evidence, not runtime assets.

## Runtime

`ReferenceBundleOpen` validates the exact member set, canonical closed
manifest, fixed profile, sizes, header/directory, padding, and all ambiguity
runs. It then retains one read-only mmap and the small ambiguity table.
Opening does not hash or scan dense sequence bytes. A successful window copies
only the packed bytes it needs, overlays intersecting ambiguity runs, performs
no heap allocation, and returns uppercase IUPAC.

The format implementation is split into shared wire/layout, one writer, and
one reader/provider. Installed runtime admission can hand that reader the exact
descriptor already authenticated by installation through an explicit unsafe
authority boundary. The resulting opaque capability retains and maps that
descriptor, never reopens its pathname, and gives safe callers only the normal
reference-provider behavior.

Explicit CLI fallback uses the stricter `open_identified` capability. It opens
one regular single-link `reference.pgr` descriptor without following symlinks,
reads at most the bounded member size while hashing it, verifies the
manifest-declared size and SHA-256, rejects descriptor mutation or pathname
replacement during hashing, maps that same descriptor, and retains it with the
provider used by scoring. This deliberately reads the complete reference
member—772,091,760 bytes for the qualified production bundle—once when a
batch actually requires explicit fallback. It is not paid by authoritative
SNV hits, and it does not change the cheap ordinary open used after a future
installer has already authenticated immutable bytes.

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
and checks four miniature, one route-test, or fourteen frozen Pangolin
contexts according to profile. There is no
public exhaustive verification command and normal gates never read the NCBI
source.

Ticket 011's retained evidence records the one-time full build and reuse
qualification: warm latency, open heap, dense bytes read during open,
allocations, logical pages, RSS/faults, installed size, and pinned Zstandard
size. Those reports and historical decisions remain the evidence for the
selected production path. The candidate codecs, benchmark executable,
feature-gated qualification adapter, and opt-in CLI heap reporter have been
removed; qualification is not a current command or ordinary test lifecycle.
The direct bounded-memory and zero-allocation production tests remain.

Post-build inspection, exhaustive certification, compatibility contexts, and
qualification policy are separate from the byte-producing adapter. Future
builds describe only byte-producing inputs with
`pangopup.reference-builder-source.v2`; the existing production identity is
unchanged.

A same-size two-bit substitution is a valid encoding and deliberately is not
detected by cheap open. Build certification detects it through member and
logical hashes; explicit CLI fallback now detects it through its identified
open. Future transport/install work must authenticate the
manifest-bound member hash before activating a downloaded bundle; adding
per-page checksums would tax the highest-priority query path and requires new
measurement.
