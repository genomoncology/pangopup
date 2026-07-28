# 028 — Use the installed runtime profile for model fallback

Status: complete

## Why

Pangopup can install and atomically activate one compatible SNV, model,
compiled GRCh38 reference, and mask profile. Ordinary lookup automatically
opens the installed SNV index, but a lookup miss or non-SNV still requires
three explicit model-side paths. The installation is therefore inspectable but
not yet sufficient to score every supported variant.

This ticket makes normal lookup use the activated model-side assets only when
the lookup-first router actually requires inference. It is the next outcome
because local end-to-end consumption must work before these derived assets are
packaged for remote delivery.

## Scope

- Add one typed installed-runtime admission API in `pangopup-assets` which:
  - accepts the exact identity of the already-open active SNV provider rather
    than selecting the SNV bundle a second time;
  - validates the active runtime pointer, canonical profile and receipt,
    production tuple, immutable member shape, permissions, link counts, sizes,
    and structural component identities;
  - returns held model, reference, and mask capabilities suitable for the
    existing providers, without returning caller-controlled paths;
  - uses the sole `PGRREF01` reader and Ticket 027's held-reference entry point;
    it must not add another decoder or scan/hash the 15 GB SNV payload or dense
    772 MB reference payload.
- Extend the existing `pangopup-model` admission/open API narrowly so
  `ModelKernel` can authenticate and initialize the exact held installed
  `model.onnx` descriptor. Preserve the pathname API for explicit fallback.
  Do not add a second kernel, scorer, model format, or public raw-file trust
  bypass.
- Change `pangopup lookup` so that:
  - authoritative SNV hits do not inspect runtime state, SQLite, reference,
    mask, or model;
  - a complete explicit model/reference/mask tuple retains precedence and does
    not inspect installed runtime state;
  - with no explicit fallback tuple, a model-required request using the active
    installed SNV bundle admits and uses its compatible installed profile;
  - an explicit `--bundle` is never combined with implicit model-side assets;
  - an explicit `--bundle` request requiring modeling without the complete
    explicit tuple retains `MODEL_ASSETS_REQUIRED` and never inspects installed
    state;
  - partial explicit fallback remains an immediate usage error;
  - `--bundle` plus cache options without the complete explicit fallback tuple
    retains the immediate `CLI_USAGE` error: `model cache options require
    --model-bundle, --reference-bundle, and --mask`.
- Allow the existing SQLite cache options and defaults on the implicit route.
  Cache configuration is resolved only after inference is required and the
  installed profile is admitted. Explicit cache options are valid on that
  route. Malformed cache environment must not affect an authoritative SNV hit.
  Do not add a second cache or an in-memory LRU.
- Preserve compact, redacted installed-state failures:
  - missing profile: exit 1, `ASSETS_MISSING`, `installed runtime profile is
    missing`, `details:null`;
  - unsafe ownership, mode, links, or entries: exit 1, `PROFILE_UNSAFE`,
    `installed runtime state is unsafe`, `details:null`;
  - malformed or changed state: exit 1, `PROFILE_CORRUPT`, `installed runtime
    profile is invalid`, `details:null`;
  - incompatible profile or SNV identity: exit 1, `PROFILE_INCOMPATIBLE`,
    `installed runtime profile is incompatible`, `details:null`.
- Add fast miniature tests through the real installed-capability boundary.
  `pangopup-assets` tests authenticate and open real held miniature descriptors
  through a crate-private trusted-profile parameter. CLI tests inject only a
  crate-private already-open runtime capability or opener and use the same
  production composition path. No public trust bypass, parallel path-based
  fixture route, or test feature may enter the shipped runtime graph.
- Update `AGENTS.md`, `README.md`, `architecture/README.md`,
  `architecture/runtime-data.md`, `architecture/service.md`,
  `planning/faq.md`, and `planning/frontier.md`. Add
  `planning/artifacts/028-installed-runtime-profile-consumption.md` with concise
  route, laziness, parity, and cache-recomposition evidence.

Excluded: network delivery or publication; production-size copying or
rehashing; format changes; inference/batching changes; cache redesign;
HTTP/service lifecycle; Docker/systemd; rollback/GC; signing/SBOM; GPU; HGVS
or transcript projection; and public external effects.

## Success Checklist

- One coherent local installation can return an authoritative precomputed SNV
  or transparently model a supported SNV miss/non-SNV without explicit
  model-side paths.
- Installed and explicit routes produce byte-identical JSONL and table results
  with identical model provenance for checked miniature requests.
