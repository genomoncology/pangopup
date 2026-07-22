# Ticket 005 deterministic SNV transport proof

Date: 2026-07-22

This report binds the deterministic split transport to a fresh full-corpus
bundle produced from the final Ticket 005 implementation. One certified bundle
and one of two byte-identical transports are retained outside Git, keyed by the
identities below. The duplicate transport, unpacked copy, stages, scratch, and
measurement logs were removed after the facts below were recorded. Input paths
and host-specific proof-store paths are intentionally not retained.

## Implementation and environment

| Field | Value |
|---|---|
| Base Git commit | `1a17a761c435cadc03e7080b7c67e180a4e2c2d1` plus the uncommitted Ticket 005 implementation under test |
| Approved implementation commit | `4161679b362805b706a5bfd2a8b24a25df5e23fb` |
| Compiler | `rustc 1.93.1 (01f6ddf75 2026-02-11)`, LLVM 21.1.8, `x86_64-unknown-linux-gnu` |
| OS | Ubuntu Linux, kernel `6.17.0-35-generic`, x86-64 |
| Host | AMD Ryzen 7 5825U, 8 cores / 16 threads; 29,340,872,704 bytes RAM; Crucial CT1000P3PSSD8 NVMe |
| Encoder | `zstd` 0.13.3 / `zstd-safe` 7.2.4 / `zstd-sys` 2.0.16+zstd.1.5.7; bundled libzstd 1.5.7 |
| Encoder settings | level 9; checksum and content-size enabled; no dictionary; zero workers; long-distance matching disabled |
| Release maintenance binary | 2,528,192 bytes; `sha256:cee31a5fc4e1002c5da4695bbc0f843c50fe6c1da331a95c7998a7d9d476991d` |

The assets crate's build script rejects `ZSTD_SYS_USE_PKG_CONFIG`; a negative
build check returned exit 101 with the explicit bundled-libzstd diagnostic.
The gate also pins the complete small-fixture frame bytes and header.

## Commands and measurement method

The release maintenance binary was built from the implementation under test:

```bash
env -u ZSTD_SYS_USE_PKG_CONFIG \
  cargo build --locked --release -p pangopup-build
```

The full build and its independent verification used explicit, read-only
operator-supplied inputs:

```bash
/usr/bin/time -v -o build.time \
  target/release/pangopup-build build \
  --source "$PANGOPUP_SOURCE_DIR" \
  --reference "$PANGOPUP_GRCH38_FASTA" \
  --output "$PROOF/bundle"

/usr/bin/time -v -o bundle-verify.time \
  target/release/pangopup-build verify "$PROOF/bundle"
```

The transport proof used:

```bash
/usr/bin/time -v -o pack-a.time \
  target/release/pangopup-build transport pack \
  --bundle "$PROOF/bundle" --output "$PROOF/transport-a"
/usr/bin/time -v -o pack-b.time \
  target/release/pangopup-build transport pack \
  --bundle "$PROOF/bundle" --output "$PROOF/transport-b"
diff -qr "$PROOF/transport-a" "$PROOF/transport-b"

/usr/bin/time -v -o transport-verify.time \
  target/release/pangopup-build transport verify \
  --transport "$PROOF/transport-a"
/usr/bin/time -v -o unpack.time \
  target/release/pangopup-build transport unpack \
  --transport "$PROOF/transport-a" --output "$PROOF/unpacked"
/usr/bin/time -v -o unpacked-verify.time \
  target/release/pangopup-build verify "$PROOF/unpacked"
cmp "$PROOF/bundle/NOTICE" "$PROOF/unpacked/NOTICE"
cmp "$PROOF/bundle/manifest.json" "$PROOF/unpacked/manifest.json"
cmp "$PROOF/bundle/scores.pgi" "$PROOF/unpacked/scores.pgi"
```

GNU `time -v` provides wall time and maximum resident set size. A five-second
sampler measured each operation's owned output/staging namespace and retained
the largest apparent-byte observation. Those disk values are lower bounds at
that sampling interval. Peak RSS includes native libzstd allocations and may
also include file-backed mmap pages. The isolated gate resource test separately
uses a tracking Rust global allocator; that measurement excludes libzstd's
native C heap.

