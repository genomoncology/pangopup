# 019 — Score supported GRCh38 variants through the Pangolin model

Status: ready

## Why

Pangopup can open the compiled GRCh38 sequence index, query the selected
GENCODE mask, and execute the twelve-channel Pangolin ONNX kernel, but those
three capabilities are not composed. A caller still cannot submit a genomic
variant and receive a Pangolin-compatible modeled splice score.

This is the smallest useful layer above the raw kernel. It defines the bounded
genomic-allele contract, constructs reference and alternate contexts, performs
the frozen ensemble/indel/masking/extrema arithmetic, and returns ordered typed
model results. Lookup routing, CLI product output, delivery, caching, HTTP, and
CPU tuning remain separate outcomes.

## Scope

### Public vocabulary

- Add an owned `Grch38Variant` to `pangopup-core` with a primary GRCh38
  contig, one-based position, and nonempty concrete uppercase A/C/G/T reference
  and alternate allele sequences. Identical alleles are invalid. The literal
  tuple is the identity: Pangopup performs no trimming, left alignment,
  equivalence collapsing, HGVS parsing, or transcript projection.
- Add ordered modeled-result vocabulary:
  - exact versioned/PAR `GencodeGeneId`;
  - `ModelGeneScoreRecord`;
  - `ModelScoreResult`;
  - `ModelWarning::NoAnnotatedSites`;
  - expected `ModelRejection` categories; and
  - operational `ModelScoringError` categories that keep reference, mask, and
    model-provider failures distinct.
- Reuse `PangolinScore` and exact hundredths for public values. Widen
  `RelativePosition` to an `i16` range of `-50..=149`, the complete range for a
  supported reference allele of length 100. Existing SNV JSON/table bytes must
  remain unchanged.
- Public model scores intentionally normalize positive and negative zero to
  numeric `0.00`, as the accepted design already requires. Binary32/binary64
  dtype and signed-zero distinctions remain observable inside compatibility
  tests but are not new public wire fields.

### One composition crate

- Add `crates/pangopup-engine` and no additional orchestration crate.
  `pangopup-core` owns caller vocabulary; `pangopup-index` retains reference
  and mask mmap details; `pangopup-model` remains a raw tensor kernel.
- Expose one concrete, mutable, single-owner `VariantScorer` composed from a
  `ReferenceProvider`, `MaskProvider`, and `ModelKernel`. Its public operation
  returns the fixed masked, distance-50 result or a typed rejection/error.
  It has no `Sync`, pool, cache, transport, or concurrency claim.
- Keep the injectable raw-kernel seam private to `pangopup-engine`. Production
  adapts `ModelKernel`; unit tests may supply exact checked raw channels. Do
  not make compatibility-capture types or a general mock trait part of the
  public runtime API.
- Preserve result order exactly: plus strand before minus strand and
  authenticated query rank within each strand. Do not sort modeled records by
  stable gene ID.

### Supported request and rejection boundary

- Fixed assembly and parameters are GRCh38, primary contigs, one-based
  coordinates, masked output, and distance 50.
- Supported literal shapes are:
  - SNV: one base to one different base;
  - equal-length MNV: two through 100 bases with unequal allele sequences;
  - left-anchored insertion: one-base REF, ALT length two through 100, and the
    first base shared; and
  - left-anchored deletion: REF length two through 100, one-base ALT, and the
    first base shared.
- Reject unequal complex replacements, unanchored indels, or a supported-shape
  allele longer than 100. The core request can represent the 101-base deletion
  oracle so the scorer, rather than an unchecked test-only constructor, proves
  the size rejection.
- Preserve the frozen first-operation order:
  1. classify shape and enforce model length;
  2. obtain the complete reference window;
  3. compare submitted REF at the anchor;
  4. reject a reference context containing a symbol outside A/C/G/T/N;
  5. construct the alternate context;
  6. query all containing genes without a filter;
  7. execute plus-reference, plus-alternate, minus-reference, then
     minus-alternate inference, skipping both calls for an empty strand.
- Map an unavailable left or right window to
  `InsufficientReferenceContext`, an anchor difference to
  `ReferenceMismatch`, and an empty gene query to `NotInGene`. No later
  provider or model call occurs after an earlier rejection. A reference
  provider error or symbol rejection never queries the mask; a mask error or
  empty query never invokes the model; a model error stops before the next
  sequence/strand call and returns no partial result.
- Submitted alleles remain concrete A/C/G/T. Context A/C/G/T/N is accepted by
  the model; another IUPAC symbol from the reference produces a typed
  unsupported-reference-symbol rejection rather than silent conversion.

### Exact context and score semantics

- For position `P`, REF length `R`, and fixed distance 50:
  - reference start is one-based `P - 5_050`;
  - anchor offset is zero-based `5_050`;
  - reference context length is `10_100 + R`; and
  - alternate context replaces exactly that REF slice and has length
    `10_100 + ALT length`.
