# FAQ and Open Choices

## Settled explanations

### Is the downloaded archive the build input?

Yes. The already-downloaded archive and its extracted 19,913 `.tsv.gz` members
are the read-only input. The builder receives their path explicitly; it does not
redownload or commit them.

### Is this an exome-only lookup?

No. “Protein-coding genes” describes which gene spans were scored, not only the
protein-coding exon bases. The files cover complete gene spans, including
intronic positions. Exonic variants away from junctions can create cryptic
splice sites or alter splicing enhancers/silencers. Intronic variants can alter
branch-point, polypyrimidine, or other regulatory sequence and can create
cryptic sites or pseudoexons away from the canonical two splice-site bases.

### Why include a gene in the result?

The files are per gene, gene spans can overlap, and masking depends on annotation
context. One genomic SNV can therefore have more than one source record. Results
must retain Ensembl gene identity even if the common query has only one hit. A
caller does not need to supply a gene: without a filter Pangopup returns every
matching source record.

### What happens at a source row whose reference is `N`?

The complete source has 30 such loci. Pangopup preserves them so a rebuilt index
can account for every published row, but `N` is not a concrete SNV reference and
the source supplies only three of four possible alternates. Normal lookup returns
a typed ambiguous-source-reference result; it never guesses or silently maps the
row onto a pinned FASTA base.

### Why not simply Tabix the files?

Tabix is a good correctness and operational baseline. It still stores repeated
text-oriented keys, uses block decompression, and does not exploit the source’s
three-alternates-per-contiguous-locus structure. Pangopup should outperform it
by direct addressing, but measurements decide.

### Why not add an LRU cache immediately?

An mmap plus the operating-system page cache already retains hot pages. A second
cache adds memory, synchronization, and invalidation behavior. Add one only if
end-to-end measurements show repeated decoding or model execution that the page
cache cannot solve.

### Is the reference definitely GRCh38.p14?

The publisher says hg38. A local 1,023,901-position check across ten genes had
zero reference mismatches against RefSeq GRCh38.p14 primary chromosomes, but the
publisher does not identify the exact FASTA or GENCODE release. Pangopup can say
GRCh38 and pin the archive checksum; it should not invent missing provenance.

### What proves the CPU model kernel is Pangolin-compatible?

Two independent checked roots, not a claim that the network merely has the same
architecture. `pangopup-compat-v1` binds Pangolin 1.0.2 source commit
`5cf94b8`, twelve checkpoint hashes, the exact RefSeq GRCh38.p14 sequence
source, GENCODE v38
masking inputs, and the pinned CPU numeric environment. Its 24 cases preserve
typed raw arrays plus independently observed CLI output. Normal tests replay
those semantics in Rust; they do not rerun Python or the neural network.
The manifest distinguishes the normative helper's forced PyTorch `1/1` thread
profile from the auxiliary unmodified CLI's observed `1/16` profile instead of
claiming the whole capture used one setting. Controlled vectors and their
expected values are pinned independently of replay, and the provenance capture
path hashes the live helper plus every imported upstream Pangolin Python module
before use.

`pangolin-model-v1` separately authenticates every tensor in every checkpoint
and 45,756 selected-channel `f32` values produced by direct per-checkpoint
PyTorch execution. The Rust CPU kernel's combined ONNX graph matches all of
them within maximum absolute tolerance `1e-5`. Normal tests execute a tiny
same-schema graph through real ONNX Runtime and do not rerun conversion.

### Does Pangopup need HGVS or transcript projection?

No. Pangopup is standalone and accepts an already identified GRCh38 genomic
variant: contig, one-based position, reference allele, and alternate allele.
That is enough to look up or model a splice score. Transcript `c.` and protein
`p.` expressions must be resolved by the caller because doing so requires a
general transcript/protein reference system and is not splice scoring.

### What reference and annotation data does model fallback need?

The lookup path needs only the fixed-v1 score bundle. The shipped model scorer
additionally needs the authenticated converted model bundle, local GRCh38 DNA
bases, and a map of gene strand plus exon boundaries. Lookup-first routing uses
the activated installed profile or three explicit override flags. One canonical runtime profile
now proves which exact SNV, model, reference, mask, and scoring-policy tuple is
compatible. Offline coherent installation, activation, lazy lookup
consumption, and deterministic local pack/verify/unpack are shipped. The exact
public payload and GPL preferred source are immutable and public; pinned
combined sync is shipped. The original checkpoint containers are
conversion/preferred-source inputs, not installed runtime inputs. The DNA is
pinned NCBI RefSeq GRCh38.p14
`GCF_000001405.40`. The boundary map is compiled from the GENCODE annotation
used by Pangolin's masking behavior. A retained comparison selected the
constant-membership domain representation from three private mmap candidates.
The exact selected bytes now have a production domains-only mmap provider,
manifest-bound local transport, an immutable public payload, and automatic
pinned asset sync. SQLite, gffutils, and raw
GTF were one-time qualification inputs and are neither runtime nor current
build-crate dependencies.

