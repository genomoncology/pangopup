# 005 — Deterministic split transport for the certified SNV bundle

Status: ready

## Why

Pangopup can build, certify, and query the complete GRCh38 SNV bundle, but it
cannot yet turn that bundle into release-sized files. The certified
`scores.pgi` member is 15,033,158,255 bytes. A retained Zstandard experiment
compressed the bundle to 1,935,000,209 bytes, which is too close to GitHub's
under-2-GiB per-asset limit to publish as one dependable asset.

This ticket adds a deterministic, streaming transport that splits the
compressed score member into 1,000,000,000-byte parts and reconstructs the
exact three-file installed bundle. It is deliberately only local packaging:
no release upload, network download, XDG discovery, managed installation,
model asset, or service behavior belongs here.

## Scope

### Library boundary

- Add a `pangopup-assets` library crate. It owns exhaustive installed-bundle
  certification, the byte-exact Pangopup `NOTICE`, the SNV transport manifest,
  deterministic compression and splitting, streaming integrity verification,
  streaming reconstruction, and typed asset errors. The dependency direction
  is exactly `pangopup-core <- pangopup-index <- pangopup-assets <-
  pangopup-build`; no reverse or cyclic dependency is permitted.
- Move the existing exhaustive `verify_bundle` implementation out of
  `pangopup-build` into a typed `pangopup-assets` API. Preserve
  `pangopup-build verify <BUNDLE>` and its existing observable error behavior
  as a thin adapter, and keep its existing tests as regressions. The bundle
  builder calls the same assets API before publication; transport pack and
  unpack call it at their certification boundaries. There is one semantic
  verifier, not copied transport-specific verification logic.
- Extend the existing deterministic builder-source digest to include sorted,
  length-framed workspace-relative `Cargo.toml` and Rust source inputs from
  `pangopup-assets`, because the builder now depends on its exact notice and
  certification rules. Preserve the digest algorithm and add a regression that
  proves a changed assets input changes the builder identity.
- Expose from `pangopup-index` one public strict bounded canonical bundle-
  manifest parser/validator used by both `BundleOpen` and `pangopup-assets`.
  It accepts at most 1 MiB, first discriminates supported schema/index versions
  so future formats are typed as incompatible, then enforces the closed v1
  schema, canonical RFC 8785 bytes, values, member set, and checked arithmetic.
  Do not add a second serde path that can interpret the same manifest
  differently.
- Keep `pangopup-core` free of file-format and delivery concerns,
  `pangopup-index` free of transport concerns, and `pangopup-build` as the thin
  maintenance-CLI adapter. Do not change `pangopup.fixed11.v1` or the runtime
  lookup path.
- Reuse the existing exhaustive bundle verifier before packing and after
  unpacking. If an appropriate test-fixture helper already exists, import it;
  otherwise define a test-only helper in the new crate rather than widening a
  production API merely for tests.

### Transport representation

One transport is a directory containing exactly:

```text
transport.json
bundle-manifest.json
NOTICE
payload.pgi.zst.part0000
payload.pgi.zst.part0001
... up to part0999
```

`bundle-manifest.json` and `NOTICE` are byte-for-byte copies of the installed
bundle's `manifest.json` and `NOTICE`. Only `scores.pgi` is compressed. This
avoids a tar implementation, tar's large-file extensions, and a second
uncompressed archive while preserving every installed byte.

`transport.json` is strict RFC 8785 canonical JSON with this closed logical
shape (the implementation may use typed nested Rust structures, but no fields
may be omitted or added):

```text
schema: "pangopup.snv-transport.v1"
transport_id: "sha256:" + 64 lowercase hex
bundle:
  bundle_id: "sha256:" + 64 lowercase hex
  manifest: {path:"bundle-manifest.json", size:safe_json_u64, sha256:sha256}
  notice: {path:"NOTICE", size:safe_json_u64, sha256:sha256}
  scores: {installed_path:"scores.pgi", size:safe_json_u64, sha256:sha256}
compression:
  format: "zstd.frame.v1"
  level: 9
  checksum: true
  content_size: true
  dictionary: false
  workers: 0
  encoder_crate: "zstd/0.13.3"
  libzstd_version: "1.5.7"
payload:
  compressed_size: safe_json_u64
  compressed_sha256: sha256
  part_size: 1000000000
  parts: 1..1000 entries in ordinal order, each
    {ordinal:u16, path:"payload.pgi.zst.partNNNN", size:safe_json_u64,
     sha256:sha256}
```

