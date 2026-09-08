---
---
# A retained score record does not carry its gene-name vintage

v0.5.0 adds `gene_names` to every structured score record and to every source-reference ambiguity. The object carries `symbol`, `hgnc_id`, `ncbi_gene_id`, `prev_symbols` and `alias_symbols`. Those labels come from the installed naming source, which a maintainer installs separately with `pangopup assets naming install --source <FILE>`.

The naming source is not a member of the runtime profile (`crates/pangopup-assets/src/runtime_profile.rs`). The service opens it after the profile, keeps it in `AppState.names`, and leaves it out of the identity preimage on purpose (`crates/pangopup-cli/src/service.rs:481-483`). Ticket 0041 published `data_set_version` over the software version and the runtime profile identity, so the naming release enters neither published value.

So two deployments of one PangoPup release on one asset set can install two naming vintages, return different `symbol` and `prev_symbols` for the same variant, and report one `data_set_version` and one `scoring_identity`. A consumer that retains a score record and its `data_set_version` cannot later tell which vintage produced the labels it stored.

The service already publishes the vintage. `/v1/status` reports `naming.release` beside `naming.available`. No published document tells a consumer to retain it. `README.md` and `architecture/compatibility.md` name `data_set_version` as the value to store and say nothing about the label vintage.

Two shapes settle it. Publish the naming release beside each score record's `gene_names`, so a receipt stands on its own the way the 0041 provenance work asked. Or state in `architecture/compatibility.md` that `data_set_version` covers scores and not labels, and tell a consumer to retain `naming.release` alongside it.

Ticket 0041 scoped itself to inputs that can change a score. Gene names are labels and change no score, so this gap sat outside it.
