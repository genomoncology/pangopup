# Retained sparse v2 publication evidence

## Scope

This retained local run prepared the complete `snv-grch38-v2` publication set without publishing it or changing an active installation. The retained root is `/Users/ian/workspace/data/pangopup/sparse-release-v2-2026-09-19`. Run A remains the complete retained set. The generated proof and profile target commit `8eb8917ff9f788c28b29287ed3a06b6b4762c554`; these checked authorities land in a later commit.

## Authenticated inputs and build

The standalone source clone has its own Git objects, no alternates, a detached `HEAD` at `8eb8917ff9f788c28b29287ed3a06b6b4762c554`, an empty status, and readable Git metadata. `origin/main` contained that commit before the build. The build mounted this source read-only and used the fresh external target `/Users/ian/workspace/data/pangopup/sparse-release-v2-2026-09-19/build-target`.

The successful container was Linux ARM64 image `rust@sha256:ecbe59a8408895edd02d9ef422504b8501dd9fa1526de27a45b73406d734d659`, Debian 13 with glibc 2.41. It supplied rustc 1.93.1 commit `01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf` and Cargo 1.93.1 commit `083ac5135f967fd9dc906ab057a2315861c7a80d`. The release executables were:

| Executable | Version | SHA-256 |
|---|---|---|
| `pangopup-build` | 0.5.0 | `53d7010273c1e34d7813b16b032bcb3a95edd40aba39e4828ab2550bccec2777` |
| `pangopup` | 0.5.0 | `28957de12148a61d8d1e451c8e0c17286784a9cc3ff215ab437683151426d48c` |

The first fresh target used Linux ARM64 image `rust@sha256:7c4ae649a84014c467d79319bbf17ce2632ae8b8be123ac2fb2ea5be46823f31`, Debian 12 with glibc 2.36. Linking stopped before either executable existed because the current ONNX Runtime objects require glibc symbols including `__isoc23_strto*` and `__cxa_call_terminate`. `evidence/build.log` and the failed target remain retained. The Debian 13 retry changed only the container image and fresh target.

The exact build commands were:

```text
docker run --rm --mount type=bind,src=/Users/ian/workspace/data/pangopup/sparse-release-v2-2026-09-19/source/pangopup.git,dst=/src,readonly --mount type=bind,src=/Users/ian/workspace/data/pangopup/sparse-release-v2-2026-09-19/build-target,dst=/target --workdir /src --env CARGO_TARGET_DIR=/target rust@sha256:ecbe59a8408895edd02d9ef422504b8501dd9fa1526de27a45b73406d734d659 sh -euxc 'test "$(git rev-parse HEAD)" = 8eb8917ff9f788c28b29287ed3a06b6b4762c554; test -z "$(git status --porcelain=v1 --untracked-files=all)"; test "$(git symbolic-ref -q HEAD || true)" = ""; uname -a; ldd --version | head -1; rustc --version --verbose; cargo --version --verbose; df -h /src /target; cargo build --locked --release --package pangopup-build --bin pangopup-build; cargo build --locked --release --package pangopup-cli --bin pangopup; sha256sum /target/release/pangopup-build /target/release/pangopup; /target/release/pangopup-build --version; /target/release/pangopup --version'
```

The candidate, candidate report, fixed-v1 manifest, notice, and fixed member were mounted read-only for both runs. Each run reauthenticated them before work:

| Input | Bytes | SHA-256 |
|---|---:|---|
| sparse `scores.pgi` | 2,035,371,437 | `354343dc1a9f6558e46693e2481b5181461be2115cd92e029ffd4d4abef4a01e` |
| candidate `report.json` | 1,177 | `e1c0a684aa70eaffe6aae26133a89b3cf38fd6baa40437d70d15e2a3fd72ae5a` |
| fixed-v1 `manifest.json` | 3,589 | `c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3` |
| fixed-v1 `NOTICE` | 1,709 | `9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7` |
| fixed-v1 `scores.pgi` | 15,033,158,255 | `6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27` |

## Exact run sequence

Run A and Run B started from absent roots under `publication-runs`. Both used the same digest-pinned image, disabled networking, read-only source, input, and executable mounts, and a writable retained-run mount. `evidence/run-a.log`, `evidence/run-a-lookup.log`, `evidence/run-a-finalize.log`, and `evidence/run-b.log` retain shell-expanded invocations and results. `evidence/run-b.sh` retains Run B's exact argument arrays. Each run executed this publication sequence:

