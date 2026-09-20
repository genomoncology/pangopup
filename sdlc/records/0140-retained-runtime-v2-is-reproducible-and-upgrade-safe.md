# Retained runtime v2 is reproducible and upgrade-safe

The retained qualification completed on macOS 26.4 arm64 with Rust 1.95.0 and the locked dependency graph at `Cargo.lock` SHA-256 `286f5a69e975e9becd695f7722b823b225b6b5de2e5900b770f731293f571170`. The retained root is `/Users/ian/workspace/data/pangopup/runtime-v2-qualification-2026-09-19`. No production activation occurred. No command published an asset or changed a production selector.

## Exact executables and inputs

- The transport-preparation runner came from clean pushed commit `863ff3e6fb6a78841efce396538de0d9212c1dc5`. Its executable is 3,114,096 bytes with SHA-256 `7628ecd599f7f4baf170e1b88564f1a4b737c1d553dbc32ffb3807a71463422f`.
- The qualification verify/install/admit runner came from clean pushed commit `6495929a2866af4afe89a207914fa5128e906840`. Its executable is 3,329,152 bytes with SHA-256 `bbf42231c271ee92784a3697044f6728fc6758b716cc2bdd1da4bc7370361f04`.
- The unchanged release preparer came from clean pushed commit `c0c7054dbf5be948f44c2e9b871ed626c57050cc`. Its executable is 31,248,496 bytes with SHA-256 `ee4a4e9e8571bc0888e575b81e41d328a5642e86c116094766e50e111e6f0a0b`.
- The release target is `103098bd39b41eddd290618ddba79f26572d455e`. The candidate `pangopup` executable is 35,395,328 bytes with SHA-256 `c7cae8b6831187e70149b35332fec3da6824a8c7d8abfba2edfd8cf0e10a2be3`. Preflight evidence places its build checkout at `863ff3e6fb6a78841efce396538de0d9212c1dc5`; `103098bd39b41eddd290618ddba79f26572d455e` was the separate release-target input. The original candidate build argument vector, environment, and exit status were not retained. Any later description of that build command is a reconstruction.
- The service-test-fixture executable came from `6495929a2866af4afe89a207914fa5128e906840`. It is 35,395,904 bytes with SHA-256 `ea10372d9a83bf577d2c4746b28a4be49c884c5de4aea3686b8d186fa1478067`.
- The v1 runtime transport was `/Users/ian/workspace/data/pangopup/runtime-v0.5.0-cache/profiles/d1caf6346bb24378f720056416fa6286f1153ccaf0c6a0778494f557035ef59e/transport`. The sparse authority was `/Users/ian/workspace/data/pangopup/sparse-release-v2-2026-09-19/publication-runs/run-a/bundle`.

## Reproducible retained output

Two independently empty runs produced byte-identical transport and release trees. The runtime transport identity is `sha256:1c893bd4ab177a7d305fd71528752b8c361c84fb87f7f1a21cfd9aea032a4e87`. The inner runtime profile identity is `sha256:ce91b332d04a776f12a4603e60cf40894dc002360c13b3541e03d3b748cd3983`. The ten-member transport contains 691,860,158 compressed bytes.

The eight model-side members match v1 byte for byte:

| Member | Bytes | SHA-256 |
| --- | ---: | --- |
| `model-manifest.json` | 3,823 | `4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43` |
| `model-NOTICE` | 648 | `fbba767913348642351d7e95b8589619a8bb4a7f3738c5ea6fe266c21434107f` |
| `model.onnx.zst` | 31,144,867 | `741642c98c0aae6a76d4096780c114ba9bd497122868ba0ecf2d85a30d8af568` |
| `reference-manifest.json` | 3,719 | `7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f` |
| `reference-NOTICE` | 793 | `1e3ce49d78cd9089407c54ce92a9e6d3adb92a9f3267185ba9ea64df8a588499` |
| `reference.pgr.zst` | 656,781,805 | `e181eb31a76c8e05782415317450c92d5d7a148cb28afd184e0c7767aa42cc25` |
| `mask-NOTICE` | 978 | `d8ee279f7a97ae25d2bf502b42a4fb480234cc517c0b58f85d6cf6547995bbeb` |
| `domains.pgm.zst` | 3,933,486 | `e8353beba3820e3c4679acb46a673622080fe6f560a02b558bc6d75f50286747` |

