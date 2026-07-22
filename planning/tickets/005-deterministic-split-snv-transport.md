# 005 — Deterministic split transport for the certified SNV bundle

Status: ready

## Why

Pangopup can build and verify the complete fixed-v1 lookup bundle, but it cannot
package it for release. The retained tar+Zstandard experiment is 1,935,000,209
bytes—below, but too close to, GitHub's under-2-GiB per-asset limit. Before an
installer or downloader can exist, Pangopup needs one deterministic split
transport that reconstructs the exact certified installed bundle.

This ticket packages data. It does not publish a release or change fixed-v1.

## Scope

- Add the `pangopup-assets` library crate. It owns transport manifest/framing,
  streaming pack/verify/unpack, and transport errors. `pangopup-build` is only a
  maintenance-CLI adapter. `pangopup-core` and `pangopup-index` remain free of
  release/install concerns.
- Record an accepted transport ADR and implement this exact strict JCS manifest
  shape (object key order on disk follows RFC 8785/JCS; arrays retain order):

  ```text
  schema: "pangopup.transport.v1"
  transport_format: "pangopup.ustar-base256-zstd-split.v1"
  transport_id: "sha256:" + 64 lowercase hex
  bundle:
    bundle_id: "sha256:" + 64 lowercase hex
    schema: "pangopup.bundle.v1"
    index_format: "pangopup.fixed11.v1"
    source_doi: string
    source_archive_md5: "md5:" + 32 lowercase hex
    source_members_sha256: "sha256:" + 64 lowercase hex
    reference_accession: "GCF_000001405.40"
    reference_input_sha256: "sha256:" + 64 lowercase hex
    reference_sequence_set_sha256: "sha256:" + 64 lowercase hex
    notice_path: "NOTICE"
    notice_license: "CC-BY-4.0"
    installed_members: exactly [
      {path:"NOTICE", size:safe_json_u64, sha256:sha256},
      {path:"manifest.json", size:safe_json_u64, sha256:sha256},
      {path:"scores.pgi", size:safe_json_u64, sha256:sha256}
    ]
  archive:
    format: "ustar.gnu-base256-size.v1"
    uncompressed_size: safe_json_u64
    uncompressed_sha256: sha256
    compressed_size: safe_json_u64
    compressed_sha256: sha256
  compression:
    format: "zstd.frame.v1"
    level: 9
    checksum: true
    content_size: true
    dictionary: false
    workers: 0
    encoder_crate: exact locked crate name/version
    libzstd_version: exact runtime/library version string
  parts: 1..1000 entries exactly [{ordinal:u16, path:string, size:safe_json_u64,
    sha256:sha256}]
  ```

  `transport_id` is SHA-256 of the JCS bytes of that object with the
  `transport_id` member omitted, then inserted and serialized again. All
  manifest numbers use `safe_json_u64`, a JSON integer in
  `0..=9_007_199_254_740_991` (`2^53-1`); internal checked arithmetic may use
  `u64`, checked sums must equal `archive.compressed_size`, and no manifest
  number may exceed the I-JSON/JCS interoperable range.
  Unknown/duplicate fields, non-JCS input, invalid
  prefixes/case, unsupported versions, and inconsistent values fail closed.
- Cross-check duplicated facts exactly, not only syntactically: `bundle_id` is
  the SHA-256 of installed `manifest.json`; installed member sizes/hashes match
  extracted bytes; source DOI/archive MD5/member-set hash, reference accession/
  input/sequence-set hashes, bundle/index versions, and CC-BY attribution match
  the strict inner canonical bundle manifest; `NOTICE` path/size/hash matches
  both manifests and contains the inner attribution. Hashes establish content
  identity/integrity, not publisher authenticity; signing remains excluded.
