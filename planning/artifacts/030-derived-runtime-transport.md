# Ticket 030 derived runtime transport evidence

## Shipped implementation

`pangopup-build runtime-transport` provides deterministic local `pack`,
streaming `verify`, and single-decode atomic `unpack`. The closed ten-file
directory contains a canonical manifest, byte-exact runtime profile and
component metadata/notices, and one pinned Zstandard frame for each of
`model.onnx`, `reference.pgr`, and `domains.pgm`. The profile's SNV identity is
metadata only; no command accepts or opens an SNV bundle.

The checked miniature fixture produces:

- runtime profile:
  `sha256:ea178659923ab4dfc7e0cb88f55b129d994ebad42be4e9dcae76f16f03794940`;
- transport:
  `sha256:0e373cf2183312d8f6f28b286aa49ad2395ea5922bf650e0f15b536908d45f6c`;
- three compressed frames totaling 654 bytes.

Two independent packs are byte-identical. Verification writes no reconstructed
payload. Unpack reconstructs exact model, reference, mask, profile, and notice
bytes. Focused tests reject corruption, truncation, substitution, extra
members, symlinks, non-regular members, unsafe inputs, and occupied outputs
without publishing a partial destination.

## Coordinator production qualification

The coordinator owns this local retained run before adversarial code review.
No generated production payload belongs in Git or a remote. Use the already
qualified retained inputs and measure each operation:

```text
/usr/bin/time -v pangopup-build runtime-transport pack \
  --profile <FOUR_ASSET_PROFILE> \
  --model-bundle <QUALIFIED_MODEL_BUNDLE> \
  --reference-bundle <QUALIFIED_REFERENCE_BUNDLE> \
  --mask <QUALIFIED_DOMAINS_PGM> \
  --output <ABSENT_RETAINED_TRANSPORT>

/usr/bin/time -v pangopup-build runtime-transport verify \
  --transport <RETAINED_TRANSPORT>

/usr/bin/time -v pangopup-build runtime-transport unpack \
  --transport <RETAINED_TRANSPORT> \
  --output <ABSENT_RETAINED_UNPACK>
```

Record the exact ten-name inventory, every file's byte size and SHA-256, each
frame's compression ratio, maximum resident set size from all three commands,
and the three canonical JSON outcomes here. Then pass the reconstructed
profile/model/reference/mask paths to the existing offline
`pangopup assets runtime install` boundary with the retained installed SNV
object. Do not run inference or use the network.

## Production result

The coordinator ran the release build against the exact Ticket 024 profile and
retained qualified members. The run used no network, did not rebuild an
upstream input, did not open or package the SNV score member, and did not
publish anything.

- runtime profile:
  `sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`
- transport:
  `sha256:415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3`
- compressed frame bytes: `691,860,158`
- closed transport directory bytes: `691,874,664`
- reconstructed directory bytes: `812,673,549`

The exact ten-file transport inventory is:

| Name | Bytes | SHA-256 |
|---|---:|---|
| `runtime-transport.json` | 3,179 | `415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3` |
| `runtime-profile.json` | 1,366 | `0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c` |
| `model-manifest.json` | 3,823 | `4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43` |
| `model-NOTICE` | 648 | `fbba767913348642351d7e95b8589619a8bb4a7f3738c5ea6fe266c21434107f` |
| `model.onnx.zst` | 31,144,867 | `741642c98c0aae6a76d4096780c114ba9bd497122868ba0ecf2d85a30d8af568` |
| `reference-manifest.json` | 3,719 | `7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f` |
| `reference-NOTICE` | 793 | `1e3ce49d78cd9089407c54ce92a9e6d3adb92a9f3267185ba9ea64df8a588499` |
| `reference.pgr.zst` | 656,781,805 | `e181eb31a76c8e05782415317450c92d5d7a148cb28afd184e0c7767aa42cc25` |
| `mask-NOTICE` | 978 | `d8ee279f7a97ae25d2bf502b42a4fb480234cc517c0b58f85d6cf6547995bbeb` |
| `domains.pgm.zst` | 3,933,486 | `e8353beba3820e3c4679acb46a673622080fe6f560a02b558bc6d75f50286747` |

The installed byte formats remain unchanged. Compression measurements were:

| Member | Original bytes | Stored bytes | Stored/original | Saved |
|---|---:|---:|---:|---:|
| model | 33,867,142 | 31,144,867 | 0.919619 | 8.04% |
| reference | 772,091,760 | 656,781,805 | 0.850653 | 14.93% |
| mask | 6,703,320 | 3,933,486 | 0.586797 | 41.32% |

The canonical command outcomes were:

```json
{"command":"runtime-transport.pack","compressed_bytes":691860158,"runtime_profile_id":"sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c","status":"ok","transport_id":"sha256:415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3"}
{"command":"runtime-transport.verify","compressed_bytes":691860158,"runtime_profile_id":"sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c","status":"ok","transport_id":"sha256:415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3"}
{"command":"runtime-transport.unpack","runtime_profile_id":"sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c","status":"ok","transport_id":"sha256:415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3"}
```

`/usr/bin/time -v` measured:

| Operation | Elapsed | Maximum RSS | Filesystem output |
|---|---:|---:|---:|
| pack | 16.98 s | 41,288 KiB | 1,351,368 blocks |
| verify | 1.70 s | 14,060 KiB | 8 blocks |
| unpack | 3.60 s | 13,752 KiB | 1,587,296 blocks |

`cmp` proved the three reconstructed payloads byte-identical to their retained
qualified inputs. To exercise the real installer without copying or hashing
15 GB, the already-retained SNV bundle was moved in place from its historical
build-output directory into the shipped receipt-bound XDG layout and admitted
by `pangopup assets status`. The runtime installer then consumed only the
reconstructed profile/model/reference/mask paths and returned:

```json
{"status":"installed","profile_id":"sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c","snv_bundle_id":"sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3","model_bundle_id":"sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43","reference_bundle_id":"sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f","mask_sha256":"sha256:714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702"}
```

Installation took 1.68 seconds with 9,712 KiB maximum RSS. A subsequent
`pangopup assets runtime status` reported the exact profile and all four
component identities ready. After the byte comparisons and installation proof,
the redundant unpacked directory was removed; the production transport, timing
logs, JSON outcomes, and installed runtime remain retained locally outside Git.