Only `runtime-profile.json` and `runtime-transport.json` differ from v1. They are 1,371 and 3,179 bytes. Their SHA-256 values equal the runtime profile and transport identities above. The complete outer release trees also match. The release-only identities are:

| Member | Bytes | SHA-256 |
| --- | ---: | --- |
| `RELEASE-NOTES.md` | 1,381 | `62284c9654ab8e57115a82aa735fe2a2f136f80cf55fcbab704dd96503367472` |
| `SHA256SUMS` | 1,023 | `c54e52f61043ce41808f8e6c6378138b72043c892f8d99c73ca7cdcf8bd4c5ba` |
| `SOURCE-SUPPLEMENT.json` | 876 | `270cb560188eb0dd42489ffe83c625d2d25e149e09408fd081aaac9ab3202e50` |
| `runtime-release-profile.json` | 8,119 | `97aaa1a06da278534063124d949dca5d5109afed933cacf9fa785ec20d52b110` |

The reviewed qualification runner verified the retained transport, installed it into an isolated root whose active sparse SNV identity is `sha256:17085bb737bd3a9e54df9cb60d643c8ad8b8b6cb2c5bf0dbe4fc4c1e95c2b4f7`, and admitted the exact installed tuple. The model identity is `sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`, the reference identity is `sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`, and the mask identity is `sha256:714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`. Supplying a different expected SNV identity returned `TRANSPORT_INCOMPATIBLE`.

The authoritative replay is `evidence/authoritative-replay-5/transcript.txt` beneath the retained root. It is 258,215 bytes with SHA-256 `850c0962c5c0bed051c121fe0aae40af375d3da36d2a3b033aa7227a8abb061e`. Its 27,712-byte harness has SHA-256 `992dfa2f6c2fa82dbcd33c478e6a04d254f91875914dd5b1b6ecbadc84ba5e36`. The transcript records every executable or container image identity, full argument vector, working directory, relevant nonsecret environment, exit status, and output hash. It covers transport and release preparation, byte comparison with the accepted retained output, qualification verification, isolated installation, exact admission, wrong-SNV refusal, and ordinary production refusal of the inactive v2 profile. That final refusal returned `ASSET_STATUS_INVALID` with nested `PROFILE_INCOMPATIBLE`, as required before activation.

The preserved replay harness and transcript incorrectly label the candidate executable source as `103098bd39b41eddd290618ddba79f26572d455e`. They remain unchanged as evidence of the executed replay. The preflight record identifies `863ff3e6fb6a78841efce396538de0d9212c1dc5` as the checkout that built that executable and identifies `103098bd39b41eddd290618ddba79f26572d455e` as the release target.

## Semantic and service evidence

The retained command comparison ran 1,000 requests in seven nonempty groups. Group sizes were 318, 320, 320, 6, 6, 6, and 24. Fixed-v1 and sparse-v2 returned equal JSON values after replacement of the independently validated `bundle_id` field only. The output contained six position-one misses, `found`, `not_found`, and `ambiguous_source_reference`, plus an overlapping result. All 2,000 rendered gain and loss score strings contained exactly two decimal places.

Both live services answered the same 1,000 requests in 16 bounded HTTP requests. Every item carried its independently derived `data_set_version` and `scoring_identity`. Each service returned 994 checked precomputed `bundle_id` values. The six position-one misses produced the same `reference_context_unavailable` rejection. Extra retained cases proved exact found, not-found, rejected ordering, a two-record overlap, and a one-record gene-filtered overlap.

| Runtime | `runtime_profile_id` | `data_set_version` | `scoring_identity` |
| --- | --- | --- | --- |
| fixed v1 | `sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c` | `sha256:e6161aa92acb6b28f07e851e5d0ff13159edffb9a4ec557f0eacf35bd4a1b632` | `sha256:bb3962cbd6d20305c48696ebffe89f0e82e494216ef83c5494f55849d908adf5` |
| sparse v2 | `sha256:ce91b332d04a776f12a4603e60cf40894dc002360c13b3541e03d3b748cd3983` | `sha256:5ffac268858dbae7d6cc4ee6ecf05e0677dcfafda3fcc0aee1ec3970bfe0c82d` | `sha256:0dd20757842838d969c3819b21b63c33fd1e93074db0460cdb13e199d80d655e` |