- Tests prove hit-path laziness, explicit precedence, no explicit/implicit
  mixing, missing/corrupt/incompatible/hostile state, and pre-return pathname
  replacement failure.
- After successful admission, replacement of a pathname cannot redirect
  queries away from the held admitted inode.
- A restart-equivalent second lookup drops every first-run router, admission,
  provider, kernel, and cache object, independently composes the command again,
  and uses the existing SQLite result without initializing ONNX or reading
  dense reference/model payloads.
- Focused tests and executable specs remain fast and use no production assets.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Bind fallback to the already-open SNV identity.** Rereading a mutable
   active pointer could combine two installations. Admission receives the
   identity coupled to the provider already serving lookup.
2. **Keep lookup-first laziness.** Optional model state cannot slow or break an
   authoritative SNV answer.
3. **Use all-explicit or all-installed assets.** Mixing sources would create an
   unprofiled tuple. A complete explicit tuple wins; an explicit SNV bundle
   cannot borrow implicit model-side assets.
4. **Trust the immutable install and perform bounded runtime admission.** The
   installer authenticated every copied byte. Runtime open rechecks small
   metadata, filesystem safety, sizes, and provider structure without
   payload-wide hashes.
5. **Reuse SQLite across compositions.** Installed identities feed the existing
   complete scoring key. No new caching layer is justified.

## Dependencies

Tickets 023, 024, 025, and 027.

## Notes

- The production profile authority is
  `planning/artifacts/024-four-asset-runtime-profile.json`.
- Inspect `stash@{0}` only for useful hunks. Never apply it wholesale: it also
  contains the rejected duplicate reference decoder, unsafe constructors,
  pathname races, and test-only production features.
- Existing explicit fallback behavior is the output, cache, and error-order
  oracle. No retained production asset is opened, copied, rebuilt, or
  published in this ticket.

## Coordinator Authorship

Coordinator: Codex `/root`

Drafted from the shipped Ticket 027 boundary, the closed handoff, historical
Ticket 026's accepted behavior, and the current frontier. The rejected
implementation is evidence, not a patch set.

## Independent Ticket Review

Reviewer: Codex `/root/ticket028_design_review`

Initial verdict: **REJECT**. The reviewer found that the ticket did not
authorize the model kernel's required held-descriptor open, left two explicit
bundle/cache grammar shapes ambiguous, did not freeze installed error messages,
allowed a weak same-composition cache proof, and did not define the cross-crate
miniature seam.

The coordinator added the narrow existing-kernel held-descriptor API, pinned
the explicit-bundle and cache grammar, froze exact redacted errors, required
all first-run objects to be dropped before independent recomposition, and
split real held-descriptor testing in assets from already-open capability
injection in CLI.

Revised verdict: **ACCEPT**. The reviewer confirmed that every finding is
resolved, the post-Ticket-027 design is feasible, and this is the correct next
bounded outcome.

## Implementation Evidence

Developer: Codex `/root/ticket028_implementation`

Implemented one held-descriptor installed-runtime admission boundary bound to
the already-open SNV bundle identity, a held model-kernel opener, and lazy CLI
composition with explicit precedence and no source mixing. Miniature tests
cover real installed capabilities, pre- and post-admission pathname
substitution, malformed and unsafe installed state, all four exact redacted
CLI error classes, hit-path laziness under malformed cache environment,
JSONL/table and SNV-index-miss parity, and restart-equivalent SQLite reuse
without dense-provider initialization.

Focused tests passed. The complete local gate also passed:

- `make lint`
- `make test`
- `make spec` — 194 passed, 3 skipped

## Adversarial Code Review

Reviewer: Codex `/root/ticket028_code_review`

Initial verdict: **REJECT**. The reviewer found that the implementation did
not directly prove the complete installed error matrix, post-admission
held-inode behavior, installed SNV-miss parity, or malformed-cache-environment
hit laziness.

Remediation added those miniature tests and exposed one small contract defect:
an unexpected installed entry was classified corrupt rather than unsafe. The
developer corrected that distinction while preserving missing-member
classification.

Revised verdict: **ACCEPT**. The reviewer confirmed every prior finding is
resolved and found no unsafe reopening, duplicate decoder, source mixing,
eager hit-path work, cache regression, fixture-only product path, unrelated
change, or stale claim.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: Codex `/root`

The coordinator inspected the final diff and current/future documentation,
then reran `make lint`, `make test`, `make spec` (`194 passed, 3 skipped`), and
`git diff --check`; all passed. No production asset was opened, copied,
rebuilt, hashed, installed, or published.