`safe_json_u64` means an integer in `0..=9_007_199_254_740_991`. Arithmetic
uses checked `u64`; the checked part-size sum must equal `compressed_size`.
`transport_id` is the SHA-256 of the canonical manifest bytes with the
`transport_id` member omitted, serialized again after inserting that identity.
Unknown or duplicate keys, noncanonical JSON, invalid hash spelling, unsupported
versions, inconsistent duplicated values, overflow, and invalid part grammar
fail closed.
Use one duplicate-aware bounded parse to obtain the schema/compression
discriminators before selecting the strict supported-v1 decoder: a well-formed
future discriminator is `TRANSPORT_INCOMPATIBLE` even if it contains fields v1
does not know, while supported v1 is closed and rejects every unknown field.

Read `transport.json` and `bundle-manifest.json` with a 1 MiB hard limit each:
check metadata first, allocate no more than the cap plus one byte, and reject an
over-limit or growing input. `NOTICE` has a 64 KiB general read cap and, for v1,
must be the current exact 1,709 bytes with SHA-256
`9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7`.
Bound directory enumeration to the three fixed metadata entries plus at most
1,000 declared parts, failing as soon as another entry is observed. The inner
declared `scores.pgi` size must be no more than
`MAX_FIXED11_BYTES = 17_179_869_184` (16 GiB); this intentionally bounded v1
transport would need a versioned change for a larger fixed index.

The bundle identity is the SHA-256 of the exact installed `manifest.json`
bytes. Require all of the following equalities rather than trusting duplicated
outer fields:

- outer `bundle.bundle_id` equals both the outer manifest descriptor SHA-256
  and the actual exact `bundle-manifest.json` SHA-256;
- the inner manifest is accepted by the one strict index parser and declares
  exactly the `NOTICE` and `scores.pgi` installed members;
- outer notice and score path/size/hash descriptors exactly equal those inner
  member descriptors and the actual bytes;
- inner attribution is exactly `NOTICE`, `CC-BY-4.0`, transformed=true, and
  the notice bytes equal Pangopup's embedded notice above; and
- the Zstandard frame's pledged decompressed size and actual decompressed
  size/hash equal the same inner `scores.pgi` size/hash.

A transport whose manifest and hashes have all been replaced can be internally
self-consistent; SHA-256 is integrity, not publisher authentication. Even such
a re-signed semantically corrupt fixed-v1 payload may pass `transport verify`,
but `transport unpack` must reject it when exhaustive installed-bundle
certification runs before publication. Signing/authentication remains excluded.

Compression is exactly one standard Zstandard frame over `scores.pgi`, using
exact Cargo requirements `zstd = "=0.13.3"`, `zstd-safe = "=7.2.4"`, and
`zstd-sys = "=2.0.16+zstd.1.5.7"`. Build the bundled libzstd 1.5.7 source: do
not enable the `pkg-config` feature or accept `ZSTD_SYS_USE_PKG_CONFIG`; fail
the Pangopup build explicitly if that override is set, so a release cannot
silently substitute a system library. Record and assert
`ZSTD_versionString() == "1.5.7"`. Encoder parameters are level 9, checksum
enabled, pledged content size set to the exact index length, content-size flag
enabled, dictionary and dictionary ID absent, `nbWorkers=0`, long-distance
matching disabled, and otherwise libzstd 1.5.7 defaults.

