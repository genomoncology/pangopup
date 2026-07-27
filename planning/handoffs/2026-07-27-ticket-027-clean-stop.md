---
status: Open
date: 2026-07-27
team: pangopup
title: Clean stop after reference provenance boundary
---

# State in one line

`main` is clean, pushed, and has no live ticket. Ticket 025 and the Ticket 027
prerequisite are complete; installed-runtime consumption is the next outcome.

## STOP — preserved rejected Ticket 026 work

One local reversible stash exists:

```text
stash@{0}: On main: ticket-026-rejected-runtime-consumption
```

Do **not** pop or apply it wholesale. It contains useful routing, exact error,
installed-capability, parity, and SQLite-recomposition work mixed with rejected
reference approaches. Code review found pathname races, test instrumentation in
the production feature graph, safe identity-forging constructors, fixture-only
tests, and finally a duplicated reference decoder. Ticket 027 removed the
underlying blocker. A new reviewed consumption ticket may inspect and
selectively reapply useful hunks only after its design is accepted.

The deferred Ticket 026 rationale is durable in commit `0d2b7dd`; its live
ticket was removed by `cca83da`. The original reviewed-ready ticket is
available from git history at commit `6de4c4c`.

## What this session completed

- Ticket 025 shipped offline coherent runtime-profile installation:
  - implementation `798a949`;
  - cleanup `253e121`;
  - one shared installer lock, immutable XDG component/profile storage,
    atomic activation, bounded status, exact collision authentication, and
    descriptor-held hostile-path handling.
- Ticket 026 attempted installed-profile consumption, but independent review
  exposed the mixed reference reader/writer provenance boundary. No rejected
  product code was committed or pushed.
- Ticket 027 fixed that prerequisite:
  - reviewed ticket `55c63b2`;
  - implementation `40a4f25`;
  - cleanup `8299ad9`.
- `PGRREF01` now has one shared wire/layout implementation, one writer, and one
  mmap reader/provider behind the unchanged
  `pangopup_index::reference::*` facade.
- Byte-producing reference build code is separate from
  certification/inspection/qualification code.
- Future reference builds use
  `pangopup.reference-builder-source.v2`, final source digest
  `09cd44449b77592e4b9948cc0756e736b01ecf5220b3d5312c52b12b6b6e9c65`.
  The v2 inventory binds causal root and facade routing, including mutation
  controls for grouped imports and outer/inner module redirection. Reader-only
  changes remain neutral.
- Installed reference admission has one documented unsafe raw authority
  boundary returning an opaque safe provider that maps the exact supplied
  descriptor; it never reopens the payload pathname.
- The miniature v1 payload and notice were reproduced byte-for-byte. Static
  tests preserve the Ticket 024 production identities. The retained 772 MB
  production reference member was never opened, read, copied, rebuilt,
  repacked, installed, or published.
- Final Ticket 027 coordinator gate:
  `make lint`, `make test`, `make spec` (`194 passed, 3 skipped`), and
  `git diff --check` all passed.

## Next — priority order

1. Draft one new installed-runtime-consumption ticket from current
   `planning/frontier.md`, the accepted behavior in historical Ticket 026, and
   the now-shipped held-reference capability. Do not simply reopen Ticket 026.
2. Independently review that ticket before implementation. Preserve these
   already-settled behaviors:
   - bind installed runtime admission to the exact already-open active SNV
     provider identity, never a second mutable active-pointer read;
   - authoritative SNV hits inspect neither runtime profile, SQLite, reference,
     mask, nor model;
   - complete explicit fallback wins; partial explicit fallback is usage
     error; an explicit SNV bundle never mixes with implicit installed model
     assets;
   - use the existing persistent SQLite cache, with a fully recomposed
     second-run proof that cache hits avoid dense reference/model reads and
     ONNX initialization;
   - test the real installed capability path for SNV miss and non-SNV in JSONL
     and table forms;
   - keep exact redacted missing/unsafe/corrupt/incompatible error contracts.
3. During implementation, inspect the rejected stash selectively. Production
   and tests must consume the same already-open component shape; test-only
   accounting may differ, but no parallel path-based fixture route is allowed.
4. After installed consumption is reviewed and shipped, return to derived
   model/reference/mask transport and publication. HTTP, container, and
   deployment remain later outcomes.

## Do not re-litigate

- SQLite is the persistent model-result cache. Do not add an in-memory LRU
  without new evidence.
- Pangopup is standalone and does not use Genome or other GenomOncology
  software.
- Do not republish raw Zenodo, NCBI FASTA, or GENCODE source inputs. Future
  release assets are Pangopup's derived SNV/model/reference/mask artifacts and
  required notices only.
- The existing production reference bundle and canonical Ticket 024 profile
  remain valid. Builder provenance is descriptive, not a runtime compatibility
  key; do not rebuild the large reference because v2 exists.
- Keep one reference decoder. Do not restore the rejected duplicate
  `reference_admission` decoder.
- Docker/systemd/Kubernetes own start/stop/restart. Pangopup's service remains
  a foreground process; no custom daemon supervisor.

## Gotchas

- The current-v1 miniature canonical manifest oracle is
  `8617204d0678ea23aa00e288e94bbf2622cf3884cf26562f65fb85eda5b18bd2`,
  not the earlier Ticket 013 value.
- Ticket 024 production reference facts are:
  - bundle `sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`;
  - member size `772091760`, SHA-256
    `sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82`;
  - sequence set
    `sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4`;
  - profile
    `sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`.
- Source-fingerprint mutation tests must bind both implementation inputs and
  the module/facade edges that route to them. A derivation test can reproduce
  an incomplete model, so keep independent rebinding controls.
- SNV v1 still intentionally hashes the mixed core file. Ticket 027 preserves
  its current digest; it does not change future SNV-v1 churn semantics.

## Pointers

- `AGENTS.md`
- `README.md`
- `planning/frontier.md`
- `architecture/reference.md`
- `architecture/runtime-data.md`
- `architecture/decisions/0021-atomic-local-runtime-profile.md`
- `architecture/decisions/0022-reference-reader-provenance-boundary.md`
- `planning/artifacts/025-local-runtime-profile-installation.md`
- `planning/artifacts/027-reference-reader-provenance-boundary.md`
- git history: `0d2b7dd`, `40a4f25`, `8299ad9`