The proof originally began in a temporary directory. After every full proof
passed, the complete invocation directory was atomically renamed into a unique
work child under the operator-supplied proof store. Source and destination were
on the same filesystem and the directory inode was unchanged across the
rename. The accepted bundle and transport were then moved without replacement
to their identity-keyed retained locations. No pre-existing path was replaced.

## Pinned full-corpus inputs and semantics

| Fact | Value |
|---|---|
| Source | `Pangolin precomputed scores`; Nils Wagner and Aleksandr Neverov; DOI `10.5281/zenodo.15649338`; masked; window 50 |
| Published archive | `Pangolin_hg38_snvs_masked.zip`; 12,988,141,317 bytes; `md5:679ef0b50e511b6102b4b88fbf811108` |
| Observed source members | 19,913; `sha256:0e40ee8e0527210cb64c26a6637117aea7d41d696e7bd95f3bb9545ee16782f6` |
| Reference input | RefSeq GRCh38.p14 / `GCF_000001405.40`; gzip 972,898,531 bytes; `sha256:11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3` |
| Required reference set | 25 records; `sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4` |
| Ignored reference extras | 680 records; `sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb` |
| Corpus | 19,913 genes; 4,099,255,665 source rows; 1,366,418,555 gene loci |
| Member directions | 10,073 ascending / 9,840 descending |
| Segments and gaps | 19,916 source segments; 19,945 index segments; 3 gap transitions; 50,002 omitted bases |
| `REF=N` shapes | 30 loci: 9 omit-A and 21 omit-T |
| Source and decoded logical streams | 4,099,255,665 records each; `sha256:dcec29e84c5f65bd76ffde2be8c7fa312d08e6abdb1e45e024dc0fe8c8da9c31` each |

## Installed bundle identity

| Fact | Value |
|---|---|
| Bundle ID / canonical manifest SHA-256 | `sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3` |
| Manifest | 3,589 bytes |
| Builder | version `0.1.0`; source `sha256:10fd5d7715a611f9b7f20040887391502535ac7860bc6a1eda2bfdda79682b64` |
| `NOTICE` | 1,709 bytes; `sha256:9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7` |
| `scores.pgi` | 15,033,158,255 bytes; `sha256:6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27` |
| Exact installed set | `NOTICE`, `manifest.json`, `scores.pgi`; 15,033,163,553 bytes total |
| Build | 1:29:15 wall; 7,476,820 KiB peak RSS; exit 0; stderr empty |
| Independent bundle verify | 22:23.47 wall; 7,585,048 KiB peak RSS; two members verified; exit 0; stderr empty |

Installed-bundle certification exhaustively decodes all 4,099,255,665 logical
records and proves the complete decoded-stream count and SHA-256 against the
source identity. It does **not** call the public `ScoreProvider` lookup API once
for every SNV. Exhaustive public-lookup conformance and performance regression
are a separate follow-on proof.

## Deterministic split transport identity

| Fact | Value |
|---|---|
| Transport ID | `sha256:3a2f4901b8f3dece302640d0257cc98aa50010a45fe61c5ef77c64a62f4660aa` |
| Canonical `transport.json` | 1,266 bytes; `sha256:f9b7501087226fb35cbfa66fa9b903cc21eb8bbbacb067363b9eeef487ee9e9a` |
| Whole compressed stream | 1,931,687,706 bytes; `sha256:8b00b8b39cb07d0b5443e506bde097406c0533e50b5e1056ca026ea92d28134d` |
| Part 0 | 1,000,000,000 bytes; `sha256:07c1f9a2e33e1a5bd929500eefd00b84764c82d56e3f573c35d380419e4ed42a` |
| Part 1 | 931,687,706 bytes; `sha256:87580144fd828676d7adb269059cf2b425b342fe5ccee442888e0b93994adc74` |
| Exact transport set | `NOTICE`, `bundle-manifest.json`, `transport.json`, and the two numbered parts; 1,931,694,270 bytes total |
| Compression ratio | 12.849514% of `scores.pgi`; 87.150486% smaller |
| Frame facts | one standard Zstandard frame; 4 MiB window; checksum; pledged content size 15,033,158,255; no dictionary, second frame, or trailing bytes |

The two production packs had identical member names, sizes, manifests, hashes,
and bytes (`diff -qr` and `cmp` both passed for every member).