- Define the deterministic archive byte contract, intentionally replacing the
  earlier host-GNU-tar/PAX experiment. It uses POSIX ustar headers with the GNU
  base-256 extension for member size so the real 15,033,158,255-byte member is
  representable. Three members occur in exact order: `manifest.json`, `NOTICE`,
  `scores.pgi`. Each 512-byte header has ASCII name in bytes 0..100; mode as
  seven zero-padded octal digits plus NUL (`0000444\0`); uid/gid/mtime and
  devmajor/devminor as their field width minus one zero-padded octal digits plus
  NUL; checksum as six zero-padded octal digits, NUL, space after calculating
  with all eight checksum bytes set to ASCII space; typeflag `0`; magic
  `ustar\0`; version `00`; and empty linkname/uname/gname/prefix.
  Member size uses eleven zero-padded octal digits plus NUL when <=
  `0o77777777777`; larger positive values are encoded as a zero-padded unsigned
  96-bit big-endian 12-byte field and then bit 7 of byte zero is set as the GNU
  base-256 flag. Reject a value whose encoding would already set that flag/value
  bit (the representable positive range is therefore below `2^95`), negative or
  otherwise noncanonical encodings, and base-256 in every other numeric field.
  Every header byte not assigned above is NUL.
  File data is padded with zeros to 512 bytes. Exactly two all-zero blocks end
  the archive; there are no pax/GNU metadata members or trailing bytes.
- Compress exactly one archive stream as exactly one standard Zstandard frame
  using the locked Rust `zstd`/`zstd-safe` implementation and recorded libzstd
  version: level 9, checksum flag on, pledged/content size on, dictionary ID and
  dictionary absent, `nbWorkers=0` (single-threaded), long-distance matching
  off, and otherwise that locked library version's defaults. Reproducible bytes
  are guaranteed for the locked Cargo implementation/libzstd version, not
  promised across encoder upgrades. An upgrade changes transport identity and
  requires new retained evidence but never changes installed bundle identity.
  Pin small golden octal-size and >8-GiB synthetic-header bytes plus a golden
  compressed fixture so settings/extension/version drift is observable.
- Split the compressed bytes at exactly 1,000,000,000-byte boundaries. Parts
  are zero-based and named exactly `payload.tar.zst.partNNNN` (`0000`..`0999`);
  all but the final part are exactly the ceiling, the final is 1..ceiling bytes,
  and no part is an independent frame. The transport manifest is named
  `transport.json` and part paths are basenames only in exact ordinal order.
- Add exact commands:

  ```text
  pangopup-build transport pack --bundle <BUNDLE> --output <ABSENT_DIR>
  pangopup-build transport verify --manifest <TRANSPORT_JSON>
    --scratch-dir <EXISTING_WRITABLE_DIR>
  pangopup-build transport unpack --manifest <TRANSPORT_JSON>
    --output <ABSENT_DIR>
  ```

  CLI usage errors exit 2 with code `CLI_USAGE`. Runtime errors exit 1 with one
  of `INPUT_IO`, `OUTPUT_IO`, `MANIFEST_INVALID`, `TRANSPORT_INCOMPATIBLE`,
  `PART_SET_INVALID`, `TRANSPORT_HASH_MISMATCH`, `ARCHIVE_INVALID`,
  `BUNDLE_INVALID`, `SCRATCH_CAPACITY`, `OUTPUT_CONFLICT`, or
  `UNSUPPORTED_PLATFORM`. Every failure
  writes no stdout and exactly one compact JSON line to stderr:
  `{"status":"error","code":string,"message":string,"details":null}`.
  Success writes one compact LF-terminated stdout object, empty stderr:
  - pack: `status=packed`, transport_id, bundle_id, manifest absolute path,
    part_count, compressed_bytes;
  - verify: `status=verified`, transport_id, bundle_id, part_count;
  - unpack: `status=unpacked`, transport_id, bundle_id, output absolute path.
  Exact fields/key order are pinned in specs; human error message text is not.
