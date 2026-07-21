# Published source inspection

The offline builder validates real, attributed Pangolin source excerpts in a
deterministic filename order. The report keeps source direction and rare gap
and ambiguous-reference facts visible rather than normalizing them away.

```bash
pangopup-build inspect ../tests/fixtures/pangolin-precompute/ | mustmatch like "file gene=ENSG00000010610 contig=chr12 direction=ascending first=6801301 last=6801539 rows=711 loci=237 segments=3 gaps=2 omitted_bases=2 ambiguous_ref_loci=0 n_omit_a=0 n_omit_t=0
...
file gene=ENSG00000185974 contig=chr13 direction=ascending first=113673020 last=113723021 rows=6 loci=2 segments=2 gaps=1 omitted_bases=50000 ambiguous_ref_loci=0 n_omit_a=0 n_omit_t=0
total genes=6 rows=6342 loci=2114 ascending=4 descending=2 segments=9 gaps=3 omitted_bases=50002 ambiguous_ref_loci=2 n_omit_a=1 n_omit_t=1"
```

Malformed source fails with a source member, one-based line, and precise
invariant. It does not print a successful total.

```bash run id=malformed exit=1 stream=stderr
pangopup-build inspect ../tests/fixtures/pangolin-precompute-malformed/
```

```text expect=malformed contains
error: ENSG00000000003.tsv.gz:4: duplicate alternate G at chr1:100 A
```

An incomplete invocation is a command-usage error rather than a source error.

```bash run id=usage exit=2 stream=stderr
pangopup-build inspect
```

```text expect=usage contains
Usage: pangopup-build inspect <SOURCE_DIR>
```
