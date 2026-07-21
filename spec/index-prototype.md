# Measured fixed-index prototype

The developer/admin command streams the checked fixture into the selected
fixed 11-byte private format, reopens it through the mmap reader, and verifies
all 6,342 ordinary and exceptional source rows exactly.

```bash
mkdir -p ../target/spec
pangopup-build prototype-roundtrip ../tests/fixtures/pangolin-precompute/ ../target/spec/prototype-fixed-11.pgi | mustmatch like "prototype format=fixed-11-v1 bytes=25040 genes=6 rows=6342 loci=2114 segments=11 exceptions=2 verified_rows=6342"
```

A corrupt artifact is rejected with a stable typed reason.

```bash run id=corrupt-index exit=1 stream=stderr
cp ../target/spec/prototype-fixed-11.pgi ../target/spec/prototype-corrupt.pgi
printf X | dd of=../target/spec/prototype-corrupt.pgi bs=1 seek=0 conv=notrunc status=none
pangopup-build prototype-open ../target/spec/prototype-corrupt.pgi
```

```text expect=corrupt-index contains
error: invalid index: wrong magic
```
