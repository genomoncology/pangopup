# 023 — Persist exact model results in SQLite

Status: complete

## Why

Supported lookup misses and non-SNVs take seconds to score with the selected
CPU model. Repeating the same request currently repeats all reference, mask,
and model work. The user selected a persistent SQLite cache because a
millisecond-scale disk lookup is negligible beside inference and remains useful
across process restarts.

This is the next unblocked outcome. It must preserve lookup-first behavior and
prove the real cache rather than add an in-memory result cache or reopen the
already-closed model graph decision.

## Scope

- Add a small `pangopup-cache` runtime crate owning one versioned SQLite model
  result database. `pangopup-engine` continues to own scoring and filtering;
  it exposes only the smallest typed unfiltered modeled-result seam needed for
  the cache. Core, index, and model crates remain cache-policy free.
- Split cache admission from expensive component opening. A model-required
  request first reads bounded canonical model/reference manifests and the
  bounded mask identity needed for its key, then checks SQLite. A validated hit
  must not construct `ModelKernel`, create an ONNX session, run its
  initialization probe, open/hash the dense reference member, or execute
  inference. A miss opens and authenticates each full component exactly once
  and proves the held components match the admitted key before scoring.
  Explicit loose paths retain their documented same-user immutable/trusted
  development-input boundary until coherent installed profiles replace them.
- Cache only successful complete unfiltered modeled records. Apply an optional
  gene filter after a hit or fill. Never cache precomputed SNV results,
  rejections, operational failures, partial results, or adapter-rendered bytes.
- Use a full typed key, not a digest alone. Table `entries` has a SHA-256 key
  digest for indexed lookup plus closed columns for contig, position, REF, ALT,
  scoring semantics, model bundle/profile/representation, CPU policy,
  reference bundle and sequence-set identities, mask bytes/SHA-256, masking
  policy, and distance window. After digest lookup, compare every full column;
  a collision is a miss and must not overwrite either key.
- Store values as canonical compact UTF-8 JSON with exact shape
  `{"schema":"pangopup-model-cache-value-v1","records":[...]}`. Each ordered
  record contains GENCODE identity, integer gain/loss hundredths, signed
  relative positions, and a closed warning array. Decode through normal typed
  constructors, reject duplicate/missing/extended/out-of-range/trailing
  content, and require byte-identical canonical reserialization.
- Use `rusqlite` with the bundled SQLite library, WAL journaling, foreign keys,
  a bounded busy timeout, fixed `application_id`, and `user_version=1`.
  Unknown application/schema versions are incompatible. The default disposable
  database is removed with its `-wal`/`-shm` siblings and recreated once; an
  explicitly selected incompatible database returns a stable error. The cache
  is never provenance or an authoritative scoring source.
- Resolve the default database at
  `${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/model-results.sqlite3`. Add lookup
  options `--model-cache <ABSOLUTE_PATH>` and
  `--model-cache-max-entries <POSITIVE_INTEGER|unlimited>`, with matching
  `PANGOPUP_MODEL_CACHE` and `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` environment
  variables. Explicit CLI values override valid environment values; every
  supplied value is validated even if overridden. Default maximum is `10000`.
- Metadata stores a monotonic `next_write_sequence`. Only a successful insert
  or update consumes the next sequence in its existing write transaction.
  Ordinary valid hits are read-only. Bounded eviction orders by
  `(write_sequence, key_digest)` and removes rows until the limit is met.
  Before integer exhaustion, deterministically renumber retained rows in that
  order. `unlimited` performs no eviction. Limit changes take effect on the
  next successful open/fill without rewriting value bytes.
- Default-cache corruption, incompatible schema, or invalid cached rows are
  handled without an accumulating quarantine: delete an individually malformed
  row when SQLite remains healthy; otherwise remove the database and both
  sidecars and recreate once. SQLite busy/write failure must not invalidate a
  successfully computed model answer. An unsafe path, invalid configuration,
  or explicitly selected cache that cannot be safely opened returns a stable
  typed cache error.
- The supported filesystem boundary is a same-user private cache directory:
  directory mode `0700`, database/WAL/SHM mode `0600`, and no symlink at the
  selected database name. Same-user mutation after safe open and root or
  hostile-kernel replacement are outside this ticket; decoded rows remain
  untrusted.
- Integrate the cache only when model fallback is enabled. Lookup-only and
  authoritative SNV-hit CLI bytes, lazy component opening, and existing asset
  download cache behavior remain unchanged.
- Cache flags without the complete model/reference/mask fallback tuple are
  usage errors. Parse and validate explicit cache flags before routing.
  Lookup-only calls ignore cache environment variables. When fallback is
  enabled, validate every present cache environment value even when a CLI
  override wins. An authoritative SNV hit with complete fallback/cache
  configuration opens neither SQLite nor any model component.
