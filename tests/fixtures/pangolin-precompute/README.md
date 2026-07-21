# Pangolin precomputed-score excerpts

These six gzip members are selected, truncated, and deterministically
recompressed excerpts of **Pangolin precomputed scores**, created by Nils
Wagner and Aleksandr Neverov and published as Zenodo record 15649338,
<https://doi.org/10.5281/zenodo.15649338>, under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).

The source archive is `Pangolin_hg38_snvs_masked.zip`, identified by
MD5 `679ef0b50e511b6102b4b88fbf811108`. The excerpts were made on
2026-07-21. Pangopup changed only which complete source rows were selected and
the gzip container representation; the eight-column header and every retained
row value are unchanged.

| Member | Inclusive position range | gzip SHA-256 | decompressed SHA-256 |
|---|---:|---|---|
| `ENSG00000010610.tsv.gz` | `chr12:6801301..6801539` | `f66ba366f622668a7c9fce01c4fae14bf8744afa86474de5cb111a92398baaea` | `691020982c5cfd49f46b8bf5696e43e267d226082a504cf54f67095154b649cd` |
| `ENSG00000141499.tsv.gz` | `chr17:7686072..7686584` | `2adfcf2fc63d64375bb780f9defb22038302760ae9b901fde1dd5c667ee31866` | `c7eda59f80b89bcf56a711d3c35c7ee53ada440c8e608a8934b9d004933a6099` |
| `ENSG00000141510.tsv.gz` | `chr17:7686072..7687427` | `290573c40d8b3651266601c143ca182ac8547f97fca56f0f16887a4734a603cf` | `2b1c88676362ac7cd0725e9449bd827a47154181a7c166e430632013a013caa2` |
| `ENSG00000169129.tsv.gz` | `chr10:114306065..114306067` | `f2a05ab651d65ad1842dc2cf7bb9d6c0516f8179316376db8bcb00e3eb5d9057` | `905e2130880ebfff3543520905c6322785c2dc68861fbf77c95b0b1499b0b1d2` |
| `ENSG00000175727.tsv.gz` | `chr12:122093259..122093261` | `c4b38706d5a1f5e5067f38be6665cb5391e5e7a6892a7204aab52c34c38ceca8` | `770a72d52492b867cc0863749bc0a05f3aa7ea6a2d56fb6131cce480b330c56d` |
| `ENSG00000185974.tsv.gz` | `chr13:113673020..113723021` | `80cbce386ce3ae42cf6a4dd0ec2c5aaac726b38aff8b035fe937f1e7e338d11a` | `d316bb6a1b338ef8b7702651198bd86734841872b3d1e8d1df1612f826dd5b1d` |

Given an extracted and verified source directory, the following command shape
reproduces each member. Repeat it with the gene, lower bound, and upper bound
from the table:

```bash skip
SOURCE_DIR=/path/to/Pangolin_hg38_snvs_masked
gene=ENSG00000010610
lo=6801301
hi=6801539
gzip -dc "$SOURCE_DIR/$gene.tsv.gz" \
  | awk -v lo="$lo" -v hi="$hi" 'NR == 1 || ($2 >= lo && $2 <= hi)' \
  | gzip -n -9 > "$gene.tsv.gz"
```

`gzip -n` omits the original filename and timestamp. Verify both identities
with `sha256sum member.tsv.gz` and `gzip -dc member.tsv.gz | sha256sum`.
