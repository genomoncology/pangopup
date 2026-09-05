# PangoPup v0.4.0 candidate release notes

## Consumer deployment order

PangoPup v0.4.0 is the first application release that adds `stable_gene` to each structured JSON score record from command-line JSONL and HTTP scoring routes. Permissive readers that ignore unknown properties remain compatible. Strict readers that reject unknown properties require coordinated adoption.

Deploy strict consumer support for `stable_gene` before deploying PangoPup v0.4.0. Consumers can then use `stable_gene` for grouping and filtering. They must retain `gene` when exact version or PAR identity matters.