```text
/built/pangopup-build sparse-release assemble --authority /authority --sparse /candidate/scores.pgi --candidate-commit 1b1d95d6b5d32112a7c57faeb3af50d88ed93f08 --output /runs/<run>/bundle
/built/pangopup-build verify /runs/<run>/bundle
/built/pangopup-build transport pack --bundle /runs/<run>/bundle --output /runs/<run>/transport
/built/pangopup-build transport verify --transport /runs/<run>/transport
/built/pangopup-build transport unpack --transport /runs/<run>/transport --output /runs/<run>/reconstructed
/built/pangopup-build verify /runs/<run>/reconstructed
/built/pangopup-build sparse-release prepare --transport /runs/<run>/transport --tooling-commit 8eb8917ff9f788c28b29287ed3a06b6b4762c554 --release-target-commit 8eb8917ff9f788c28b29287ed3a06b6b4762c554 --output /runs/<run>/release
/built/pangopup assets install --transport /runs/<run>/transport --data-dir /runs/<run>/install
/built/pangopup status --data-dir /runs/<run>/install
```

The lookup scripts derive the ordered command arguments from `tests/fixtures/snv-regression/requests.tsv`. They issue explicit and isolated-discovery commands for all 994 authoritative found or ambiguity requests. They issue one checked explicit batch for the six misses, each isolated-discovery miss separately, and one combined discovery batch. The retained scripts and logs preserve every exact variant argument.

## Publication identities and sizes

The sparse bundle identity is `sha256:17085bb737bd3a9e54df9cb60d643c8ad8b8b6cb2c5bf0dbe4fc4c1e95c2b4f7`. Its assembler fingerprint is `sha256:07a6884d3d8d94c4fbe28c6a1cb2a263eed21a236b30dde2568b0a7b3c1ac623`. Its candidate association is commit `1b1d95d6b5d32112a7c57faeb3af50d88ed93f08`.

The transport identity is `sha256:e2a9090a71c4fb68dfcf496e5389db2b29b767a5b95d8fa50acdf2b75458be0e`. Its two compressed parts total 1,310,940,560 bytes.

| Publication file | Bytes | SHA-256 |
|---|---:|---|
| `transport.json` | 1,265 | `23de0b99c2a9e3cae1d4044f385687c6cb5ecdce9112075495ea8700cac2ccdf` |
| `bundle-manifest.json` | 3,924 | `17085bb737bd3a9e54df9cb60d643c8ad8b8b6cb2c5bf0dbe4fc4c1e95c2b4f7` |
| `NOTICE` | 1,709 | `9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7` |
| `payload.pgi.zst.part0000` | 1,000,000,000 | `454a989476759852687a2aba513703e14cc9b3717c4d75cffe82df293ede33bb` |
| `payload.pgi.zst.part0001` | 310,940,560 | `888d1a13d73aa5841c222cd1e4f1433202e8a729a106e1a005eeb63cd7cb8ac2` |
| `proof-receipt.json` | 5,405 | `9c5f945af8b52d21d44331325d908cf66dfbc6430d301d43d4dfa3a23e1c727c` |
| `release-profile.json` | 4,755 | `37ae3f859e23cdf73b6cc4a5f49f3f8fdeff11b955571c827aabb273e89dd1b0` |
| `SHA256SUMS` | 595 | `9498baf3031104f244d2bc7e501af4a34af4f9bcc80aa80bc583f4e9eb659158` |
| `release-notes.md` | 790 | `efec188c2b4b9fa4f177a3cada21cbe838f54c7302a5f85cc6020cb75bb5928e` |

The installed three-file SNV bundle totals 2,035,377,070 bytes. A fresh download plus installed members totals 3,346,324,528 bytes. The checked proof records both exact sums and their members.

## Qualification result

Both runs' assembled and reconstructed bundles passed exhaustive certification with two members verified. Transport verification passed before each reconstruction. Both isolated installs reported `partial`, with the exact v2 SNV bundle ready and runtime missing. Removing `data_dir` and the isolated bundle path made the two status records byte-identical with SHA-256 `a6152c6005aa2c7b0ee16f2a7fdcc3475f787c309df9d03bace4ae3c6078dc03`.

