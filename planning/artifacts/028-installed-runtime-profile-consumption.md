# Ticket 028 — installed runtime consumption evidence

## Route and laziness

- Ordinary lookup opens the active SNV bundle first and binds any later
  fallback admission to that provider's exact bundle identity.
- An authoritative SNV answer returns before runtime-profile, SQLite,
  reference, mask, or model admission.
- A complete explicit model/reference/mask tuple wins. An explicit `--bundle`
  never borrows installed model-side assets.

## Held installed capabilities

- Runtime admission validates the active pointer, canonical trusted profile,
  receipt, immutable topology, ownership, modes, link counts, sizes, and
  structural component identities through held directory/file descriptors.
- The sole `PGRREF01` reader maps the admitted reference descriptor. Runtime
  does not hash or scan the 15 GB SNV member or 772 MB reference member.
- Mask identity is derived from its held descriptor. The model session reads
  and authenticates `model.onnx` through the admitted descriptor only when
  inference is required.
- A controlled pathname replacement before capability return is rejected.
  A separate successful-admission test then replaces all three model,
  reference, and mask pathnames with same-size corrupt files and proves the
  returned capabilities still query/reference, query/mask, and score/model
  through the admitted original inodes.

## Output and cache recomposition

Miniature JSONL and table cases compare the implicit installed route with the
existing explicit route byte for byte, including a one-base SNV index miss.
The test drops the first command
composition, reparses the request, reopens SQLite, repeats bounded admission,
and returns the stored complete result without initializing dense providers.

All focused tests use checked miniature SNV, reference, mask, and ONNX fixtures.
No production asset is opened, copied, rebuilt, hashed, or published.

Malformed profile bytes, unsafe mode, unsafe link count, unexpected installed
entry, missing profile, and incompatible SNV identity are exercised through
real miniature admission. CLI composition separately pins compact redacted
`ASSETS_MISSING`, `PROFILE_UNSAFE`, `PROFILE_CORRUPT`, and
`PROFILE_INCOMPATIBLE` JSON with `details:null`. A process-isolated CLI test
sets malformed cache environment values and proves an authoritative installed
SNV hit neither resolves cache configuration nor discovers missing runtime
state.
