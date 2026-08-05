# 060 — README branding and performance overview

Status: complete

## Why

The compact README explains PangoPup but opens as unbranded prose. A new user
should immediately see the PangoPup and GenomOncology identities and understand
the lookup-first performance design. The already-reviewed presentation artwork
contains that concise story, but it currently lives only in a gitignored local
experiment.

## Scope

- Add repository-owned, web-sized PangoPup and GenomOncology logo assets under
  `docs/images/`. Record the pangolin artwork's author, item URL, and public-
  domain terms, and record the organization-owned logo's canonical internal
  source and redistribution authority.
- Add a web-sized copy of the finished PangoPup performance overview under
  `docs/images/` and place it near the top of `README.md`.
- Add a compact centered brand lockup near the README title, with useful alt
  text and ordinary relative paths that render on GitHub.
- Correct the approved hero before export: model fallback applies only to
  supported variants, not “everything else”; the fallback route is a supported
  non-SNV, supported lookup miss, or explicit model override. Remove the
  release number so the durable architecture image cannot become stale at the
  next application release.
- Put a concise text equivalent directly after the hero. It must state the
  lookup/model/cache routing, mmap behavior, the four measurements and their
  retained Ryzen/Linux/warm-cache scope, and the two principal prior-art
  sources. It may use a collapsed `<details>` block to keep the first-use path
  compact.
- Preserve the README's current quick-start-first structure and factual claims.
- Update `NOTICE` only if needed to make the imported icon's attribution and
  public-domain terms explicit.
- Exclude presentation source, PowerPoint, renderer dependencies, and changes
  to product behavior, release assets, benchmarks, or published scoring data.

## Success Checklist

- The README visibly includes both project/organization branding and the
  lookup-first performance overview before Quick start.
- Every referenced image exists, renders from a relative GitHub path, and has
  meaningful alt text. The hero is at most 2000×1125 and 400,000 bytes; each
  logo is at most 800×400 and 100,000 bytes.
- A focused test rejects image links that do not resolve, assets beyond those
  limits, and SVGs containing scripts, event handlers, `foreignObject`, or
  external/data references.
- The adjacent text equivalent makes the hero's routing, mmap, measurements,
  provenance, and prior art available without reading rasterized text.
- The overview accurately communicates mmap lookup, CPU ONNX fallback, SQLite
  reuse, measured performance, download/install size, and the two principal
  prior-art sources.
- Visual review and the adjacent text preserve these exact retained claims:
  `0.441 µs` already-open filtered SNV lookup p50 (Ticket 004); about `12 MiB`
  one-SNV CLI peak RSS; `4.3 s → 0.7 ms` uncached model versus fresh-service
  SQLite-hit medians; and `2.44 GiB` download / `14.76 GiB` installed (the
  latter three from Ticket 053). The Ryzen 7 5825U/Linux/warm-page-cache scope
  remains visible, and no value is presented as a cross-host guarantee.
- Imported artwork has a durable source/license record; no opaque or
  unattributed third-party binary is added.
- A focused README image-link/size check passes, followed by `make lint`,
  `make test`, and `make spec`.

## Decisions

1. **Retain a checked-in hero image.** Linking to an external image is smaller
   but can disappear or change; a small repository-owned PNG makes the README
   stable and reviewable.
2. **Use separate reusable logo assets.** Cropping logos only inside the hero
   would make them hard to reuse and less accessible; dedicated assets support
   the README lockup without presentation tooling.
3. **Keep the README compact.** The hero supplements the short prose and quick
   start; it does not restore engineering history or duplicate the full
   architecture documents.
4. **Use measured claims already retained by the project.** The hero and text
   identify the retained AMD Ryzen 7 5825U/Linux/warm-page-cache context and do
   not imply cross-host guarantees.
5. **Keep architecture art release-neutral.** Embedding `v0.3.0` would make a
   stable flow diagram stale at the next release, so the checked-in derivative
   omits the application version.

## Dependencies

Ticket 059 (compact first-user README and retained performance facts).

## Notes

- Approved local source artwork is in
  `/home/ian/workspace/experiments/164-pangopup-terminal-loop/polished/`.
- The finished 2000×1125 source image is
  `output/pangopup-performance.png`; create a web-appropriate derivative rather
  than committing presentation-only files.
- The pangolin mark is “Pangolin” by OpenClipart user AreYouPrepared, item 355622
  (`https://openclipart.org/detail/355622/pangolin`), published as CC0/public
  domain. Prefer the retained vector source over a raster crop.
- The organization logo source is
  `/home/ian/workspace/archive/marketing/.agents/skills/go-pptx/assets/biomcp/go-logo.png`.
  It is GenomOncology-owned branding added to a GenomOncology repository; state
  that provenance in the durable asset record and do not imply an open-content
  license for the logo.
- “Genome Ecology” in voice transcription refers to the GenomOncology logo
  shown in the approved slide.

## Coordinator Authorship

Coordinator: Codex (`/root`), 2026-08-05. Drafted from the shipped v0.3.0
README and the user-approved presentation artwork.

## Independent Ticket Review

Reviewer: pending

