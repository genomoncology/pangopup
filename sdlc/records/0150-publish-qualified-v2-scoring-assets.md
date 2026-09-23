# Publish the qualified v2 scoring assets

Ian authorized publication of PangoPup 0.5 and its scoring assets on 2026-09-20. The operator authenticated GitHub CLI 2.92.0 as `imaurer`, checked that immutable releases were enabled, and confirmed that neither v2 release nor tag existed before the first write. Tickets 0137 and 0140 contain the independently reviewed build, qualification, source, and target provenance. This operation used their exact retained Run A outputs. It did not rebuild data, change scores, or select v2 in production.

| Release | GitHub ID | Target commit | Published UTC | Assets | Asset bytes |
| --- | ---: | --- | --- | ---: | ---: |
| [`snv-grch38-v2`](https://github.com/genomoncology/pangopup/releases/tag/snv-grch38-v2) | 394675189 | `8eb8917ff9f788c28b29287ed3a06b6b4762c554` | 2026-09-23 13:20:12 | 8 | 1,310,958,213 |
| [`runtime-grch38-v2`](https://github.com/genomoncology/pangopup/releases/tag/runtime-grch38-v2) | 394675298 | `103098bd39b41eddd290618ddba79f26572d455e` | 2026-09-23 13:23:37 | 16 | 859,644,459 |

Both releases have `draft=false`, `prerelease=false`, `immutable=true`, and `make_latest=false`. Their titles are `Pangopup GRCh38 sparse SNV scores v2` and `Pangopup GRCh38 Pangolin runtime v2`. The release bodies have SHA-256 `efec188c2b4b9fa4f177a3cada21cbe838f54c7302a5f85cc6020cb75bb5928e` and `e187980734404f42b00e6a875d19d17d392b5d9b0214deaaef32d923811bf9ea`, respectively. The SNV body came from Run A's `release/release-notes.md`. The runtime body came from the separately staged `RELEASE-NOTES-PUBLIC.md`. That public text corrects one sentence in the retained `RELEASE-NOTES.md`: the upstream source archive does contain the original checkpoint containers. The retained file remains unchanged at SHA-256 `62284c9654ab8e57115a82aa735fe2a2f136f80cf55fcbab704dd96503367472`.

## Exact public inventory

Each SHA-256 below was compared with the retained local file, the draft's GitHub asset digest, and a fresh anonymous download after publication. GitHub reports the same byte counts and digests in the public release API.

| `snv-grch38-v2` asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `transport.json` | 1,265 | `23de0b99c2a9e3cae1d4044f385687c6cb5ecdce9112075495ea8700cac2ccdf` |
| `bundle-manifest.json` | 3,924 | `17085bb737bd3a9e54df9cb60d643c8ad8b8b6cb2c5bf0dbe4fc4c1e95c2b4f7` |
| `NOTICE` | 1,709 | `9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7` |
| `payload.pgi.zst.part0000` | 1,000,000,000 | `454a989476759852687a2aba513703e14cc9b3717c4d75cffe82df293ede33bb` |
| `payload.pgi.zst.part0001` | 310,940,560 | `888d1a13d73aa5841c222cd1e4f1433202e8a729a106e1a005eeb63cd7cb8ac2` |
| `proof-receipt.json` | 5,405 | `9c5f945af8b52d21d44331325d908cf66dfbc6430d301d43d4dfa3a23e1c727c` |
| `release-profile.json` | 4,755 | `37ae3f859e23cdf73b6cc4a5f49f3f8fdeff11b955571c827aabb273e89dd1b0` |
| `SHA256SUMS` | 595 | `9498baf3031104f244d2bc7e501af4a34af4f9bcc80aa80bc583f4e9eb659158` |

| `runtime-grch38-v2` asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `runtime-transport.json` | 3,179 | `1c893bd4ab177a7d305fd71528752b8c361c84fb87f7f1a21cfd9aea032a4e87` |
| `runtime-profile.json` | 1,371 | `ce91b332d04a776f12a4603e60cf40894dc002360c13b3541e03d3b748cd3983` |
| `model-manifest.json` | 3,823 | `4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43` |
| `model-NOTICE` | 648 | `fbba767913348642351d7e95b8589619a8bb4a7f3738c5ea6fe266c21434107f` |
| `model.onnx.zst` | 31,144,867 | `741642c98c0aae6a76d4096780c114ba9bd497122868ba0ecf2d85a30d8af568` |
| `reference-manifest.json` | 3,719 | `7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f` |
| `reference-NOTICE` | 793 | `1e3ce49d78cd9089407c54ce92a9e6d3adb92a9f3267185ba9ea64df8a588499` |
| `reference.pgr.zst` | 656,781,805 | `e181eb31a76c8e05782415317450c92d5d7a148cb28afd184e0c7767aa42cc25` |
| `mask-NOTICE` | 978 | `d8ee279f7a97ae25d2bf502b42a4fb480234cc517c0b58f85d6cf6547995bbeb` |
| `domains.pgm.zst` | 3,933,486 | `e8353beba3820e3c4679acb46a673622080fe6f560a02b558bc6d75f50286747` |
| `runtime-release-profile.json` | 8,119 | `97aaa1a06da278534063124d949dca5d5109afed933cacf9fa785ec20d52b110` |
| `SOURCE-SUPPLEMENT.json` | 876 | `270cb560188eb0dd42489ffe83c625d2d25e149e09408fd081aaac9ab3202e50` |
| `SHA256SUMS` | 1,023 | `c54e52f61043ce41808f8e6c6378138b72043c892f8d99c73ca7cdcf8bd4c5ba` |
| `pangolin-5cf94b8-source.tar.zst` | 166,859,188 | `c9b457e8cc527dea27f9e491f0ae68278886c6f9b1a07cb16c3b8dd7309f3174` |
| `pangopup-e6d8497-source.tar.zst` | 865,435 | `a7a93f7f0f8b10d5f257a131253030ef85f2cad21f26d152d6ee5db42274c645` |
| `Pangolin-GPL-3.0.txt` | 35,149 | `3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986` |

The three preferred-source supplements were freshly downloaded from the immutable public v1 runtime release and checked against its public digests and the v2 `SOURCE-SUPPLEMENT.json` before upload. The other runtime members came from the retained Run A `release/` directory. SNV transport members came from Run A `transport/`; its proof, release profile, and checksum list came from Run A `release/`.

## Publication and verification

The operation first compared every retained member's name, size, and SHA-256 with the ticket 0137/0140 authorities and checked the exact release target commits. The GitHub repository reported immutable releases enabled. The operator created private drafts with `gh release create <tag> --draft --latest=false --target <exact commit> --title <exact title> --notes-file <exact body file> -R genomoncology/pangopup`. The operator uploaded explicit admitted paths only with `gh release upload <tag> <paths> -R genomoncology/pangopup`, without `--clobber`. Before either public transition, the operator checked its draft ID, tag, target, title, body digest, and complete remote asset inventory against the local retained bytes.

The two publication calls were `gh api --method PATCH repos/genomoncology/pangopup/releases/394675189 -F draft=false -f make_latest=false` and the same command for release `394675298`. The operator inspected each release ID after the write. No retry, replacement, or draft deletion occurred. No standalone command transcript was retained; these command forms and the public release metadata record the operation.

Fresh unauthenticated API reads verified both published releases, direct tags, titles, target commits, bodies, immutable state, and inventories. `curl -q -fLsS --output <private temporary file>` fetched all 24 assets without credentials or an existing cache. `stat -f` and `shasum -a 256` matched every row above. The normalized public API snapshots of the v1 SNV and runtime releases matched their pre-publication snapshots at SHA-256 `c54e5e4c5e635ddbb00a8c7d680b61e8071dfb3ae741f26d19d32ea6d265ea69` and `087ca197647fafdf03554563886e4ca20624867c2b4acb2111f398a94d4b6091`. Both v1 releases remain public and immutable. GitHub's `releases/latest` still returned v0.4.1, release ID `383676522`.

The two private anonymous-download directories `/tmp/pangopup-snv-v2-anon.4owDe1` and `/tmp/pangopup-runtime-v2-anon.2WYWdt` were removed after their full comparisons. The source-supplement stage at `data/pangopup/runtime-v2-source-supplement-stage-2026-09-23` remains on the operator's machine for audit. No public v1 asset, active installation, production selector, software tag, executable, or container changed. Ticket 0151 owns activation; ticket 0152 owns software publication.

On macOS, `make lint`, `make test`, `make spec`, and `git diff --check` passed from the record worktree. The specification gate reported 207 passed.
