# 020 — Route lookup misses and non-SNVs to model scoring through the CLI

Status: complete

## Why

Pangopup now has both halves of its answer path: an exact mmap lookup for
published GRCh38 SNVs and a Pangolin-compatible Rust model scorer for supported
literal variants. They are still separate library capabilities. The shipped
CLI accepts only SNVs and cannot return a model result.

This ticket joins those existing capabilities without adding another service
layer. An SNV result already present in the published lookup remains
authoritative and fast. A pure lookup miss or supported non-SNV can use one
explicitly supplied local model/reference/mask set. The outcome is a stable CLI
contract that can later sit unchanged behind HTTP.

## Scope

### One lookup-first routing boundary

- Add routing to the existing `pangopup-engine` crate; do not create another
  orchestration crate.
- Expose one small typed router over a `ScoreProvider`. Its inspect operation
  consumes an owned request containing the literal `Grch38Variant` and
  optional stable-gene filter. It returns either an owned precomputed routed
  result or an owned `ModelRequired { variant, gene }` token.
- Model completion must consume that exact `ModelRequired` token. Bind one
  mutable `VariantScorer` and the identities captured from the same concrete
  reference, identified mask, and model components in one small
  `ModelFallback`; it atomically returns an owned modeled routed result or
  preserves the scorer's expected rejection/operational error distinction.
  There is no public API that accepts provenance independently of its scorer.
- Do not add a trait per result type, a route-plan graph, a cache, or a public
  model factory abstraction. A private injected CLI opener/counter is allowed
  only to prove lazy and exactly-once opens, analogous to the existing private
  `run_with_sync` test seam.
- A one-base-to-one-base request is looked up first with the caller's optional
  stable-gene filter. A result containing at least one score record or one
  source-reference ambiguity is authoritative. Only a result with neither is
  model-required.
- A non-SNV is model-required without calling the precomputed provider.
- Model scoring always queries and masks all containing GENCODE genes. Apply
  the optional stable-gene filter only to the completed ordered model records;
  never pass it into the mask query. Preserve the scorer's plus-before-minus
  and authenticated within-strand order after filtering.

### CLI asset and request contract

- Extend the existing `pangopup lookup` command with one all-or-none explicit
  local fallback set:
  - `--model-bundle <DIR>`;
  - `--reference-bundle <DIR>`; and
  - `--mask <FILE>`.
- Keep `--bundle` versus `--data-dir`, `--gene`, and `--format` behavior
  unchanged. With none of the three model flags, the command remains the
  legacy SNV lookup-only mode: authoritative hits, ambiguities, and pure
  `not_found` results all retain their exact existing output.
- Parse `--variant GRCh38:CONTIG:POS:REF:ALT` into nonempty literal uppercase
  A/C/G/T allele strings before opening any asset. Continue accepting the
  current primary contig spellings and exact RefSeq accessions. Reject lowercase,
  ambiguous symbols, empty alleles, zero/overflow positions, and identical
  alleles as `INVALID_VARIANT` with exit 2.
- The general parser performs no trimming, left alignment, equivalence
  collapsing, HGVS parsing, transcript projection, or mutation-type inference.
- A partially supplied fallback set is `CLI_USAGE` with exit 2.
- A non-SNV cannot run in lookup-only mode. If any request is non-SNV and no
  fallback set was supplied, fail with `MODEL_ASSETS_REQUIRED`, exit 2, one
  compact JSON line on stderr, and no stdout. A pure SNV lookup miss without
  fallback flags is not this error; it renders the legacy precomputed
  `not_found` result.
- When the complete fallback set is supplied, a batch first resolves every
  request's lookup decision and routes pure SNV misses/non-SNVs to the model.
  Open the reference, identified mask, and model exactly once, and only if at
  least one decision requires the model. A hit-only batch must neither inspect
  nor validate the three supplied fallback paths, so nonexistent fallback
  paths do not slow or break authoritative lookup hits.
- If fallback is required, component-open precedence is deterministic:
  reference first, identified mask second, model third. Report the first
  failure and do not inspect later components.
