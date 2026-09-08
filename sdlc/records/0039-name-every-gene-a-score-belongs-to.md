---
base: 33a000a
head: 0654535
---

# Name every gene a score belongs to

A score record now names its gene. Every structured record and every source-reference ambiguity carries a `gene_names` object holding the approved symbol, the HGNC identifier, the NCBI Gene identifier, previous symbols and alias symbols. A consumer reads BRAF, HGNC:1097 and 673 beside `ENSG00000157764` without supplying its own cross-reference. An unnamed gene omits the whole object rather than reporting an empty one.

The naming source is the HGNC complete set, dated monthly release 2026-09-04, pinned at 16903161 bytes and SHA-256 `6f43d6ff43aa9fdfa5fb2f20a20a7cace66e6e02e2a0dcf19d9b726e2e248d20`. HGNC publishes no upstream checksum file, so the pin is a byte count and a self-computed digest. `pangopup assets naming install --source <FILE>` installs it offline through the existing asset path. `pangopup-build naming inspect` reports what a release yields before an operator installs it.

The component installs as a sibling of `bundles/` and `runtime/` under the data root. The runtime profile was the obvious home and it is closed to this. `RuntimeProfile` is admitted by whole-struct equality and its canonical bytes hash to `runtime_profile_id`, which feeds `scoring_identity`. Naming inside the profile would move the scoring identity on every install. A live service reported `sha256:bb3962cbd6d20305c48696ebffe89f0e82e494216ef83c5494f55849d908adf5` identically across four states: no source installed, one installed, updated to a second dated release, and removed.

Three rules decide when a gene goes unnamed. An accession carrying more than one approved HGNC record reports no name; the release holds exactly three, `ENSG00000175658`, `ENSG00000230417` and `ENSG00000250413`. A symbol containing a period is a clone-derived string and reports no name; no approved symbol in the release contains one, and the 7,411 approved symbols carrying a hyphen survive. A gene the source does not name reports the accession alone.

Alias and previous symbols ship and are not identifiers. 1,097 of 43,406 alias symbols point at more than one gene and 462 are another gene's approved symbol. `architecture/compatibility.md` states that both are ambiguous and must never be the sole basis for an automated match.

Independent design and code reviews accepted the result after repairs. The design's miniature fixture had stripped the double quotes HGNC uses around a multi-valued cell, and 11,046 of the release's 45,045 alias cells carry them. A parser splitting on the pipe with no quote handling passed against that fixture and returned `"T4` and `Leu-3"` against the published file. The design review rebuilt the fixture from the published bytes, added a hyphenated symbol so an over-broad placeholder rule cannot pass, and reconciled five shipped assertions that hardcoded the 0.4.1 scoring identity. The code review listed the install verb in help after finding `assets naming --help` returned a usage error, replaced a generic manifest fault with a message naming the naming component, and pinned both the installed release on `/v1/status` and the scoring identity across an install.

The code stage reused two existing error kinds rather than adding an `AssetErrorKind` variant. `crates/pangopup-assets/src/error.rs` sits in the hashed inventory behind `EXPECTED_SNV_SHA256`, so a new variant moves the published builder provenance the Boundary protects.

The workspace version is 0.5.0 and `architecture/compatibility.md` carries the v0.5 response-shape inventory with its consumer-first deployment order. The published executable, container, release identifier and container digest pins stay at v0.4.1 because this ticket publishes no release. `NOTICE` now attributes the HGNC complete set and the GENCODE annotation the mask is built from.

`make lint`, `make test` and `make spec` passed with 534 tests passing, 8 ignored, and 292 specifications passing with 7 retained-asset specifications skipped by design. Specification coverage rose from 286.

Draft 0042 records the one regression left standing. A command-line lookup parses the whole naming source on every invocation and rises from 6 ms to 155 ms once one is installed. The service parses it once at startup and is unaffected.