Before decompression, parse the real frame header and require a standard (not
skippable) single frame, known pledged content size exactly equal to the inner
score size, checksum flag 1, and dictionary ID 0. Pin the complete golden frame
header and compressed bytes for a small production-encoder fixture. During
decode, stop after at most the declared score size plus one byte; producing the
extra byte is an error. Reject an incomplete frame, checksum failure, second
frame, or any trailing byte even if a convenience decoder would accept it.
Deterministic compressed bytes are promised only for the locked encoder and
libzstd versions. Encoder upgrades create a new transport identity and require
new evidence; they do not change the installed bundle identity.

Split the compressed byte stream at exact 1,000,000,000-byte boundaries. Every
nonfinal part is exactly that size; the final part is 1..1,000,000,000 bytes.
Parts are fragments of one frame, not independently decompressible frames.

### Commands and observable behavior

Add these maintenance commands:

```text
pangopup-build transport pack --bundle <BUNDLE> --output <ABSENT_DIR>
pangopup-build transport verify --transport <TRANSPORT_DIR>
pangopup-build transport unpack --transport <TRANSPORT_DIR> --output <ABSENT_DIR>
```

The subcommand and long flags above are exact. Flags may appear in either
order where there are two, but each required flag appears exactly once with
one OS-path value; no positional arguments, aliases, short flags, or additional
flags are accepted. `pack` requires an existing bundle directory and absent
output. `verify` requires an existing transport directory. `unpack` requires
an existing transport directory and absent output. Validate grammar before
platform support so the same malformed invocation is `CLI_USAGE` everywhere.

The pure manifest/part/frame `verify` command is portable. `pack` and `unpack`
are Linux-only because they publish directories durably with Linux
`renameat2(RENAME_NOREPLACE)`; a grammatically valid invocation on another
platform returns `UNSUPPORTED_PLATFORM` before creating or changing output.

Usage failures exit 2 with `CLI_USAGE`. Runtime failures exit 1 with one of:

```text
INPUT_IO
OUTPUT_IO
MANIFEST_INVALID
TRANSPORT_INCOMPATIBLE
PART_SET_INVALID
TRANSPORT_HASH_MISMATCH
COMPRESSION_INVALID
BUNDLE_INVALID
OUTPUT_CONFLICT
UNSUPPORTED_PLATFORM
```

Each failure writes no stdout and exactly one compact, LF-terminated JSON
object to stderr using the existing error envelope:

```json
{"status":"error","code":"PART_SET_INVALID","message":"human-readable text","details":null}
```

Human-readable message text is not stable. Successful commands write empty
stderr and one compact, LF-terminated stdout object with exact field order:

```text
pack:   status, transport_id, bundle_id, part_count, compressed_bytes
verify: status, transport_id, bundle_id, part_count, compressed_bytes
unpack: status, transport_id, bundle_id
```

Use status values `packed`, `verified`, and `unpacked` respectively. Do not
emit host paths in success output or retained artifacts.

The CLI maps typed asset failures by this closed table; tests pin precedence at
boundaries where more than one defect is possible:

| Code | Exact class |
|---|---|
| `CLI_USAGE` | Missing/duplicate/unknown command or flag, positional argument, or missing flag value |
| `UNSUPPORTED_PLATFORM` | Valid `pack`/`unpack` attempted off Linux |
| `INPUT_IO` | Cannot open/read/stat a required user-supplied bundle or transport input |
| `OUTPUT_IO` | Cannot create/write/sync/remove this invocation's output staging for a reason other than destination conflict |
| `MANIFEST_INVALID` | Over-limit, malformed, duplicate-key, noncanonical, overflowed, or internally inconsistent supported transport manifest |
| `TRANSPORT_INCOMPATIBLE` | Well-formed discriminator names an unsupported transport/compression schema or version |
| `PART_SET_INVALID` | Directory/member count, name, ordinal, type, symlink, declared size, gap, duplicate, or exact-set violation |
| `TRANSPORT_HASH_MISMATCH` | Actual copied metadata, part, whole compressed stream, or decompressed payload size/hash differs from its valid descriptor |
| `COMPRESSION_INVALID` | Invalid/unsupported frame header or flags, decode/checksum error, size expansion, incomplete frame, second frame, or trailing bytes |
| `BUNDLE_INVALID` | Inner canonical manifest, exact notice/provenance, input bundle certification, or reconstructed bundle certification fails |
| `OUTPUT_CONFLICT` | Requested final output already exists or loses a no-replace publication race; the existing destination is untouched |

