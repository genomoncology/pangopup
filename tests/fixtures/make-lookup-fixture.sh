#!/usr/bin/env bash
set -euo pipefail

destination=$1
case "$destination" in
  */target/spec/*|target/spec/*) ;;
  *) printf 'fixture destination must be under target/spec\n' >&2; exit 2 ;;
esac
rm -rf "$destination"
mkdir -p "$destination/source"

{
  printf 'chrom\tpos\tref\talt\tgain_score\tgain_pos\tloss_score\tloss_pos\n'
  printf 'chr1\t3\tG\tA\t0.0\t-50\t-0.10\t2\n'
  printf 'chr1\t3\tG\tC\t0.0\t-50\t-0.0\t-50\n'
  printf 'chr1\t3\tG\tT\t0.0\t-50\t-0.0\t-50\n'
  for pos in $(seq 5 104); do
    printf 'chr1\t%s\tA\tC\t0.01\t1\t-0.0\t-50\n' "$pos"
    printf 'chr1\t%s\tA\tG\t0.0\t-50\t-0.0\t-50\n' "$pos"
    printf 'chr1\t%s\tA\tT\t0.0\t-50\t-0.0\t-50\n' "$pos"
  done
} | gzip -n -9 > "$destination/source/ENSG00000000003.tsv.gz"

{
  printf 'chrom\tpos\tref\talt\tgain_score\tgain_pos\tloss_score\tloss_pos\n'
  printf 'chr1\t3\tN\tA\t0.0\t-50\t-0.0\t-50\n'
  printf 'chr1\t3\tN\tC\t0.0\t-50\t-0.0\t-50\n'
  printf 'chr1\t3\tN\tG\t0.0\t-50\t-0.0\t-50\n'
  printf 'chr1\t105\tN\tC\t0.0\t-50\t-0.0\t-50\n'
  printf 'chr1\t105\tN\tG\t0.0\t-50\t-0.0\t-50\n'
  printf 'chr1\t105\tN\tT\t0.0\t-50\t-0.0\t-50\n'
  printf 'chr1\t106\tA\tC\t0.0\t-50\t-0.10\t2\n'
  printf 'chr1\t106\tA\tG\t0.0\t-50\t-0.0\t-50\n'
  printf 'chr1\t106\tA\tT\t0.0\t-50\t-0.0\t-50\n'
} | gzip -n -9 > "$destination/source/ENSG00000000004.tsv.gz"

{
  printf 'chrom\tpos\tref\talt\tgain_score\tgain_pos\tloss_score\tloss_pos\n'
  printf 'chr17\t7686072\tG\tA\t0.09\t25\t-0.0\t-50\n'
  printf 'chr17\t7686072\tG\tC\t0.03\t25\t-0.0\t-50\n'
  printf 'chr17\t7686072\tG\tT\t0.35\t25\t-0.0\t-50\n'
} | gzip -n -9 > "$destination/source/ENSG00000141499.tsv.gz"

{
  printf 'chrom\tpos\tref\talt\tgain_score\tgain_pos\tloss_score\tloss_pos\n'
  printf 'chr17\t7686072\tG\tT\t-0.0\t-50\t-0.0\t-50\n'
  printf 'chr17\t7686072\tG\tC\t-0.0\t-50\t-0.0\t-50\n'
  printf 'chr17\t7686072\tG\tA\t0.0\t-50\t-0.0\t-50\n'
} | gzip -n -9 > "$destination/source/ENSG00000141510.tsv.gz"

{
  for accession in NC_000001.11 NC_000002.12 NC_000003.12 NC_000004.12 NC_000005.10 \
    NC_000006.12 NC_000007.14 NC_000008.11 NC_000009.12 NC_000010.11 NC_000011.10 \
    NC_000012.12 NC_000013.11 NC_000014.9 NC_000015.10 NC_000016.10; do
    printf '>%s lookup fixture\n' "$accession"
    if [[ "$accession" == NC_000001.11 ]]; then printf 'NNG'; printf 'A%.0s' $(seq 4 106); printf '\n'; else printf 'A\n'; fi
  done
  printf '>NC_000017.11 lookup fixture\n'
  perl -e 'print "A" x 7686071, "G\n"'
  for accession in NC_000018.10 NC_000019.10 NC_000020.11 NC_000021.9 NC_000022.11 \
    NC_000023.11 NC_000024.10 NC_012920.1; do printf '>%s lookup fixture\nA\n' "$accession"; done
} | gzip -n -9 > "$destination/reference.fa.gz"