- Query the mask at the submitted anchor and always request every containing
  gene. Do not pass a stable-gene filter into masking: filtering an earlier
  same-strand overlap would change a later gene's compatible result.
- For each populated strand, run reference and alternate once. Treat channels
  1–3, 4–6, 7–9, and 10–12 as four tissue groups of three replicates.
  Reconcile each replicate before subtraction/averaging:
  - equal-length variants subtract directly in `f32`;
  - insertions collapse the center-expanded alternate interval with its first
    maximum and remain `f32`; and
  - deletions insert binary64 zeroes after center index 50 and retain the
    upstream `f64` promotion.
- Average each three-replicate group with upstream-compatible arithmetic, then
  select per-position minimum loss and maximum gain over the four tissues.
- Mask one shared gain/loss pair while visiting genes in authenticated order.
  Clamp gain to zero at annotated boundaries and loss to zero away from
  annotated boundaries. Empty boundary lists clamp every negative loss and add
  `NoAnnotatedSites`. Later same-strand genes observe earlier mutations.
- Select the first index on extrema ties. Convert the selected value to public
  exact hundredths as follows: perform `round_ties_even(value * 100)` in the
  selected array's retained `f32` or `f64` dtype, require the rounded gain to
  be in `0..=100` and rounded loss to be in `-100..=0`, convert loss to its
  nonnegative magnitude, and collapse either signed zero to zero. A rounded
  value with the wrong sign or outside the public range is an invalid model
  output. Positions are `index - 50`.

### Authenticated production mask qualification

- Preserve cheap `MaskDomainsOpen::open` for ordinary mmap use. Add a separate
  qualification open that:
  - opens the path once with the existing no-follow, regular, single-link
    checks;
  - hashes the bounded member through that held descriptor;
  - compares exact expected byte length and SHA-256 before mapping;
  - constructs the provider from the same descriptor/inode without reopening
    the pathname; and
  - returns the verified member identity for the qualification receipt.
- Keep the descriptor alive with the mapped provider. Normal tests prove a
  wrong digest, replacement path, symlink, and mutation before verification
  fail. This is not a general mask bundle, transport, install, or startup hash
  policy; the later coherent asset profile owns ordinary runtime
  authentication.

### Tests, qualification, and documentation

- Fast normal tests consume the authenticated
  `tests/fixtures/pangolin-compat-v1` and
  `tests/fixtures/pangolin-model-v1/kernel-golden.jsonl` only as dev/test
  evidence. `pangopup-engine` must not depend on `pangopup-build` at runtime or
  copy its replay implementation.
- Replay all 14 modeled cases and all 36 reference/alternate/strand raw
  evaluations. Assert exact context bytes and calls, exact typed post-ensemble
  arrays from the kernel goldens, ordered genes, masked public scores, and
  positions.
- Replay all six rejection cases with spy providers and all four controlled
  post-processing cases. Add synthetic controls for unanchored indels,
  overlength MNV/insertion, identical alleles, unsupported reference symbols,
  provider failures, exact call order/no partial result, output order, a
  selected extremum at `+149`, and positive/negative half-centi ties in both
  `f32` and deletion-promoted `f64`.
- Add one small real-ONNX integration using the existing checked synthetic
  `ModelKernel` plus in-memory reference/mask providers.
- Add one ignored maintainer qualification using the preserved production
  model, reference, and mask paths. Run it once after normal tests, compare the
  14 scored cases' masked hundredths, positions, and exact gene order with the
  compatibility oracle, record a concise receipt, and do not create a routine
  `verify all` command.
- Add ADR
  `architecture/decisions/0015-variant-level-model-scoring.md` and retained
  evidence `planning/artifacts/019-variant-level-model-scoring.md`.
- Update `Cargo.toml`, `AGENTS.md`, `README.md`,
  `architecture/README.md`, `architecture/design.md`,
  `architecture/runtime-data.md`, `planning/faq.md`, and
  `planning/frontier.md`. The docs must say variant-level scoring is shipped
  while lookup routing, CLI model output, CPU tuning, delivery, caching, and
  HTTP remain future.

### Explicit exclusions

- No precomputed-score lookup or lookup-first routing.
- No CLI product command or JSON/table contract change and no new mustmatch
  behavior. Existing specs still run in the final gate.
- No gene filter in `VariantScorer`; the following router filters only after
  complete ordered model masking.
- No model/reference/mask transport, XDG installation, GitHub asset, rebuild,
  conversion, or publication.
- No SQLite/model-result cache, HTTP, Docker, systemd, status, or lifecycle
  work.
- No ONNX graph, threading, batching, session-pool, accelerator, quantization,
  or performance-policy change.