An unreadable `transport.json` is `INPUT_IO`; readable bytes that exceed or
violate the transport schema are `MANIFEST_INVALID`. A supported outer
manifest that faithfully describes invalid inner bundle metadata is
`BUNDLE_INVALID`. Part type/name/declared-size checks precede part hashing;
valid part structure with wrong bytes is `TRANSPORT_HASH_MISMATCH`; a
hash-consistent but invalid compressed frame is `COMPRESSION_INVALID`.

`pack` first runs exhaustive verification on the supplied certified bundle.
It creates a unique sibling stage on the output filesystem, then streams
`scores.pgi` through the encoder, whole-stream hasher, and part writer in one
pass; it never holds a complete member or compressed stream in heap and never
creates a raw archive. Write `transport.json` last. Sync every staged file and
the staged directory, publish the complete staged directory with no-replace
rename, and fsync the output parent before success.

`verify` validates the closed manifest, exact directory member set, copied
small members, every part's opened regular-file size/hash, canonical ordering,
whole compressed size/hash, exactly one Zstandard frame with no trailing bytes,
and the decompressed score size/hash. It streams decompressed bytes to a hash
sink and does not create a 15 GB temporary file. This command proves transport
integrity and exact installed bytes, not publisher authenticity.

`unpack` performs the same transport verification while streaming the score
member into a sibling staging directory, installs the small members under
their original names, then runs the existing exhaustive bundle verifier on
that staged three-file bundle. Only a fully verified directory is atomically
published to the absent output path. An ordinary failure removes only staging
created by that invocation and never exposes a partial final bundle.

For both `pack` and `unpack`, a race for the same absent output has one winner;
the loser returns `OUTPUT_CONFLICT`. Use a cryptographically unpredictable
unique same-filesystem sibling stage, synced files/directories,
`renameat2(RENAME_NOREPLACE)`, and parent-directory fsync. Ordinary failure
removes only that invocation's still-unpublished stage. Never delete, replace,
chmod, inspect as a candidate stage, or otherwise mutate an existing/conflicting
final path. A SIGKILL may leave only an unpublished uniquely named stage; if it
lands after rename, the final directory is already complete, though the command
may not have returned success or completed its parent fsync.

Reject symlinks and non-regular transport files, open at most one part at a
time, and stream from the same opened handle that was inspected.
Do not add XDG paths, persistent locks, marker recovery, arbitrary stale-file
cleanup, or an adversarial multi-user directory protocol; those belong to the
managed installer. Neither command recognizes or removes a prior abandoned
stage as part of this ticket.

### Tests and retained proof

- `make test` owns inside-out tests for canonical manifest round trips and
  rejection, the single shared bounded inner-manifest parser, typed bundle-
  certification API and preserved build-adapter regressions, transport identity,
  deterministic Zstandard bytes/header, exact split boundaries, one-part and
  multi-part iteration, bounded handles/reads/enumeration, regular-file checks,
  atomic staged publication, and typed error-to-CLI mapping. Pin a small golden
  compressed fixture produced by the production encoder. An internal test-only
  small split threshold may test boundary logic; the production parser must
  accept only 1,000,000,000.
- Tests must corrupt each layer independently: outer manifest, copied inner
  manifest, notice, individual part, part order/name/count/size, whole stream,
  Zstandard magic/type/checksum/dictionary/content size, truncated input,
  declared-size-plus-one expansion, trailing bytes/second frame, decompressed
  size/hash, and reconstructed fixed-v1 bytes. Include a self-consistent
  rehashed but semantically invalid fixed-v1 payload: transport verify may pass
  its integrity contract, while unpack must fail `BUNDLE_INVALID` without
  publication. Test destination conflict, ordinary cleanup, read-only input,
  SIGKILL leaving no partial final, and one concurrent publication race.
