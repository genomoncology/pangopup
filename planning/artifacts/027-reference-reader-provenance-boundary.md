# Ticket 027 — reference reader/provenance boundary

## Result

Pangopup retains one `PGRREF01` format, writer, structural parser, packed
decoder, ambiguity overlay, and provider. The public
`pangopup_index::reference::*` facade is unchanged, while the implementation is
now split into wire/layout, writer, and reader files. Installed admission can
map the exact descriptor authenticated by installation through one documented
unsafe constructor and returns an opaque safe provider.

Future reference builds emit
`pangopup.reference-builder-source.v2`. The checked inventory contains the
wire codecs, writer, byte-producing build adapter, shared causal errors,
locked dependency/root projections, and a compiled-behavior projection for
all 25 `Grch38Contig` values. Reader, runtime admission, certification,
qualification, CLI, delivery, and service source are excluded. The retained
SNV v1 fingerprint is unchanged.

## Preservation proof

- Current-v1 miniature oracle:
  - `reference.pgr`: 4,560 bytes,
    SHA-256 `0ef815ffb3fbb897e880e56afcb57e1edb41f3707784f591c0457581c2e9a3d5`
  - `NOTICE`: 279 bytes,
    SHA-256 `faea3b1976bf4e15f95bad3906144d83b4441f860d3c5b87ab406205e47262db`
  - canonical v1 manifest SHA-256
    `8617204d0678ea23aa00e288e94bbf2622cf3884cf26562f65fb85eda5b18bd2`
- The v2 miniature reproduces the exact payload and notice. Replacing only
  its builder source digest reconstructs the pinned v1 canonical manifest.
- Reference v2 builder source SHA-256:
  `09cd44449b77592e4b9948cc0756e736b01ecf5220b3d5312c52b12b6b6e9c65`.
- The checked root projection binds the `source_fingerprint` module edge, and
  a separate derived facade projection binds the crate-level public reference
  module, byte-producing build entry point, wire layout, and writer reexports.
  Rebinding any of those edges changes v2; reader-only facade export changes do
  not.
- A deterministic pathname substitution test proves held admission continues
  to query the supplied inode.
- Selected-input mutations change v2; representative reader,
  certification, admission, CLI, delivery, and service changes do not.
- The behavior-derived core projection checks codes 1–25, canonical names,
  round trips, uniqueness/order, and `chrM`.

Static production preservation pins:

- bundle ID
  `sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`
- member: 772,091,760 bytes,
  `sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82`
- sequence set
  `sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4`
- Ticket 024 profile
  `sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`

No production asset was opened, read, built, copied, installed, repacked, or
published.