- Perform one coordinator-owned production qualification comparing uncached,
  first-fill, same-process hit, and reopened-process hit for frozen M09
  one-strand insertion and M10 two-strand insertion under ordinary sequential
  `1/1`. Use three rotated fresh-process rounds, exact result validation, and
  retain machine-readable evidence reporting p50/p95, SQLite
  open/validated-hit time, component identity-admission time, complete
  fresh-process hit time, complete miss/uncached time, inference count,
  hit/miss/fill/eviction counts, RSS, database/WAL/SHM bytes, samples,
  identities, and break-even hit rate. Remove the expensive qualification
  harness after a passing receipt; it is evidence, not a recurring verifier.
- Retain SQLite unless it violates exactness, safe recovery, or bounded
  operation. SQLite-open plus validated-row-hit p95 must be at most 20 ms.
  Complete fresh-process hit p95 must be at least 20 times faster than complete
  uncached p50 and count zero ONNX session/probe/inference operations. Complete
  miss overhead must be at most 10 percent. For 1,000 representative entries,
  database plus WAL plus SHM must total at most 16 MiB, and incremental process
  RSS must separately remain at most 16 MiB. If a bound fails, remediate and
  remeasure rather than substitute an in-memory cache or silently omit
  persistence.
- Update `AGENTS.md`, `README.md`, `architecture/README.md`,
  `architecture/service.md`, `architecture/runtime-data.md`,
  `planning/faq.md`, and `planning/frontier.md`. Add one cache ADR and one
  retained measurement artifact. Do not draft the asset-profile ticket here.

Explicit exclusions: no in-memory result cache, cache of precomputed SNVs,
concurrent-fill coalescing, model/session pool, HTTP, service lifecycle,
coherent asset installation,
asset publication, Docker, systemd, GPU/MPS/CUDA, quantization, model graph
change, raw-source rebuild, repair/GC command, or cache sharing across machines.

## Success Checklist

- Repeated CLI model requests return byte-identical modeled JSON/table results
  while a reopened process uses SQLite without invoking the model.
- Gene-filtered and unfiltered requests share one unfiltered cached result and
  preserve exact stable-gene filtering and output order.
- A changed variant or any scoring/model/reference/mask/policy identity misses;
  a hash collision cannot alias entries because the full typed key is stored
  and compared.
- Default `10000`, positive configured bounds, `unlimited`, deterministic
  insertion/update-order eviction, limit reduction, empty cache, and reopen
  persistence work. Ordinary valid hits perform no SQLite write.
- A reopened hit records zero model-kernel construction, ONNX initialization
  probes, and inference calls. A miss performs one full authentication/open and
  does not hash or initialize the same component twice.
- Wrong schema, malformed rows, truncation, SQLite corruption, stale WAL/SHM,
  unsafe ownership/mode, database symlinks, busy database, write failure, and
  interrupted mutation never return an unverified cached value or damage an
  existing successful model result under the stated private-directory threat
  model.
- Normal tests use only temporary SQLite databases and checked miniature
  assets. They perform no production inference, Python, network, or source-data
  access.
- The coordinator's retained M09/M10 evidence passes the exactness and
  performance/resource bounds and records honest host-specific limitations.
- Existing 1,000-SNV, lookup-first CLI, compatibility, model-routing, and
  asset-sync tests remain unchanged and green.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Select persistent SQLite, not an LRU comparison.** Persistence across
   restarts is the requested value. Inference takes seconds, so SQLite need not
   compete with memory latency; it must only be exact, safe, bounded, and
   decisively cheaper than recomputation.
2. **Cache complete unfiltered modeled records.** The model and masking work is
   independent of the caller's optional stable-gene filter. Filtering after
   retrieval maximizes reuse without changing upstream-compatible masking.
3. **Treat cache failure as an optimization failure.** Cached bytes are always
   reconstructed and validated. Busy/corrupt/write state becomes a miss when
   safe; it never overrides a correct inference or makes SQLite authoritative.
4. **Bound by entries with explicit unlimited mode.** The default `10000` is
   predictable and simple. Operators may select another positive limit or
   accept unbounded disk growth explicitly with `unlimited`.
5. **Keep cache policy above the scoring engine.** The shared typed cache
   belongs at runtime composition so the current CLI and future HTTP service
   can use identical semantics without teaching the model, mmap, or scoring
   layers about SQLite.
6. **Defer concurrent-fill coalescing to HTTP.** The current CLI is sequential,
   while the future service owns admission, queues, and the single mutable
   model worker. Inventing that concurrency contract here would put policy in
   the wrong layer.

## Dependencies