- Command publication/durability is supported on Linux only. Non-Linux command
  execution returns `UNSUPPORTED_PLATFORM` (exit 1); the pure codec may still be
  unit-tested elsewhere. Linux uses trusted opened parent dirfds, no-follow
  relative operations, `renameat2(RENAME_NOREPLACE)`, and file/directory fsync.
  No cross-platform atomic-publication promise is implied.
- Every output parent and verify scratch base is an absolute opened real
  directory owned by euid and not group/world writable, or a command-created
  child mode `0700` beneath such a directory. Hold its dirfd; perform all
  descendant operations relative with no-follow/beneath resolution; reject
  device changes and symlink/non-directory/untrusted components. Lock files are
  regular mode `0600`, staging wrappers/directories mode `0700`, and ownership
  markers regular mode `0400`.
- Before reconciling staging, `pack`/`unpack` take a no-follow regular sibling
  lock file derived from the canonical output basename; `verify` takes a
  no-follow lock derived from transport ID in the scratch base. Locks use
  nonblocking `flock` and fail immediately with `OUTPUT_CONFLICT` when held.
  Kernel ownership, not marker contents, protects live staging from cleanup;
  stale lock files are harmless and may be reused only after lock acquisition.
  Different outputs/transports do not serialize.
- `pack` requires an absolute absent output and a trusted existing writable
  parent. It first exhaustively verifies the bundle, then creates a uniquely
  named sibling wrapper `.OUTPUT.pangopup-pack.<32-hex-nonce>/`. The wrapper
  contains only mode-0400 `marker.json` (binding schema, euid, canonical output,
  bundle ID, nonce) and mode-0700 `payload/`. Archive/compression/part hashing
  streams into `payload/`; `transport.json` is written last. Transport files
  become mode `0444` and payload mode `0555`. After syncing every file and
  wrapper/payload/parent directory, publish only `payload/` by no-replace rename
  to output; fsync both source and destination parent dirfds after rename;
  unlink the exact still-open/no-follow-validated `marker.json`; `rmdir` the
  now-empty wrapper; then fsync its parent again. Thus no marker is published.
  It never creates a second uncompressed tar or buffers scores/compressed output.
- `verify` requires the manifest parent and scratch base to be absolute,
  existing, writable, trusted directories. Before work, `statvfs` must report
  at least the checked sum of installed member sizes plus 64 MiB available;
  preflight failure is `SCRATCH_CAPACITY`, while later `ENOSPC` is still handled
  and cleaned. It creates one unique owned child, stages exactly one extracted
  three-file bundle there, runs exhaustive bundle verification, and deletes the
  child on normal success/failure. This unavoidable 15 GB extracted bundle is
  distinct from—and the only allowed uncompressed temporary artifact.
  `unpack` requires an absolute absent output/trusted parent and uses sibling
  wrapper `.OUTPUT.pangopup-unpack.<nonce>/{marker.json,payload/}` with the same
  marker fields plus transport ID. It extracts the exact three bundle members
  only under `payload/`, exhaustively verifies there, sets member modes `0444`
  and payload `0555`, fsyncs, publishes only `payload/` with no-replace rename,
  fsyncs source and destination parent dirfds, unlinks the exact validated
  marker, removes the now-empty wrapper, and fsyncs the wrapper parent again.
- Manifest-directory collection is bounded to 1..1000 declared parts. Reject
  duplicate ordinals/names, gaps, bad grammar, zero/oversized parts, checked-sum
  overflow or aggregate mismatch, any nonfinal part not exactly 1,000,000,000
  bytes, ordinal/path mismatch, missing parts, and undeclared files matching
  `payload.tar.zst.part[0-9]{4}`; unrelated other files are ignored. Resolve
  basenames only beside the manifest. Open manifest/parts as regular
  non-symlink handles, validate with `fstat`, and stream those same handles to
  avoid check/use races. Reject links/special files, reordered/truncated/
  trailing compressed data, multiple frames, archive traversal/links/extra or
  duplicate members, size expansion beyond exact declared sizes, and all inner
  mismatches.
