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

The builder validates source reference alleles for internal consistency within
and across source records. That does not establish external reference identity.
A production certification compares every unique locus to one pinned GRCh38
reference index, uses an explicit versioned primary-contig alias/accession table,
and records the resulting identity and mismatch count.

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
