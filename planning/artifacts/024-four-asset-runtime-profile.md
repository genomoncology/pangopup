# Ticket 024 four-asset runtime profile evidence

## Result

The production-only preparation command successfully created one canonical,
path-free compatibility profile for the already-qualified SNV, model,
reference, and mask assets.

- profile schema: `pangopup.runtime-profile.v1`
- profile ID:
  `sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`
- exact file:
  `planning/artifacts/024-four-asset-runtime-profile.json`
- bytes: 1,366
- file SHA-256:
  `0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`
- mode: `0600`
- links: 1
- final byte: `0x7d` (`}`), proving there is no trailing newline

The profile ID is the SHA-256 of the exact RFC 8785/JCS bytes, so the profile
ID and file digest are intentionally identical.

## Inputs

- SNV bundle:
  `/home/ian/workspace/data/pangopup/bundles/sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3`
- model bundle:
  `/home/ian/workspace/data/pangopup-model-018/bundle`
- reference bundle:
  `/home/ian/workspace/data/pangopup-reference-production-011/1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01/bundle`
- mask:
  `/home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb/prepare/candidates/domains.pgm`

The command inspected the retained compiled assets only. It did not read raw
Zenodo, NCBI FASTA, GENCODE, or checkpoint inputs; initialize ONNX Runtime; run
inference; copy or rebuild a runtime asset; install or activate a profile; use
the network; or publish anything.

## Command

The output path was confirmed absent before this one successful invocation:

```text
target/debug/pangopup-build runtime-profile prepare \
  --snv-bundle /home/ian/workspace/data/pangopup/bundles/sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3 \
  --model-bundle /home/ian/workspace/data/pangopup-model-018/bundle \
  --reference-bundle /home/ian/workspace/data/pangopup-reference-production-011/1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01/bundle \
  --mask /home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb/prepare/candidates/domains.pgm \
  --output planning/artifacts/024-four-asset-runtime-profile.json
```

The command returned:

```json
{"status":"ok","command":"runtime-profile.prepare","profile_id":"sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c","bytes":1366}
```

## Boundaries

This small JSON file is the compatibility authority for later local
installation and activation work. It is not a public release manifest and
contains no local paths, URLs, timestamps, hostnames, credentials, or mutable
aliases.

The command held the SNV manifest, notice, and score descriptors but did not
read, hash, decode, mmap, or page through the 15 GB score payload. That
payload's complete integrity remains owned by the existing
installer/certifier. Model, reference, and mask authentication reused their
existing bounded or full held-descriptor production checks.