- Buffer the complete batch and write stdout only after every lookup/model
  request and serialization succeeds. An expected model rejection is
  `MODEL_REJECTED`, exit 2; an operational scorer failure is `MODEL_SCORING`,
  exit 1. Component-open failures are distinct stable, redacted exit-1 codes:
  `REFERENCE_BUNDLE_INVALID`, `MASK_INVALID`, and `MODEL_BUNDLE_INVALID`.
  Existing lookup bundle and corruption codes remain unchanged.

### Exact model provenance

- Ordinary `MaskDomainsOpen::open` remains the cheap installed-runtime
  operation. Add one descriptor-held, bounded `open_identified` operation for
  this explicit-path CLI: hash and structurally open the same held regular
  single-link inode, return its observed byte length/SHA-256 identity, and keep
  the descriptor with the mmap. This identifies caller-supplied bytes; it is
  not an installed compatibility profile or a claim that an arbitrary mask is
  trusted. ADR 0013's immutable-inode threat model remains explicit.
- Direct `open_identified` tests must prove symlink rejection, mutation while
  hashing, pathname replacement while hashing, and that the returned identity
  and subsequent queries use the same retained mapped descriptor. A
  hash-path-then-reopen implementation must fail acceptance.
- Capture model identity/profile before moving `ModelKernel`, reference
  provenance before moving `ReferenceBundleOpen`, and observed mask identity
  before moving `MaskDomainsOpen` into `VariantScorer`.
- Modeled JSON provenance is closed and ordered with:
  - `kind: "model"`;
  - scoring-semantics contract `pangopup-variant-score-v1`;
  - model bundle ID and model profile;
  - reference bundle ID, reference profile, and sequence-set SHA-256;
  - mask byte length and `sha256:<hex>` member identity;
  - `masked: true`; and
  - `window: 50`.
- `pangopup-variant-score-v1` names ADR 0015's algorithm and public rounding
  contract; it does not claim that caller-supplied assets are the strict
  `pangolin-1.0.2-5cf94b8-grch38-v1` tuple. This explicit tuple is reported,
  not activated or compatibility-checked as a future installed four-asset
  profile. The CLI therefore never emits that strict compatibility-profile
  claim. The retained coordinator qualification separately proves that the
  accepted production identities match its oracle.

### Stable JSONL and table output

- Preserve every existing precomputed JSONL and table byte exactly, including
  the 1,000-case oracle and `provenance.kind: "precomputed"`.
- Modeled JSONL uses the same top-level request/status/records/ambiguities
  shape. It emits exact versioned/PAR GENCODE gene IDs, exact centi-score
  strings, integer relative positions, per-record warning names, an empty
  source-reference-ambiguity list, and the complete model provenance above.
- The only current warning is exactly `"no_annotated_sites"`. `warnings` is
  always present on modeled records as an ordered JSON array, including when
  empty; precomputed records remain byte-identical and never gain a warnings
  field.
- Modeled status is `found` when records remain after the optional filter and
  `not_found` when none remain. Route identity lives in provenance rather than
  overloading status.
- Keep the existing table header and columns byte-for-byte. Modeled rows place
  the exact GENCODE ID in `GENE`, use the model bundle ID in `BUNDLE_ID`, and
  leave source-reference columns as dots. A filtered modeled miss emits the
  existing single `not_found` row shape with the model bundle ID. The compact
  table intentionally has no warning/provenance extension; JSONL is the
  provenance-complete model format.

The exact modeled JSON key order and nesting are fixed by these two renderer
unit-test lines (the repeated digits are deliberately synthetic identities):