- `verify` names scratch children
  `pangopup-verify.<transport-id-hex>.<32-hex-nonce>` and writes a strict marker
  at wrapper `marker.json`, binding schema/euid/scratch-base dev+ino/transport
  ID/nonce, then extracts only into wrapper `payload/`. Exhaustive verification
  targets `payload/`; marker/wrapper never enter the verified member set.
  It never sweeps arbitrary scratch. At the next verification of the same
  transport under the same base, it may remove only direct real directories
  whose no-follow marker exactly matches name, euid, base dev+ino, and transport
  ID; any malformed lookalike is left untouched and reported `OUTPUT_CONFLICT`.
- Ordinary failures remove only the invocation's positively identified staging.
  SIGKILL may leave marked unpublished staging, including one with fully written
  output; the next pack/unpack for the exact same canonical output and bundle/
  transport, or verify for the same transport/scratch base, applies the same
  exact marker rule and deletes it before restarting. Never traverse a symlink,
  cross a filesystem, clean a different output/transport, or mutate a final
  destination. Any prefixed pack/unpack/verify lookalike with a missing or
  malformed marker is left untouched and returns `OUTPUT_CONFLICT`, with one
  crash-safe exception while holding the same lock: an exact expected-prefix,
  real, same-device, euid-owned wrapper with no marker may be removed only when
  no-follow dirfd enumeration proves it completely empty. Missing/malformed
  marker plus any entry remains conflict.
  SIGKILL after payload publication but before wrapper cleanup is safe: the next
  operation under the same lock recognizes the exact marker even when final
  output exists, fsyncs the published parent if needed, unlinks the marker,
  removes the wrapper, fsyncs the staging parent, and then reports the existing
  final according to the command contract. Verify, which never publishes its
  payload, removes the entire positively identified wrapper.
- Add small production-writer transport fixtures and byte-exact specs for
  repeated pack, verify, unpack, outputs/errors, golden archive/frame bytes,
  read-only inputs, every part/manifest/archive/member corruption above,
  capacity/permission/conflict, injected failures, SIGKILL recovery, and exact
  installed reproduction. Failpoints cover post-rename/pre-parent-fsync and
  post-publication marker-unlink/wrapper-rmdir/final-fsync boundaries, including
  SIGKILL after marker unlink/before rmdir and removal of only the proven-empty
  markerless wrapper. A valid
  64-MiB transport (one final part) proves the
  real streaming path. Separately unit-test the bounded ordered-part iterator
  with 1000 small synthetic regular handles below manifest/canonical-splitting
  validation; do not call it a valid transport. A tracking allocator and FD
  counter enforce peak allocator delta <=16 MiB and open-FD delta <=8
  independent of aggregate bytes/handle count.
- Rebuild from explicit `PANGOPUP_SOURCE_DIR` and `PANGOPUP_GRCH38_FASTA`; do
  not retain paths. The publisher-declared ZIP MD5
  `679ef0b50e511b6102b4b88fbf811108` is embedded provenance, not a hash of the
  extracted directory. The actual supplied extracted-member identity must be
  observed-member SHA-256
  `0e40ee8e0527210cb64c26a6637117aea7d41d696e7bd95f3bb9545ee16782f6`,
  reference input SHA-256
  `11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3`,
  and sequence-set SHA-256
  `2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4`.
  Require 19,913 genes; 4,099,255,665 rows; 1,366,418,555 gene loci;
  10,073/9,840 ascending/descending members; 19,916/19,945 source/index
  segments; 3 gaps; 50,002 omitted bases; 30 REF=N loci (9 omit-A/21 omit-T);
  logical source/decoded SHA-256
  `dcec29e84c5f65bd76ffde2be8c7fa312d08e6abdb1e45e024dc0fe8c8da9c31`;
  and `scores.pgi` size 15,033,158,255/hash
  `6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27`.
  Ticket source changes will alter builder-source digest and bundle ID; record
  the new identities rather than expecting an old bundle ID.
