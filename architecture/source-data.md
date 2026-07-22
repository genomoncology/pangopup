# Source Data and Provenance

## Pinned source

The initial index source is:

| Field | Value |
|---|---|
| Title | Pangolin precomputed scores |
| Creators | Nils Wagner; Aleksandr Neverov |
| DOI | `10.5281/zenodo.15649338` |
| Publication date | 2025-06-12 |
| Archive | `Pangolin_hg38_snvs_masked.zip` |
| Archive size | 12,988,141,317 bytes |
| Archive MD5 | `679ef0b50e511b6102b4b88fbf811108` |
| License | CC BY 4.0 |
| Coordinates | described by the publisher as hg38 |
| Parameters | masked; window size 50 nt; GENCODE splice-site annotations |
| Pangolin software/model version | unspecified by the publisher |

The analyzed extracted copy contains 19,913 per-gene `.tsv.gz` files. Builders
receive its path explicitly. Neither the extracted data nor the 13 GB archive
belongs in Git.

## Reference-build evidence

“hg38” establishes the GRCh38 coordinate assembly but does not, by itself,
identify the exact FASTA release, patch naming, or GENCODE release used by the
dataset creators. The Zenodo description does not currently state those exact
inputs.

A local compatibility check compared 1,023,901 distinct positions across ten
target genes with the primary chromosome sequences in RefSeq GRCh38.p14 and
found zero reference-allele mismatches. That is strong evidence that those
primary sequences agree for the checked corpus. It is not proof of the exact
FASTA or GENCODE release used to produce the full dataset.

Pangopup therefore uses these terms carefully:

- API assembly: GRCh38;
- source identity: the exact Zenodo archive checksum;
- annotation identity: masked against an unspecified GENCODE release unless the
  creators or additional source metadata establish it;
- compatibility evidence: recorded separately from source provenance.

The production builder now compares every ordinary source locus to the 25
primary sequences in NCBI RefSeq GRCh38.p14 assembly `GCF_000001405.40`. Its
explicit alias table maps `chr1`…`chr22`, `chrX`, `chrY`, and `chrM` to the
build-qualified RefSeq accessions. Source `REF=N` loci remain lossless source
exceptions and are not claimed as external reference matches. The manifest
records both the supplied FASTA byte identity and a canonical required-sequence
set identity, keeping observed compatibility separate from publisher claims.

Plain and ordinary single-member gzip FASTA are accepted as explicit read-only
inputs. One sequential pass validates IUPAC bytes and accession uniqueness,
hashes the supplied bytes, and writes only the normalized primary sequences to
private disk scratch. Extra records are ignored for certification but bound by
a sorted-accession count and digest. No external `.fai` or `.gzi` is consumed.

The complete certification against the supplied RefSeq GRCh38.p14 genomic
FASTA found zero ordinary-reference mismatches across 1,366,418,525 ordinary
gene loci. All 30 source `REF=N` loci were preserved separately. The supplied
gzip identity is
`sha256:11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3`;
the normalized 25-sequence identity is
`sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4`.
See the retained [Ticket 003 build report](../planning/artifacts/003-full-index-build.md).

## License handling

The dataset remains CC BY 4.0. Every distributed derived index must include:

- the title, creators, DOI, and CC BY 4.0 reference;
- the exact input archive checksum;
- a statement that Pangopup transformed, reordered, validated, indexed, and
  losslessly packed the per-gene TSV data;
- an indication of any further modifications.

The source code’s GPL-3.0 license does not replace the dataset’s CC BY license.
`NOTICE` is part of every bundle and release packaging contract.

## Distribution

The transformed sparse index is distributed as a separately named GitHub
release asset, not as a Git object. Transport compression is permitted because
installation expands and verifies the direct mmap bundle once; it does not put
decompression on the lookup path. See [`delivery.md`](delivery.md).
