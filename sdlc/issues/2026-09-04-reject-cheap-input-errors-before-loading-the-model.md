# Reject cheap input errors before loading the model

## Observation

An allele longer than 100 bases is a string-length check. `pangopup lookup` takes 5.4 s to report it, because the model and reference load first.

```
5.55s  lookup --variant GRCh38:chr12:6801305:G:GA          (REF does not match GRCh38)
5.41s  lookup --variant GRCh38:chr12:6801303:G:G<102 bases> (allele length)
5.40s  lookup --variant GRCh38:chr12:6801313:G:GT           (accepted, full inference)
0.01s  lookup --variant GRCh38:chr12:6801303:G:GA           (cached)
0.01s  lookup --variant GRCh38:chr12:6801301:G:A            (precomputed)
```

Rejecting bad input costs the same as scoring good input. The length limit lives in `crates/pangopup-core/src/lib.rs:592`, after the profile is open.

## Why this matters

Scoring is fast and the lookup path is very fast. The fixed setup cost is invisible to anyone using `serve`, which opens the profile once, and unavoidable for anyone using the CLI per variant. A typo costing the same as a real query is a poor first impression for a tool whose headline is speed.

The REF check needs the reference bundle and belongs where it is. The allele-length check needs nothing.

## Suggested direction

Move the checks that read only the submitted variant ahead of profile loading, and keep the reference-dependent checks where they are. This changes when a batch fails, not whether it fails, so the no-partial-result guarantee is unaffected.
