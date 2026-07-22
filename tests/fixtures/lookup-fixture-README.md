# SNV lookup fixture

`make-lookup-fixture.sh` deterministically creates the compact production-build
fixture used by `spec/snv-lookup.md`. Its gzip FASTA has all 25 required
GRCh38.p14 primary RefSeq accessions and a highly compressible chr17 record long
enough for the real overlap coordinate. Synthetic chr1 loci provide ambiguity,
mixed, miss, and 100-distinct-hit cases.

The six rows for `ENSG00000141499` (WRAP53) and `ENSG00000141510` (TP53) are
unchanged rows selected from **Pangolin precomputed scores**, Nils Wagner and
Aleksandr Neverov, Zenodo record 15649338, DOI
<https://doi.org/10.5281/zenodo.15649338>, CC BY 4.0. The source archive is
`Pangolin_hg38_snvs_masked.zip`, MD5
`679ef0b50e511b6102b4b88fbf811108`. Pangopup adds only the deterministic
synthetic rows and reference needed to exercise the adapter contract.
