# The model route has more precision than it reports

Status: deferred by the two-route precision rule

## 2026-09-15 superseding scope decision

Ian requires the SNV index and the model route to report the same score precision. If either route cannot supply a verified third decimal, both routes keep two decimals. A model-only third digit is out of scope.

The official Zenodo v1 archive completed the 4,099,255,665-row source build recorded in [`003-full-index-build.md`](../artifacts/003-full-index-build.md). Its score parser in [`pangopup-build/src/snv.rs`](../../crates/pangopup-build/src/snv.rs) rejects more than two fractional digits. It does not truncate them. The certified source and decoded bundle have the same logical digest. These records support exact hundredths in the official v1 dataset and no accidental loss of a third digit in the index builder. An independently verified source row with a third score digit would reopen this finding.

The model retains finer internal values, but that does not meet Ian's two-route condition. Keep the current hundredth score type, output, cache, CLI, and HTTP contract. This defers the precision draft; it does not block 0.5.

## Observation

Scores are reported in hundredths. The published corpus contains hundredths, so
the lookup route cannot do better. The model route can. It produces a float and
this project rounds it away.

[`crates/pangopup-engine/src/lib.rs:1255`](../../crates/pangopup-engine/src/lib.rs):

```rust
fn gain_hundredths(self) -> Result<u16, ModelScoringError> {
    checked_gain((self * 100.0_f32).round_ties_even() as f64)
}
```

`loss_hundredths` at line 1259 does the same. Both are called at lines 1308 and
1311, after `apply_mask` and after the extremum is selected. The full-precision
value exists in the window array until that call.

Ticket 0059 recorded the same mechanism from the other side: "PangoPup finds the
extremum of the raw 101-value window array and rounds afterwards, and the
published dataset appears to round the array to hundredths first"
([`planning/artifacts/0059-route-disagreement-rate.md`](../artifacts/0059-route-disagreement-rate.md)).

## Why it matters

A consumer applying a decision threshold between two hundredths cannot act on a
hundredth. A threshold at 0.105 separates 0.104 from 0.106, and both arrive as
0.10 and 0.11 with the ordering preserved but the distance destroyed.

The rounding exists to make the two routes report the same granularity. That is
a real reason. It is also the reason the extra precision is unavailable on the
route that has it.

## Earlier escalation idea, now out of scope

The earlier idea was to re-run the model near a caller-stated threshold and
return an unrounded value. It would produce different reported precision by
route. Ian's superseding decision rules it out.

Three properties make this cheap. The band is narrow, so few variants trigger
it. The model result cache
([ADR 0019](../../architecture/decisions/0019-persistent-model-result-cache.md))
means a triggered variant costs 4.3 seconds once and 0.7 milliseconds after. The
lookup-first routing already exists
([ADR 0016](../../architecture/decisions/0016-lookup-first-cli-model-routing.md)).

Ticket 0059 supplies limited route-comparison evidence. Over 2,790
compared gene records the two routes disagreed on value in 2 records, 0.07
percent, and in 0.52 percent of the 381 records carrying a non-zero score on at
least one route. The routes agree at hundredths granularity, so a precomputed
0.11 is a reliable signal that the model's exact value is near 0.11.

That measurement covers transversions only and was taken once on one host
against v0.5.0 assets. 0059 states both limits.

## What is not established

**Reproducibility across execution providers.** ONNX CPU inference is
deterministic for a given build and host. A different provider, a different ONNX
Runtime version, or a different host can move the low float bits. Whether it
moves the third decimal has not been measured. This intersects the accelerator
work in
[`2026-09-11-model-throughput-on-accelerators.md`](2026-09-11-model-throughput-on-accelerators.md),
and one measurement can answer both.

**Whether the third digit means anything.** Reporting 0.106 reproducibly is a
different claim from 0.106 and 0.104 supporting different conclusions. Pangolin
was not calibrated to that resolution as far as this repository has established,
and no source here says it was. A consumer setting a threshold at that
resolution needs literature support, and none has been checked.

**The output contract.** Scores are currently a typed magnitude in hundredths as
a `u16`. Carrying a finer value changes the public type, the serialized output,
and the compatibility statement in
[`architecture/compatibility.md`](../../architecture/compatibility.md). A record
must also say which route produced it and at what precision, because a
hundredths value and a thousandths value must never be compared as if they were
the same measurement.

**Position offsets are separate.** 0059 measured 4.49 percent position
disagreement, 17 of 379 comparable records, and attributed it to the rounding
order rather than to the model. Finer scores do not fix positions and may expose
that difference more often.

## Preconditions before any precision ticket

- A verified three-decimal score source that can populate a complete SNV index.
- Measured cross-provider and cross-host agreement at the proposed precision,
  with a stated tolerance decided in advance.
- A literature check on whether Pangolin scores are meaningful below hundredths,
  per the clinical-questions rule in the workspace instructions. If no source
  supports it, the finer value ships as a reported number with that limit stated
  and not as a recommended threshold resolution.
- The contract change for both routes together, including core score types,
  cache, CLI, HTTP, and exact comparison behavior.