The independent identity calculation hashed canonical JSON preimages over PangoPup `0.5.0`, the checked runtime profile, and `sequential:1/1` for `scoring_identity`. The data-set preimage omitted the CPU policy. Status and every retained result matched those calculations. A real modeled insertion produced `0.00` gain and `-0.09` loss.

## Upgrade, cache, rollback, and immutability

The authenticated public v0.4.1 Linux x86-64 executable is 29,035,720 bytes with SHA-256 `06ad5c0e3aff8f030116262045cede3bc97cdfc716b3e72029f02b6d58d6f822`. Its release manifest names commit `ba8b62180ecd5750a575944d2070f83ca585f4ed`. The authoritative replay ran it in exact Docker image `sha256:a99cfc517144bc59b1978475ec53b46ecabec7e43635402ee5b77cc54cd1b20a`.

Public v0.4.1 created a real 16,384-byte layout-1 SQLite cache with SHA-256 `5ad762e94784223f63532cb4323daa1f09848d841f0161564dc7f78ba9e8bc80`. Candidate 0.5 reported one earlier-layout discard, recomputed the same normalized model result, and created layout 2. A second closed-process request emitted no discard, kept `next_write_sequence` at 2, and left the database unchanged at that observation with SHA-256 `1e6d0c70c156b4f0a969c9e81603327b5a01e707b0b21335e61f8b27c1666032`. Later service startup, WAL use, and checkpoint activity can change SQLite's physical main file without adding a logical cache entry. A live main-file hash therefore cannot prove cache reuse. The setup row stores software version 0.5.0 and the exact model, reference, mask, semantics, masking policy, and window fields. It does not store an SNV bundle, runtime profile, data-set version, service identity, or rendered response.

The bounded live-service rerun started with an empty private cache. Fixed v1 reported `next_write_sequence=1` and zero entries, wrote the modeled insertion once, then reported sequence 2 and one entry after both the first and repeated request. Sparse v2 opened that same cache at sequence 2 with one entry. Both sparse requests left sequence 2 and one entry. Fixed and sparse returned the same `0.00` gain and `-0.09` loss after removal of the route-specific identity fields. Each route returned its current `data_set_version` and `scoring_identity` from the table above.

Both services exited zero after `SIGINT`. Each post-shutdown `PRAGMA wal_checkpoint(TRUNCATE)` returned `0|0|0`, and each integrity check returned `ok`. Each `.backup` produced the same complete 20,480-byte database with SHA-256 `c008f1a80e0e702596f7224423835a9e2ecb90fb5eb508102b25a85562acd372`. Both backups reported sequence 2 and one entry, and their SQL dumps matched byte for byte. The retained transcript records executable path and hash, full argument vectors, working directory, relevant nonsecret environment, every command exit, service exits, and output hashes. It is `evidence/live-cache-reuse-rerun-5/transcript.txt` beneath the retained root. It is 24,087 bytes with SHA-256 `a437b225b65a69f7a05b665c3927185823a36d436b1d323f5ee46f767defe1fc`. The exact 13,114-byte harness has SHA-256 `123c48354c5b4039b93f73430e42dc92515283d725ff5b81c0b94c3cdb9b737e`. No container participated in this rerun.

Fixed-v1 ordinary admission and a fresh read both succeeded after the sparse qualification. Neither asset set was deleted.

Final snapshots match every before-snapshot field by path. The existing fixed-v1 installation has 35 records and canonical path-ordered snapshot SHA-256 `7894595f38eb542808931d10aef8ca34c79592a96c472692a5eb789483cf1db3`. The v1 transport has 11 records and SHA-256 `07cb3ae60344d23a29bb717b82f524ebbf8f49994d824629282ab3964cbb124a`. The sparse bundle has four records and SHA-256 `0495dafbaf154acfceb66a96960c7e6ef764f410c0e8d8254c2f3ccafadcff12`. The fixed-v1 snapshot's serialized before/after hash differs because the before snapshot used traversal order and the post-run snapshot used path order. The record maps are exactly equal.

