# ADR 0007: Deterministic SNV transport

Status: accepted

## Decision

Pangopup transports a certified SNV bundle without introducing a general
archive. `bundle-manifest.json` and `NOTICE` are exact installed-member copies.
Only `scores.pgi` is encoded as one checksummed, content-sized Zstandard frame
and divided at exact 1,000,000,000-byte boundaries. The installed bundle and
fixed-v1 lookup bytes never change.

Compression is byte-deterministic only for the pinned Rust codec and bundled
libzstd 1.5.7 settings. An encoder upgrade produces a new transport identity;
it does not change the bundle identity. Canonical RFC 8785 metadata binds the
inner bundle, copied members, complete compressed stream, and every numbered
part.

Verification has two deliberately different layers. `transport verify` streams
and checks the closed manifest, exact member set, hashes, one frame, pledged
size, checksum, no dictionary or trailing frame, and exact decompressed member
identity. It proves integrity, not publisher authenticity or fixed-v1 semantic
validity. Pack certifies its input, and unpack exhaustively certifies the
reconstructed bundle before durable Linux no-replace publication.

## Consequences

- Runtime lookup remains mmap-only; request handling never decompresses data.
- Pack, verify, and unpack use bounded buffers and at most one part handle.
- A changed or missing part fails closed, and unpack never exposes partial
  final output.
- The transport is release-sized but is not yet an installer or download
  protocol.

## Excluded

Network access, XDG discovery, persistent install locks, stale-stage recovery,
publisher signing, GitHub publication, model/reference/mask assets, and service
lifecycle are separate decisions.