### What does the four-asset runtime profile do?

It is a small compatibility receipt, not another database. It names the exact
compiled SNV index, ONNX model, compact RefSeq sequence bundle, GENCODE mask,
and scoring rules that were qualified together. The local runtime installer
uses it to reject a mixed set before changing one coherent active pointer. The
profile contains no local paths or download URLs and does not itself download
or publish anything.

### Why is any gene information needed at all?

The neural network needs only sequence and strand to produce raw changes, but
Pangolin's default masked result uses exon boundaries. It suppresses changes
that do not make biological sense relative to the annotated splice sites. It
also evaluates overlapping genes separately. Pangopup therefore needs only
gene ID, span, strand, and exon-boundary positions—not gene descriptions,
aliases, transcripts, proteins, or disease knowledge.

### Can the large files be GitHub release assets?

Yes, but the SNV bundle should be split for transport. GitHub currently permits
up to 1,000 assets per release, requires each asset to be under 2 GiB, and
states no aggregate release-size or bandwidth quota. The certified fixed-v1
member is 15,033,158,255 bytes. A historical tar+Zstandard experiment measured
1,935,000,209 bytes—too close to the per-file ceiling for comfortable headroom,
and not the accepted format. The shipped local lookup transport compresses only
`scores.pgi` as one deterministic Zstandard frame and splits it into ordered
1,000,000,000-byte parts bound by a canonical manifest. Unpack reconstructs the
same mmap member. Executable, lookup-data, and future model assets remain
separately versioned.

The complete maintenance catalog is available through successful
`pangopup-build --help`; namespace and leaf help such as
`pangopup-build transport --help` and
`pangopup-build transport pack --help` state the exact closed grammar without
touching an asset. The transport operations are `transport pack`, `transport
verify`, and `transport unpack`. `pangopup assets install` installs
an explicit transport into Linux XDG data, and `pangopup status` reports the
combined SNV/runtime state. `pangopup-build release prepare` deterministically generates
the pinned `snv-grch38-v1` profile, proof copy, checksums, and notes from bounded
metadata without opening payload parts. None fetches or publishes remote files;
the immutable `snv-grch38-v1` release is published separately. Pangopup ships
no release uploader. The reviewed `runtime-grch38-v1` publication lifecycle has
the coordinator use authenticated official `gh` directly after exact local
reauthentication and the publication-ready remote gate. It completed with
every remote name, size, and digest checked after each one-time upload; the
public release reports `immutable=true`. Publication remains outside runtime
download and lookup.

### Does Pangopup install missing assets automatically?

Explicit remote download is shipped as `pangopup sync`. It uses binary-pinned
SNV and runtime profiles, safely resumes members into disposable XDG cache,
and passes only complete transports to the existing installers.
`--offline` forbids network access and can install a completed cached transport.
Later `pangopup lookup` discovers and cheaply reuses the active immutable bundle
without `--bundle` or network access. Lookup does not download implicitly.
The three model-side assets can separately be installed from trusted local
inputs with `assets runtime install`. Their exact immutable public release is
available, and the combined top-level CLI installs its cached transport
directly after the compatible SNV. Exact persistent download progress and
container prefetch remain future.

### Will asset sync download whatever release is latest?

No. That would make startup irreproducible and allow a mutable remote choice to
change scoring. The binary pins one release-profile identity, including literal
URLs, sizes, hashes, formats, source identities, and licenses. Sync fetches
exactly that transport or fails; it does not fetch discovery metadata or accept
a user URL.

### Where will managed assets be installed?

The shipped Linux installer uses
`${XDG_DATA_HOME:-$HOME/.local/share}/pangopup/`. Temporary downloads use
`${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/`. `PANGOPUP_CACHE_DIR` and
`--cache-dir` override that disposable location. The data directory is
authoritative; it is not disposable cache. `PANGOPUP_DATA_DIR` and `--data-dir`
override durable discovery. Other-platform support is future work.

### Will model fallback run from a FASTA file?

The GRCh38 reference source is distributed by NCBI as FASTA, but raw FASTA is
build input. The shipped reference builder compiles all 25 required primary
sequences into the production `PGRREF01` mmap bundle, and its provider copies a
bounded sequence window without parsing FASTA or loading the whole reference
into heap memory. The variant scorer consumes that provider and composes it
with the mask and raw CPU kernel. The CLI can use the activated installed tuple
or one complete explicit override. Offline installation, coherent activation,
and automatic lookup discovery are implemented.