```json
{"assembly":"GRCh38","contig":"chr1","position":5051,"ref":"A","alt":"AC","status":"found","records":[{"gene":"ENSG00000000001.1","gain_score":"0.35","gain_position":25,"loss_score":"-0.10","loss_position":2,"warnings":["no_annotated_sites"]}],"source_reference_ambiguities":[],"provenance":{"kind":"model","scoring_semantics":"pangopup-variant-score-v1","model_bundle_id":"sha256:1111111111111111111111111111111111111111111111111111111111111111","model_profile":"pangopup-model-kernel-mini-v1","reference_bundle_id":"sha256:2222222222222222222222222222222222222222222222222222222222222222","reference_profile":"pangopup-reference-route-test-v1","reference_sequence_set_sha256":"sha256:3333333333333333333333333333333333333333333333333333333333333333","mask_bytes":512,"mask_sha256":"sha256:4444444444444444444444444444444444444444444444444444444444444444","masked":true,"window":50}}
{"assembly":"GRCh38","contig":"chr1","position":5052,"ref":"A","alt":"AG","status":"not_found","records":[],"source_reference_ambiguities":[],"provenance":{"kind":"model","scoring_semantics":"pangopup-variant-score-v1","model_bundle_id":"sha256:1111111111111111111111111111111111111111111111111111111111111111","model_profile":"pangopup-model-kernel-mini-v1","reference_bundle_id":"sha256:2222222222222222222222222222222222222222222222222222222222222222","reference_profile":"pangopup-reference-route-test-v1","reference_sequence_set_sha256":"sha256:3333333333333333333333333333333333333333333333333333333333333333","mask_bytes":512,"mask_sha256":"sha256:4444444444444444444444444444444444444444444444444444444444444444","masked":true,"window":50}}
```

The matching table unit-test bytes are:

```text
ASSEMBLY	CONTIG	POS	REF	ALT	STATUS	GENE	GAIN_SCORE	GAIN_POS	LOSS_SCORE	LOSS_POS	SOURCE_REF	PUBLISHED_ALTS	OMITTED_ALT	BUNDLE_ID
GRCh38	chr1	5051	A	AC	found	ENSG00000000001.1	0.35	25	-0.10	2	.	.	.	sha256:1111111111111111111111111111111111111111111111111111111111111111
GRCh38	chr1	5052	A	AG	not_found	.	.	.	.	.	.	.	.	sha256:1111111111111111111111111111111111111111111111111111111111111111
```

### Fast fixtures and tests

- Add one compact checked coherent runtime fixture for ordinary Rust and
  mustmatch model-route tests:
  - reuse the existing synthetic ONNX model bundle;
  - add the distinct reader/builder profile
    `pangopup-reference-route-test-v1` using `PGRREF01`, assembly
    `synthetic-route-test`, the matching profile string as assembly accession,
    and fixture-only `urn:pangopup:fixture:reference-route-test-v1:*` source
    URLs;
  - its checked source FASTA has the 25 required RefSeq accessions in canonical
    order, all `A`: chr1 is exactly 10,101 bases and every other primary contig
    is exactly one base (10,125 total). Its checked assembly report says those
    exact lengths. Its exact notice is:
    `Pangopup synthetic route reference fixture\n\nThis pangopup-reference-route-test-v1 bundle contains synthetic GPL-3.0-only fixture data created by Pangopup. It is not a biological reference and must not be used for biological interpretation.\nLicense: https://www.gnu.org/licenses/gpl-3.0.html\n`;
  - build and check that reference once through the existing bounded reference
    builder. Hard-pin its source-file, sequence-set, NOTICE, member, manifest,
    and bundle identities in tests and retained evidence before code review;
  - add one tiny domains member whose logical oracle contains exactly
    plus-strand `ENSG00000000001.1`, rank 0, `(start,end] = (1,10101]`, and no
    annotated boundaries. Query position 5,051 must return that gene and thus
    exercise the exact `"no_annotated_sites"` warning;
  - reuse the existing checked SNV bundle for authoritative lookup cases.
- The file-backed modeled request is exactly
  `GRCh38:chr1:5051:A:AC`. Its one-base REF requires the complete 10,101-base
  scorer read beginning at chr1:1; tests must reject any off-by-one truncation.
- Check in the route mask's semantic JSON oracle and `domains.pgm`. A
  test-local fixed-fixture encoder may encode only that literal oracle and must
  reproduce the exact checked bytes; hard-pin byte length and SHA-256 in the
  direct reader test and retained evidence. Do not restore a general,
  production, CLI, or library mask writer.
- Keep the existing production bundle/member identity and the reference-mini
  profile name plus source/member bytes unchanged. The route-test profile is
  explicit, synthetic, checked, and cannot be installed or mistaken for
  GRCh38 production evidence.
