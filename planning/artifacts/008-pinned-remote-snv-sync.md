# Ticket 008 — pinned remote SNV sync evidence

Date: 2026-07-23

## Implemented boundary

`pangopup assets sync` is the only shipped network entry point. Production
validates the compiled canonical `snv-grch38-v1` release contract and uses its
five literal URLs; it does not fetch a profile, query GitHub's API, select
`latest`, accept a user URL, or read ambient GitHub credentials. Exact active
reuse completes before an HTTP client or cache filesystem is created.

The disposable cache namespace is keyed by the complete profile SHA-256,
`sha256:63f3842ea6cb40ebc0a2b6ca23fba4f35d53f829d96c33f597a2c5bcac238ca6`
over the checked 2,821-byte profile. One nonblocking cache lock protects
private partial, completed-member, and atomically published transport state.
Only the closed published five-member `transport` directory reaches the
existing installer. The installer remains the integrity, decompression, and
semantic publication boundary.

The cache root is opened component by component without following symlinks.
Private descendants and members remain behind held directory descriptors;
creation, open, promotion, eviction, and durability operations are relative to
those descriptors. New directories are created as mode 0700 without a wider
permission window. Final-root, intermediate-root, and member-symlink tests fail
closed without changing the symlink target.

Root/profile and lock-file creation are race-idempotent. A sync acquires the
profile lock before creating or inspecting `partial`, `members`, or
`transport`. A real two-thread test starts from an absent cache, holds the owner
inside its first response, and proves the competing sync returns
`ASSET_LOCKED` without making a request; the owner then installs successfully.
Directory validation and eviction consume `RawDir` entries one at a time. A
2,048-entry hostile transport is rejected and completely removed without an
entry-name vector.

## HTTP and resource contract

- HTTP client: `ureq` 3.2.0 with automatic redirects disabled, ambient proxy
  disabled, and only the `rustls` feature.
- TLS stack locked by `Cargo.lock`: `rustls` 0.23.42, `rustls-webpki` 0.103.13,
  and `webpki-roots` 1.0.9.
- Production connect, response-header, and body read-idle bounds are 30, 30,
  and 120 seconds. There is no whole-transfer deadline.
- Production permits HTTPS only, default HTTPS port only, and the exact
  `github.com` and `release-assets.githubusercontent.com` hosts. Redirects are
  explicit and capped at five. Range and If-Range survive every allowed hop.
- `Accept-Encoding: identity` is explicit; any non-identity response encoding
  fails.
- Members download sequentially through one 131,072-byte stack buffer. The
  implementation never allocates an asset-sized body. A fresh stream is hashed
  as it is written; a resumed prefix is read and hashed exactly once before its
  suffix is appended.

## Miniature exactness and transfer evidence

The miniature transport members used by the library tests are 1,099, 3,370,
1,709, and 1,770 bytes, totaling 7,948 bytes. The fresh path made four requests
in exact profile order and reported `downloaded_bytes=7948` and
`resumed_bytes=0`. The real in-process TCP server test streamed that transport
through the actual ureq body reader, installed it through `install_transport`,
and reopened the active bundle.

The interrupted/resume case retained a 549-byte prefix of the 1,099-byte first
member, sent exact `Range` and `If-Range`, accepted only the matching 550-byte
`206` suffix, and reported 549 resumed bytes. The redirect/restart case
propagated range headers on both requests, accepted a strong-ETag full `200`
into a distinct fresh file, reported 1,099 downloaded and zero resumed bytes,
and removed the old prefix only after promotion. Active reuse and complete-cache
offline installation made zero requests and reported both counters as zero.

Tests cover missing/weak/changed validators, malformed range responses, 416,
short and long bodies, wrong digest and declared length, non-identity encoding,
redirect host/scheme/port/limit failures, path precedence, symlink/nonregular
cache shapes, oversized/noncanonical resume state, concurrent locking, and
published-cache eviction after installer integrity failure. Injected crash
tests interrupt sidecar create/sync, partial create/write/sync, member
rename/directory sync, and final transport rename/directory sync; every state
recovers without publishing a partial transport or active bundle.

The declared-length control is deliberately split by state: a wrong fresh
`Content-Length` creates neither a completed member nor a retained partial; a
wrong resumed suffix length leaves the existing prefix byte-for-byte unchanged
and creates no completed member. A timeout injected through `sync_with` while
it owns the profile lock publishes neither cache transport nor active bundle;
the same cache lock is immediately reacquired nonblocking and a subsequent
retry installs successfully. Path preservation uses a second fully valid,
nonmatching miniature active bundle. Both a broken cache root and a complete
cache followed by a hostile install-lock path in that same data root return an
error while the original active receipt and bundle reopen unchanged.

Fresh-body and resumed-suffix wrappers count exactly the declared network
bytes; a separate prefix audit counts exactly the retained partial length. The
cleanup-failure control proves an installer cache-content error keeps its
original typed error kind while reporting sanitized eviction context and
leaving the unremoved cache visible.

The real loopback transport test proves response-header and body-stall timeout
classification. Connect-timeout classification is deterministic through the
injected ureq timeout value, so normal tests never rely on an unroutable
external address.

## Miniature provenance regeneration

Ticket 008 changes `pangopup-assets` and `Cargo.lock`, both inputs to the
builder source identity embedded in the checked regression fixture. The fixture
was regenerated after source stabilization with its documented Rust tool.
The final bundle ID is
`sha256:9c03f5c7397f2b6203d23a2bc373ddfc345aac1008fe3f77e6cdc0d2715993bf`
and builder source identity is
`sha256:28d863ac7997ac15cc1ff7e2943d8a2148678a6420c2589701ab19d40dd42e56`.

The old and new `requests.tsv`, `scores.pgi`, and `NOTICE` were byte-identical.
Every aggregate and gene-filtered expected JSONL file was byte-identical after
normalizing only its derived `bundle_id`. The stable payload hashes are:

```text
requests.tsv: sha256:042fcc0e550f7dfccad742a6a2e6a89b0c4e245673b0222bcefb7d42b1ffe52d
scores.pgi:   sha256:fb0a77425456bd39e6aab7ad3447a24757f6889e82f7b27df01c214b78f8a6b9
```

## Release-mode measurements

These are miniature local measurements, not production network claims.

```text
release test process:
  miniature active-reuse call: 375,487 ns
  whole fixture pack + fresh install + offline install + active reuse test: 0.21 s
  maximum resident set: 5,760 KiB
  major page faults: 0

release CLI --help startup:
  elapsed reported below /usr/bin/time's 0.01 s display resolution
  maximum resident set: 2,484 KiB
  major page faults: 0
```

Commands:

```text
cargo test --release -p pangopup-assets --lib \
  sync::tests::fresh_sync_installs_then_active_reuse_is_zero_network --no-run
/usr/bin/time -v target/release/deps/pangopup_assets-1d4f2acbdcc0131d \
  --exact sync::tests::fresh_sync_installs_then_active_reuse_is_zero_network --nocapture
cargo build --release --locked -p pangopup-cli
/usr/bin/time -v target/release/pangopup --help
```

No command in implementation, review, measurement, or the normal gates read,
hashed, rebuilt, verified, or downloaded a production payload member. No test
contacted GitHub.

Final local gates on this diff:

```text
make lint  -> pass
make test  -> pass
make spec  -> 111 passed
```
