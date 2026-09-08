---
flow: build
priority: 4
---
# Name every gene a score belongs to

A result identifies its gene only by Ensembl accession. A repository-wide, case-insensitive search of `crates/` for `gene_name`, `gene_symbol` and `hgnc` returns no match, and `MaskQueryGene` (`crates/pangopup-index/src/mask.rs:66-93`) exposes `identity`, `stable_identity`, `contig`, `strand`, `start`, `end` and `query_rank` with no naming accessor. A consumer receives `ENSG00000157764` and cannot say the gene is BRAF without supplying its own cross-reference. Every consumer therefore builds one, and a consumer that cannot build one has no way to attribute the score it was given.

The identifiers exist in one public file. HGNC's complete set carries an Ensembl gene accession beside its HGNC identifier, approved symbol, NCBI Gene identifier, RefSeq accessions, previous symbols and alias symbols. Measured 2026-09-08 against the installed `pangopup.gencode-v38-domains.v1` mask, which holds 60,605 distinct stable accessions across 60,649 records:

| Identifier | Genes carrying it | Share of the 41,957 joined |
| --- | ---: | ---: |
| HGNC identifier | 41,957 | 100.0% |
| Approved symbol | 41,957 | 100.0% |
| NCBI Gene identifier | 41,496 | 98.9% |
| RefSeq accessions | 40,820 | 97.3% |
| Previous symbols | 11,826 | 28.2% |
| Alias symbols | 21,495 | 51.2% |

41,957 of the 60,605 scoreable accessions join to an HGNC record and 19,218 of those are protein-coding.

Three limits are known and none of them blocks this work.

18,648 accessions (30.8%) do not join. HGNC publishes no Ensembl crosslink for them. That is not proof they are unnamed: 2,684 approved HGNC records carry no Ensembl accession at all, 43 of them protein-coding. Those genes stay nameless after this ticket and the motivating problem persists for them.

The two sources are different vintages. The mask is GENCODE v38 from 2021 and the HGNC set is current. 401 HGNC Ensembl accessions are absent from the mask, 36 of them protein-coding, including `CCL3L1` and `C4A_2`. Drift in the other direction sits inside the 18,648 and this measurement cannot separate it.

Three accessions in the joined set carry two approved HGNC records each: `ENSG00000175658` (`DRD5P2`, `DRD5P3`), `ENSG00000230417` (`LINC00595`, `LINC00856`) and `ENSG00000250413` (`SLC2A9-AS1`, `SLC2A9-AS3`).

Ian settled these choices on 2026-09-08.

The Ensembl accession stays the identity. A name is a label a consumer displays, never a key it stores or matches on. Symbols get renamed and accessions do not. `crates/pangopup-core/src/lib.rs:229` already records that a stable accession is not globally unique across PAR identities, and naming does not change that.

A consumer can read the HGNC identifier, the approved symbol, the NCBI Gene identifier and RefSeq accessions for a named gene, plus previous and alias symbols so a consumer holding an older name can still match.

A gene the naming source cannot name reports no name. A clone-derived string such as `AC092143.1` reads like a symbol without being one, and an absent field is honest where a convincing placeholder is not. Reject placeholders of that kind, not the annotation that carries them.

An accession with conflicting approved records reports no name. Picking one arbitrarily would be invisible to a consumer and unpinnable by a test.

Naming is not a scoring fact. `spec/http-service.md:40` establishes that `request_contract` does not enter `scoring_identity`, and README tells consumers to store that identity as a data-set version. A naming refresh changes no score, so it must not force every consumer to treat unchanged scores as new data.

The candidate ships as 0.5.0. Publication is separate work, as tickets 0036 through 0038 established.

Done, observably:

- A named gene reports its approved symbol, HGNC identifier, NCBI Gene identifier and RefSeq accessions alongside the Ensembl accession it reports today. Fields the naming source does not supply for that gene are absent rather than empty.
- Previous symbols and alias symbols are readable for a named gene that has them.
- Every identifier that can hold several values for one gene is readable as all of them, not one arbitrary pick. 75 joined genes carry more than one RefSeq accession, alias symbols reach 22 per gene and previous symbols reach 18.
- A gene the naming source does not name, and a gene whose accession carries conflicting approved records, both report the Ensembl accession alone and no name. A test pins one of the three known conflicting accessions.
- The specification states which output surfaces gain names. Structured score records and the gene reported on a source-reference ambiguity are in scope. The human-readable table is out of scope unless the specification says otherwise.
- The service runs, reports ready and scores normally when no naming source is installed. A score record then reports no name and `/v1/status` states that naming is unavailable.
- `/v1/status` names the naming source release that is installed, so a consumer can tell one naming vintage from another.
- The naming source is pinned to an immutable dated release file, not to a URL whose bytes are replaced on each publication, and it is pinned by byte count and a self-computed cryptographic digest. HGNC publishes no upstream checksum file, so parity with the GENCODE annotation's `MD5SUMS` pin is not available and is not required.
- The naming source installs and updates through the existing offline asset path with no network access.
- `scoring_identity` is unchanged by installing, updating or removing a naming source. A test proves that with the software version held fixed.
- The published compatibility document carries a v0.5 response-shape inventory naming every field this release adds, states the consumer-first deployment order for it in the form the existing gate requires, and the version gate and its independent portable copy both enforce the new entries.
- The candidate workspace version is 0.5.0 and `scripts/check-version-consistency.py` passes. The published executable, container, release identifier and container digest pins stay at their v0.4.1 values, because no 0.5 artifact is published by this ticket.
- `make lint`, `make test` and `make spec` pass without reducing specification coverage.

Boundary: this ticket adds identifiers to an existing result. It must not change any score, position, rejection code, rejection reason, request limit, or which genes a variant returns. It must not make naming a filter, a lookup key, or a request parameter; `gene_filter` continues to accept exactly the three forms `/v1/status` publishes today, the bare accession, the versioned accession, and the versioned `_PAR_Y` accession. It must not change `mask_sha256`, the model cache key, or invalidate a cached model result, so the naming data must not be folded into the mask component. It must not add a network fetch at scoring time or at service start. It must not publish a release, move a published artifact pin, or rewrite immutable release-note bodies, historical tickets or completion records.
