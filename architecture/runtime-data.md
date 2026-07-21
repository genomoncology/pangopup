# Standalone Runtime Data

## Lookup path

An indexed SNV lookup needs only the Pangopup sparse score bundle. The variant's
GRCh38 contig, position, reference, and alternate select the record. The bundle
already contains the source Ensembl gene identity, masked gain/loss values, and
their relative positions. It does not need a FASTA, GTF, transcript database,
or network call on this path.

The request's reference allele remains part of the key. A wrong reference
therefore fails or misses rather than returning the score for a different
allele. The full model reference is not loaded merely to repeat that check for
an indexed SNV.

## Model fallback path

Running Pangolin for a non-SNV genuinely needs three additional local facts:

1. **Model weights.** The twelve version-2 checkpoints loaded by the current
   upstream program, or a verified equivalent conversion.
2. **GRCh38 reference bases.** Pangolin reads a long DNA window around the
   variant, verifies the submitted reference allele, and scores reference and
   alternate sequences. Pangopup pins NCBI's RefSeq GRCh38.p14 assembly,
   accession `GCF_000001405.40`, and stores only the primary reference sequence
   representation required by the supported input scope.
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

## Reproduction boundary

The precomputed dataset publisher calls its coordinates hg38 and says scores
were masked with GENCODE annotations, but does not identify the exact FASTA,
GENCODE files, Pangolin package commit, or checkpoint identity. A local check of
1,023,901 positions across ten genes found no reference-base differences from
RefSeq GRCh38.p14, so the reference is compatible over that checked region; it
does not prove the publisher used the same FASTA.

Pangopup therefore versions the lookup artifact and the fallback model artifact
separately. Before claiming parity, a retained corpus must compare both routes.
Small numeric differences with identical masking are more likely to come from
model/checkpoint or numeric-runtime differences than from reference bases once
the submitted reference allele has been verified.

The current Python implementation also mutates its gain/loss arrays while
masking each gene. For overlapping genes on the same strand, a later gene may
therefore observe an array already masked for an earlier gene. The Rust fallback
must test this case and expose a clearly versioned compatibility policy; it must
not silently change upstream behavior while claiming identical Pangolin output.

## What Pangopup deliberately does not ship

- a transcript-alignment or general sequence database;
- an HGVS parsing or coordinate-projection engine;
- transcript and protein sequences;
- gene descriptions, aliases, disease knowledge, or consequences;
- PostgreSQL, SQLite, or gffutils as a runtime dependency.

The complete standalone deployment is therefore the executable plus the sparse
score bundle; installations enabling model fallback additionally receive the
weights, compact GRCh38 sequence member, and compact Pangolin masking member.
