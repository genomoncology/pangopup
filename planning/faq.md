# FAQ and Open Choices

## Settled explanations

### Is the downloaded archive the build input?

Yes. The extracted 19,913 `.tsv.gz` files already present under
`/home/ian/workspace/data/pangolin-precompute/` are the read-only input. The
builder will not redownload or commit them.

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
must retain Ensembl gene identity even if the common query has only one hit.

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

## Open priority choices

### What should CLI v1 require?

- **Recommended:** accept canonical genomic HGVS plus an optional Ensembl gene;
  without a gene, return all matching gene records.
- Require a gene for every query: simplest single-result contract, but awkward
  for callers that begin with only a genomic variant.
- Return one “best” gene: smallest response, but biologically lossy unless a
  separate explicit selection policy is defined.

### How much HGVS should Pangopup own?

- **Recommended:** only canonical GRCh38 genomic SNVs at first, plus an explicit
  coordinate/allele form for bulk use. Reuse Genome later for broader HGVS.
- Depend on Genome immediately: avoids a tiny duplicate parser, but couples the
  first index proof to a much larger evolving library.
- Build a full parser here: self-contained, but duplicates a solved problem and
  expands scope sharply.

### What corpus should prove the first index?

- **Recommended:** a checked-in miniature containing normal loci, both genomic
  directions, overlapping genes, zero scores, boundary positions, and malformed
  rows; then certify all 19,913 files before calling v1 complete.
- Start directly with the full 13 GB source: realistic, but slow feedback and
  poor failure isolation.
- Limit the product to the current ten clinical genes: fastest product proof,
  but creates a temporary scope and artifact that would soon be discarded.

### What is the primary optimization objective?

- **Recommended:** exactness first, then choose the smallest layout within a
  measured lookup-latency envelope. Report both cold and warm behavior.
- Absolute minimum file size: favors compression and may hurt random access.
- Absolute minimum warm latency: favors padding/precomputed directories and may
  inflate the artifact.

### What should the first output look like?

- **Recommended:** JSON Lines for stable machine use plus a concise human table.
- JSON only: simplest contract, less pleasant for direct inspection.
- Tabular only: convenient interactively, fragile for downstream integration.
