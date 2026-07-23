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

### Does Pangopup need HGVS or transcript projection?

No. Pangopup is standalone and accepts an already identified GRCh38 genomic
variant: contig, one-based position, reference allele, and alternate allele.
That is enough to look up or model a splice score. Transcript `c.` and protein
`p.` expressions must be resolved by the caller because doing so requires a
general transcript/protein reference system and is not splice scoring.

### What reference and annotation data does model fallback need?

The lookup path needs only the fixed-v1 score bundle. The planned model path
additionally needs the model checkpoints, local GRCh38 DNA bases, and a map of
gene strand plus exon boundaries. The DNA is pinned NCBI RefSeq GRCh38.p14
`GCF_000001405.40`. The boundary map is compiled from the GENCODE annotation
used by Pangolin's masking behavior. It is a compact Pangopup mmap member, not
a runtime database.

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

The available maintenance commands are `pangopup-build transport pack`,
`transport verify`, and `transport unpack`. `pangopup assets install` installs
an explicit transport into Linux XDG data, and `pangopup assets status` reports
the active state. `pangopup-build release prepare` deterministically generates
the pinned `snv-grch38-v1` profile, proof copy, checksums, and notes from bounded
metadata without opening payload parts. None fetches or publishes remote files;
the external public release is not yet complete. The separate coordinator-only
`pangopup-build release upload-asset` command can stream one exact reviewed
asset during publication. It executes a sealed GitHub CLI snapshot, seals small
assets, protects a large payload with a monitored Linux read lease, and bounds
the child request to 21,600 seconds with process-group cleanup. Catchable
interrupts use that same cleanup path, while child-side parent-death protection
covers abrupt coordinator loss for the direct upload process. It is not a
runtime command and never downloads.

### Does Pangopup install missing assets automatically?

Local installation is shipped, but automatic remote download is not. Callers
run `pangopup assets install --transport <DIR>` once; later `pangopup lookup`
discovers and cheaply reuses the active immutable bundle without `--bundle` or
network access. `--bundle` remains an override. The immutable public release
must be completed and observed first. The later remote target is for
`pangopup assets sync` to resolve that binary-pinned manifest, resume/download
to a temporary cache, and pass the verified transport to this installer.
Download progress and offline/container prefetch remain future. Public-release
metadata preparation is shipped; the external publication step is still
pending coordinator evidence.

### Will asset sync download whatever release is latest?

No. That would make startup irreproducible and allow a mutable remote choice to
change scoring. The future binary or an explicit user selection pins one
release-manifest identity, including URLs, sizes, hashes, formats, source
identities, and licenses. Sync fetches that identity or fails. Immutable
publication and clean-machine manual testing come first; remote sync remains a
separate later slice.

### Where will managed assets be installed?

The shipped Linux installer uses
`${XDG_DATA_HOME:-$HOME/.local/share}/pangopup/`. Temporary downloads may use
`${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/`. The data directory is authoritative;
it is not disposable cache. `PANGOPUP_DATA_DIR` and `--data-dir` override
discovery. Other-platform support is future work.

### Will model fallback run from a FASTA file?

The GRCh38 reference source is distributed by NCBI as FASTA, but raw FASTA will
be build input. The planned asset builder compiles the required primary
sequence into a compact indexed mmap member. Model fallback will read a bounded
sequence window without parsing FASTA or loading the whole reference into heap
memory. None of that model/reference runtime is implemented yet.

### What latency should we expect?

The retained Ticket 004 evidence reports measured warm one-open library lookup,
fresh CLI batch, open-only, and serialization-only costs separately. It does
not project those measurements onto HTTP or model inference. Cold behavior is
explicitly unmeasured on the development host because neither dataset size nor
an OS/device procedure proved the queried pages were nonresident.

### Is JSON output still future work?

No. The shipped `pangopup lookup` command already emits stable compact JSON
Lines by default and exact tab-separated rows with `--format table`. Batch
validation is transactional: an invalid request prevents partial stdout. The
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
future model cache key must include the normalized variant, gene/masking
context, checkpoint, reference and mask identities, window, and inference
parameters. A ticket must first demonstrate a representative repeated workload
whose latency or compute cost improves enough to justify memory/disk use,
locking, eviction, corruption recovery, and invalidation.

## Settled product choices

### What does CLI v1 require?

Accept an explicit GRCh38 contig, position, reference, and alternate plus an
optional Ensembl source-gene filter. Without a filter, return every matching
gene-specific score. No implicit best-gene selection.

### How much HGVS does Pangopup own?

None beyond possibly recognizing an exact genomic RefSeq accession as a contig
alias. Pangopup does not accept transcript/protein HGVS or call a projection
service.

### What corpus should prove the first index?

- **Recommended:** a checked-in miniature containing normal loci, both genomic
  directions, overlapping genes, zero scores, boundary positions, and malformed
  rows; then certify all 19,913 files before calling v1 complete.
- Start directly with the full 13 GB source: realistic, but slow feedback and
  poor failure isolation.
- Limit the product to the current ten clinical genes: fastest product proof,
  but creates a temporary scope and artifact that would soon be discarded.

### What is the primary optimization objective?

Exactness is mandatory and lookup speed is the primary optimization objective.
Resident memory and pages touched come next; compressed download size is third.
The fixed 11-byte mmap layout is the selected and shipped private v1 format.
Hierarchical sparse, compressed-block, and Tabix layouts are retained only as
historical measured candidates.

### How are large artifacts delivered?

The target is separately versioned GitHub release assets: executable, CC BY
fixed-v1 lookup transport set, GPL model weights, GRCh38 reference member, and
GENCODE masking member. The lookup set is canonical metadata, copied small
bundle members, and deterministic parts of one compressed score stream; it is
not one tar archive. Verify and reassemble it once during local installation,
then map the expanded data at runtime. Remote release publication and sync are
not shipped; local pack/verify/unpack and Linux install/status/active discovery
are shipped.

### What does lookup output look like?

JSON Lines is the stable default, with one compact provenance-bearing object per
request. `--format table` selects exact tab-separated output. Both preserve
request order and return ordinary gene records before source ambiguities.
