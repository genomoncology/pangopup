# 014 — Use the selected GENCODE mask as a runtime provider

Status: ready
Contract identity (SHA-256 with this value set to `pending`):
`4fe67d2db72e92e864f649c5872b6fba3a7662b682939fda0f53b72a01884eef`
Base revision: `8681dee6d0afce7ab437d413a5e786dd49e7e018`

## Why

Ticket 012 already compiled and exhaustively checked GENCODE v38, compared three
representations, and selected the constant-membership `domains` member. That
exact member is preserved locally:

```text
/home/ian/workspace/data/pangopup-mask-qualification-012/
  ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb/
  prepare/candidates/domains.pgm
```

It is 6,703,320 bytes with SHA-256
`714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`.
Its retained comparison proved exact logical equivalence, 171/331 ns p50/p95,
zero warmed allocations, and the required ordering and exon-boundary behavior.

The prior Ticket 014 attempted to create a second format and a large
qualification/publication system. That work was not committed and has been
discarded. Pangopup instead needs the smallest runtime boundary that opens and
queries the already-selected bytes so CPU model inference can consume them.

## Outcome

`pangopup-index` exposes a typed, read-only, thread-safe mask provider that
opens the existing selected `domains.pgm` format without rewriting or rebuilding
the asset and returns the exact ordered genes and exon boundaries needed by
Pangolin masking.

## Scope

### Included

- Add a production-facing `pangopup_index::mask` API for the selected domains
  representation only.
- Open the existing `PGMBEN01` v1 bytes only when their codec discriminator is
  `domains`; interval-tree and binned-postings members are rejected.
- `pangopup_index::mask` owns the domains-only runtime reader, query types,
  reusable buffer, provider trait, and error surface. Production code must not
  expose candidate writing, codec selection, cancellation, complete inspection,
  or page-tracing APIs.
- Expose:
  - `MaskDomainsOpen::open(path) -> Result<MaskDomainsOpen, MaskError>`;
  - `MaskProvider: Send + Sync`, whose `query(contig, position, stable_gene,
    output) -> Result<(), MaskError>` accepts typed `Grch38Contig`,
    `GenomicPosition`, optional `EnsemblGeneId`, and caller-owned
    `MaskQueryBuffer`;
  - `MaskStrand`, `MaskQueryGene`, and buffer accessors for plus, minus, and
    each returned gene's boundary slice.
- A successful miss clears the buffer and returns empty plus/minus slices.
  Every error clears the buffer before returning.
- Preserve Pangolin's effective `(start, end]` membership, plus-before-minus
  ordering, within-strand authenticated rank, exact versioned/PAR gene identity,
  optional stable-gene filtering, and normalized exon boundaries.
- Keep open bounded to header/directory validation; do not hash or scan the
  complete payload during ordinary open.
- Keep warmed queries allocation-free when the caller-owned buffer has
  sufficient capacity.
- Add ADR 0013, which explicitly supersedes ADR 0011's requirement for a
  separately re-specified production format. It records why byte-identical
  promotion is safe and puts exact asset identity at download/install
  verification rather than ordinary mmap open.
- Update `AGENTS.md`, `README.md`, `architecture/README.md`,
  `architecture/design.md`, `architecture/delivery.md`,
  `architecture/index.md`, `architecture/runtime-data.md`, and
  `planning/frontier.md` so current-state claims agree with ADR 0013. Keep the
  Ticket 012 report and ADR 0011 as historical evidence; ADR 0013 records the
  superseding consequence rather than rewriting history.

### Excluded

- No GTF, gffutils, Python, canonical-export, candidate, or full-source run.
- No new mask format, reheadering, copying, recompression, or production build.
- No bundle manifest, XDG installation, network download, GitHub upload, or
  release publication.
- No model execution, variant routing, cache, HTTP, Docker, or accelerator.
- No broad deletion of historical candidate code; that is the next
  independently reviewed repository-diet outcome.
- No source-fingerprint, publication, quarantine, inode, race, allocator, or
  qualification lifecycle.

## Decisions

1. **Use the selected bytes unchanged.** The historical `PGMBEN01` magic is an
   internal implementation detail, not a reason to create a second equivalent
   asset. Runtime compatibility is the exact format/version/codec plus the
   release manifest checksum added by later asset packaging. ADR 0013
   supersedes only ADR 0011's consequence that required a distinct production
   magic; the selected domains semantics and performance decision remain.
2. **Reader only.** Pangopup already has the complete selected asset. This
   ticket adds no production builder and performs no unchanged full-data work.
3. **Narrow production API over existing decoding.** The selected decoder is
   promoted behind a domains-only API. Other experimental codecs remain
   inaccessible through that API and are removed by the following cleanup
   ticket.
