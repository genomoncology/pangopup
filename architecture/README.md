# Pangopup Architecture

Pangopup's target combines exact published Pangolin SNV lookup with compatible
model inference. The shipped functional runtime answers GRCh38 SNV queries from
the Wagner/Neverov precomputed dataset through a fixed 11-byte mmap index and
typed CLI, plus Linux local installation, active-bundle discovery, the
immutable public `snv-grch38-v1` release, and pinned resumable remote sync.
The checked `pangopup-compat-v1` oracle now fixes upstream model and
post-processing behavior. Model fallback and HTTP remain future work on the
same standalone Rust core.

Reference payload selection remains isolated behind three benchmark-only
codecs. The selected `acgt2-rle-v1` payload now has a separate production
`PGRREF01` bundle, authenticated builder, cheap-open mmap reader, and typed
caller-buffer provider.

GENCODE masking is at a different boundary. Ticket 012 authenticated an exact
ordered GENCODE v38 logical source and compared three private `PGMBEN01`
candidate layouts. The retained full-source run covers 60,649 genes and 88,202
constant-membership domains. It selected `domains` at the first p95 speed step
after all candidates passed exhaustive semantic and corruption controls.
ADR 0013 promotes those exact selected bytes behind the domains-only
`pangopup_index::mask` production provider, superseding ADR 0011's requirement
for a separately renamed format. There is no mask delivery asset yet; the
alternate codecs and qualification path remain historical evidence.

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