- Ticket 019 exact variant-level scorer and compatibility corpus.
- Ticket 020 lookup-first model fallback and stable CLI modeled output.
- Ticket 021 selected ordinary CPU policy.
- Ticket 022 retained singleton reference/alternate execution.

## Notes

- The retained production model, reference, and mask paths are coordinator-only
  measurement inputs. The developer and reviewers use checked miniature
  assets and temporary databases.
- The cache directory is disposable XDG cache, not the authoritative XDG data
  store and not the asset-download transport namespace.
- Normal process shutdown checkpoints/truncates WAL only when safe; correctness
  must not depend on a graceful exit.
- Public status fields for service use are future. The retained measurement
  receipt is evidence only; this ticket does not add `pangopup status`.

## Coordinator Authorship

Coordinator: Codex `/root`

Drafted from the accepted persistent-SQLite plan, current lookup-first fallback
boundary, exact component identities, and the post-Ticket-022 frontier.

## Independent Ticket Review

Reviewer: Codex sub-agent `/root/ticket023_design_review`

Initial verdict: **REJECT**. The reviewer found that the first draft would
still initialize ONNX before a cache lookup, conflated SQLite-row latency with
fresh-process component authentication, included premature concurrent-fill
policy, underspecified the original hit-refresh contract, promised an infeasible
pathname threat model around SQLite sidecars, and left cache CLI/environment
precedence ambiguous.

The coordinator added lazy manifest/identity admission with zero model opens on
a hit, separated all performance intervals, deferred coalescing to HTTP, froze
the v1 key/value/application/write-order contracts, narrowed the supported private
directory boundary, and made cache configuration and lookup-first laziness
explicit. Resource limits now apply separately to database-plus-sidecars and
incremental RSS.

Revised verdict: **ACCEPT**. The reviewer confirmed the ticket is feasible,
appropriately bounded, dependency-correct, and directly proves persistent hits
without ONNX initialization or duplicate miss authentication.

Coordinator production measurement later disproved the drafted 10 ms
SQLite-open-plus-hit ceiling: two independent 30-process samples measured
M09/M10 p95 `13.264/13.452` ms and complete fresh-process p95
`23.441/23.563` ms, while retaining exact output and zero ONNX work. The
coordinator revised only that arbitrary SQLite sub-interval ceiling to 20 ms;
the 20-times complete speedup, 10-percent miss-overhead, accuracy, zero-ONNX,
disk, and RSS gates remain unchanged. This material acceptance change requires
the same reviewer's approval before implementation continues.

Threshold revision verdict: **ACCEPT**. The same reviewer confirmed that the
60 fresh-process observations disprove only the original unmeasured 10 ms
assumption. The 20 ms ceiling leaves useful headroom without changing the
architecture, user-visible behavior, or any of the end-to-end, correctness,
resource, recovery, or security gates.

The subsequent `r4` qualification exposed the actual source of the variable
hit time: SQLite opened in under 1 ms, while every valid hit performed a write
transaction solely to maintain exact LRU order. A direct 30-process diagnostic
measured M09 SQLite open/get p50/p95/max at
`8.225/15.523/26.435` ms; the outlier is in the old hit-refresh write, not
opening the database or reading the validated row.

Read-only-hit revision verdict: **ACCEPT**. The same reviewer accepted
deterministic insertion/update-order eviction because exact LRU was never a
product requirement. An old hot entry may eventually be evicted after 10,000
later writes, but that causes only a correct recomputation. This simpler policy
removes write amplification and lock contention from valid hits while
preserving the bound, persistence, exactness, and every other acceptance gate.
The unshipped v1 schema, documentation, and tests must use honest write-sequence
names.

## Implementation Evidence

Developer: Codex sub-agent `/root/ticket023_implementation`

Implemented the bundled-SQLite runtime cache, bounded model/reference/mask
identity admission, full-key/canonical-value validation, deterministic
insertion/update-order eviction, read-only valid hits, private-path and schema
recovery, lookup-first CLI composition, reopened-hit and post-hit gene-filter
behavior, isolated specs, and documentation. Established SQLite opens validate
the fixed identities, WAL mode, and both table column sets without rerunning
idempotent schema DDL. A valid hit performs no SQLite transaction and does not
mutate database, WAL, or write order; explicit insertion or update assigns the
next deterministic write sequence. Shutdown checkpoints only an instance that
actually wrote.

Focused evidence: 16 cache tests pass, including byte-identical database/WAL
and unchanged write-sequence assertions across a valid hit; CLI
unit/model-routing tests pass, including busy-open fallback to an exact model
answer without a cache fill;
engine, index, and model suites pass; source-fingerprint hard identities remain
unchanged; `cargo clippy --locked --workspace --all-targets -- -D warnings`
passes; and `make spec` passes 170 scenarios with 2 skipped. A full `make test`
initially exposed that placing admission code in the production reference
codec would churn its hard builder fingerprint; admission was moved into a
runtime-only module and the hard identity test then passed unchanged.

