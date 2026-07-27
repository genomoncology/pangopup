# Pangopup Architecture

Pangopup's target combines exact published Pangolin SNV lookup with compatible
model inference. The shipped functional runtime answers GRCh38 SNV queries from
the Wagner/Neverov precomputed dataset through a fixed 11-byte mmap index and
typed CLI, plus Linux local installation, active-bundle discovery, the
immutable public `snv-grch38-v1` release, and pinned resumable remote sync.
The checked `pangopup-compat-v1` oracle now fixes upstream model and
post-processing behavior. The shipped authenticated CPU kernel now executes
the twelve raw selected Pangolin channels through one ONNX Runtime session.
The shipped `pangopup-engine` composition now constructs supported literal
GRCh38 variants, preserves compatible ensemble/indel/masking arithmetic, and
routes authoritative lookup or explicit-path model fallback into ordered exact
CLI results. Complete-request CPU qualification keeps the portable ordinary
session at sequential `1/1` while retaining fixed `8/1` as this host's measured
frontier result. The first zero-padded and paired-strand batching run is
ineligible because the v2 exporter omitted its declared dynamic axes. The
corrected full experiment retained singleton through both the drift and
replacement gates. Persistent SQLite model-result reuse and the canonical
four-asset compatibility profile are established. Coherent installation,
activation, delivery, and HTTP remain future work.

The closed three-codec reference comparison selected `acgt2-rle-v1`; its
candidate modules, miniature, benchmark executable, and CLI have been removed
from the compiled workspace. Retained reports and decisions preserve that
historical selection evidence. The separate production `PGRREF01` bundle,
authenticated builder, cheap-open mmap reader, and typed caller-buffer provider
are the current compiled GRCh38 sequence-index implementation.

GENCODE masking is at a different boundary. Ticket 012 authenticated an exact
ordered GENCODE v38 logical source and compared three private `PGMBEN01`
candidate layouts. The retained full-source run covers 60,649 genes and 88,202
constant-membership domains. It selected `domains` at the first p95 speed step
after all candidates passed exhaustive semantic and corruption controls.
ADR 0013 promotes those exact selected bytes behind the domains-only
`pangopup_index::mask` production provider, superseding ADR 0011's requirement
for a separately renamed format. There is no mask delivery asset yet; the
alternate-codec and qualification results remain in durable historical
evidence, while their one-time source and executable surfaces are no longer
compiled.

SNV and production-reference construction now have separate, artifact-local
builder provenance. The checked source/dependency evidence is compiled into
the builder, unrelated subsystems do not churn either identity, and existing
v1 assets carrying the former repository-wide fingerprint remain valid.
ADR 0012 defines that descriptive provenance boundary.

## Boundaries

- [`design.md`](design.md) — typed API, crate ownership, lookup flow, and scope.
- [`index.md`](index.md) — candidate index shape, build invariants, validation,
  and performance method.
- [`source-data.md`](source-data.md) — dataset identity, observed properties,
  reference evidence, and CC BY obligations.
- [`runtime-data.md`](runtime-data.md) — the exact local assets needed for
  standalone lookup and model fallback.
- [`reference.md`](reference.md) — production reference format, build,
  integrity, and provider contracts.
- [`decisions/0008-strict-upstream-compatibility-profile.md`](decisions/0008-strict-upstream-compatibility-profile.md)
  — the frozen source/model/numeric profile and order-sensitive replay policy.
- [`decisions/0009-reference-format-selection.md`](decisions/0009-reference-format-selection.md)
  — the accepted two-bit/ambiguity-run reference payload selection.
- [`decisions/0010-production-reference-bundle.md`](decisions/0010-production-reference-bundle.md)
  — the production container, provider, and integrity policy.
- [`decisions/0011-gencode-mask-format-selection.md`](decisions/0011-gencode-mask-format-selection.md)
  — the accepted constant-membership domain mask representation selection.
- [`decisions/0012-artifact-specific-builder-provenance.md`](decisions/0012-artifact-specific-builder-provenance.md)
  — separate causal builder identities for future SNV and reference artifacts.
- [`decisions/0013-byte-identical-gencode-mask-promotion.md`](decisions/0013-byte-identical-gencode-mask-promotion.md)
  — the selected mask member's domains-only production runtime boundary.
- [`decisions/0014-authenticated-onnx-cpu-kernel.md`](decisions/0014-authenticated-onnx-cpu-kernel.md)
  — the authenticated combined ONNX representation, independent qualification,
  and single-owner CPU kernel boundary.
- [`decisions/0015-variant-level-model-scoring.md`](decisions/0015-variant-level-model-scoring.md)
  — the literal request boundary, provider composition, dtype-aware
  post-processing, and ordered masking contract.
- [`decisions/0016-lookup-first-cli-model-routing.md`](decisions/0016-lookup-first-cli-model-routing.md)
  — authoritative lookup, lazy identity-bound fallback, and stable modeled CLI output.
- [`decisions/0017-measured-cpu-policy.md`](decisions/0017-measured-cpu-policy.md)
  — complete-request CPU measurement, host frontier, and portable ordinary default.
- [`decisions/0018-reference-alternate-batching.md`](decisions/0018-reference-alternate-batching.md)
  — the corrected exactness/performance experiment that retained singleton.
- [`decisions/0019-persistent-model-result-cache.md`](decisions/0019-persistent-model-result-cache.md)
  — persistent exact model results in bounded disposable SQLite.
- [`decisions/0020-four-asset-runtime-profile.md`](decisions/0020-four-asset-runtime-profile.md)
  — canonical path-free binding of the exact compatible runtime tuple.
- [`delivery.md`](delivery.md) — release assets, installation, and immutable
  bundles.
- [`decisions/0007-deterministic-snv-transport.md`](decisions/0007-deterministic-snv-transport.md)
  — accepted no-tar transport, deterministic codec boundary, and verification
  layers.
- [`service.md`](service.md) — planned lookup-first HTTP boundary, foreground
  lifecycle, deployment, and operational proof.
- [`decisions/`](decisions/) — accepted cross-cutting decisions.

Current work, unresolved priorities, and hypotheses belong in
[`../planning/`](../planning/). Observable CLI behavior belongs in
[`../spec/`](../spec/).
