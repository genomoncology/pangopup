# 045 — Resilient asset sync with live progress

Status: ready

## Why

The first independent Apple Silicon container run downloaded 348,683,581 bytes,
then failed after 31.88 seconds with `ASSET_TIMEOUT`; a second invocation resumed
those exact bytes and completed. The retained resume, range, ETag, checksum, and
atomic-install behavior worked, but the user had to restart the command and saw
no useful feedback during the long successful retry.

This is not only ordinary network variability. The pinned `ureq 3.2.0` client is
configured with a 30-second response-header timeout and a 120-second body
timeout. Its timing implementation continues considering the completed
response-header timer while awaiting body input, so the 30-second timer can cap
an otherwise active multi-gigabyte body. The next outcome must remove that
false cutoff, retry genuinely transient failures automatically, and expose
bounded byte/phase feedback without weakening the existing trust boundary or
breaking noninteractive JSON behavior.

## Scope

- Replace only `pangopup-assets`' production HTTP adapter with exactly pinned
  `reqwest 0.12.22`, default features disabled, using `blocking` and
  `rustls-tls-webpki-roots`. Its blocking `.timeout(120 seconds)` applies
  independently to response acquisition and each body `Read`, so successful
  reads reset the body guard and there is no whole-body deadline. Also set the
  connect timeout to 30 seconds, disable proxy discovery and transparent
  response decompression, and disable automatic redirects. Accept that a
  connected server may take at most 120 seconds to return headers. Keep the
  existing manual five-hop HTTPS and host allowlist.
- Replace the current `ureq::http::Uri` re-export with an exact direct
  `http = 1.4.2` dependency and keep `validate_url`'s accepted/rejected matrix,
  messages, userinfo/fragment/port rules, and host/scheme comparisons
  byte-compatible. This is dependency ownership cleanup forced by removing the
  production `ureq` adapter, not a URL-policy change.
- Preserve the private transport-client seam so scaled local servers and
  scripted responses prove behavior without production downloads.
- Give each transport member four total attempts. Wait 1, 2, then 4 seconds
  before attempts two through four. Retry only connection/request transport
  failures, per-read timeouts, premature EOF, and HTTP 408, 429, 500, 502, 503,
  and 504. Re-enter the existing member acquisition path so an accepted strong
  ETag and exact range resume the retained prefix.
- Introduce a private `AttemptFailure` containing the public `AssetError`, a
  typed optional public `SyncRetryReason`, and bytes received by that failed
  attempt. Transport execution and body streaming create this type directly;
  retry selection must never inspect error-message strings. Convert back to
  the unchanged public error only after a fatal failure or the fourth exhausted
  attempt.
- Do not retry invalid URL/redirect/host/scheme state, changed or weak ETags,
  malformed or inconsistent range/length headers, unexpected content encoding,
  checksum mismatch, cache/profile corruption, disk errors, or installation
  failures. Exhaustion returns the existing public error family and preserves
  the safe partial state for a later invocation.
- Add the following public, typed, synchronous observation boundary for
  combined synchronization. The existing public no-observer entry points
  remain and delegate to a no-op observer:

  ```text
  SyncComponent = Snv | Runtime
  SyncPhase = Checking | Downloading | Installing | Reused | Ready
  SyncTransferMode = Cached | Fresh | Resume | Restart
  SyncRetryReason = Timeout | Connect | Request | BodyRead | PrematureEof | HttpStatus(u16)
  SyncEvent =
    Phase { component, phase }
    Transfer { component, asset_name, attempt, max_attempts, mode,
               member_bytes, member_total, invocation_downloaded_bytes,
               invocation_resumed_bytes }
    Retry { component, asset_name, failed_attempt, next_attempt, max_attempts,
            reason, delay_seconds }
    Complete { downloaded_bytes, resumed_bytes }
  ```

  Attempt zero is reserved for a `Cached` member that required no request;
  network attempts are one through four. Events never contain URLs or local
  paths. `Cached` means an already-complete member and no request; `Fresh`
  means no retained prefix and a 200 response; `Resume` means a retained prefix
  and accepted 206 response; `Restart` means a retained prefix but a 200
  response is written to the separate safe fresh file. Reqwest failures map in
  strict order: timeout first, connect second, request/response acquisition
  third. A non-timeout error while reading an obtained body is `BodyRead`.
- Extend only `pangopup sync` with `--progress` and `--quiet`. Progress is
  enabled automatically when stderr is a terminal, `--progress` forces it for
  Docker or captured logs, and `--quiet` suppresses it. Duplicate flags and the
  combination are `CLI_USAGE` before path or network work.
