# 0017 — Complete-request CPU policy selection

Status: accepted

## Decision

Keep one compiled CPU policy for ordinary `ModelKernel::open` callers and one
typed low-level constructor for qualification. The retained measurement
selects affinity-aware sequential `auto/1` only if it clears the exactness,
memory, and improvement gates; otherwise ordinary callers retain sequential
`1/1`. On the retained host, `auto/1` failed the M10 improvement gate, so the
ordinary default remains sequential `1/1`. Pangopup does not expose thread
flags, environment configuration, a session pool, or a backend abstraction.

The retained Ticket 021 comparison qualifies exactly eight policies against
complete `VariantScorer` requests: sequential `auto/1`, fixed `1/1`, `2/1`,
`4/1`, and `8/1`, plus parallel `1/2`, `1/4`, and `1/8`. Every candidate must
reproduce the exact M09 one-strand and M10 two-strand public records. A
candidate may displace fixed `1/1` only when both p50 values improve by at
least 20 percent and high-water RSS stays within twice the baseline. Fixed
thread results characterize the measured host; only affinity-aware `auto/1`
may become the portable ordinary default.

The comparison measures authenticated component open separately from warmed
variant scoring. Each candidate runs in a fresh process pinned to one logical
CPU from each physical core. The retained artifact records p50, descriptive
p95, RSS, topology, runtime versions, asset identities, exactness, and the
mechanical selection.

Sequential fixed `8/1` was the measured host frontier winner, reducing the
worse complete-request p50 ratio to about 0.305 while staying within the RSS
bound. That fixed count is host characterization for later scheduling work,
not a portable ordinary default.

## Context

The original raw ONNX kernel deliberately used sequential `1/1`. Its retained
single-context measurements were useful diagnosis but not product latency:
one complete variant needs reference and alternate inference and may need both
strands. A fixed host-optimal thread count would also be a poor default on
machines with different CPU topology or process affinity.

ONNX Runtime applies intra-op threads within graph nodes. Inter-op threads only
apply to parallel graph execution. The bounded candidates therefore test those
two families separately instead of creating an oversubscribed Cartesian
search.

## Consequences

- `CpuPolicy` and `ModelKernel::open_with_cpu_policy` are public low-level
  library APIs so cross-crate qualification can be explicit and typed.
- Ordinary callers receive one compiled policy and cannot change it through
  CLI flags or process-global state.
- The model graph, optimization level `All`, CPU provider, authenticated
  descriptor-held loading, initialization probe, and public score semantics
  are unchanged.
- Host-specific fixed-policy results inform later service concurrency but do
  not become a portable default.
- Graph batching, request concurrency, pooling, caching, accelerators,
  quantization, and alternative runtimes remain separate evidence-gated work.