- Independently verify the rebuilt bundle; pack it twice to distinct outputs;
  require identical manifests/parts; run full `transport verify`; unpack once;
  independently verify the unpacked bundle and prove all three members equal.
  Delete all full generated bundles, scratch, transports, and unpacked copies.
- Retain `planning/artifacts/005-snv-transport.md` with identities, commands,
  locked encoder versions/settings, names/sizes/hashes, total size/ratio,
  reproducibility, wall time, faults, peak RSS and its file-backed caveat, and
  allocator evidence. Measure apparent-byte high-water separately for (a) pack
  output+owned staging excluding input bundle, (b) verify owned scratch
  excluding transport input, and (c) unpack output+owned staging excluding
  transport input; disclose sampler interval/lower-bound limits.
- Update README, delivery/index architecture, ADR index, FAQ, and frontier.
  Keep publication, managed install, network download, and auto-start future.
- Excluded: fixed-v1 changes, GitHub publication, network, XDG/install locks,
  automatic startup, model/reference/mask/executable assets, signing, and HTTP.

## Success Checklist

- The exact closed manifest, ustar/base-256, Zstandard, and part contracts are pinned by
  golden fixtures; two full packs are byte-identical and every part is <=1 GB.
- Verify/unpack stream with bounded heap/FDs, fail closed at every layer, use the
  declared scratch/publication lifecycle, and never expose a partial bundle.
- Unpacked members are byte-identical and pass exhaustive verification.
- Full-corpus evidence reproduces every pinned semantic/data identity, runs
  transport verify, records resource/disk boundaries, and leaves no full asset.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. Split one compressed stream, never fixed-v1, preserving installed bytes.
2. Use a decimal 1 GB part ceiling for clear release headroom and <=1000 parts.
3. Replace host PAX tar with byte-pinned ustar/GNU-base-256 size headers plus one
   locked Zstandard frame; pure ustar cannot represent the 15 GB score member.
4. Verify part, whole compressed stream, archive, installed members, duplicated
   provenance, and full bundle semantics; each layer covers a different fault.
5. Materialize one verified bundle in explicit scratch because exhaustive
   fixed-v1 verification needs random access; forbid an additional raw tar.
6. Keep network and authenticity/signing outside this local integrity slice.

## Dependencies

None. Tickets 001–004 are shipped on `main`.

## Notes

- Operator supplies full input paths only through `PANGOPUP_SOURCE_DIR` and
  `PANGOPUP_GRCH38_FASTA`; committed evidence records identities, never paths.
- This dependency-free draft may become `ready` only after independent review.

## Independent Ticket Review

Reviewer: `next_packet_review` (independent, read-only)

Initial result: changes required. The reviewer agreed with the two-ticket
dependency chain but required a closed transport schema and whole-stream digest;
exact archive/compressor bytes; explicit scratch/materialization; bounded part
collection; exact CLI/errors; transport-vs-bundle identity separation; Linux
publication/locking/symlink boundaries; full-corpus identities; deterministic
interruption recovery; and removal of local paths and premature cache/network
scope. The coordinator accepted every finding and rewrote both drafts.

Subsequent re-reviews caught and resolved feasibility and durability defects:
pure ustar could not represent the 15 GB member, so v1 now pins GNU base-256
size encoding; JCS numbers now use the `2^53-1` safe range; staging markers live
outside published payloads; publication is Linux-qualified and fsyncs after
rename/cleanup; part sizing is canonical; pack/verify/unpack use exact locks,
trusted dirfds, wrappers, scratch, and SIGKILL recovery; and a markerless wrapper
is removable only under the same lock when exact-prefix, same-device, euid-owned,
and proven completely empty through a no-follow dirfd.

Final result: approved with no remaining findings. The reviewer confirmed this
is the sole dependency-free next slice and is self-contained and ready.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending
