# 022 — Select the fastest exact reference/alternate model representation

Status: ready

## Why

Pangopup currently invokes ONNX Runtime separately for the reference and
alternate DNA contexts on each active strand. A one-strand variant needs two
model runs and a two-strand variant needs four. Ticket 021 showed that fixed
eight-thread CPU execution can reduce those complete requests substantially,
but the graph still accepts only one sequence at a time.

Naively adding a batch dimension is not a complete answer. Insertions and
deletions make the reference and alternate contexts different lengths, and one
ordinary tensor batch requires every item to have the same shape. This ticket
compares the practical single-session representations, proves their accuracy,
and retains only a materially faster winner.

## Scope

### Three representations

Build and compare exactly these candidates from the same twelve authenticated
Pangolin checkpoints:

1. **Singleton baseline.** The unchanged accepted graph and current separate
   reference/alternate calls.
2. **Zero-padded batch.** One input/output graph with dynamic batch size
   `B=1..4` and dynamic sequence length. After each context is strand-oriented,
   right-pad shorter encoded contexts with zeroes—the exact encoding of `N`—to
   the longest context in that request. Run one batch, retain only each item's
   original `length - 10,000` output prefix, then reverse retained minus-strand
   outputs into genomic orientation. Padding is runtime tensor shape only and
   is never exposed as biological sequence. Pangolin crops 5,000 bases from
   each edge, so every retained output's receptive field should remain wholly
   inside its original context; the raw oracle must confirm or reject that
   assumption.
3. **Paired strand batch.** A graph with independent reference and alternate
   inputs named `reference` with shape `[B,4,N_ref]` and `alternate` with shape
   `[B,4,N_alt]`, plus outputs `reference_scores` shaped
   `[B,12,N_ref_minus_10000]` and `alternate_scores` shaped
   `[B,12,N_alt_minus_10000]`. `B` is exactly the number of active strands
   (`1` or `2`), so one ONNX invocation accepts a complete supported variant
   even when reference and alternate lengths differ.

The converter must reuse one authenticated checkpoint/model set and shared
initializers rather than embedding duplicate weights. Record graph and bundle
sizes and reject a paired graph whose model bytes exceed the singleton graph
by more than 5 percent.

Do not add multiple sessions, request-level threads, a session pool, user
thread/batch settings, biological-sequence padding or coordinate changes,
cache, SQLite, GPU, MPS, CUDA, quantization, another runtime, HTTP, Docker,
asset download/upload, or publication.

### Candidate graph and runtime contracts

- Preserve the exact v1 parser/canonical bytes for the old singleton
  `pangopup-model-bundle-v1`. Add a distinct closed
  `pangopup-model-bundle-v2` manifest/parser with an explicit representation
  discriminator, representation-specific exporter settings, and exact named
  input/output shapes. Do not add a serde default to v1 or reserialize it
  through v2. Candidate bundles receive new profiles and identities; no
  manifest may mislabel changed graph bytes as the accepted singleton model.
- Keep `ModelContext` validation and strand encoding in `pangopup-model`.
  Add the smallest typed batch result needed to retain each item’s channel
  order, score length, and genomic orientation. No variant, gene, mask, or
  post-processing knowledge enters the model crate.
- `pangopup-engine` owns grouping active strand/reference/alternate work for
  the selected representation. Preserve plus-before-minus result order and
  the existing fail-fast error order. Do not move concurrency policy into the
  engine.
- Candidate implementations must have bounded checked batch counts, original
  lengths, padding, dimensions, element multiplication, allocation,
  input/output shape, finite score, and `[0,1]` validation. Malformed candidate
  graphs and wrong output batch/length shapes fail before partial public
  results.
- Keep ordinary CPU policy sequential `1/1`. Performance qualification also
  uses sequential fixed `8/1` on the exact Ticket 021 physical-core affinity
  to compare the best known eight-core behavior. Graph selection must not
  silently hardcode that host-specific thread count as the portable default.

### Reproducible candidate construction

- Update the authenticated converter and miniature fixture generator so both
  candidate representations are reproducible from absent output paths.
  Normal tests use only checked miniature graphs and never invoke Python,
  PyTorch, checkpoints, production assets, or network access.
- The coordinator builds each production candidate once into a new immutable
  scratch root under `/home/ian/workspace/data/pangopup-model-022/`, using the
  already downloaded `/home/ian/foss/Pangolin` checkout and retained evidence.
  Do not mutate or replace the Ticket 018 production bundle.
- Before timing, qualify every candidate against all 36 retained raw sequence
  evaluations and 45,756 scalar comparisons. Preserve the existing `1e-5`
  maximum absolute-error ceiling against independent PyTorch evidence.