The cheap post-rename publication check passed without reading either large
payload. The accepted and retained bundle directory remained device/inode
`66306:37371125`; the accepted and retained transport directory remained
`66306:37372036`; and both former source names were absent. The destination
check opened an exact closed set of regular, non-symlink members, matched every
declared size, compared the small notice and copied manifests byte-for-byte,
and rechecked the bounded canonical small-manifest identities. The observed
retained member tuples were:

| Member | Device:inode:size |
|---|---|
| Bundle `NOTICE` | `66306:37371661:1709` |
| Bundle `manifest.json` | `66306:37371666:3589` |
| Bundle `scores.pgi` | `66306:37371654:15033158255` |
| Transport `NOTICE` | `66306:37372038:1709` |
| Transport `bundle-manifest.json` | `66306:37372037:3589` |
| Transport `payload.pgi.zst.part0000` | `66306:37372039:1000000000` |
| Transport `payload.pgi.zst.part0001` | `66306:37372040:931687706` |
| Transport `transport.json` | `66306:37371113:1266` |

This continuity check deliberately did not hash, decode, or scan
`scores.pgi` or either compressed part again. It applies because these paths
were newly published by same-filesystem rename; reuse of a pre-existing keyed
path would instead require fresh full verification.

## Reconstruction and resource results

| Operation | Wall time | Peak RSS | Peak owned disk added | Result |
|---|---:|---:|---:|---|
| Pack A | 25:19.44 | 9,790,056 KiB | 1,931,695,431 bytes | two parts; exit 0; stderr empty |
| Pack B | 26:31.20 | 7,748,844 KiB | 1,931,695,431 bytes | byte-identical to A; exit 0; stderr empty |
| Integrity-only transport verify | 20.71 s | 7,696 KiB | 0 payload scratch bytes | exact compressed and decompressed identities; exit 0; stderr empty |
| Unpack plus semantic certification | 21:47.69 | 5,793,908 KiB | 15,033,164,679 bytes | exact bundle identity; exit 0; stderr empty |
| Independent verify of unpacked bundle | 21:27.65 | 5,433,396 KiB | 0 payload scratch bytes | two members verified; exit 0; stderr empty |

The pack and unpack disk figures are five-second sampled lower bounds and
include the operation's small logs/metadata in addition to the published
directory. Transport verify created no decompressed payload file. All three
unpacked members passed byte-for-byte `cmp` against the original certified
bundle.

The isolated subprocess resource proof reported:

| Mode | Rust allocator peak delta | FD peak delta | Native-inclusive RSS delta | Fixture payload |
|---|---:|---:|---:|---:|
| Pack | 9,248,385 bytes | 2 | 11,948,032 bytes | 1,100,448 bytes |
| Verify | 269,283 bytes | 1 | 188,416 bytes | 1,100,448 bytes |
| Unpack | 9,254,704 bytes | 2 | 1,277,952 bytes | 1,100,448 bytes |

The Rust allocator figures do not include libzstd's native C heap. RSS is
native-inclusive but can include file-backed pages and is therefore not a pure
heap measurement. SHA-256 and frame checks prove integrity, not publisher
authentication.

The retained assets are the certified bundle and one canonical transport named
by the IDs above. After the approved implementation commit, the coordinator
recomputed the committed builder-source digest, proved it still equals
`sha256:10fd5d7715a611f9b7f20040887391502535ac7860bc6a1eda2bfdda79682b64`,
and atomically published the 2,194-byte canonical
`pangopup.proof-receipt.v1`. The receipt hashes to
`sha256:9ddae771d200fe73bda5f31f5a04a52227b77c5d3f225dc7ee52294cd9aea475`
and binds implementation commit
`4161679b362805b706a5bfd2a8b24a25df5e23fb`. It records only relative
identity-keyed verification commands; no local path or host identity enters
committed evidence.

## Gate

`make lint`, `make test`, and `make spec` passed; the spec suite reported 80
passing executable examples. Tests cover strict manifests and exact error
precedence, the shared installed-manifest parser, deterministic frame bytes,
all 1,000 synthetic part handles, independent corruption layers, read-only and
symlink behavior, atomic conflicts/races, SIGKILL publication behavior, and
isolated streaming resource limits.
