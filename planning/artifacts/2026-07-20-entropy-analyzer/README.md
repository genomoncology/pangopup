# Reproduce the complete-corpus entropy scan

This is the retained analysis-only Rust program used to produce
[`../2026-07-20-full-dataset-entropy.md`](../2026-07-20-full-dataset-entropy.md).
It is not Pangopup product code and is not a workspace member.

Environment used for the recorded scan:

- Rust 1.93.1 (`01f6ddf75 2026-02-11`)
- 8 Rayon workers for the distribution/compression pass
- 4 Rayon workers for the exact joint-locus histogram
- `flate2`/`zlib-rs` 1.1.9, `zstd` 0.13.3, `lz4_flex` 0.11.6
- source ZIP verified as `md5:679ef0b50e511b6102b4b88fbf811108`

Run the exact distribution and joint-locus entropy pass:

```bash skip
SOURCE_DIR=/path/to/Pangolin_hg38_snvs_masked
RAYON_NUM_THREADS=4 RUST_MIN_STACK=134217728 \
  cargo run --locked --release -- "$SOURCE_DIR"
```

Run the practical block-compression measurements by setting the compression
flag (the output also repeats the distribution):

```bash skip
SOURCE_DIR=/path/to/Pangolin_hg38_snvs_masked
PANGOPUP_COMPRESS=1 RAYON_NUM_THREADS=8 RUST_MIN_STACK=67108864 \
  cargo run --locked --release -- "$SOURCE_DIR"
```

The analyzer fails on malformed headers, score signs/ranges, relative positions,
row grouping, alternate sets, duplicate positions, and direction changes. It
reports rather than rejects the two observed sparse-coordinate exception files
and the 30 `REF=N` loci.
