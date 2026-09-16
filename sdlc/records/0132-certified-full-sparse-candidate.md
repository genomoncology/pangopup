# The complete sparse candidate passes size and logical parity

Commit `1b1d95d6b5d32112a7c57faeb3af50d88ed93f08` built the retained candidate on macOS 26.4 ARM64. The command read the certified fixed-v1 bundle with identity `sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3`. Its `scores.pgi` member was 15,033,158,255 bytes with SHA-256 `6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27`.

The first complete attempt stopped during fixed certification. It reported `BUNDLE_INDEX: invalid index: exception gene without segment`. The fixed reader omitted genes that existed only in the 30-record exception section. It ran for 742.63 seconds, reached 14,617,182,208 bytes of maximum resident memory, and removed every partial output it owned. The remediation merged ordinary and exception-only genes in numeric order without adding a corpus-sized allocation. Focused tests cover exception-only genes before, between, and after ordinary genes, a mixed gene, an entirely exception-only index, the allocation guard, and fixed-to-sparse conversion. Independent Astra XHigh review accepted the correction.

The second complete attempt succeeded. It ran for 1,604.63 seconds and reported 17,582,276,608 bytes of maximum resident memory. That operating-system measurement includes pages mapped from the 15,033,158,255-byte input. The bounded heap contract applies to the materialized one-gene buffer. The largest gene used 2,473,538 loci, capacity 2,473,538, and 118,729,824 bytes of `InputLocus` capacity. The declared limits remained 3,000,000 loci and 512 MiB.

The retained files are under `data/pangopup/sparse-candidate-2026-09-16` outside the repository:

| File | Bytes | SHA-256 |
|---|---:|---|
| `scores.pgi` | 2,035,371,437 | `354343dc1a9f6558e46693e2481b5181461be2115cd92e029ffd4d4abef4a01e` |
| `report.json` | 1,177 | `e1c0a684aa70eaffe6aae26133a89b3cf38fd6baa40437d70d15e2a3fd72ae5a` |

The candidate is 1.896 GiB. It is 86.461% smaller than the fixed member and passes ADR 0027's 3,221,225,472-byte size ceiling. The output directory contains only the candidate and report. No scratch or publication-stage file remains.

The canonical `pangopup.sparse-candidate-report.v1` completion report records 19,913 genes, 19,945 segments, 343,759 blocks, 1,366,418,555 loci, 1,366,418,525 ordinary loci, 30 exceptions, and 4,099,255,665 logical records. The fixed source and decoded candidate both produce `sha256:dcec29e84c5f65bd76ffde2be8c7fa312d08e6abdb1e45e024dc0fe8c8da9c31` over all logical records. Writer and decoded counts agree.

The exact pushed command commit passed seven focused fixed-reader tests, its allocation test, and six builder integration tests in a disposable Linux AMD64 container. The source was read-only, build output used anonymous container storage, and the container was removed. On macOS, the focused writer, reader, builder, allocation, and asset suites passed before the retained run. `make lint`, `make test`, and `make spec` passed; the specification gate ran 193 blocks. Strict `pangopup-build` lint also passed after marking its Linux-only staging-path use explicitly for other platforms. The optional builder-wide `full_bundle` test still reaches the documented unsupported macOS atomic directory-publication path. That issue does not change this candidate build or its measured result.

This record qualifies size and complete logical parity only. The candidate has no bundle, installed asset identity, runtime profile, or route. PangoPup does not serve it. ADR 0027 still requires the fixed comparison workload, absolute and relative latency gates, corruption qualification, final identities, and rollback evidence before activation.

Independent Astra XHigh result review checked the retained fixed input, candidate, report, implementation, tests, and claims. It accepted ticket closure without further remediation.