## Success Checklist

- `pangopup-engine` composes the three existing runtime providers and exposes
  one masked distance-50 scorer over the supported literal GRCh38 subset.
- All 14 scored cases replay through exact checked raw channels; all six
  rejections and four controlled cases pass without Python or production
  assets.
- Tests prove 36 exact raw evaluations, both strand orders, order-mutating
  masking, deletion `f64`, first ties, rounding, warnings, provider
  short-circuiting, and the full `-50..=149` position range.
- A checked synthetic ONNX integration exercises the concrete
  `ModelKernel` adapter.
- The single retained production qualification reuses, and does not rebuild,
  the accepted assets and matches the oracle's masked public numeric results,
  positions, and gene order. Its receipt records identities returned by the
  opened model, reference, and descriptor-verified mask providers rather than
  relying on a path checked separately.
- Normal gates touch no production SNV/reference/mask/model member and invoke
  no network, Python, PyTorch, FASTA, GTF, or SQLite.
- Documentation contains no claim that lookup routing, CLI model output,
  delivery, caching, HTTP, or optimized CPU policy is already implemented.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Owning layer:** add one `pangopup-engine` composition crate. Putting
   genomic orchestration in `pangopup-model` violates ADR 0014; putting it in
   core would introduce implementation dependencies; adding separate scoring
   and routing crates would create unnecessary layers.
2. **Variant identity:** accept a literal, concrete, shared-anchor subset and
   perform no normalization. This is stable and standalone without importing a
   general HGVS/VCF normalization system. Equivalent spellings remain distinct
   caller inputs.
3. **Bounds:** cap every supported model allele at 100 bases, matching the
   authenticated kernel's 10,001–10,200 context contract. Requests beyond that
   boundary receive a typed rejection rather than an oversized allocation.
4. **Public numeric type:** reuse exact hundredths and widen only the position
   type. Exposing raw floats would complicate the future unified route and leak
   NumPy formatting artifacts; internal tests still prove exact dtype-aware
   behavior.
5. **Gene filtering:** query and mask all containing genes before any filter.
   Early filtering is faster but incompatible for overlapping same-strand
   genes because upstream mutates shared arrays.
6. **Compatibility:** preserve upstream's observed ordered mutation behavior
   under profile `pangolin-1.0.2-5cf94b8-grch38-v1`. A corrected independent
   per-gene policy would require a separately named future profile.
7. **Testing seam:** use a private raw-kernel capability plus frozen goldens
   for fast exact tests and one retained real-asset qualification. Do not put
   production assets or a long-running verifier into normal gates.
8. **Mask qualification trust:** hash the mask once through the same descriptor
   used to create the mmap and retain that descriptor. A pathname hash followed
   by an ordinary open has a replacement race; hashing on every normal startup
   would defeat the existing cheap-open boundary.

## Dependencies

Tickets 009, 011, 014, and 018 complete.

## Notes

- Base commit:
  `184359efbdc8ed014290d6490553c65b92f9264d`.
- Preserved production inputs, to be opened but never rebuilt:
  - model:
    `/home/ian/workspace/data/pangopup-model-018/bundle`;
  - reference:
    `/home/ian/workspace/data/pangopup-reference-production-011/1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01/bundle`;
  - mask:
    `/home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb/prepare/candidates/domains.pgm`.
- Accepted production identities must be checked before qualification:
  - model bundle
    `sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`;
  - reference bundle
    `sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`;
    and
  - mask member SHA-256
    `714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`.
- The compatibility corpus's M01–M04 precomputed observations are documentary
  cross-route observations, not expected equality with modeled scores.
- Normal tests may dev-depend on bounded corpus authentication helpers, but no
  builder/capture dependency enters a shipped runtime path.
- This ticket has no public or irreversible external effect.

## Coordinator Authorship

Coordinator: Codex (`/root`)

Drafted from the shipped Ticket 018 kernel, ADRs 0008 and 0014, the rolling
frontier, direct inspection of the current provider APIs, the frozen corpus and
kernel goldens, and three independent read-only planning audits. The
coordinator owns ticket-review remediation but does not implement product code
or approve its own ticket.

## Independent Ticket Review

Reviewer: Codex sub-agent (`/root/ticket019_design_review2`)

Accepted after one remediation round. The initial review rejected three
underspecified boundaries: qualification hashed the mask separately from the
opened mmap, provider/inference call order was not exact enough for the spy
tests, and public centi-score conversion was not distinguished precisely from
the retained NumPy renderer. The coordinator added a descriptor-held verified
mask open and receipt identity, fixed the complete rejection/provider/strand
call sequence, and specified dtype-local ties-to-even conversion, sign/range
validation, signed-zero collapse, and half-centi controls. The same reviewer
then returned `ACCEPT`.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
