# 0008 — Strict upstream Pangolin compatibility profile

Status: accepted

## Decision

Pangopup names its first model-compatibility boundary
`pangolin-1.0.2-5cf94b8-grch38-v1`. The checked
`tests/fixtures/pangolin-compat-v1` corpus is the acceptance oracle for that
profile. It binds the exact Pangolin source commit and twelve checkpoints,
RefSeq GRCh38.p14 sequence identity, GENCODE v38 annotation inputs, CPU numeric
environment, 14 scored genomic cases, six rejection cases, and four controlled
post-processing cases.

The environment identity separates two execution roles. The normative raw
array helper explicitly runs PyTorch intra-op/inter-op at `1/1`. The auxiliary
unmodified CLI witness runs CPU-only with `OMP_NUM_THREADS=1`; the pinned fresh
process reports PyTorch inter-op default `16`. Capture preflight probes both
roles independently. The CLI role authenticates public rendering and eligible
rejection observations; it is not the normative raw-array producer.

Capture authenticates the exact live GPL helper immediately before spawning
Python, rather than trusting only the helper bytes embedded at Rust build time.
It also authenticates every tracked Pangolin Python file executed by imports:
the package initializer, model module, and Pangolin module, plus packaging
metadata. A clean commit alone is not accepted as proof of live worktree bytes.

Raw post-ensemble arrays are retained as IEEE-754 bits with their observed
dtype. SNVs, equal-length MNVs, and anchored insertions use `f32`; upstream
deletion reconciliation promotes arrays to `f64`, which must not be narrowed.
Rust replay derives masking, first-index extrema, relative positions, and the
observed dtype-aware public rendering from those raw values.

The three controlled vector cases are independently fixed in Rust as complete
literal inputs, ordered genes/boundaries, and expected masked/unmasked results.
Replay is a second check; it cannot make a hash-rebound vector mutation valid by
deriving a matching expectation from the mutated bytes.

The strict profile also preserves upstream's observed same-strand behavior:
masking mutates an array in the recorded SQLite gene order, so a later
overlapping gene can see an earlier gene's mutations. A runtime claiming this
profile must pass that order-sensitive corpus. Independent per-gene masking may
be a useful corrected behavior, but it requires a separately named profile.

Normal lint, test, and spec gates run only the bounded Rust inspector. They do
not run Python, PyTorch, checkpoints, FASTA, GTF, SQLite, or a network request.
The capture command exists only to make the frozen evidence reproducible from
explicit pinned maintainer inputs; it is not a recurring verifier.

Capture publication cleans only its named unpublished sibling staging tree on
capture, staging-sync, or no-replace rename failure. Once atomic rename
succeeds, a parent-directory sync failure is reported as an I/O error but the
complete published corpus is preserved for inspection and recovery; it is no
longer staging and must not be deleted as cleanup.

## Consequences

- A future CPU model implementation has an independent exact source oracle
  before performance work or accelerator selection begins.
- Exact raw observations are preserved without prematurely requiring a future
  Rust tensor runtime to be bit-identical at every intermediate operation.
- Numeric dtype, signed zero, public rendering, overlap order, and rejection
  boundaries cannot drift behind rounded headline scores.
- A behavior correction or changed source/model/reference/mask identity creates
  a new profile instead of silently changing this one.