- Adding the third reference profile changes the causal reference-builder
  source fingerprint. Refresh the current reference fingerprint assertion and
  every current miniature manifest/bundle-ID expectation that causally embeds
  it, and record the new future-build source identity. Preserve all existing
  miniature source/member bytes, legacy manifests, production bundle/member
  bytes and identity, and readability of the retained production reference.
- Unit tests use spy providers to prove:
  - authoritative SNV record and ambiguity results do not request a model;
  - pure filtered/unfiltered SNV misses do request it;
  - non-SNVs never call the lookup provider;
  - all-gene model masking precedes stable-gene filtering;
  - request order and model gene order are preserved; and
  - one rejection/error leaves no partial routed batch.
- CLI/unit/integration tests prove closed flag grammar, general allele parsing,
  every stable error code/exit/stream, redaction, exact legacy SNV
  lookup-only misses, hit-only zero fallback opens, exactly one fallback open
  for mixed/modeled batches, and exact JSONL/table bytes. The private opener
  seam must also prove reference-before-mask-before-model failure precedence
  and that later components are untouched.
- Add `spec/model-routing.md` using only checked synthetic assets. It must
  exercise authoritative hit with nonexistent model paths, model-required
  non-SNV without paths, a legacy SNV miss without paths, a modeled non-SNV, a
  flagged SNV miss `GRCh38:chr1:5051:A:C` that reaches the model, a mixed
  hit/model batch in request order, model output with and without `--gene`,
  table output, invalid fallback grammar, rejection, asset-open failure, and
  transactional stdout.
- Retain the existing seven-batch/1,000-request precomputed regression
  unchanged in legacy no-fallback mode; it includes six honest `not_found`
  requests and must not be described as hit-only.

### Qualification and performance evidence

- After normal tests, run one coordinator-owned retained real CLI inference
  using a single frozen non-SNV compatibility case and the preserved
  production SNV, model, reference, and mask assets. Compare its exact modeled
  records and provenance with the existing compatibility oracle and accepted
  identities. Do not rebuild, convert, download, or scan the complete source
  corpus, and do not add a routine production verifier.
- Keep the existing 1,000-request benchmark/regression honest as legacy
  provider/render evidence; its six misses mean it is not a hit-only router
  corpus. Add a separate derived routed-hit corpus from the same frozen
  requests by selecting the 994 authoritative record/ambiguity cases in
  original order and, for the 1,000 row only, appending the first six of those
  hits again. Benchmark warm 1/10/100/1,000 routed hits and compare the exact
  correspondingly selected/repeated oracle bytes. Keep the fresh-process
  legacy CLI row. Run it once on the final implementation and record the
  routed results beside the Ticket 006 provider/render baseline. This is
  diagnostic evidence, not a hardware-specific wall-clock gate. Model CPU
  tuning and batching are the next ticket, not this one.

### Documentation

- Add ADR
  `architecture/decisions/0016-lookup-first-cli-model-routing.md` and retained
  evidence `planning/artifacts/020-lookup-first-cli-model-routing.md`.
- ADR 0016 explicitly supersedes ADR 0015 only where ADR 0015 kept routing and
  gene filtering outside `pangopup-engine`: `VariantScorer` remains unfiltered,
  while the small router now lives beside and above it.
- Update `Cargo.toml`, `Cargo.lock`, `crates/pangopup-cli/Cargo.toml`,
  `AGENTS.md`, `README.md`,
  `architecture/README.md`, `architecture/design.md`,
  `architecture/index.md`, `architecture/reference.md`,
  `architecture/runtime-data.md`, `architecture/service.md`, `spec/cli.md`,
  `planning/faq.md`, and `planning/frontier.md`.
- Documentation must say lookup-first CLI model output is shipped while CPU
  tuning/batching, model-result caching, coherent asset delivery/activation,
  HTTP, Docker, systemd, and process lifecycle remain future.

### Explicit exclusions

- No model/reference/mask download, XDG installation, active pointer, release
  upload, GitHub asset publication, or raw upstream-source distribution.
- No ONNX graph change, thread-policy change, reference/alternate batching,
  accelerator, quantization, session pool, or concurrent inference claim.
- No SQLite or other model-result cache.
- No HTTP, Docker, systemd, start/stop/restart/status service, or background
  daemon.
