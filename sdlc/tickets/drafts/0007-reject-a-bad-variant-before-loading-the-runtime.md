---
flow: build
priority: 4
---
# A variant rejected on its own terms fails before the runtime is loaded

Refusing bad input costs as much as scoring good input, because the model and
reference are opened before the submitted variant is examined.

Measured on the retained Ryzen host, with the assets installed and warm:

```
5.41s  allele longer than 100 bases   (refused)
5.55s  REF does not match GRCh38      (refused)
5.40s  a variant that is scored        (full inference)
0.01s  a variant answered from cache
0.01s  a variant answered from the precomputed index
```

Three model rejections are decided by the submitted variant alone: an unsupported unanchored insertion or deletion shape, an allele longer than 100 bases, and a position that cannot supply the fixed 5,050-base left context. They need no model, reference, or mask. A caller waits five seconds to learn one of these facts.

The lookup path is the fastest thing this project has, and its first impression
for anyone trying the command with a typo is that the tool is slow. The service
opens the profile once and does not show this; the command pays it every run.

Checks that need the reference stay where they are. They include the REF comparison, unsupported symbols in the retrieved window, and insufficient context at the right end of a contig. Gene-mask and model checks also remain unchanged.

The engine remains the single owner of model eligibility and rejection precedence. It exposes one request-only validation boundary and uses that same boundary inside scoring before provider access. The CLI calls the shared boundary before it admits or opens model runtime assets. It does not copy the rules.

The CLI validates every model-bound variant and reports the first request-only rejection in input order before it opens runtime assets. Request-only rejection therefore takes precedence over an asset or reference-dependent failure elsewhere in the same batch. This changes cross-layer error precedence deliberately. Stopping preflight at the first unresolved item would preserve that precedence but would leave later cheap mistakes paying the full startup cost. Input order among request-only rejections remains unchanged, and the batch still emits no partial output.

`--model-only` validates every submitted variant before runtime admission. Lookup-first operation may still open the SNV bundle to route the request. It validates only variants that require the model before opening reference, mask, or model assets. An authoritative SNV remains eligible for precomputed lookup even when its position would lack model context.

Issue: `sdlc/issues/2026-09-04-reject-cheap-input-errors-before-loading-the-model.md`

Done, observably:

- Unsupported shape, an allele longer than 100 bases, and insufficient fixed left context are refused without reference, mask, or model assets being admitted or opened.
- Such a refusal reports the same code and message it reports today, and the
  command exits as it does today.
- REF mismatch, unsupported retrieved reference symbols, and insufficient right context are still judged through the reference provider and retain their current result.
- The engine's early validator and scoring path return the same rejection type and precedence for every request-only rejection.
- A batch containing several request-only rejections reports the first one in input order and yields no partial results. A request-only rejection takes precedence over runtime admission and later provider work.
- `--model-only` proves the early refusal without runtime admission. Lookup-first proves that it can open the SNV bundle but opens no model-side asset for an early model rejection. An authoritative SNV still completes through lookup without model-context validation.
- `architecture/design.md` describes the shared validation and runtime-opening order. `spec/model-routing.md` pins the observable CLI behavior with a case that fails before the change.

Boundary: do not change which variants are refused or what any refusal says. Do not move the reference-dependent checks. Do not change request-only rejection order, the precomputed, cached, or successful scoring paths, the batch atomicity guarantee, or anything the HTTP service does. The service opens the profile once at startup and is unaffected. The earlier precedence of asset and provider failures over a request-only rejection is intentionally superseded for the CLI.
