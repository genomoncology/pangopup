# Ticket 033 runtime release preparation evidence

## Shipped implementation

`pangopup-build runtime-release prepare` is the offline, production-only
boundary between the retained qualified ten-file runtime transport and a later
GitHub draft release. It accepts only the pinned transport/runtime-profile
identities, copies each retained member exactly once through a held no-follow
descriptor, and atomically publishes a private read-only stage.

Before copying, it streams each Zstandard frame once through bounded
decompression to authenticate both stored and reconstructed identities. It
does not materialize an uncompressed runtime file or payload-sized heap value;
the retained stored members are subsequently reread once for the byte-exact
copy.

The stage contains the ten byte-identical transport files plus canonical
`runtime-release-profile.json`, deterministic `SHA256SUMS`, and deterministic
`RELEASE-NOTES.md`. The profile records exact component identities, immutable
future URLs, the twelve upstream checkpoint source records, `model.py` and
license records, and converter paths at the target commit. No raw upstream
input, local path, timestamp, hostname, credential, rebuild, recompression,
materialized decoded payload, network operation, tag, release, or upload is
involved.

## Miniature implementation proof

The focused Rust suite uses the hidden test-build-only preparation contract and
proves:

- two preparations are byte-identical closed thirteen-file sets;
- every file is mode `0400`, link count one, and the directory is mode `0500`;
- profile parsing is closed and canonical, checksums have exact order, and
  release notes state the excluded raw inputs;
- the public production entry rejects the valid miniature transport;
- invalid commits, wrong identities, missing/extra/corrupt/truncated,
  symlinked, multiply linked, and non-regular members fail closed;
- injected copy, file-sync, directory-sync, and publication failures leave no
  final output;
- source pathname replacement is detected through retained descriptors; and
- FIFO replacement between nonblocking path admission and readable open is
  rejected without blocking; and
- post-rename parent-sync failure reports a visible publication whose
  durability is unconfirmed.

## Production result

The coordinator committed the reviewed implementation as
`e6d8497aaf1e3db521360ad969252a2ec6fd14e4`; GitHub Actions run
`30582599182` passed the exact `lint`/`test`/`spec` gate before production
preparation. The production command was:

```text
target/debug/pangopup-build runtime-release prepare \
  --transport /home/ian/workspace/data/pangopup-runtime-transport-030/transport \
  --target-commit e6d8497aaf1e3db521360ad969252a2ec6fd14e4 \
  --output /home/ian/workspace/data/pangopup-runtime-release-033/e6d8497aaf1e3db521360ad969252a2ec6fd14e4
```

It returned `status=ok`, tag `runtime-grch38-v1`, transport identity
`sha256:415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3`,
runtime-profile identity
`sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`,
and upload-asset count 12.

The closed output has 13 regular files, each mode `0400` and link count one,
inside one mode-`0500` directory:

| File | Bytes | SHA-256 |
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
| `runtime-release-profile.json` | 8,043 | `d1caf6346bb24378f720056416fa6286f1153ccaf0c6a0778494f557035ef59e` |
| `SHA256SUMS` | 934 | generated proof file |
| `RELEASE-NOTES.md` | 1,267 | local release body |

`sha256sum -c SHA256SUMS` passed all 11 listed upload/profile members. Direct
`cmp` passed for every one of the ten copied transport members. Their exact
stored-byte total is 691,874,664 bytes; the complete local stage occupies
691,884,908 bytes.

`/usr/bin/time -v` observed 53.38 seconds elapsed, 52.44 seconds user CPU,
0.49 seconds system CPU, 17,684 KiB maximum RSS, 1,352,200 filesystem-input
blocks, and 1,351,400 filesystem-output blocks. This was verification plus
byte-exact packaging only: no source rebuild, recompression, network request,
GitHub tag, release, or upload occurred.
