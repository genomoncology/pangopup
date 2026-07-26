# 0018 — Reference/alternate graph batching

Status: accepted

## Experiment contract

Preserve the exact `pangopup-model-bundle-v1` singleton parser and canonical
bytes. Compare it with two distinctly identified
`pangopup-model-bundle-v2` candidates built from the same authenticated twelve
checkpoints:

- `zero-padded-batch`: one dynamic `[B,4,N]` input for two or four oriented
  contexts, right-padded with the zero encoding of `N`, followed by
  original-length output slicing;
- `paired-strand-batch`: independent reference and alternate inputs, each with
  a dynamic active-strand batch of one or two.

Both candidates use one ONNX Runtime session and one invocation per complete
supported variant. The model crate owns bounded encoding, graph shapes,
finite/range validation, output slicing, and genomic orientation. The engine
owns plus-before-minus grouping and all unchanged variant/post-processing
semantics.

Sequential `1/1` remains the portable default; fixed `8/1` is a host
qualification policy, not a new user setting.

## Decision

Retain the accepted singleton graph and its separate reference/alternate
invocations. Corrected zero-padded and paired candidates passed the complete
independent raw oracle and exact public corpus, stayed inside model-size and RSS
limits, and reduced each complete request to one ONNX invocation.

Neither policy comparison may select a replacement: singleton fresh-process
p50 drift exceeded 20 percent for M07/M08/M12 at sequential `1/1` and
M09/M10/M12/M13 at `8/1`. Independently, neither candidate improved both M09
and M10 by the required 20 percent without a greater-than-5-percent regression.
At `1/1`, paired was close to singleton but improved M09/M10 by less than two
percent. At `8/1`, zero padding materially regressed five cases and paired
materially regressed four. The paired parallel `1/8` diagnostic was slower.

Code review found that the v2 exporter was not given the dynamic axes claimed
by the candidate manifests; only final graph metadata was rewritten. The
first-run result remains explicitly ineligible historical evidence. Corrected
conversion passed the axes into PyTorch export, changed both candidate graph
and bundle identities, and repeated the complete qualification and measurement
matrix before this decision.

Ordinary model opening and scorer construction remain singleton-only.
Closed v2 contracts, maintainer converter modes, bounded candidate execution,
both tiny checked candidate fixtures, and the ignored comparison harness remain
to reproduce and safely exercise the experiment. The corrected and historical
arithmetic plus both sets of 19 raw JSON records are in
[`../../planning/artifacts/022-reference-alternate-batching.md`](../../planning/artifacts/022-reference-alternate-batching.md).

## Exclusions

This decision does not add multiple sessions, request concurrency, a pool,
cache, user thread/batch controls, accelerators, quantization, asset
publication, or HTTP.