- A streaming resource test uses a deterministic fixture large enough to span
  many internal I/O buffers. Run the pack/verify/unpack measurement modes as
  isolated subprocesses so the tracking global allocator has no parallel-test
  noise. It proves peak **Rust allocator** growth <=16 MiB and no more than
  eight additional open file descriptors, independent of total payload bytes
  and declared part count. This allocator cannot see libzstd's native C heap;
  record subprocess peak RSS separately, and disclose that RSS can also include
  file-backed pages. A separate synthetic iterator test covers all 1,000
  declared part handles without treating tiny parts as a valid production
  transport.
- `make spec` owns exact CLI grammar, exit codes, stdout/stderr JSON, repeated
  deterministic pack, verify, unpack, conflict behavior, corruption failures,
  and byte-identical installed reconstruction using a checked-in miniature
  bundle. Add `spec/snv-transport.md`.
- Full-corpus proof rebuilds a bundle from explicit
  `PANGOPUP_SOURCE_DIR` and `PANGOPUP_GRCH38_FASTA`, runs the existing
  exhaustive verifier, and requires every established production identity and
  semantic invariant below:

  - source title `Pangolin precomputed scores`; creators Nils Wagner and
    Aleksandr Neverov; DOI `10.5281/zenodo.15649338`; archive
    `Pangolin_hg38_snvs_masked.zip`, 12,988,141,317 bytes,
    `md5:679ef0b50e511b6102b4b88fbf811108`; 19,913 observed members with
    `sha256:0e40ee8e0527210cb64c26a6637117aea7d41d696e7bd95f3bb9545ee16782f6`;
    masked=true and window=50;
  - reference RefSeq GRCh38.p14 / `GCF_000001405.40`; gzip input
    972,898,531 bytes with
    `sha256:11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3`;
    exactly 25 required primary records with sequence-set
    `sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4`;
    680 ignored extra records with sorted-accession
    `sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb`;
  - 19,913 genes; 4,099,255,665 source rows; 1,366,418,555 gene loci;
    10,073/9,840 ascending/descending source members; 19,916/19,945
    source/index segments; three gap transitions; 50,002 omitted bases; 30
    `REF=N` loci comprising 9 omit-A and 21 omit-T shapes;
  - canonical source and complete decoded streams each contain 4,099,255,665
    records and each hash to
    `sha256:dcec29e84c5f65bd76ffde2be8c7fa312d08e6abdb1e45e024dc0fe8c8da9c31`;
  - installed member set exactly `NOTICE`, `manifest.json`, `scores.pgi`;
    `NOTICE` is the exact 1,709-byte embedded value with the hash above and
    exact CC BY 4.0 attribution; canonical `manifest.json` is 3,589 bytes with
    its new exact hash recorded; `scores.pgi` is 15,033,158,255 bytes with
    `sha256:6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27`;
    and the installed bundle totals 15,033,163,553 bytes.

  Source-moving refactors in this ticket change the builder-source digest and
  therefore the exact canonical `manifest.json` and bundle ID. Record the new
  builder source SHA-256, manifest size/hash/bundle ID, transport ID, whole
  compressed identity, part identities, and unpacked identity from the actual
  final diff; do not expect Ticket 003's old builder or bundle IDs.

  Pack that bundle twice to different outputs and require byte-identical exact
  directory member sets. Verify one transport, unpack it, exhaustively certify
  the result through the moved shared API and independently through the
  preserved build CLI, and byte-compare all three installed members.
- Retain `planning/artifacts/005-snv-transport.md` with the source/reference,
  bundle, manifest/NOTICE/member, transport, encoder, frame, part, and unpacked
  identities and all counts above; exact commands; compressed size and ratio;
  wall time; isolated Rust allocator and FD proof; native-inclusive peak RSS;
  and peak owned disk for pack, verify, and unpack. Explain that sampled
  RSS/disk values are lower bounds at the stated sampling interval, the Rust
  allocator excludes libzstd's native heap, and RSS can include file-backed
  pages. Delete all full generated bundles, transports, staging, and unpacked
  copies after recording proof. Never retain local source paths.