- Then run the complete 14-case compatibility corpus and require the same 21
  public records, ordering, warnings, centi-scores, positions, rejection
  behavior, and accepted reference/mask identities as the singleton baseline.

### Complete-request performance experiment

- Add one ignored coordinator-only release harness. Each representation/policy
  combination runs in a fresh process pinned to
  `0,2,4,6,8,10,12,14`, opens one model session plus the same retained
  reference and mask, and reports exact identities, graph/model/bundle bytes,
  component-open time, peak RSS, ONNX session-invocation count, logical context
  evaluations, batch sizes, padded input elements, warmups, samples, p50, and
  descriptive p95 as machine-readable JSON. Do not call one paired invocation
  one model computation: it still evaluates reference and alternate branches.
- Measure frozen cases:
  - M07 equal-length, one strand;
  - M08 equal-length, two strands;
  - M09 insertion, one strand;
  - M10 insertion, two strands;
  - M12 deletion, one strand; and
  - M13 deletion, two strands.
- Run three fresh-process rounds per representation/policy, rotating candidate
  order in each round. Use one warmup and five consumed samples per case.
  Validate every warmup and timed result against the frozen public oracle.
  Retain every raw round and rank the median of the three per-process p50s.
  Report within-process p95 descriptively only.
- Run all three representations at sequential `1/1` and fixed `8/1`.
  Additionally run the paired representation once with parallel `1/8` only as
  a diagnostic for its two top-level graph branches. That result is ineligible
  for selection and has no invented singleton parallel baseline; Ticket 021
  already showed no reason for a wider parallel-policy matrix.
- For every case/policy/representation, aggregate latency as the median of the
  three fresh-process p50 values. Eligibility and ranking use that aggregate
  divided by the corresponding singleton aggregate. Aggregate memory as the
  maximum peak RSS across the three rounds and compare it with the maximum
  singleton RSS across its three rounds.
- A candidate is eligible only if all raw and public accuracy checks pass,
  model size stays within the 5-percent bound, aggregate peak RSS stays within
  twice the corresponding singleton baseline, and no case's aggregate p50 is
  more than 5 percent slower than its singleton aggregate.
- Declare a policy comparison inconclusive rather than selecting a graph if the
  three singleton rounds' fastest and slowest per-process p50 differ by more
  than 20 percent on any of the six measured cases. Raw rotated rounds remain
  evidence and the current singleton is retained.
- Within each CPU policy, rank eligible representations by the lowest
  worst-case p50 ratio across the six cases. Require at least a 20-percent
  p50 improvement on both M09 and M10 before replacing the singleton graph;
  otherwise retain it. Break an exact tie by lower RSS, then smaller model,
  then fewer ONNX session invocations, then fewer padded input elements.
- If the same representation wins `1/1` and `8/1`, select it. If the winners
  differ, prefer the `1/1` winner unless the `8/1` winner is no more than 5
  percent slower under `1/1` and has the better eight-core worst-case ratio.
  Record the mechanical calculation; do not choose by intuition.

### Staged adoption and cleanup

- The developer first implements generic candidate construction, fast tests,
  and the ignored harness without production assets. The coordinator then
  builds and measures production candidates. The same developer resumes,
  applies the mechanical selection, and removes losing production dispatch
  paths. The coordinator reruns selected production proof before independent
  code review.
- Retain compact maintainer-only converter modes and the ignored experimental
  harness needed to regenerate and compare all candidates. Retain the selected
  checked miniature fixture and the unchanged legacy singleton fixture; remove
  losing checked fixtures and production dispatch branches. Durable evidence
  includes the losing candidates without making them permanent runtime modes.
- Re-run the selected representation in a fresh process at ordinary `1/1` and
  fixed `8/1`, then repeat full raw-model and 14-case public qualification.
- The selected production model bundle remains a local retained asset for
  later reviewed publication. This ticket performs no GitHub release,
  download, active-profile switch, or external effect.

### Fast tests and documentation

- Checked miniature tests cover singleton compatibility, both candidate graph
  contracts, zero-padded `B=1`, `B=2`, and `B=4`, paired `B=1` and `B=2`,
  equal and unequal reference/alternate lengths, zero-padding/slicing at both
  length extremes, both strands, item/order preservation, `B>4` rejection,
  overflow/resource bounds, malformed dimensions, wrong output batch/length,
  non-finite/out-of-range values, and fail-fast behavior.
- Engine spy tests prove the exact ONNX session-invocation and batch-item
  grouping for all six measured shapes, plus-before-minus public order, no
  partial result on failure, and identical post-processing inputs/results.
- Existing lookup-first CLI bytes, 1,000-SNV regression, model-routing specs,
  reference/mask formats, and lookup performance remain unchanged.