4. **Cheap open, touched-byte validation.** Ordinary startup validates bounded
   structure. Queries validate the records they touch. SHA-256 remains a
   download/install responsibility, not a per-startup payload scan.
   Structurally valid semantic mutation cannot be detected without the asset
   checksum. Concurrent in-place mutation or truncation of an opened mmap is
   outside the supported threat model; assets are immutable after verified
   installation.

## Discriminating Controls

- A domains member opens and answers independent miniature fixture queries.
- Interval-tree and binned-postings members fail through the production API.
- Start, start+1, end, and end+1 queries prove `(start, end]`.
- Plus/minus and same-strand overlaps prove exact result order.
- Stable-gene filtering does not merge chrX and `_PAR_Y` chrY records.
- Truncated header/directory and corrupted touched gene/boundary/domain/posting
  records return typed errors without panic or partial results. Checks cover
  section and record ranges, reserved fields, canonical contigs, spans, exact
  gene identity, domain/posting bounds, boundary ranges/order, strand/rank
  ordering, and posting references.
- A sufficiently reserved query buffer reports zero allocations over a repeated
  representative query loop.
- A local, read-only ignored integration test opens the pinned
  6,703,320-byte member, confirms its exact SHA-256, and exercises
  `planning/artifacts/012-performance-manifest.jsonl`. For each query it encodes
  the result as canonical JCS
  `{"plus":[{"id":...,"boundaries":[...]}],"minus":[...]}`, appends LF, and
  compares the SHA-256 with `expected_sha256`.
- The exact retained-member command is:

  ```text
  PANGOPUP_MASK_MEMBER=/home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb/prepare/candidates/domains.pgm \
    cargo test --locked -p pangopup-index --test mask_retained_member \
    -- --ignored --exact retained_domains_member_matches_oracle
  ```

  An explicit invocation with a missing, absent, wrong-size, or wrong-digest
  member fails rather than skipping. The test compiles or invokes no capture,
  preparation, benchmark, or qualification lifecycle code. Portable miniature
  tests remain in the normal gate.

## Acceptance

- `pangopup_index::mask` is the sole production-facing mask API. The temporary
  historical qualification API may remain until the next cleanup ticket, but
  production code does not depend on it.
- The exact pinned member opens read-only and all retained workload results
  match the Ticket 012 expected hashes.
- The production `mask` API rejects both unselected candidate codecs.
- Existing SNV lookup, install/sync, and reference-provider behavior is
  unchanged.
- Focused mask tests pass.
- `make lint`, `make test`, and `make spec` pass.
- The complete diff contains no generated production data or new large file.

## Dependencies

- Ticket 012 selected member and retained evidence: complete.
- Ticket 013 is not extended; its fingerprint machinery is irrelevant here.

## Work Ownership

- Coordinator-authored ticket and review dispositions: primary agent.
- Independent design reviewer: pending.
- Implementer: pending; must be different from the design reviewer.
- Adversarial code reviewer: pending; must be different from both.
- The abandoned prior Ticket 014 diff was agent-generated, was archived under
  `/tmp/pangopup-ticket014-abandoned-20260725`, and is not part of this work.
- No user-authored or unrelated working-tree changes remain at this base.

## Long-running Jobs

None. This ticket forbids a production build or benchmark.

## Independent Design Review

Reviewer: `/root/ticket014_design_review`.

The first review rejected four ambiguities: the prior ADR required a distinct
production format; reader ownership and signatures were underspecified; the
retained-member oracle lacked an exact command/encoding; and the unhashed mmap
threat boundary overstated corruption detection. The coordinator revised this
contract to add superseding ADR 0013 and every stale documentation surface,
make `pangopup_index::mask` the domains-only owner with exact buffer/error
behavior, pin the explicit JCS+LF retained-member test, and separate structural
validation from asset checksum identity.

The same reviewer re-reviewed the complete revision at base
`8681dee6d0afce7ab437d413a5e786dd49e7e018`, found no residual blocking issue,
confirmed Ticket 012 complete and Ticket 013 unnecessary, and recorded
`ACCEPTED AS READY`.

## Implementation Evidence

Developer: pending.

## Adversarial Code Review

Reviewer: pending.

## Acceptance Trace

| Acceptance clause | Command or evidence | Result |
|---|---|---|
| Domains-only runtime provider | pending | pending |
| Exact retained workload | pending | pending |
| Bounded/corrupt-input behavior | pending | pending |
| Existing product paths unchanged | pending | pending |
| Full repository gates | pending | pending |

## External Effect Evidence

Coordinator: not applicable. The ticket performs no public or irreversible
external effect.

## Coordinator Final Check

Coordinator: pending.

## Coordinator Authorship

Coordinator: Codex primary agent.
