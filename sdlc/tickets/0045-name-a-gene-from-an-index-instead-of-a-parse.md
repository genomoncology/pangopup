---
flow: build
priority: 2
---
# Name a gene from an index instead of a parse

Naming costs a command-line lookup twenty-five times its own runtime. `run_lookup_with_runtime_opener` opens the installed naming source as soon as it resolves a data root, and `NamingSource::parse` reads all 16,903,161 bytes of the release and builds a map of every accession before the command knows whether a single record needs a name. Measured on 2026-09-08 with a release build and a warm page cache, `pangopup lookup --variant GRCh38:chr7:140753336:A:T` takes 6 ms with no naming source installed and 155 ms with one installed.

The repository already answers this shape of question at the right speed. The reference index opens `PGRREF01` through a memory map and reads only the bytes a query touches (`crates/pangopup-index/src/reference_reader.rs`). Naming reads everything and keeps nothing.

Naming also reaches fewer genes than the available sources support. HGNC alone names 41,954 of the mask's 60,605 scoreable accessions. NCBI's `Homo_sapiens.gene_info` carries an Ensembl crosslink for 38,267 accessions, and 2,586 of those are scoreable genes HGNC does not reach. Together the two sources name 44,540 scoreable accessions and leave 16,065 unnamed, against 18,651 unnamed today.

Ticket 0039 evaluated NCBI as a second source and rejected it, on the ground that one source carries the work and a second introduces conflicts needing an arbitration rule. Ian reversed that on 2026-09-08. The conflicts are counted below and the arbitration rule is settled here.

The two sources are pinned differently and the difference is not fixable. HGNC publishes immutable dated monthly releases. NCBI replaces `Homo_sapiens.gene_info.gz` at a fixed URL, most recently 2026-09-08 at 5,180,589 bytes, and publishes no dated archive of it. A digest recorded today cannot be re-fetched tomorrow. So the built index is the durable artifact and the repository carries it.

Ian settled these choices on 2026-09-08.

Both sources contribute. HGNC is the naming authority. Where HGNC names an accession, HGNC supplies that accession's whole record and NCBI is not consulted for it. NCBI names only accessions HGNC does not reach. Field-level merging is not done, so the 105 accessions where the two sources report different NCBI Gene identifiers and the 125 where they report different symbols never need arbitration.

A record states which source named it. A gene named by NCBI carries no HGNC identifier, because none exists, and its symbol is not HGNC-approved. A consumer cannot tell those two cases apart without being told.

An accession carrying more than one record in the source that names it reports no name. This extends ticket 0039's rule to NCBI, which holds 261 such accessions against HGNC's 3.

The built index ships in the repository. The two source downloads do not. The index is derived, an order of magnitude smaller than its inputs, and it is the only form that survives NCBI's rolling publication.

Installing a naming source separately retires. The index ships with the build, so naming is always available and a deployment cannot vary it. This deletes `pangopup assets naming install`, the `naming.available: false` state and the installed-release reporting that ticket 0039 added, none of which is published, because the v0.5 pins still name v0.4.1. The accepted cost is that a consumer refreshes gene names by upgrading PangoPup and cannot refresh them alone. Names change slowly and the repository already ships dated releases. Ian can overturn this and keep both paths.

That choice closes draft 0044. One build carries one naming vintage, and `data_set_version` already hashes the software version, so a retained record's labels are traceable through a value a consumer is told to store.

This ticket supersedes draft 0042.

Done, observably:

- A single-variant `pangopup lookup` that renders gene names costs no more than 1 ms above the same lookup with naming unavailable. A committed benchmark measures both and fails when the difference exceeds the budget.
- Resolving every gene name a single-variant lookup returns costs under 1 ms, measured from a cold process against the shipped index.
- The command-line tool and the service read the same index and report the same names for the same accession.
- The index reaches every gene both sources name. A test pins one gene named only by NCBI, one named by HGNC where NCBI disagrees, and one the sources leave unnamed.
- A score record states which source named its gene. A gene named by NCBI reports no HGNC identifier rather than an empty one.
- An accession carrying more than one record in its naming source reports no name. A test pins one such accession from each source.
- Previous and alias symbols survive from both sources, and the published documentation still states that both are ambiguous and must never be the sole basis for an automated match.
- `make` carries a target that downloads both sources, builds the index, writes it to its committed path, and reports the source digests and row counts it built from. The target reaches the network and no gate invokes it.
- Rebuilding the index from identical source bytes produces identical index bytes. A test proves the builder is deterministic without reaching the network.
- The repository records, beside the committed index, the two source releases it was built from, their byte counts and their digests. The record states that the NCBI source is not re-fetchable at those bytes.
- `NOTICE` records the NCBI source, its publisher and its terms, beside the HGNC and GENCODE entries already there.
- No published identity moves when the index changes. `scoring_identity`, `data_set_version` and `runtime_profile_id` are unchanged by a naming refresh, and a test proves it with the software version held fixed.
- The v0.5 response-shape inventory names every field this work adds or removes, and the version gate and its independent portable copy both enforce the new entries.
- `make lint`, `make test` and `make spec` pass without reducing specification coverage.

Boundary: this ticket changes how a name is stored, found and sourced. It must not change any score, position, rejection code, rejection reason, request limit, or which genes a variant returns. It must not make a name a filter, a lookup key or a request parameter; `gene_filter` continues to accept exactly the three forms `/v1/status` publishes. It must not change `mask_sha256`, the model cache key, or invalidate a cached model result. It must not add a network fetch to any gate, to scoring time, or to service start. It must not publish a release or move a published artifact pin. Reference and SNV index formats stay as they are; this work adds its own and reuses neither.
