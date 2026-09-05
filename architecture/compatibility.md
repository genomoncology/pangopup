# Compatibility

## Structured JSON score records

`stable_gene` adds one property to every structured JSON score record from the command-line JSONL and HTTP scoring routes. This addition changes the score-record object shape. It does not change scores, statuses, routing, ordering, provenance, or the human-readable table.

Permissive JSON readers that ignore unknown properties remain compatible. Strict readers that reject unknown properties must accept `stable_gene` before deploying a PangoPup application revision that emits it. This consumer-first order prevents a strict reader from rejecting an otherwise valid record during an application upgrade.

`stable_gene` is the stable Ensembl grouping and filter key. `gene` remains the source-reported identity. Consumers must retain `gene` when exact version or PAR identity matters.
