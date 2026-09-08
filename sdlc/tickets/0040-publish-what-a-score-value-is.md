---
flow: build
priority: 3
---
# Publish what a score value is

A consumer cannot cite what a PangoPup score value is without reading Rust. `ScoreMagnitude` is a `u8` constrained to 0 through 100 hundredths (`crates/pangopup-core/src/lib.rs:311-320`) and renders as `{}.{:02}` (line 326). Modeled scores reach that representation through `round_ties_even` on the value multiplied by 100 (`crates/pangopup-engine/src/lib.rs:1256-1268`). A search of `spec/`, `architecture/` and `README.md` for `hundredth`, `round` and `ties` returns no statement of any of it.

So the published contract describes the transport of a score and never its value. `spec/http-service.md` gives the field, the shape and the identity that produced it. It does not say the value space is 101 discrete points, that the rounding is ties-to-even rather than half-up, or that a caller receiving `0.35` is receiving an exact decimal rather than a rounded float.

A consumer building a clinical decision rule on a published threshold must record what it compared against. A threshold expressed more finely than one hundredth cannot be applied to this value at all: the nearest representable neighbours of 0.106 are 0.10 and 0.11, and no PangoPup response can distinguish them. That is a legitimate property of the service and it is currently discoverable only by reading source.

The same question covers reproducibility. `crates/pangopup-assets/src/active_identity.rs:12-17` folds `effective_cpu_policy` into the scoring identity, which implies a deployment's worker and thread settings might change a result. Nothing states whether they can. Rounding to hundredths plausibly absorbs any floating-point variation a thread count introduces, and "plausibly" is not a property a clinical record can cite.

Done, observably:

- The published specification states the score value space: discrete hundredths from 0.00 through 1.00 inclusive, and no value between two of them.
- It states the rounding mode by name, states that ties round to even, and gives a worked example that distinguishes ties-to-even from half-up.
- It states the rendered form, including that a value always carries exactly two decimal places.
- It states that a threshold finer than one hundredth cannot be evaluated against a PangoPup score, and names the two representable neighbours of such a threshold as the only comparisons available.
- It states whether the precomputed route and the model route produce the same value for the same variant, or names the conditions under which they differ.
- It states whether a modeled score is identical across CPU policies, worker counts and thread counts. If it is, a test proves it by scoring the same variants under at least two different policies and comparing exact rendered output. If it is not, the specification names the variation a consumer must expect and the test pins that bound instead.
- A consumer can cite a document rather than a source line for every sentence above.
- `make lint`, `make test` and `make spec` pass without reducing specification coverage.

Boundary: this ticket publishes the behavior that exists and adds one determinism test. It must not change a score, a rounding mode, a value space, a rendered form, a field, a limit, a code, or the scoring identity. If the determinism test finds that scores vary across CPU policies, that finding is recorded and reported; repairing it belongs to its own ticket.
