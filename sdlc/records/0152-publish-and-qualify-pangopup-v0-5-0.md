# Publish and qualify PangoPup v0.5.0

PangoPup v0.5.0 is public from source commit `f5cae030e1fb04db4769a8d303876fa6b26bb68d`. GitHub release [`v0.5.0`](https://github.com/genomoncology/pangopup/releases/tag/v0.5.0), ID `394840043`, was published at `2026-09-23T16:16:44Z`. Public release metadata reported `immutable=true`, non-draft, non-prerelease, and Latest. The direct tag resolves to that source commit. The executable and container select the coherent public v2 scoring assets after provisioning. No scoring code or public tag changed during this record update.

## Publication inventory

Executable package run [`35886548198`](https://github.com/genomoncology/pangopup/actions/runs/35886548198), native container stage run [`35886551496`](https://github.com/genomoncology/pangopup/actions/runs/35886551496), and finalize run [`35887750804`](https://github.com/genomoncology/pangopup/actions/runs/35887750804) succeeded at that source commit. The six public release assets matched their final package by name, byte count, and SHA-256 through anonymous downloads:

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `LICENSE` | 35,149 | `3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986` |
| `NOTICE` | 4,987 | `e0e67c5c3d2ef7fb96cfc2b94f88ce4fc513e516043de01794f905d0ebf5442d` |
| `pangopup-linux-x86_64` | 31,684,520 | `b76814ff677fe014aba4ccce497e124644a4e3dbf9ba130a2eeb5923072daab0` |
| `pangopup-linux-x86_64.cdx.json` | 201,980 | `4ee38581e63d0a4253ccdc0466dce25cf071c4984639588a341606c13b9f578b` |
| `pangopup-linux-x86_64.sha256` | 88 | `1503271cb98aa8f170059a44d69acfee2ae76f217e3c5b228d0d8610fcedd649` |
| `release-manifest.json` | 950 | `3965a8c06c1a122e78caa4ac2ab25b5a42a7070ff3f5debb7a8103f360ca399f` |

The final packaged executable was byte-identical to the candidate executable used in the complete fresh and upgrade qualification. The tagged `install.sh` matched the tagged source and installed version 0.5.0. Its isolated code-only uninstall passed.

Anonymous registry reads showed GHCR `0.5.0`, `v0.5.0`, and `latest` at OCI index `sha256:43fbaeaf800beae6e300e7f8b9dcdfcabd234ce3f53a48411cbd7849457b1174`. Its native AMD64 leaf is `sha256:82fc2687b0bb8b2756a614d225c3e2e016c4eae667dd6007bbeebd1db1698aac`; its ARM64 leaf is `sha256:15c81da8345994d01a999fad33cc4b5b7cb120a49a1550da9e4065c883da4fb3`. The prior `0.4.1` index remained `sha256:2177c02fc045136a2ef066dbbfa669f59d56dc15e44765e7b7bfbbc9969a6eb8`.

## Public qualification

A complete fresh public sync downloaded `2,002,822,127` bytes with zero resumed bytes. An offline repeat downloaded zero bytes. The retained oracle passed 1,000 exact SNV requests in seven groups. Model and HTTP status/result identities passed the corrected release checker. Public output kept exactly two decimal places. The first fresh container attempt stopped at the HTTP harness because that test image lacked Python; the complete rerun with Python passed.

A real public v0.4.1 to packaged v0.5.0 upgrade discarded the SQLite layout-1 cache once. Layout 2 recorded software version 0.5.0. The repeated request produced one cache entry with next write sequence 2. Offline v2 activation downloaded zero bytes and retained the model-cache hash, one entry, and next write sequence 2. The v1 bundle and profile directories remained. Explicit v1 lookup passed after the v2 activation.

Anonymous AMD64 and ARM64 image pulls passed. Both reported version 0.5.0 and selected the v2 SNV bundle `17085bb7…` and runtime profile `ce91b332…` in status. AMD64 lookup and insertion model checks passed with exact two-decimal `0.00/0.00` output. The public image's SNV and model path was not repeated on ARM64 beyond its native stage and version/status qualification.

The isolated native Linux full-uninstall test removed the test executable, data, and cache. An earlier attempt used a volume path named `data`; the safety guard returned `UNINSTALL_UNSAFE` and removed nothing. The test then used paths named `pangopup` and passed. A macOS Docker bind mount could not `fchmod` cloned read-only assets, so the upgrade and full-uninstall checks ran against a native Linux volume. That test volume, `pangopup-0152-upgrade-118c62e-20260923`, was removed after full uninstall. Original data remained unchanged.

Superseded staging runs `35881617631` and `35881621947` used commit `118c62e`. They were never tagged or finalized. Small outputs for the first attempt, fresh rerun, and offline check remain under `experiments/pangopup-release-0152-qW7zAG/evidence-{first-attempt,fresh,offline}-output` in the workspace. Three large isolated Mac test-data trees were moved recoverably to `/Users/ian/.Trash/pangopup-0152-qualification-first-20260923`, `/Users/ian/.Trash/pangopup-0152-qualification-complete-20260923`, and `/Users/ian/.Trash/pangopup-0152-upgrade-copy-20260923`. They still occupy disk space until Trash is emptied. The final native Docker test volume was removed. Earlier v0.4.1 release and container identities remain historical evidence.

The post-publication documentation branch passed `make lint`, `make test`, and `make spec` on macOS. The specification gate reported 207 passed. The version checker pins the published release ID, source commit, and container index while preserving the v0.4.1 historical release checks.