- No SNV index, reference production format, mask production format, or model
  production bundle change.
- No HGVS, transcript/gene/protein projection, general normalization, or
  dependency on another GenomOncology project.

## Success Checklist

- `pangopup-engine` exposes one simple typed lookup-first router and no new
  orchestration crate.
- Existing lookup-only CLI output remains byte-identical across all 1,000
  checked requests.
- Without fallback flags, pure SNV misses stay byte-identical `not_found`;
  non-SNVs fail transactionally with `MODEL_ASSETS_REQUIRED`.
- A supported lookup miss/non-SNV produces exact modeled JSONL/table output
  from one explicit local fallback set, with complete JSON provenance.
- Hit-only batches perform zero fallback opens; a mixed or modeled batch opens
  the reference, identified mask, and model once.
- All routing, filter, ordering, rejection/error, redaction, and transactional
  cases pass without production assets, Python, PyTorch, network, SQLite, or
  another project.
- The checked file-backed synthetic route exercises the real CLI, PGRREF01
  reader, domains mmap reader, ONNX Runtime kernel, and variant scorer through
  `make spec`.
- One retained production CLI inference matches the frozen oracle without
  rebuilding an asset.
- The hit-only benchmark records exactness and current 1/10/100/1,000
  router diagnostics over an explicitly derived all-authoritative corpus,
  without turning wall-clock observations into a gate.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Authority:** any filtered precomputed record or source-reference
   ambiguity wins; only a pure result miss falls back. This preserves the
   published dataset and avoids recomputing authoritative SNVs.
2. **Laziness:** collect lookup decisions before opening fallback assets. A
   closure/factory framework would add abstraction without product value;
   the CLI can fill a small vector of typed decisions and open one scorer only
   when needed.
3. **Filtering:** precomputed lookup may filter at the provider, but modeled
   output filters only after complete all-gene scoring. Earlier mask filtering
   breaks compatible same-strand overlap behavior.
4. **Assets:** use three explicit local flags in this slice. Automatic
   installation must bind all four runtime identities atomically and belongs
   to the later coherent-profile ticket.
5. **Output:** preserve precomputed bytes and the compact table. Model-only
   fields live in a closed JSON provenance/record shape; route kind is
   provenance, not status.
6. **Testing:** add one coherent synthetic file-backed route rather than
   touching retained production assets in normal gates or adding a hidden
   in-memory CLI mode.
7. **Performance:** put the router inside the hit-only benchmark path and
   record it once. Do not tune the model while introducing routing because
   that would confound compatibility and latency changes.
8. **Explicit mode selection:** absence of all fallback flags preserves the
   existing SNV-only lookup contract; presence of all three enables fallback.
   This keeps six real `not_found` cases in the frozen 1,000-request oracle
   stable without special-casing fixture data. Non-SNV input still makes the
   need for model assets explicit.

## Dependencies

Tickets 004, 006, 009, 011, 014, 018, and 019 complete.

## Notes

- Base commit:
  `2798344be8245dd49b18250d9e36cdf4a7c5c110`.
- Preserved production inputs may be opened only by the coordinator's one
  retained qualification and must never be rebuilt:
  - SNV bundle:
    `/home/ian/workspace/data/pangopup/bundles/sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3`;
  - model:
    `/home/ian/workspace/data/pangopup-model-018/bundle`;
  - reference:
    `/home/ian/workspace/data/pangopup-reference-production-011/1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01/bundle`;
  - mask:
    `/home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb/prepare/candidates/domains.pgm`.
- Accepted identities:
  - SNV
    `sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3`;
  - model
    `sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`;
  - reference
    `sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`;
  - mask member SHA-256
    `714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`.
- Use M09 (`chr12:6801303 G>GA`) for the retained CLI qualification; it is a
  supported plus-strand insertion and avoids rerunning all 14 cases.
- Normal tests and specs use only checked synthetic assets and must not invoke
  network, Python, PyTorch, the retained production paths, or another
  repository.
- This ticket has no public or irreversible external effect.

## Coordinator Authorship

Coordinator: Codex (`/root`)