## Attempts and cleanup

The first public-v0.4.1 container used Debian bookworm and failed because the binary requires GLIBC 2.38. Debian trixie authenticated and ran it. The first v0.4.1 cache attempt rejected a 0755 parent before creating SQLite; the private 0700 retry passed. One SQLite inspection assumed `software_version` lived in `entries`; the schema showed it in `setup`, and the corrected query passed. One shell snapshot attempt tried `/dev/stdout` and failed before an asset or cache command ran. The first retained command comparison assumed `software_version` was top-level; observed output placed it under `provenance`, and the corrected complete run passed. The first service comparison expected 17 HTTP batches after all calls completed; the correct arithmetic is 16, and the retained responses passed without repeated calls. One status command used `assets status`; the correct root `status` command passed. The first fixed-service start rejected its cache because the parent was not private; a 0700 isolated parent passed. The authoritative replay labels the corpus variant order and gene mapping as reconstructed from the retained successful fixed-v1 output. It labels the cache variant and exact asset paths as reconstructed from the retained public-v0.4.1 output. The replay applied both reconstructions to fresh isolated output and cache paths.

Four cache-reuse rerun attempts preceded the complete fifth attempt. The first signaled a background shell instead of the service process and then placed macOS `env -u` options after an assignment. Cleanup stopped the service. The second completed the fixed side but named `/usr/bin/test`; macOS provides `/bin/test`. The third completed the fixed side but incorrectly required WAL and shared-memory pathnames to disappear after the backup verification reopened the database. Its checkpoint had already returned `0|0|0`, and its WAL was zero bytes. The fourth used a 0755 cache parent and the service rejected it before creating SQLite. The fifth used a private 0700 parent and recorded WAL as zero bytes after both checkpoints. Each failed attempt has its own transcript under `evidence/live-cache-reuse-rerun*`.

Four failed authoritative replays preceded the complete fifth replay. Replay 1 failed in the snapshot helper before touching retained inputs. Replay 2 passed transport preparation and then used the sparse-release subcommand against a runtime transport. The corrected command is `runtime-release prepare-v2`. Replay 3 completed preparation and release but tried runtime installation before installing its sparse SNV authority. Replay 4 completed qualification admission and then incorrectly expected ordinary production admission of an inactive v2 profile. Two bounded debug continuations proved the remaining service and rollback sequence. The first debug continuation failed only in a shell-escaped identity assertion after both services had stopped. The corrected second continuation passed. Each attempt remains under its named `evidence/authoritative-replay*` and `isolated/authoritative-replay*` paths.

No service process remains. Both successful preparation scratch directories were removed. No private publication stage remains. Failed attempts retained data only under explicitly named isolated qualification and evidence paths, including `live-cache-reuse-rerun`, `live-cache-reuse-rerun-2`, `live-cache-reuse-rerun-3`, and the authoritative replay paths above. Ordinary production admission, sync, discovery, and fixed-v1 authority remain unchanged.

The authoritative replay transcript above contains the exact replay commands without abbreviated paths plus standard-output and standard-error hashes. The hashed outputs live beside it. Identity preimages are `evidence/fixed-v1-data-set-version-preimage.json`, `evidence/fixed-v1-scoring-identity-preimage.json`, `evidence/retained-v2-data-set-version-preimage.json`, and `evidence/retained-v2-scoring-identity-preimage.json`. Complete SQLite observations are `evidence/layout2-after-recompute.txt` and `evidence/live-cache-reuse-rerun-5/fixed-cache.sql`. The separate retained outputs also hold request-response data, before-and-after snapshots, service exits, rollback, and final fingerprint comparisons. All five retained input trees matched their before snapshots byte for byte.

Independent final evidence review recomputed all six executable hashes, both authoritative harness and transcript hashes, 348 logged output hashes, both service identities, every retained command-line and HTTP score shape, both complete cache backups, and all five input snapshot pairs. It found no score, cache, rollback, or publication-byte failure. It rejected the two provenance statements corrected above before accepting the retained facts.

The exact final repository tree passed `make lint`, `make test`, `make spec`, and `git diff --check`. The specification gate passed 207 executable examples.