- Render human progress to stderr at phase/retry/member boundaries and whenever
  another 64 MiB of a member has arrived. Keep one bounded writer and no rate or
  completion-time estimate. Successful final JSON remains the sole stdout
  record. With progress inactive, existing exact success and error streams stay
  byte-compatible; with progress active, the final typed error follows the
  human progress lines on stderr.
- Freeze the line grammar as follows, using lowercase component/mode/reason
  names and decimal byte counts:

  ```text
  sync: checking <component> assets
  sync: downloading <component> assets
  sync: <component> <asset> <mode> attempt <N>/4 <BYTES>/<TOTAL> bytes (<DOWNLOADED> downloaded, <RESUMED> resumed)
  sync: <component> <asset> retry <NEXT>/4 after <reason>
  sync: installing <component> assets
  sync: reusing installed <component> assets
  sync: <component> assets ready
  sync: ready (<DOWNLOADED> downloaded, <RESUMED> resumed)
  ```

  Retry-reason spellings are `timeout`, `connect`, `request`, `body-read`,
  `premature-eof`, and `http-status-503`. A component emits
  `Checking`, then either `Reused` or `Downloading`; a download emits ordered
  member transfers, then `Installing` and `Ready`. After SNV then runtime, one
  global `Complete` renders the final ready line. Offline failure emits only
  the component `Checking` phase before the existing error.
- Keep downloads sequential and the 128-KiB transfer buffer. Do not add
  parallelism, persistent progress files, `status` byte reporting, repair/GC,
  configurable retry/timeout policy, JSON progress, or any public asset/release
  mutation.
- Define accounting once for the whole invocation. `resumed_bytes` counts only
  a valid prefix that existed when the invocation began and was incorporated
  into the final verified member. Bytes first received by a failed attempt in
  this invocation are always `downloaded_bytes` when a later attempt resumes
  them. `downloaded_bytes` counts every body byte received during this
  invocation, including duplicate bytes from failed attempts or a server that
  ignores a range and forces a fresh replacement; it may therefore exceed the
  transport size. A transfer event's `member_bytes` is the monotonic high-water
  progress toward one complete member: the maximum of the retained valid
  prefix and accepted bytes in the current fresh replacement, capped at
  `member_total`. Invocation counters are committed and never provisional:
  `invocation_resumed_bytes` remains unchanged while a resumed member is in
  flight and increases by that member's invocation-start prefix only after the
  member verifies and that prefix is incorporated. If a fresh replacement
  wins, resumed credit never increases. `invocation_downloaded_bytes` may
  increase as reads succeed, including reads from attempts that later fail.
  `Complete` and the existing final JSON use those same committed counters.
  All counters never decrease and use checked arithmetic.
- Update `README.md`, `architecture/delivery.md`, `spec/cli.md`,
  `spec/remote-assets.md`, and `planning/frontier.md`. Docker first-sync examples
  must force progress; documentation must continue distinguishing durable data
  from disposable resumable cache.

## Success Checklist

- A scaled local streaming response remains active longer than the former
  response timer while making progress more frequently than the per-read
  timeout, and completes with exact bytes. Retain the current `ureq 3.2.0`
  adapter under `cfg(test)` with `ureq` as a dev-dependency. Run the identical
  slow streaming fixture against scaled production ratios: the legacy client
  has a 25-ms response-header and 100-ms body setting, while the replacement
  has a 25-ms blocking-operation setting; chunks arrive less than 25 ms apart
  but the whole body exceeds 25 ms. The legacy control must time out and the
  replacement must succeed, proving timer carry-forward rather than a merely
  fast response.
- A server that stalls longer than the per-read timeout produces
  `ASSET_TIMEOUT`; transient timeout, premature EOF, and selected HTTP-status
  cases retry at most four total attempts. A dropped first response is resumed
  with the exact `Range` and `If-Range` and publishes byte-identical content.
- Non-retryable status, redirect, ETag, range, encoding, checksum, local I/O,
  and install failures make one attempt, remain typed/redacted, expose no
  partial installation, and retain or discard cache state according to the
  existing rules.
- Retry byte accounting counts bytes already present at invocation separately
  from bytes received by this invocation and remains checked for overflow.
  Progress toward a member is monotonic even when a server ignores a range and
  the safe fresh-download path is used.
- Observer tests pin exact event ordering and fields for fresh, resumed,
  already-complete, retry-success, retry-exhaustion, offline, SNV-install, and
  runtime-install paths. Callback overhead does not allocate per 128-KiB body
  chunk.
- CLI tests pin auto/forced/quiet selection, mutual exclusion, representative
  exact progress lines, the final error after active progress, and unchanged
  non-progress stdout/stderr. Root and focused sync help include the two flags;
  unrelated help and operational error contracts remain unchanged.
