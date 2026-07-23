# Goals

## Proposed program goal

Deliver Pangopup as a standalone, high-performance splice-scoring service that
returns exact published scores for covered GRCh38 SNVs through a memory-mapped
lookup, falls back to a pinned Pangolin-compatible model for supported misses
and non-SNVs, and can be installed and operated from reproducible, verified,
license-complete release assets without another application or database.

The program is complete when:

- an ordinary user can install a released executable and its pinned assets on
  a clean supported machine, verify them, work offline after installation, and
  reproduce the documented lookup and inference results;
- SNV hits remain exact and lookup-first, with measured latency, memory/page
  behavior, and transport size retained as release evidence;
- a retained upstream-derived corpus proves supported CPU model inference,
  masking, errors, and numeric tolerances against the pinned Pangolin version;
- model weights, compact GRCh38 sequence, and compact GENCODE masking data have
  pinned identities, reproducible builders or conversions, checksums, notices,
  and compatibility rules;
- one typed routing API reports whether each result came from precomputed data
  or model inference, and supported lookup misses/non-SNVs fail or score
  deterministically rather than silently changing semantics;
- the foreground HTTP service exposes stable batch JSON, readiness, liveness,
  and status behavior; Docker and native service-manager examples prove clean
  startup, shutdown, restart, and read-only asset use;
- caching is either omitted with retained evidence that it does not help, or is
  bounded, identity-safe, concurrency-safe, and justified by measured repeated
  model inference;
- release automation publishes immutable, separately versioned executable,
  lookup, model, reference, and mask assets with provenance, checksums,
  attribution, security metadata, and a clean-machine acceptance proof; and
- user, operator, architecture, and contributor documentation describes the
  shipped system accurately and the complete lint/test/spec gate is green.

The program advances through small dependency-ordered tickets, not a frozen
backlog. The coordinator writes one ticket at a time after reconciling the
previous shipped outcome with the rolling frontier. Three distinct sub-agents
then ticket-review, implement, and code-review it. Findings return through the
coordinator/reviewer pair for planning or developer/code-reviewer pair for
product work. The coordinator records evidence, runs final gates, and commits
and pushes approved outcomes. Documentation is named in each ticket and passes
through implementation, code review, and the coordinator's final stale-claim
check with the code.

A material final-gate or stale-documentation finding returns to the same
developer and code reviewer. If that finding exposes defective scope rather
than implementation, it returns to the coordinator and same ticket reviewer.
Developers never commit or push.

## Durable outcomes

1. **Exact, gene-aware SNV annotation.** A GRCh38 genomic SNV returns every
   matching source-gene record with the exact published masked gain/loss scores
   and positions and no floating-point drift.
2. **Performance, then memory, then download size.** Runtime touches only small
   directory and payload regions. Format choice first minimizes measured query
   latency and work, then resident memory, then compressed transport size.
3. **Reproducible artifacts.** Rust builders stream pinned inputs, prove their
   invariants, write deterministically, certify outputs, and record enough
   provenance to reproduce them.
4. **Standalone typed service core.** CLI and HTTP adapters share one Rust API;
   lookup and model fallback require no external application, database, or
   network service.
5. **Compatible model fallback.** Lookup misses and supported non-SNV variants
   run through versioned model, reference, and masking assets with measured
   parity against the upstream implementation.
6. **Operationally simple delivery.** Immutable executable, lookup, model,
   reference, and masking assets are separately versioned, downloaded
   automatically when missing, verified, installed atomically, and opened once.
7. **License-complete packaging.** GPL source/model obligations and the score
   dataset's CC BY attribution are explicit, separate, and retained in every
   applicable release artifact.

## Permanent non-goals

- GRCh37/liftover;
- clinical classification thresholds;
- HGVS parsing, transcript/protein projection, and general gene annotation;
- gene descriptions, aliases, disease knowledge, or clinical interpretation;
- an embedded relational transcript/reference database;
- an internal daemon supervisor: `pangopup serve` runs in the foreground while
  Docker, systemd, or another external manager owns start/stop/restart.

Application-level caching is not a goal by itself. It becomes implementation
scope only if end-to-end model measurements show a repeat workload worth the
memory, invalidation, and operational complexity.
