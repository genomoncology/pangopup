---
base: e2b8b62
head: f141eae
---

# Keep a deployment setting out of the scoring identity

A consumer now has one published value to store as its data-set version that a deployment cannot move. `/v1/status` carries `data_set_version`, a SHA-256 over a canonical preimage of the runtime profile identity, the schema and the software version. It holds still across worker and thread settings. It moves when an asset changes and when the software version changes.

Measured against the real installed assets across four settings, with the asset digests identical in every run:

| workers | threads | `effective_cpu_policy` | `scoring_identity` | `data_set_version` |
| ---: | ---: | --- | --- | --- |
| 1 | 1 | `sequential:1/1` | `sha256:bb3962cb…` | `sha256:e6161aa9…` |
| 1 | 4 | `sequential:4/1` | `sha256:e2fad6d0…` | `sha256:e6161aa9…` |
| 4 | 1 | `sequential:1/1` | `sha256:bb3962cb…` | `sha256:e6161aa9…` |
| 2 | 8 | `sequential:8/1` | `sha256:dbfe3c45…` | `sha256:e6161aa9…` |

`README.md` no longer tells a consumer to store `scoring_identity` as the data-set version. `architecture/service.md` no longer claims the identity's inputs cover every active policy that can change an answer, which ticket 0040 measured and disproved.

`effective_cpu_policy` stays in `ActiveScoringIdentityPreimage` and `scoring_identity` keeps its exact value. Removing the field would have changed what a published value means while its name, shape and position stayed identical. The preimage also hashes `software_version`, so the value moves on every release, and a consumer would credit the change to the version bump and never notice that thread counts had stopped moving it. Removal would also convert a result measured on one host against one build into a structural guarantee. `architecture/compatibility.md` already publishes a refusal to make that guarantee.

The accepted cost is that `scoring_identity` keeps a name that invites the mistake, a consumer who never reads the changed guidance keeps recording spurious versions, and three identity-shaped values sit on the status root. A score item still carries exactly one identity, so a retained record is unaffected. Retiring the policy stays available to a later ticket once thread-invariance is measured on a second host and build, and `data_set_version` is correct either way, so no consumer changes anything twice.

`runtime_profile_id` is published for the first time. It hashes the whole runtime profile, so it covers `assembly`, `semantics`, `distance` and `masking_policy`, the four score-affecting inputs no published digest reached. `every_leaf_fact_changes_the_profile_identity` walks all 25 profile leaves and proves each one moves it. Publishing it is safe because `production_runtime_profile()` hardcodes `cpu_policy: "sequential:1/1"` and `require_trusted_production` admits an installed profile only on whole-profile equality. The documentation warns against pinning it alone. It hashes the profile and not the software version, so a version change can move an answer while it sits still.

`scoring_semantics` is published on the status root. It reads from the same `const fn` a modeled item's provenance uses, so the two cannot drift. A precomputed score item carries six provenance fields and a modeled item carries twelve, and a modeled item carries none of `bundle_id`, `source_doi` or `source_archive_md5`. The specification states which facts a precomputed item omits and where a consumer reads them. Closing the gap at the item level would restate `"kind":"precomputed"` in 2,015 places, including the frozen regression corpora, because `render_jsonl` is shared with `pangopup lookup`. The ticket permits the documentation branch and this work took it.

Independent design and code reviews accepted the result after repairs. The design review found that `runtime_profile_id` shipped with no warning attached, so a consumer pinning it would under-version themselves. It also found the README proof gated only the new guidance and not the old sentence's removal, so a code stage that appended rather than replaced would have passed while the README named two different values to store. It replaced a flat thread-invariance sentence with one scoped to the measurement, and replaced a pinned policy string that had no regression target with two exact-value assertions.

The code review proved the exhaustive status assertion is still exhaustive. It added a throwaway field to `StatusOutput`, watched `health_status_and_route_errors_are_exact_json_lines` fail with the extra key, and reverted. That assertion is the only always-on gate catching an unannounced status field, and it grew by three fields under a pre-authorized restatement.

The code review also corrected a replacement sentence that was itself false. `architecture/service.md` had been rewritten to say the preimage inputs cover every installed component the service admitted. The service admits an installed naming source and keeps it out of the preimage deliberately, so a naming-release change moves every record's `gene_names` while moving neither identity. Draft 0044 records the consequence: a retained score record does not carry the naming vintage that produced its names.

`make lint`, `make test` and `make spec` passed with 539 tests passing, 9 ignored, and 300 specifications passing with 7 retained-asset specifications skipped by design. Specification coverage rose from 298. The `installed_success` lifecycle suite, which carries this ticket's central proof and sits behind a non-default feature, passed all 11 of its tests.