- Normal tests use local/scaled fixtures and never download the production
  multi-gigabyte assets. The transfer path still uses a 128-KiB buffer and never
  materializes a response body.
- Record parent/candidate stripped release executable and Docker image sizes in
  implementation evidence on native `linux/amd64`. The exact parent is
  `2a6d393`; create a detached temporary worktree for it and build both trees
  with their checked-in digest-pinned Dockerfiles, `--pull`, version `0.1.0`,
  and their exact revision SHA as build arguments. Require image architecture
  `amd64`; extract `/usr/local/bin/pangopup` from each image and record raw
  `stat` bytes, then record raw `docker image inspect --format '{{.Size}}'`
  bytes. The candidate image may grow by no more than 10,485,760 bytes; larger
  growth rejects the dependency shape rather than being waived. Remove the
  temporary containers, images, and worktree after recording evidence.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Replace the adapter rather than lengthen the total cutoff.** Raising the
   effective total duration still fails sufficiently slow healthy downloads;
   removing all timeouts can hang indefinitely. Reqwest's blocking timeout is
   applied separately to response acquisition and every body `Read`, so the
   narrow adapter gains a progress-reset body guard while all admission and
   download logic remains owned here. The deliberate trade-off is raising the
   connected response-header wait from 30 to 120 seconds.
2. **Own retries above the HTTP library.** Hidden client retries cannot report
   attempts or reliably re-enter Pangopup's ETag/range/cache rules. The adapter
   performs one request; the member loop owns four visible, deterministic
   attempts and injectable sleeping for fast tests.
3. **Terminal-auto progress with explicit override.** Always-on stderr would
   break existing automation, while explicit-only feedback repeats the poor
   first-use experience. Terminal auto plus `--progress`/`--quiet` keeps the
   default noninteractive contract and makes Docker logs intentional.
4. **Plain bounded feedback, not a second protocol.** Human phase lines and
   64-MiB byte crossings are deterministic and useful without clocks, rates,
   ANSI control sequences, or an unstable JSON event API. Typed events remain
   the library/CLI seam; only the final sync result is machine-readable output.
5. **Sequential transfer remains the resource contract.** This is a resilience
   and observability ticket. Parallel fetching would independently alter
   memory, disk pressure, rate-limit behavior, and retry ordering and requires
   separate evidence.

## Dependencies

- Ticket 044 focused runtime help: complete.
- Immutable `snv-grch38-v1` and `runtime-grch38-v1` profiles: complete and
  unchanged.

## Notes

- The authoritative external reproducer is the Apple Silicon report retained
  outside this Linux checkout; its relevant observed facts are copied into
  `Why`, so development and review do not depend on that machine.
- `reqwest::blocking::Response` implements `Read`; keep the current bounded
  streaming/hash/write path. Disable reqwest redirects and proxy discovery so
  the existing explicit security policy remains authoritative.
- Exact delays belong behind the injected test seam. Normal tests must not
  sleep for production durations.
- This ticket has no public or irreversible external effect.

## Coordinator Authorship

Coordinator: Codex

Drafted from the accepted next-frontier plan, the Apple Silicon evidence, the
current `ureq` timing source, and the shipped synchronization implementation.
The coordinator does not implement product code or approve this ticket.

## Independent Ticket Review

Reviewer: Kepler

Initial verdict: REJECT. The reviewer found that `read_timeout` is not exposed
by reqwest's blocking builder; retry classification and cross-attempt byte
accounting were ambiguous; the observer and human grammar were not frozen; the
legacy control was underdefined; the size gate lacked a reproducible procedure;
and removal of the `ureq::http::Uri` re-export was omitted.

Coordinator disposition: corrected the adapter to the blocking per-operation
timeout and explicitly accepted its 120-second header wait; added typed attempt
failures with no string classification; froze invocation accounting, the event
schema, line grammar and order; retained the exact legacy adapter as a scaled
test control; froze the native parent/candidate size procedure; and preserved
URL validation through a direct exact `http` dependency. Returned to the same
reviewer.

Second verdict: REJECT. The reviewer found that provisional resumed credit
could contradict the monotonic committed counter and that generic request
failure lacked an event reason; transfer modes also needed literal meanings.

Coordinator disposition: resumed credit now commits only after a verified
range-resumed member and remains zero if a safe fresh replacement wins;
downloaded credit advances on every accepted network read. Added `Request`,
froze timeout/connect/request classification order, and defined all four modes.
Returned to the same reviewer.

Final verdict: ACCEPT. The reviewer confirmed that the timeout/dependency
policy is feasible, retry classification and transfer modes are closed, byte
accounting remains monotonic through safe restarts, observable progress and
streams are exact, the old-client control proves the diagnosed defect, and the
resource/size procedures and exclusions are decision-complete.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