For these caller-supplied paths, Pangopup hashes the complete bounded compiled
reference member and verifies its manifest digest before scoring through an
mmap of that same retained descriptor. That extra full-file read prevents
provenance from describing different same-size sequence bytes. It occurs only
when a request actually needs explicit fallback; authoritative SNV hits skip
all three fallback paths. Managed installation authenticates the immutable
reference once and retains cheap structural startup.

### Does Pangopup repeat model inference for the same variant?

Not after the first successful model result. Pangopup stores the complete
unfiltered typed result in a disposable SQLite cache and applies any requested
gene filter afterward. The key includes the variant and every model, reference,
mask, window, masking, and CPU-policy identity, so a changed scoring input is a
miss instead of a stale hit.

The default keeps the 10,000 most recently inserted or explicitly updated
results under XDG cache and survives process restarts. Ordinary valid hits are
read-only and do not refresh that order. Precomputed SNV hits do not use
SQLite. Deleting the database is always safe; Pangopup recomputes model results
as needed.

### Which compact reference encoding will Pangopup use?

`acgt2-rle-v1`: two-bit ACGT with exact ambiguity runs. In the one retained
six-contig comparison its headline p50/p95 were 4,469/4,880 ns, versus
16,272/18,366 for uppercase ASCII and 34,267/41,522 for exact four-bit IUPAC.
That satisfied the required five-percent speed win at both quantiles against
both alternatives. It also produced the smallest member and Zstandard frame,
although IUPAC4 touched two fewer logical pages. Normal tests use the small
25-contig synthetic production fixture. Ticket 011 hardened the winner as the
complete production `PGRREF01` bundle/provider. The discarded candidate codecs,
miniature, benchmark executable, and CLI have been removed; retained reports
and decisions preserve the selection evidence. Local packaging and immutable
publication, pinned typed remote delivery, and combined CLI provisioning are
shipped. Model integration is shipped through
`pangopup-engine`.

### Which compact GENCODE mask encoding will Pangopup use?

Constant-membership domains. Ticket 012 compared interval-tree, domains, and
binned-postings candidates behind one exact query API over the complete pinned
GENCODE v38 logical source. The closed selector chose domains at its first p95
speed step: headline p50/p95 were 171/331 ns, compared with 241/401 for
interval-tree and 241/431 for binned postings. ADR 0013 promotes the exact
selected `PGMBEN01` v1 domains bytes behind the production
`pangopup_index::mask` provider instead of rebuilding identical data under a
second format identity. The other-codec results remain in retained historical
evidence; their writer/readers and qualification machinery have been removed.
Local mask packaging, byte-exact reconstruction, and immutable publication are
shipped, including pinned remote delivery through `pangopup sync`.

The mask retains exact versioned identifiers and `_PAR_Y`, and its effective
gene membership is `(start,end]` because that is what the upstream point query
actually produces. Overlapping same-strand gene order matters: Pangolin mutates
one score array while visiting genes, so changing that order can change a later
gene's masked result. An unversioned filter is applied within the already
selected contig and never merges chrX/chrY pseudoautosomal copies.

### What latency should we expect?

The retained Ticket 004 evidence reports measured warm one-open library lookup,
fresh CLI batch, open-only, and serialization-only costs separately.

An informal same-host probe on the AMD Ryzen 7 5825U observed direct
twelve-checkpoint PyTorch inference over one raw context at p50 0.684 seconds
for 10,101 bases and 0.663 seconds for 10,200 bases with its default thread
settings. Forced to `1/1`, it observed 3.033 and 3.191 seconds. The accepted
Rust/ONNX Runtime `1/1` kernel measured 2.335 and 2.222 seconds respectively.
This suggests Rust is faster in the like-for-like single-thread comparison,
while default multithreaded PyTorch is about 3.3–3.5 times faster than the
ordinary Rust policy. The one-off PyTorch command and raw output were not
retained, so those Python figures guide follow-up work but are not qualification
or release evidence.

Those are raw single-context calls, not complete variant, concurrent, CLI,
HTTP, accelerator, or end-to-end measurements. A modeled variant requires
reference and alternate work and can require both strands, so multiplying
these numbers into a product latency claim would be misleading. The retained
complete-request comparison found fixed sequential `8/1` fastest on this host,
at about 1.05 seconds p50 for M09 and 2.64 seconds for two-strand M10.
Affinity-aware `auto/1` failed the M10 improvement gate, so ordinary sessions
remain portable sequential `1/1`; fixed host results are not latency promises.
Reference/alternate graph batching was corrected and fully remeasured after
the first run's exporter mismatch. Both corrected policy comparisons exceeded
the singleton drift limit, and neither candidate met the independent
replacement gates, so ordinary sessions remain singleton.

Cold lookup behavior is explicitly unmeasured because neither dataset size nor
an OS/device procedure proved the queried pages were nonresident.