- Add ADR `architecture/decisions/0018-reference-alternate-batching.md` and
  retained evidence
  `planning/artifacts/022-reference-alternate-batching.md`. Update
  `AGENTS.md`, `README.md`, `architecture/README.md`,
  `architecture/design.md`, `architecture/runtime-data.md`,
  `planning/faq.md`, and `planning/frontier.md` with the selected
  representation, honest one-host measurements, retained limitations, and the
  next single outcome. Do not draft the next ticket.

## Success Checklist

- The singleton, zero-padded batch, and paired-strand batch candidates are
  reproducible, distinctly identified, size-bounded, and compared under the
  same assets, affinity, policies, cases, warmups, and samples.
- Every candidate passes 36 raw evaluations/45,756 scalar comparisons within
  `1e-5` and all 14 public cases/21 records exactly before performance counts.
- Retained evidence reports all rotated candidate raw JSON, session
  invocations, logical contexts, batch sizes, padding, sizes, open cost, RSS,
  timings, drift check, eligibility arithmetic, winner, and selected reruns.
- The mechanically selected representation is adopted only if it materially
  improves M09 and M10 without a material regression elsewhere; otherwise the
  singleton graph remains.
- Losing runtime/fixture cruft is removed while the evidence remains
  reproducible and the old accepted singleton bundle remains readable.
- Fast tests prove batch/shape/resource/error/order behavior without production
  assets or timing gates.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Compare representations that handle real indels.** A raw dynamic batch
   axis still requires equal tensor shapes. Zero-padding plus output slicing
   and independent paired inputs are the two compared one-session ways to
   handle unequal reference/alternate lengths.
2. **Use one session and one CPU budget.** Multiple sessions can hide latency
   by duplicating model memory and consuming concurrency needed by the future
   HTTP service. That is a service scheduling question, not this graph-format
   experiment.
3. **Accuracy precedes speed.** Independent raw-array qualification and exact
   public compatibility must pass before any timing is eligible.
4. **Select across representative shapes, not one lucky variant.** Equal-length,
   insertion, deletion, one-strand, and two-strand cases prevent a
   fixture-specific winner.
5. **Retain reproducibility, delete production cruft.** Maintainer converter
   modes and the ignored comparison harness remain; losing checked fixtures
   and runtime dispatch do not.

## Dependencies

- Ticket 018 authenticated singleton ONNX model and independent evidence.
- Ticket 019 exact variant-level scorer and 14-case compatibility corpus.
- Ticket 021 complete-request CPU policy and physical-core baseline.

## Notes

- Existing production assets are read-only inputs. Candidate outputs go only
  to the new absent Ticket 022 scratch root and are never published here.
- The converter environment remains CPython 3.13.5, PyTorch 2.7.1+cpu, NumPy
  2.5.1, and ONNX 1.19.1. Source/checkpoint identities remain unchanged.
- Production candidate creation and measurements are coordinator-owned. The
  developer implements and tests the reproducible machinery without opening
  production assets, then the same developer resumes after measurement to
  adopt the mechanical winner.

## Coordinator Authorship

Coordinator: Codex `/root`

Drafted from the shipped Ticket 021 evidence, current converter/model/scorer
code, the exact context-length behavior for supported indels, and the rolling
frontier.

## Independent Ticket Review

Reviewer: Codex sub-agent `/root/ticket022_design_review`

Initial review accepted the accuracy oracle and six-shape scope but rejected
six design details. The coordinator:

- added zero-padded all-context batching after the reviewer identified that
  Pangolin's cropped receptive field may preserve exact original outputs;
- gave paired inputs independent reference/alternate dimension symbols and
  separated exact legacy v1 parsing from the closed v2 graph contract;
- made parallel `1/8` diagnostic-only;
- replaced one-process timing with three rotated fresh-process rounds and
  mechanical drift handling;
- made the developer/coordinator/adoption stages and retained reproducibility
  boundary explicit; and
- separated ONNX invocations, logical contexts, batch sizes, and padding.

Re-review found four remaining ambiguities. The coordinator removed stale
equal-length candidate text, defined median-of-three latency and
maximum-of-three RSS aggregation/ties, applied drift protection to all six
cases, and added zero-padded `B=4` plus `B>4` and exact invocation/item grouping
coverage.

Final review: **ACCEPT**. The reviewer found the revised candidate set,
v1/v2 contracts, diagnostic boundary, selection arithmetic, accuracy proof,
resource metrics, staged ownership, and reproducibility plan technically
valid, bounded, and ready for implementation.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

This ticket creates only local reproducible experiment assets and performs no
download, upload, release, activation, or publication.

## Coordinator Final Check

Coordinator: pending
