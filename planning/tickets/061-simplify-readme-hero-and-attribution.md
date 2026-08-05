# 061 — Simplify README hero and organization attribution

Status: complete

## Why

The new README branding is visually strong but repeats two standalone logos
above a hero that already contains both marks. The hero's title and route
labels are also more technical than the intended first impression. A new user
should see one image, one plain product statement, and clear attribution to
GenomOncology and BioMCP.

## Scope

- Re-render `docs/images/pangopup-performance.png` from the approved local HTML
  source with:
  - title `High-performance splice-score predictions`;
  - blue route label `SNV`;
  - orange route label `non-SNV`;
  - concise subtitle identifying PangoPup as an open-source, GPL-licensed,
    Rust-based service built on the Pangolin model;
  - a short qualification inside the routing card that supported SNV misses
    and explicit `--model-only` requests use the model. This keeps the requested
    arrow labels short without presenting them as an absolute partition.
- Keep the rest of the reviewed flow, performance figures, mmap/ONNX/SQLite
  explanation, logos, prior-art panel, measurement qualification, dimensions,
  and image-size limits intact.
- Remove the two standalone logo `<img>` elements from `README.md` so the hero
  is the only displayed image. Retain the underlying logo assets because they
  are the documented source marks embedded in the hero.
- Add one concise sentence near the hero: PangoPup was built by
  [GenomOncology](https://genomoncology.com/), which also makes
  [BioMCP](https://biomcp.org/).
- Update the adjacent accessible text and the following named files to match
  the simplified public presentation: `README.md`,
  `docs/images/pangopup-performance.png`, `docs/images/README.md`,
  `scripts/check-readme-images.sh`, `tests/readme-branding.sh`,
  `spec/readme-first-use.md`, and `planning/frontier.md`.
- Exclude product behavior, release/version changes, benchmark changes,
  installer/container changes, and publication outside the normal git push.

## Success Checklist

- The README displays exactly one image and that relative image resolves.
- The hero visibly uses the reviewed title, SNV/non-SNV labels, the in-image
  miss/override qualification, GPL/Rust/Pangolin description, existing
  performance flow, four retained measurement figures with their existing
  approximation and host qualifications, two prior-art sources, and both marks
  without clipping or overlap.
- The README contains working HTTPS links to GenomOncology and BioMCP and
  identifies their relationship in plain English.
- The accessible text remains biologically exact: lookup is used for covered
  SNVs, while supported misses, supported non-SNVs, or explicit override can
  use the model even though the simplified route labels show the ordinary
  SNV/non-SNV distinction.
- Image validation and mutation tests now enforce one displayed README image
  while continuing to validate the retained source marks and fail-closed SVG
  policy. `tests/readme-branding.sh` must prove that a second displayed README
  image, a missing hero, an extra/missing retained source mark, and existing
  SVG script/reference mutations are rejected.
- `spec/readme-first-use.md` pins both exact attribution links, the adjacent
  supported-miss/non-SNV/override semantics, and absence of standalone logo
  `<img>` elements.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **One displayed image.** The hero already carries both logos, so separate
   header marks add height without adding information.
2. **Short route labels.** `SNV` and `non-SNV` make the ordinary fast/fallback
   paths scannable; exact miss and override semantics remain in adjacent text
   and command documentation.
3. **Describe the license, language, and upstream model in the subtitle.** This
   answers what PangoPup is before explaining implementation details.
4. **Retain reusable source marks.** Removing them from README presentation does
   not erase their provenance or their role in regenerating the hero.

## Dependencies

Ticket 060.

## Notes

- Approved editable source:
  `/home/ian/workspace/experiments/164-pangopup-terminal-loop/polished/pangopup-performance.html`.
- Verified public links returned HTTP 200 on 2026-08-05:
  `https://genomoncology.com/` and `https://biomcp.org/`.
- Ian explicitly requested the organizational wording that GenomOncology
  “also makes BioMCP” in this ticket. That direct project-owner instruction is
  the authority for the stronger relationship claim; it is not inferred from
  the public site's HTTP response.
- “Genome Ecology” in voice transcription refers to GenomOncology.
- Do not let the simplified orange label erase exact documented behavior:
  supported SNV lookup misses and `--model-only` can also invoke the model.

## Coordinator Authorship

Coordinator: Codex (`/root`), 2026-08-05. Drafted from the user-requested
visual simplification and the shipped Ticket 060 README.

## Independent Ticket Review

Reviewer: pending

Initial verdict: REJECT. The reviewer required in-image qualification of the
requested SNV/non-SNV arrow labels, qualified rather than “exact” measurement
language, authority for “makes BioMCP,” named changed files, and explicit
one-image/source-mark/SVG/spec mutations. The coordinator incorporated all
findings above. Re-review is pending.

Re-review verdict: ACCEPT. The reviewer approved the in-image qualification of
the requested short route labels, qualified measurement wording, recorded
authority for the BioMCP relationship, named files, and explicit checker/spec
coverage on 2026-08-05.

## Implementation Evidence

Developer: Codex (`/root/ticket061_implementation`), 2026-08-05.

- Updated the approved editable HTML and re-rendered the 2000×1125 hero with
  the exact title, concise GPL/open-source/Rust/Pangolin subtitle, blue `SNV`
  and orange `non-SNV` labels, and the supported-miss/`--model-only` model
  qualification inside the router. Visual inspection found no clipping,
  overlap, or displaced retained content.
- Replaced the repository hero with that 257,351-byte render while retaining
  both documented source marks, four qualified measurements, both prior-art
  sources, the host qualification, and the existing flow.
- Removed both standalone README logo images, added the linked GenomOncology /
  BioMCP maker sentence, and kept the exact routing semantics in adjacent
  accessible text.
- Updated provenance, the rolling frontier, the static first-use contract, and
  the fail-closed image checker. Mutation coverage rejects a second displayed
  image, a missing hero, an extra or missing source mark, oversized/malformed
  raster input, and the retained SVG script/reference cases.
- Focused evidence passed: `bash scripts/check-readme-images.sh`,
  `bash tests/readme-branding.sh`, `mustmatch test spec/readme-first-use.md`
  (10 passed), `bash -n scripts/check-readme-images.sh
  tests/readme-branding.sh`, and `git diff --check`.
- Replacement remediation developer: Codex (`/root/ticket061_remediation`),
  2026-08-05. The original developer's agent thread reached its tool limit, so
  the coordinator assigned the review findings to this replacement. The
  checker now requires the exact hero Markdown line exactly once, rejects all
  other Markdown image forms and case-insensitive HTML `img`/`picture`/`source`
  tags, and scans normalized SVG content so a line break cannot hide a linked
  `href` or `src`. New reference-style Markdown, uppercase HTML, and multiline
  SVG mutations cover those findings without changing the accepted hero or
  README content.

## Adversarial Code Review

Reviewer: Codex (`/root/ticket061_code_review`), 2026-08-05.

- Initial verdict: REJECT. The reviewer found that uppercase HTML and
  reference-style Markdown could bypass the exactly-one-displayed-image rule,
  and that a multiline SVG linked attribute could bypass the existing
  line-oriented safety scan. The README, hero, links, semantics, measurements,
  provenance, and visual layout otherwise passed.
- The original developer's follow-up thread reached the tool limit, so the
  coordinator assigned only those findings to replacement remediation
  developer Codex (`/root/ticket061_remediation`).
- Remediation requires the exact hero line once, rejects every other Markdown
  image form and case-insensitive HTML `img`/`picture`/`source` tag, and scans
  normalized SVG content for multiline linked attributes. Exact mutations
  cover every reported bypass.
- Re-review verdict: ACCEPT. The same reviewer reproduced the prior bypasses,
  confirmed they now fail, reran the image checker, mutation suite, 10-case
  README spec, shell syntax and diff check, and reported no remaining findings.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: Codex (`/root`), 2026-08-05.

- `make lint`: passed. Existing advisory-only duplicate dependency and
  `zstd-sys` semver-metadata warnings remain unchanged; advisories, bans,
  licenses, and sources are accepted.
- `make test`: passed, including the complete Rust workspace, README image
  production/mutation checks, executable delivery, and production release
  qualification.
- `make spec`: 276 passed, 7 skipped.
- `git diff --check`, asset hashes, one-image enforcement, exact attribution
  links, and visual inspection passed. The frontier records the simplified
  presentation as current, not future work.