### Is JSON output still future work?

No. The shipped `pangopup lookup` command already emits stable compact JSON
Lines by default and exact tab-separated rows with `--format table`. Batch
validation and model scoring are transactional: an invalid or rejected request
prevents partial stdout. Modeled JSON identifies the model, sequence bundle,
observed mask bytes, masking policy, and score semantics. The
future HTTP service will define a separate batch JSON envelope over the same
typed results; it does not replace or postpone the CLI contract.

### Will Pangopup implement start, stop, restart, and status commands?

The planned server runs in the foreground as `pangopup serve`, and
`pangopup status` will expose its non-secret health/readiness, software, route,
and asset identities. Docker, systemd, Kubernetes,
or another external process manager owns start, stop, and restart. Keeping one
foreground process avoids building a second supervisor and produces the same
service behavior in containers and native deployments.

### Will non-SNV inference use a persistent cache?

Only if measurements justify it. The operating-system page cache already helps
the SNV mmap path, while model results have a more complicated identity. Any
future model cache key must include the literal variant, gene/masking context,
checkpoint, GRCh38 sequence-index and mask identities, window, and
inference parameters. A ticket must first demonstrate a representative repeated
workload whose latency or compute cost improves enough to justify memory/disk
use, locking, eviction, corruption recovery, and invalidation.

## Settled product choices

### What does CLI v1 require?

Accept an explicit GRCh38 contig, position, reference, and alternate plus an
optional Ensembl source-gene filter. Without a filter, return every matching
gene-specific score. Literal uppercase A/C/G/T multi-base alleles are accepted
when a complete explicit reference/mask/model fallback set is supplied. No
implicit best-gene selection, normalization, or projection.

### How much HGVS does Pangopup own?

None beyond possibly recognizing an exact genomic RefSeq accession as a contig
alias. Pangopup does not accept transcript/protein HGVS or call a projection
service.

### What corpus proved the first index? (historical decision)

A checked-in miniature supplied fast discriminating controls, followed by
complete certification of all 19,913 files. The project did not limit the
product to ten genes or use repeated full-source scans as an ordinary test.

### What is the primary optimization objective?

Exactness is mandatory and lookup speed is the primary optimization objective.
Resident memory and pages touched come next; compressed download size is third.
The fixed 11-byte mmap layout is the selected and shipped private v1 format.
Hierarchical sparse, compressed-block, and Tabix results remain as historical
evidence; their comparison implementations and benchmark target have been
removed.

### How will users install the executable?

The repository contains a checksum-verifying `install.sh` for Linux x86_64.
After Ticket 038 publishes `v0.1.0`, the latest form will be:

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/main/install.sh | bash
```

For reproducibility, use the tagged script with `--version 0.1.0`. The only
platform baseline is Linux x86_64 with GLIBC 2.35; prerequisites are Bash,
curl or wget, and sha256sum, shasum, or openssl. It
installs under `${PANGOPUP_INSTALL_DIR:-$HOME/.local/bin}`, prints PATH guidance
when needed, and directs the user to `pangopup sync` and `pangopup status`. It
does not use sudo, edit shell files, or download data automatically. No public
executable exists until Ticket 038 completes.

Before publication, the candidate must pass a clean isolated Linux run using
the real pinned data: online sync, offline reuse, combined ready status, all
1,000 retained SNVs in seven batches, and one exact non-SNV model result. The
SNV comparison ignores only the installation-specific bundle ID; it still
checks every biological score and position plus all other provenance. This
qualification is prepared locally and has not created a public executable.

### How are large artifacts delivered?

The target is separately versioned GitHub release assets: executable, CC BY
fixed-v1 lookup transport set, GPL converted model bundle, compiled GRCh38
sequence index, and compiled GENCODE masking member. These are derived
files Pangopup directly maps or executes. Pangopup does not republish the raw
Zenodo archive/TSVs, NCBI FASTA/assembly report, GENCODE GTF/SQLite database, or
other upstream data inputs. Original Pangolin checkpoints are conversion inputs
rather than installed runtime members. The public model release accompanies
the converted model with the complete exact upstream Pangolin tree, the exact
Pangopup converter tree, and a standalone GPLv3 license; none is an installed
runtime member.

The lookup set is canonical metadata, copied small bundle members, and
deterministic parts of one compressed score stream; it is not one tar archive.
Verify and reassemble it once during local installation, then map the expanded
data at runtime. Immutable SNV release publication, pinned resumable sync,
local pack/verify/unpack, and Linux install/status/active discovery are shipped.

### What does lookup output look like?

JSON Lines is the stable default, with one compact provenance-bearing object per
request. `--format table` selects exact tab-separated output. Both preserve
request order and return ordinary gene records before source ambiguities.