### Documentation and exclusions

Update these exact durable documents in the implementation diff:

- `README.md`: shipped local pack/verify/unpack commands, transport contents,
  and the distinction between transport verification and installed-bundle
  certification;
- `architecture/delivery.md`: replace the target-only split description with
  the shipped no-tar score-stream transport and leave network/install as target;
- `architecture/design.md`: replace the future-assets placeholder with the
  shipped dependency direction and shared certification ownership;
- `architecture/index.md`: state that transport never changes fixed-v1 bytes;
- `architecture/README.md`: add the new accepted transport ADR;
- `architecture/decisions/0007-deterministic-snv-transport.md`: record the
  representation, determinism boundary, verification layers, and exclusions;
- `planning/faq.md`: answer what can now be packaged and what still cannot be
  downloaded or installed automatically;
- `planning/frontier.md`: advance the current boundary to local deterministic
  transport and keep managed delivery as the next front.

Do not edit `planning/goals.md` merely to report ticket completion: its durable
outcomes do not change. Excluded from this ticket are GitHub API/publication,
release signing or authenticity, networking/resume/retry, XDG or platform data
directories, managed install locks and garbage collection, automatic startup,
executable/model/reference/mask assets, model inference/cache, HTTP, Docker,
and any fixed-v1 or lookup change.

## Success Checklist

- A certified three-file bundle packs twice into byte-identical canonical
  transport directories whose parts are each <=1,000,000,000 bytes.
- Transport verification streams and fails closed at every declared integrity
  layer without materializing the score member.
- Unpack never publishes partial output; its installed files are byte-identical
  to the input bundle and pass exhaustive existing bundle verification.
- Small production-path tests prove exact CLI behavior, corruption handling,
  bounded heap/FD use, and deterministic bytes.
- Full-corpus evidence proves the pinned fixed-v1 member, deterministic
  transport, exact reconstruction, resource boundaries, and deletion of all
  generated full assets.
- The exact durable docs named above describe what ships and keep network,
  managed installation, model fallback, and service work explicitly future.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

### 1. Transport the large member directly instead of inventing an archive

- **Consideration:** The installed bundle has two tiny metadata files and one
  15 GB index; ordinary ustar cannot represent that member without extensions.
- **Options:** deterministic tar with a pinned large-file extension; a new
  general archive format; or copy the two exact small files and compress only
  `scores.pgi`.
- **Trade-offs:** Tar is familiar but adds header/version/large-size edge cases
  that provide no value for three fixed members. A general container could be
  reusable but expands this ticket. Direct transport is deliberately
  SNV-specific but simplest to inspect and reconstruct.
- **Decision:** Publish the two exact small files beside a split Zstandard
  stream of `scores.pgi`. The installed bundle remains the existing three-file
  format, byte for byte.

### 2. Split one deterministic compressed stream at decimal 1 GB boundaries

- **Consideration:** Every hosted part needs comfortable headroom below 2 GiB,
  while compression ratio and reproducibility matter more than independent
  part decompression during one-time installation.
- **Options:** independent per-part frames; split by index section/contig; or
  fixed chunks of one frame.
- **Trade-offs:** Independent frames improve partial recovery but require block
  boundaries and can reduce compression. Semantic splitting couples delivery
  to private fixed-v1 internals. One frame is simplest and preserves the
  measured compression behavior, but every part is required to decode.
- **Decision:** Use one frame and exact 1,000,000,000-byte chunks, numbered from
  zero and capped at 1,000 parts.

### 3. Bind deterministic bytes to the locked encoder implementation

- **Consideration:** The Zstandard format is stable, but different encoder
  versions may legally emit different bytes for the same input and settings.
- **Options:** promise only decompressed equality; vendor a compressor forever;
  or pin and record the Rust/libzstd versions and treat upgrades as new
  transports.