In each run, the 994 explicit lookup results matched the accepted fixture oracle after removing only the v1/v2 bundle identity, software-version addition, and checked naming additions. Discovery from the isolated install produced byte-identical output for all 994. The six checked explicit position-1 misses returned `not_found` and matched their fixture rows. Each corresponding discovered request exited 2, wrote no stdout, and ended with `{"status":"error","code":"MODEL_REJECTED","message":"insufficient GRCh38 reference context","details":null}`. The combined six-request discovery command also exited 2 with no partial stdout and reported its first rejection. This corrects the reviewed ticket's earlier missing-runtime assumption without changing product behavior. The failed assumption check remains in `evidence/run-a-lookup.log`; the accepted behavior is in `evidence/run-a-finalize.log` and `evidence/run-b.log`.

The exact comparison command, run from the retained root, was:

```text
python3 evidence/compare-runs.py publication-runs/run-a publication-runs/run-b evidence/run-comparison.jsonl
```

It compared each required directory's complete file-member set before reading both files together and checking bytes, sizes, and SHA-256 values. All 55 selected files matched. `evidence/run-comparison.jsonl` has SHA-256 `5a3c33ab132c7c12b83796714606f289093c6d0c2696be6762b15cb8585122a7`.

| Compared category | Files | Bytes |
|---|---:|---:|
| assembled bundle | 3 | 2,035,377,070 |
| transport | 5 | 1,310,947,458 |
| reconstructed bundle | 3 | 2,035,377,070 |
| release set | 4 | 11,545 |
| isolated install | 6 | 2,035,377,799 |
| explicit lookup output | 8 | 683,544 |
| selected discovery evidence | 26 | 682,384 |

## Retention and unchanged inputs

`zsh evidence/postflight.sh` produced a second 35-entry snapshot of the existing installation. The before and after snapshot files match byte-for-byte with SHA-256 `721fa9e7c81769fa101f5ea74c4647a60ae75ad1073d6cf98a204d03641a04e5`. The snapshot covers paths, kinds, modes, owners, sizes, content hashes, device and inode identities, link counts, modification times, and symlink targets. It excludes access and change times. Postflight also reauthenticated the candidate and report, compared every checked small authority to Run A's generated byte, and reconfirmed the standalone source clone's exact commit, empty status, detached state, and lack of object alternates.

Run A remains the complete retained publication set. After the full comparison and postflight check, `evidence/run-b-duplicate-removal.md` recorded 17 duplicate Run B files totaling 7,417,079,397 logical bytes. Only Run B's `bundle`, `transport`, `reconstructed`, and `install` directories were removed. Run B retains its release set, status, lookup results, normalized oracle output, member inventory, and exact log. The candidate, fixed-v1 authority, current installation, and all Run A outputs remain unchanged.

## Evidence map

| Evidence | Purpose |
|---|---|
| `evidence/preflight-inputs.txt` and `evidence/preflight-host-and-container.txt` | Input, source, platform, container, and storage authentication |
| `evidence/build.log` and `evidence/build-trixie.log` | Failed Debian 12 link and successful Debian 13 locked release build |
| `evidence/run-a.log`, `evidence/run-a-lookup.log`, and `evidence/run-a-finalize.log` | First complete publication, install, lookup, and corrected miss evidence |
| `evidence/run-b.log` and `evidence/run-b-oracle.log` | Independent second complete publication, install, lookup, and oracle evidence |
| `evidence/run-comparison.jsonl` and `evidence/run-comparison-summary.json` | Exact two-run member, byte, size, and hash comparison |
| `evidence/existing-install-before.jsonl`, `evidence/existing-install-after.jsonl`, and `evidence/postflight.log` | Existing-install preservation and final input checks |
| `evidence/run-b-duplicate-inventory.jsonl` and `evidence/run-b-duplicate-removal.md` | Exact duplicate scope and removal record |
| `evidence/make-lint.log`, `evidence/make-test.log`, and `evidence/make-spec.log` | Final repository gates |

After the small authorities and records were added, `make lint`, `make test`, `make spec`, and `git diff --check` passed. The specification gate ran 199 blocks. The three gate logs have SHA-256 values `c87f30ce6d9ca88df3cf9d3212a483888ff6d6631b6c8b0eb11041c3c000169d`, `370abd02eaec82d1885dfd402f7e85cba9293486b3244a00e124fa018f09742d`, and `91099b7b4c22bc412ec271b212199f2ac7f44ce05fd1ad0d943611c4b99e3027` in command order.

## Boundary

This evidence does not publish GitHub assets, activate v2 outside the isolated retained roots, or change the compiled sync authority. It does not claim command/service parity, unfiltered production latency, or complete-file throughput. Those activation gates remain open.