Initial verdict: REJECT. The reviewer required correction of the overbroad
model claim, an adjacent accessible text equivalent, complete artwork
provenance, objective image/SVG gates, and removal of the release-coupled hero
version. The coordinator incorporated all five findings above. Re-review is
pending. The first re-review required the ticket itself to freeze all four
measurement values and meanings rather than delegating them to the local
image; those exact claims are now in the success checklist. Second re-review
verdict: ACCEPT. All findings are resolved; the reviewer approved the bounded
scope, supported routing language, exact measurements, accessible text
equivalent, provenance, objective asset gates, and release-neutral hero on
2026-08-05.

## Implementation Evidence

Developer: Codex (`/root/ticket060_implementation`), 2026-08-05.

- Added the public-domain PangoPup vector mark, the organization-owned
  GenomOncology logo, and a corrected 2000×1125 performance overview under
  `docs/images/`. The hero is 265,885 bytes; the logos are 14,843 and 9,244
  bytes. All are within the reviewed limits.
- Corrected the approved local HTML source before rendering: model fallback is
  limited to supported non-SNVs, supported lookup misses, and explicit model
  override; the exact `0.441 µs` value is shown; and the release number is
  absent. A visual review of the final repository PNG found no clipped or
  overlapping content.
- Added the centered README brand lockup, hero, and adjacent collapsed text
  equivalent before Quick start. The text records routing, mmap behavior, all
  four measurements and their Ryzen/Linux/warm-cache scope, retained evidence,
  and both principal prior works.
- Added durable asset provenance and hashes in `docs/images/README.md`, with
  the public-domain icon attribution also carried in `NOTICE`.
- Added `scripts/check-readme-images.sh` and the gate-level
  `tests/readme-branding.sh`. Red-first evidence: the focused test initially
  failed on the absent `docs/images/pangopup.svg`. After implementation it
  accepts the production assets and rejects deliberate missing-image,
  byte-limit, dimension-limit, SVG script, event-handler, `foreignObject`,
  external-reference, and data-reference mutations.
- Focused checks passed:
  `bash -n scripts/check-readme-images.sh tests/readme-branding.sh`,
  `bash tests/readme-branding.sh`, direct
  `scripts/check-readme-images.sh`, and `git diff --check`.
- Code-review remediation extends README image discovery to single-quoted HTML
  `src` attributes and rejects every SVG `href`, `xlink:href`, or `src`
  attribute, plus every CSS `url(...)`, `@import`, document type, and entity
  declarations, without rejecting normal namespace declarations. Exact
  negative mutations cover the single-quoted missing link, external and
  relative CSS URLs, CSS import, external entity, relative linked image, and
  JavaScript link forms. The adjacent text equivalent now explicitly states
  that every score identifies precomputed lookup, Pangolin model, or SQLite
  cache provenance.
- Per the repository lifecycle, final `make lint`, `make test`, and `make spec`
  remain for the coordinator after independent code-review approval.
- Final-gate remediation updates the executable README contract from 220 to a
  tight 260-line ceiling while retaining the 1,700-word ceiling. The added
  allowance is bound to checks that both logo paths, the hero path, and the
  adjacent performance text/provenance statement all appear before Quick
  start. The spec also permits exactly the two retained measurement-evidence
  links and the required warm-page-cache scope while continuing to reject
  other internal-maintainer links and trivia.

## Adversarial Code Review

Reviewer: Codex (`/root/ticket060_code_review`), with final replacement
verification by Codex (`/root/ticket060_final_code_review`), 2026-08-05.

- Initial review rejected two bypasses: single-quoted README image links were
  not discovered, and CSS-linked SVG resources were incompletely rejected. It
  also required the accessible text to state lookup/model/cache provenance.
- Re-review drove the SVG boundary to a simple fail-closed contract: reject all
  SVG `href`, `xlink:href`, and `src` attributes; all CSS `url(...)` and
  `@import`; script/event/`foreignObject`; and document type/entity forms.
  Exact mutations now cover every discovered bypass while the production SVG
  and ordinary namespace declarations remain accepted.
- The original reviewer reached the agent thread's follow-up limit after
  identifying the last relative CSS URL case. A fresh independent read-only
  reviewer reproduced that exact case against the remediation, reran the
  focused suite, reviewed the full diff and assets, and returned ACCEPT with no
  material findings.
- Final review confirmed visual integrity, release neutrality, metric accuracy
  and scope, supported routing language, accessibility, attribution and hashes,
  objective asset limits, test integration, and absence of unrelated changes.
- The first full spec gate exposed the previous README's 220-line ceiling. The
  developer replaced it with a tight 260-line/1,700-word contract that reserves
  the additional space for both logos, hero, accessible performance text and
  provenance; fixes heading order; and permits only the two retained
  measurement links. The replacement independent reviewer accepted that
  remediation after a 10/10 focused mustmatch run.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: Codex (`/root`), 2026-08-05.

- `make lint`: passed. Existing advisory-only duplicate dependency and
  `zstd-sys` semver-metadata warnings remain unchanged; advisories, bans,
  licenses, and sources are all accepted.
- `make test`: passed, including the production README assets and every
  deliberate image/SVG rejection mutation.
- Initial `make spec`: 274 passed, 1 failed, 7 skipped because the pre-ticket
  220-line README ceiling did not admit the reviewed branding/accessibility
  block. That finding returned to the developer and independent reviewer.
- Final `make spec`: 276 passed, 7 skipped.
- `git diff --check` and the documentation stale-claim scan passed. The hero is
  release-neutral, its metrics link to retained evidence, and the frontier now
  records this outcome as established rather than future work.