- **Trade-offs:** Decompressed-only equality cannot prove reproducible release
  assets. Permanent vendoring is unnecessary operational burden. Version-bound
  determinism makes upgrades explicit while preserving installed identity.
- **Decision:** Lock settings and encoder versions, pin golden bytes, and allow
  an encoder upgrade only with a new transport identity and retained proof.

### 4. Separate transport integrity from installed semantic certification

- **Consideration:** Full fixed-v1 certification needs a materialized mmap, but
  a transport integrity check should not need another 15 GB scratch copy.
- **Options:** make every verify extract to scratch; weaken unpack to hashes;
  or stream hashes in `transport verify` and run full certification before pack
  and before unpack publication.
- **Trade-offs:** Scratch verification is costly and duplicates unpack.
  Hash-only publication could accept a self-consistent but structurally invalid
  bundle. Layered verification keeps the cheap integrity command honest and
  the publication boundary strict.
- **Decision:** `transport verify` proves byte integrity without authenticity;
  `pack` and `unpack` additionally call the existing exhaustive bundle verifier.

### 5. Keep local packaging separate from managed installation

- **Consideration:** Atomic output matters now, while XDG discovery, download
  locks, crash ownership, retries, and garbage collection are a separate user
  workflow with platform policy.
- **Options:** build the full installer now; omit safe publication; or provide
  Linux local commands with bounded streaming and atomic no-replace output.
- **Trade-offs:** A full installer hides network/platform decisions in a data
  codec ticket. Plain writes expose partial output. A local atomic boundary is
  useful immediately and composes with the future installer.
- **Decision:** Implement Linux local pack/unpack with unique sibling staging
  and no-replace publication, plus portable integrity-only verify. Do not add
  persistent locks, XDG, network, or stale-stage sweeping in this ticket.

### 6. Put reusable certification below the builder adapter

- **Consideration:** Transport pack/unpack and the existing build command need
  the same exhaustive installed-bundle proof, while the index crate should not
  learn delivery policy and assets must not depend upward on the builder.
- **Options:** duplicate verification in assets; make assets depend on build;
  move all verification into index; or put orchestration in assets over one
  strict index parser and keep build as an adapter.
- **Trade-offs:** Duplication risks different acceptance rules. A reverse
  dependency creates a cycle. Index-only verification would mix CC BY notice
  and bundle policy into the binary codec. Assets orchestration requires a
  small source move but produces a reusable typed boundary for this and future
  installation.
- **Decision:** `pangopup-assets` owns exact notice and exhaustive bundle
  certification over `pangopup-index`; `pangopup-build` depends on it and
  preserves its CLI/API behavior as a thin adapter. `pangopup-index` supplies
  the sole bounded canonical bundle-manifest parser.

## Dependencies

Tickets 001–004 are shipped on `main`. No unfinished ticket is a dependency.

## Notes

- The development agent receives only this ticket and the repository. The
  downloaded Pangolin dataset and RefSeq FASTA are external, read-only inputs
  passed only through `PANGOPUP_SOURCE_DIR` and `PANGOPUP_GRCH38_FASTA` for the
  full proof. Never commit source data, generated full bundles, transport
  parts, unpacked outputs, local absolute paths, usernames, hostnames, temp
  paths, or credentials.
- Preserve `NOTICE` and the inner bundle's CC BY 4.0 attribution exactly in
  every transport. Generated data belongs in release assets later, never Git,
  Git LFS, or test fixtures.
- Use Rust for shipped implementation. Do not add Python or shell as a runtime
  or build requirement. One-off evidence helpers, if unavoidable, use `uv` and
  are retained only when they are reproducible and path-free.
- The authoritative gate is exactly `make lint`, `make test`, and `make spec`;
  there is no `make check`. `make test` proves library/codec/resource behavior,
  `make spec` proves observable CLI behavior, and the retained full-corpus
  artifact proves scale, reproducibility, resource, and exact-identity claims.
  Do not route the multi-gigabyte full-corpus run through ordinary tests or
  specs.