Drafted from the shipped Ticket 019 scorer, the existing lookup CLI and
1,000-case byte oracle, ADRs 0002/0003/0008/0013/0015, and the rolling frontier.
The coordinator owns ticket-review remediation but does not implement product
code or approve its own ticket.

## Independent Ticket Review

Reviewer: Copernicus the 2nd (`/root/ticket020_design_review`)

Initial verdict: **REJECT** at base
`2798344be8245dd49b18250d9e36cdf4a7c5c110`. Read-only review; no tests,
production assets, or files were changed.

The review found ten material ambiguities. This revision:

1. replaces the false strict-compatibility claim for arbitrary inputs with the
   exact `pangopup-variant-score-v1` semantics name and observed identities;
2. pins complete warning-bearing found and filtered-miss JSONL plus table
   bytes;
3. makes inspect own the variant/filter and completion consume that exact
   `ModelRequired` token through an identity-bound fallback;
4. requires direct same-descriptor, symlink, mutation, and replacement controls
   for `open_identified`;
5. fixes the route reference profile, source, notice, all 25 lengths, anchor,
   variant, 10,101-base read, mask oracle, and fixture reproduction boundary;
6. names the causal reference fingerprint and miniature expectation refresh
   while preserving production and fixture source/member bytes;
7. requires ADR 0016 to supersede ADR 0015's former router placement;
8. moves the router inside the measured hit-only benchmark path;
9. adds the omitted CLI dependency, reference architecture, and CLI spec files;
   and
10. permits only a private open-counter seam and fixes reference→mask→model
    failure precedence.

First re-review verdict: **ACCEPT**. The same reviewer confirmed that all ten blockers
are resolved and that the ticket is independently implementable, byte-exact,
ownership-safe, and explicit about fixture identities, descriptor integrity,
ADR supersession, benchmark coverage, failure precedence, and documentation
scope. No material findings remain.

Implementation then exposed one contradictory acceptance pair: the frozen
1,000-request CLI oracle contains six real pure SNV misses, so it cannot remain
byte-identical if every miss without model flags becomes
`MODEL_ASSETS_REQUIRED`. The coordinator amended only the CLI mode boundary:
no flags retain legacy SNV `not_found`; all three flags enable SNV-miss
fallback; non-SNV input without them still requires model assets. Router
library semantics remain unchanged.

First amendment re-review: **BLOCKER**.

The amendment reviewer then found that the new mode boundary lacked a flagged
SNV-miss model case and that the frozen 1,000 requests were still mislabeled
hit-only despite their six misses. This revision adds the exact
`chr1:5051 A>C` flagged model case, names the 1,000-case legacy evidence
honestly, and defines a separate stable 1/10/100/1,000 all-authoritative routed
corpus (994 original hits plus six deterministic repeats only for the
1,000-row).

Second amendment re-review: **ACCEPT**. The same reviewer accepted the explicit
flagged SNV-miss case and the separately derived true hit-only router corpus.
No material ticket findings remain.

## Implementation Evidence

Developer: Codex (`/root/ticket020_implementation`)

Implemented the accepted ticket without opening production assets, committing,
pushing, downloading, or publishing. The implementation adds:

- the owned `LookupFirstRouter`/`ModelRequired`/`ModelFallback` boundary in
  `pangopup-engine`, with all-gene masking before stable-gene filtering;
- descriptor-coupled identified opens for both the reference bundle and mask;
- a checked 25-contig route-reference fixture and a checked 260-byte literal
  route-mask fixture;
- lazy, exactly-once explicit-path CLI fallback with transactional stable
  failures and exact modeled JSONL/table rendering;
- unchanged no-flag SNV lookup behavior plus enabled flagged SNV-miss and
  non-SNV model paths;
- the separate 994-hit routed benchmark corpus and its exact selected/repeated
  oracle comparison; and
- ADR 0016, updated architecture/user docs, executable specs, frontier, and
  retained artifact 020.

Implementation exposed the contradictory original no-flag miss requirement and
then the missing flagged-SNV/true-hit benchmark cases. Development paused the
affected semantics while the coordinator returned both amendments to the same
ticket reviewer. After the second amendment was accepted, the implementation
preserved all six frozen no-flag misses and added the exact
`GRCh38:chr1:5051:A:C` model route.

