---
base: 6972263e49d54774773283686d560dafe4110a2c
head: aef16a856aa7edf135cc7718ea085e77410f7816
---

# Every item carries the string that produced it

Every `/v1/score` item now carries `input` with the exact submitted string. Found results, cache hits, model rejections, invalid values, contig aliases, RefSeq accessions, exact edits, and duplicate strings all preserve the caller's text. Existing fields keep their relative order and meaning. The scoring identity remains last.

A downstream consumer can deduplicate exact query strings and correlate each result through one input-to-callers mapping. Duplicate occurrences remain ordered and do not gain a synthetic identifier. Consumers must retain response count, membership, duplicate, type, and shape validation.

Independent design and code reviews accepted the change without findings. Focused package tests, Clippy with warnings denied, HTTP and README specifications, formatting, the README size gate, and `git diff --check` passed.