- Intended infrastructure changes are the new workspace crate, its locked
  bundled Zstandard dependency pins, the certification source move and strict
  index manifest API, the new maintenance CLI subcommands, one new spec, one
  ADR, and the named documentation/artifact files. Reviewers should treat those
  as in scope; dependency drift unrelated to this transport is not. The lockfile
  and retained evidence must show exactly zstd 0.13.3, zstd-safe 7.2.4,
  zstd-sys 2.0.16+zstd.1.5.7, and libzstd 1.5.7 from bundled source.
- Numerical identities and example JSON in this ticket are requirements or
  illustrative shapes as explicitly labeled. Do not commit execution logs or
  generated evidence copied verbatim from this ticket; create the named
  path-free evidence report from the actual implementation run.

## Ticket Authorship

Author: `ticket_005_author`

Date: 2026-07-22

Self-assessment: This is the smallest coherent release-transport slice I can
justify. It keeps deterministic split packaging, integrity verification, exact
reconstruction, and full-corpus proof together because none is useful alone.
It removes the earlier draft's tar/GNU-base-256 format, scratch-only verify,
persistent locks, ownership markers, stale-stage protocol, and SIGKILL recovery;
those mechanisms either solved an avoidable archive problem or belonged to the
future managed installer. The remaining hard parts are intentionally explicit:
one shared certification boundary, strict canonical metadata, bounded frame
decoding, version-bound compression reproducibility, atomic local publication,
and a full production rebuild. The largest implementation risk is the narrow
low-level Zstandard wrapper needed to set and inspect every pinned frame fact;
it should stay private, golden-tested, and no broader than that contract.

### Revision dispositions

These are the author's dispositions of the first independent review findings,
not reviewer approval:

1. **Dependency direction:** accepted. Certification and notice ownership move
   to `pangopup-assets`; assets depends on index and build depends on assets.
   Index exposes the sole strict bounded canonical inner-manifest parser, while
   the existing build verifier remains a behavior-compatible thin adapter.
2. **Atomic publication:** accepted. Both writing commands now require unique
   same-filesystem sibling staging, complete sync, no-replace rename, parent
   fsync, ordinary cleanup, untouched conflicts, and no partial final after a
   pre-publication kill. Lock/marker/stale-stage machinery remains excluded.
3. **Provenance cross-checks:** accepted. Bundle identity, outer descriptors,
   canonical inner members, actual bytes, exact notice/CC BY attribution, and
   frame/decompressed identities are explicitly equalized. The integrity vs.
   authenticity and semantic-certification boundary is stated and tested.
4. **Bounds:** accepted. Both manifests, notice, directory enumeration, part
   count/size, fixed-v1 bytes, frame pledged size, and decompressed size+1 now
   have explicit limits and tests.
5. **Encoder identity:** accepted. Exact Rust crate requirements, bundled
   libzstd 1.5.7, system-library override rejection, frame flags/header checks,
   pledged size, and golden bytes are pinned.
6. **Platform and CLI contract:** accepted. Verify is portable; writing
   commands are Linux-only. Exact flag grammar, validation ordering, success
   fields, and a closed typed error mapping are specified.
7. **Resource evidence:** accepted. Rust allocator bounds run in isolated
   subprocesses; native libzstd memory is observed through RSS with its
   file-backed-page limitation disclosed.
8. **Production proof:** accepted. The complete source, reference, corpus,
   logical-stream, notice, and installed-member invariants are restored, while
   new builder/bundle/transport identities must come from the final diff.

## Independent Ticket Review

Reviewer: `ticket_005_new_review` (independent, read-only)

Date: 2026-07-22

Result: approved after one remediation cycle. The reviewer confirmed that all
eight findings recorded above are closed, the dependency graph is acyclic,
publication and decoding contracts are bounded and atomic, provenance and
semantic certification are explicit, the deterministic Zstandard contract is
implementable, the production proof is complete, and no installer, network,
XDG, or stale-stage recovery scope was reintroduced.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## Coordinator Final Check

Coordinator: pending