Final formatting changed the causal reference-builder source fingerprint after
the route fixture's first build. The developer refreshed only the synthetic
manifest and current miniature provenance expectation from a final builder
run; `reference.pgr`, NOTICE, source inputs, legacy manifests, and production
bytes/identities remained unchanged. The final reference source fingerprint is
`4bc0e93b83b28e235a7d0f498976bfe1e97b39d13e4f8c940d4c03cfd3d641bf`;
the checked route manifest/bundle ID is
`6773713ad79462b8bfb2bce7f194041e85a0804b38f68282c965adc5f43f9493`.

Focused results:

```text
cargo test --locked -p pangopup-engine
  10 passed; 1 production qualification ignored
cargo test --locked -p pangopup-index identified_open
  2 passed
cargo test --locked -p pangopup-index identified_reference
  2 passed
cargo test --locked -p pangopup-index --test route_mask
  1 passed
cargo test --locked -p pangopup-build --test reference
  15 passed
cargo test --locked -p pangopup-build source_fingerprint
  15 passed
cargo test --locked -p pangopup-build --test builder_provenance
  4 passed
cargo test --locked -p pangopup-cli --lib
  4 passed
cargo test --locked -p pangopup-cli --bin pangopup
  8 passed
cargo test --locked -p pangopup-cli --test model_routing --test snv_regression
  6 passed; all 1,000 direct requests and seven CLI batches exact
mustmatch test spec/model-routing.md
  15 passed
mustmatch test spec/cli.md
  2 passed
cargo test --locked -p pangopup-cli --bench snv_regression --no-run
  compiled
cargo fmt --all --check
  passed
```

The routed benchmark ran once and its exact command/results are retained in
`planning/artifacts/020-lookup-first-cli-model-routing.md`. Coordinator-only
pre-review and post-reference-remediation M09 qualifications passed with the
exact frozen record and accepted model/reference/mask identities recorded in
that artifact. Independent code re-review accepted every remediation.

## Adversarial Code Review

Reviewer: Codex (`/root/ticket020_code_review`)

Initial verdict: **REJECT**.

The initial code review found four material defects:

1. explicit fallback trusted the reference manifest identity without
   authenticating the scored `reference.pgr` bytes;
2. an unflagged non-SNV could expose an SNV-bundle failure before the promised
   `MODEL_ASSETS_REQUIRED` error;
3. the mask pathname-replacement test replaced the path before hashing rather
   than from the hash callback; and
4. `architecture/runtime-data.md` still claimed lookup misses were not routed
   to the model.

Developer dispositions:

1. `ReferenceBundleOpen::open_identified` now performs a bounded full-member
   SHA-256 check against the canonical manifest, rejects descriptor/path races,
   mmaps the same authenticated descriptor, and retains it for all queries.
   Tests cover same-size corruption, symlinks, mutation and replacement during
   hashing, and post-open path substitution.
2. CLI input is pre-scanned before the SNV bundle is opened when fallback flags
   are absent. Non-SNV-only and mixed batches now return the exact
   `MODEL_ASSETS_REQUIRED` failure transactionally even when the supplied SNV
   bundle path does not exist.
3. The mask replacement control now fires inside the hashing chunk callback.
4. Runtime-data and the associated reference/design/ADR/user documentation now
   describe lookup-first routing and the explicit fallback authentication
   cost correctly.

Re-review verdict: **ACCEPT**. The same reviewer confirmed that reference bytes
and provenance now come from the same authenticated retained descriptor,
missing model assets take precedence for non-SNV requests, mask replacement is
tested during hashing, and runtime documentation matches implemented routing.
Focused Rust tests, all 15 model-routing specs, and `git diff --check` passed.
No material findings remain.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: Codex (`/root`)

Post-remediation retained M09 production qualification: **PASS**. The
strengthened explicit reference open authenticated the complete member before
returning the same frozen record and accepted identities.

Final gate:

```text
make lint
  passed
make test
  passed (workspace exit 0)
make spec
  167 passed, 2 skipped
git diff --check
  passed
```

Ticket 020 is complete. Production assets were opened only for the two bounded
review-stage M09 requests and were never rebuilt, converted, downloaded, or
modified.