The first post-revision full qualification (`r4`, pinned CPU 0) completed all
expensive samples but failed the 20 ms M09 SQLite-open-plus-hit p95 gate before
the old harness emitted its receipt. The preserved database family and log
were not reused or deleted. Sixty immediate independent fresh-process hits
against those exact prepared databases, using the same release binary and CPU
affinity, measured SQLite open-plus-hit p50/p95/max of
`9.955/13.522/13.905` ms for M09 and `10.014/12.902/13.153` ms for M10;
complete-hit p50/p95 was `23.335/28.941` ms and `23.484/27.499` ms,
respectively. Their JSONL receipt SHA-256 is
`20009e4bcaaabeca3e6bf19909d01db90003bbf8fb4d36fe37c090cdb6a6834a`.
The later diagnostic separated open from get and identified the old
valid-hit write transaction as the variable interval.

The accepted read-only-hit revision passed the final one-time `r5`
qualification on pinned CPU 0. M09 reopened-hit p50/p95 was
`7.572/8.995` ms, SQLite open-plus-validated-hit p95 was `0.827` ms, and
speedup over uncached p50 was 1,448.23×. M10 measured `7.677/12.927` ms,
`1.084` ms, and 2,109.20×. Every reopened hit counted zero model
construction, session, probe, and inference work. The 1,000-entry database
family was 2,142,208 bytes and incremental RSS was 3,186,688 bytes. The full
receipt SHA-256 is
`e8d53b18225925c278f07905058e6591c8c70e048733610bdf3b7ca807abe416`;
complete method, identities, results, and log digest are retained in
`planning/artifacts/023-persistent-model-result-cache.md`.

After that passing receipt, the expensive harness, subprocess child/parser,
and measurement-only instrumentation were removed. Fast cache/CLI/integration
tests and the retained evidence remain; there is no tempting long-running
verify-all entry point in the shipped tree.

## Adversarial Code Review

Reviewer: Codex sub-agent `/root/ticket023_code_review`

Initial verdict: **REJECT**. The reviewer found four blockers: a busy cache
during open aborted correct inference; public raw cache-key fields allowed
unvalidated identities; schema validation compared column names but not the
fixed v1 types, nullability, primary keys, strictness, or metadata sequence;
and the tests did not drive every key field through real miss behavior or
force a same-digest/full-key mismatch.

The developer made busy-open an unavailable-cache path for that invocation,
while preserving errors for unsafe, incompatible, and invalid explicit
configuration. `CacheKey` now accepts only a validated `Grch38Variant` and a
closed, validated `CacheIdentity`; its fields are private. Established opens
validate strict `table_xinfo` shapes plus the single valid metadata sequence.
Focused tests cover malformed identities, all sixteen stored key fields,
same-digest/no-alias/no-overwrite behavior, wrong-shape schema recovery, bad
metadata sequence, and CLI busy-open inference with no fill.

Remediation evidence: `cargo test -p pangopup-cache` passes 16/16;
`cargo test -p pangopup-cli --test model_routing` passes 4/4; `make lint`,
`make test`, `make spec` (170 passed, 2 skipped), and `git diff --check` all
pass. The expensive production qualification was not rerun and its harness
remains removed.

The reviewer correctly noted that complete STRICT table-shape and metadata
validation adds reads to the reopen path after the retained `r5` receipt.
Narrow final-release fresh-process checks therefore exercised only prepared
production cache hits on CPU 0. Thirty exact M09 hits measured complete-process
p50/p95/max `11.406/11.790/11.966` ms; thirty exact M10 hits measured
`10.945/15.153/16.255` ms. Complete process time includes identity admission,
SQLite open/schema validation/get, rendering, and startup, so it upper-bounds
SQLite open plus hit below 20 ms. Millisecond completion versus the retained
10.966/16.193-second uncached p50 proves no model inference occurred. The M09
raw-sample SHA-256 is
`33bf161da73ce8faae02b1754996f9d906dd25c3163b9e175fca69c21fde9f60`;
the 1,136-byte M10 receipt SHA-256 is
`1a3fea5c9e603f29dfbf1921ded169dc89cd9d88f353e1ae3717a35b39ea7bce`.
The retained artifact distinguishes the original `r5` proof from this
final-source spot evidence.

Remediation re-review: **ACCEPT**. The same reviewer verified all four
functional remediations, independently reran the focused cache and CLI tests,
confirmed the final-source M09/M10 spot measurements and retained hashes, and
found no remaining blocker.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: Codex `/root`

Final coordinator gate after accepted review: `make lint`, `make test`,
`make spec` (170 passed, 2 skipped), and `git diff --check` all pass.
