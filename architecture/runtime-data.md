# Standalone Runtime Data

## Lookup path

An indexed SNV lookup needs only the Pangopup fixed-v1 score bundle. The variant's
GRCh38 contig, position, reference, and alternate select the record. The bundle
already contains the source Ensembl gene identity, masked gain/loss values, and
their relative positions. It does not need a FASTA, GTF, transcript database,
or network call on this path.

On Linux, `pangopup assets sync` downloads the exact compiled-in public
transport into disposable XDG cache and passes it to the same
`pangopup assets install` boundary that reconstructs a supplied transport under
XDG user data. The installer records its canonical receipt and atomically
selects it in `active.json`. Normal lookup discovers that active bundle without a
`--bundle` argument and performs only cheap manifest/size/structure checks.
`--bundle` remains an explicit override. Lookup never downloads data or scans
the complete score payload at startup; only the explicit sync command uses the
network.

The request's reference allele remains part of the key. A wrong reference
therefore fails or misses rather than returning the score for a different
allele. The full model reference is not loaded merely to repeat that check for
an indexed SNV.

## Planned model fallback path

Model fallback is not implemented. Its accepted data boundary requires three
additional local facts:

Running Pangolin for a lookup miss or non-SNV genuinely needs:

1. **Model weights.** The twelve version-2 checkpoints loaded by the current
   upstream program, or a verified equivalent conversion.
2. **GRCh38 reference bases.** Pangolin reads a long DNA window around the
   variant, verifies the submitted reference allele, and scores reference and
   alternate sequences. Pangopup pins NCBI's RefSeq GRCh38.p14 assembly,
   accession `GCF_000001405.40`. The source FASTA is compiled into a compact,
   indexed mmap member; normal installations do not parse or retain raw FASTA.
3. **Gene strand and exon boundaries.** Pangolin first finds every gene body
   containing the variant and runs the appropriate strand. In masked mode it
   keeps splice loss at annotated exon boundaries and splice gain away from
   annotated boundaries. Without these facts Pangopup could return unmasked
   neural-network output, but not the same masked product as the precomputed
   archive.

Upstream Pangolin obtains item 3 from a gffutils database generated from
GENCODE. Its documented GRCh38 default is GENCODE release 38 with
`Ensembl_canonical` transcripts. Pangopup should compile the required gene
intervals, strand bits, identifiers, and exon-boundary positions into a compact
immutable mmap member. It does not ship SQLite/gffutils at runtime.

The pinned sequence source is the [NCBI RefSeq GRCh38.p14 assembly](https://www.ncbi.nlm.nih.gov/datasets/genome/GCF_000001405/).
The masking source is the archived [GENCODE release 38](https://www.gencodegenes.org/human/release_38.html)
annotation used by the upstream instructions, not a moving "latest" release.

RefSeq and GENCODE have different roles here. RefSeq supplies the GRCh38 DNA
bases. GENCODE supplies the gene/exon map required by Pangolin's masking rules
and by the Ensembl-gene identities used in the precomputed dataset. This does
not introduce general gene annotation into the public API.

Pangopup now pins that boundary in `tests/fixtures/pangolin-compat-v1`. The
227,060-byte corpus contains 14 scored genomic cases, six rejection cases, and
four controlled post-processing cases captured from source commit `5cf94b8`
and twelve exact checkpoints. It retains exact RefSeq GRCh38.p14 contexts,
GENCODE v38 gene/exon facts, typed raw arrays, masked and unmasked output,
overlapping-gene order, and rejection witnesses. Its Rust inspector replays the
semantics offline. This corpus—not architectural similarity—is the acceptance
oracle for CPU inference and any later conversion. Its controlled vectors and
expectations are fixed independently from replay, and its future capture path
authenticates the live helper and all imported upstream Python modules before
execution.

## Reproduction boundary

The precomputed dataset publisher calls its coordinates hg38 and says scores
were masked with GENCODE annotations, but does not identify the exact FASTA,
GENCODE files, Pangolin package commit, or checkpoint identity. A local check of
1,023,901 positions across ten genes found no reference-base differences from
RefSeq GRCh38.p14, so the reference is compatible over that checked region; it
does not prove the publisher used the same FASTA.

Pangopup therefore versions the lookup artifact and the fallback model artifact
separately. Before claiming parity, the checked corpus must compare both routes.
Small numeric differences with identical masking are more likely to come from
model/checkpoint or numeric-runtime differences than from reference bases once
the submitted reference allele has been verified.

CPU compatibility is proved before accelerator selection. MPS, CUDA,
alternative runtimes, quantization, or other optimizations are accepted only if
they preserve the defined result/error behavior within explicit retained
tolerances and improve measured end-to-end performance or resource use.

The current Python implementation also mutates its gain/loss arrays while
masking each gene. The strict compatibility profile retains observed SQLite
gene order and proves that a later same-strand gene sees earlier mutations. A
Rust fallback claiming this profile must preserve that behavior; an improved
independent-per-gene policy requires a separately named profile.

## What Pangopup deliberately does not ship

- a transcript-alignment or general sequence database;
- an HGVS parsing or coordinate-projection engine;
- transcript and protein sequences;
- gene descriptions, aliases, disease knowledge, or consequences;
- PostgreSQL, SQLite, or gffutils as a runtime dependency.

The shipped standalone lookup deployment is therefore the executable plus the
fixed-v1 score bundle. The target complete deployment adds weights, a compact
GRCh38 sequence member, and a compact Pangolin masking member; a future
lookup-only profile can omit those three.
