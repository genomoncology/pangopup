# 029 — Make the maintainer CLI accurately describe itself

Status: complete

## Why

`pangopup-build` owns the existing source, bundle, model, reference, transport,
release, compatibility, and runtime-profile maintenance commands. Its root
usage text still lists only four early prototype commands, while `--help`,
`--version`, and nested help are treated as usage failures.

The next product outcome will add packaging for three derived runtime assets.
Before that expands the maintenance surface, the current executable needs one
truthful, tested command catalog. This ticket changes discoverability only; it
does not add, remove, or change any maintenance operation.

## Scope

- Give `pangopup-build` conventional informational behavior:
  - `pangopup-build --help` and `pangopup-build -h` write the complete current
    command catalog to stdout and exit 0;
  - `pangopup-build --version` and `pangopup-build -V` write
    `pangopup-build <workspace-version>` to stdout and exit 0;
  - `<namespace> --help|-h` lists that namespace's current actions;
  - every supported leaf path accepts `--help|-h`, describes
    its exact required arguments and closed value choices, performs no
    operational work, writes stdout only, and exits 0.
- Recognize informational behavior only for these exact argument shapes, with
  no operands or additional flags:
  - `[--help|-h]`;
  - `[--version|-V]`;
  - `[LEAF,--help|-h]`;
  - `[NAMESPACE,--help|-h]`;
  - `[NAMESPACE,ACTION,--help|-h]`.
  A help flag anywhere else—such as `--help reference`, `reference build
  --profile x --help`, or a help path with extra tokens—continues through the
  established usage-error path and cannot trigger operational work.
- Cover exactly the shipped dispatcher:
  - `inspect`, `prototype-roundtrip`, `prototype-open`,
    `benchmark-corpus`, `build`, and `verify`;
  - `reference build|inspect|window`;
  - `transport pack|verify|unpack`;
  - `release prepare|upload-asset`;
  - `compatibility inspect|capture`;
  - `model evidence|convert|inspect|qualify`;
  - `runtime-profile prepare`.
- Keep no-argument, unknown-command, missing-value, duplicate-flag, and
  operational failure behavior byte-for-byte unchanged, including exit class,
  redaction, and existing stream. Most command families emit JSON failures on
  stderr; the `reference` namespace deliberately emits its established JSON
  usage and operational failures on stdout. Do not normalize that exception in
  this ticket.
- Establish one checked command/help catalog in `pangopup-build` as the source
  of command-path recognition. Map cataloged paths to a small internal leaf
  enum and dispatch that enum exhaustively, or use an equivalently single-owned
  structure. A command path cannot be recognized unless it is cataloged, and
  help is rendered from the same entry. Do not maintain an independent second
  dispatcher list, scan Rust source/Markdown text, adopt a CLI framework, or
  add a runtime dependency for this documentation-only change.
- Add `spec/build-cli.md` proving top-level help/version, namespace and leaf
  help, stdout/stderr/exit behavior, complete catalog coverage, no side
  effects, and preservation of representative legacy JSON usage failures.
  Add inside-out unit tests for catalog uniqueness/completeness and
  informational-position parsing.
- Update `README.md`, `planning/faq.md`, and `planning/frontier.md`. Resolve
  `planning/issues/2026-07-24-maintainer-interface-and-documentation-drift.md`
  only after its help/version and narrow stale-catalog requirements are
  executable. Historical ADR consequence sections remain acceptance-time
  snapshots; current-state documents must not present stale behavior as
  shipped.

Excluded: new packaging or publication commands, asset-format changes, public
uploads, GitHub settings, release subprocess lifecycle, runtime `pangopup`
grammar, HTTP, Docker, service lifecycle, broad prose linting, generated shell
completion, man pages, or a CLI framework migration.

## Success Checklist

- A maintainer can discover every currently supported command and exact leaf
  grammar through successful stdout help without touching an input or output
  path.
- `pangopup-build --version` derives from the workspace package version.
- A checked catalog-to-dispatch test fails if a supported command is added,
  removed, duplicated, or left undocumented.
- Representative pre-ticket no-argument, unknown, partial, duplicate, and
  operational errors remain byte-for-byte compatible. The oracle covers both
  ordinary JSON-on-stderr commands and the `reference` namespace's established
  JSON-on-stdout exception.
- `spec/build-cli.md` fails against the pre-ticket binary and passes after the
  change.
- No command performs file, process, network, model, or release work on a help
  path.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Repair the current hand-written CLI instead of adopting a framework.**
   A framework could generate polished help but would also rewrite parsing and
   error behavior immediately before release work. A small checked catalog
   provides truthful help with much lower compatibility risk.
2. **Document namespaces and leaves, not only the root list.** Root-only help
   would still force maintainers to provoke errors to learn required flags.
   Exact leaf help makes the closed grammar usable without opening assets.
3. **Preserve operational errors.** Existing scripts and executable specs rely
   on compact JSON failures and their existing streams. Help/version are new
   successful informational positions; malformed operational requests do not
   change meaning. In particular, reference failures remain on stdout.
4. **Use a narrow structural stale check.** The test binds dispatcher command
   names to the catalog and checks named current-state docs. It does not scan
   every Markdown sentence or become a slow general verifier.
5. **Do not mix packaging into this ticket.** Packaging needs this reliable
   interface but has separate format, size, attribution, and release decisions.

## Dependencies

Ticket 028.

## Notes

- The repository gate is exactly `make lint`, `make test`, and `make spec`;
  there is no `make check`.
- `make spec` builds both `pangopup` and `pangopup-build` and discovers every
  `spec/*.md` file.
- Preserve public-repository hygiene: no machine-absolute paths, secrets,
  production asset reads, network requests, or external mutations.
- Existing specs are the failure-output oracle. Evidence shown in this ticket
  is illustrative and must not become generated bookkeeping.

## Coordinator Authorship

Coordinator: Codex `/root`

Drafted from the shipped Ticket 028 outcome, the current dispatcher, the
release-ready frontier, and the open maintainer-interface issue. This removes
the narrow blocker before derived-asset packaging.

## Independent Ticket Review

Reviewer: Codex `/root/ticket029_design_review`

Initial verdict: **REJECT**. The reviewer found that the draft incorrectly
claimed every preserved failure used stderr even though the `reference`
namespace intentionally uses stdout, left accepted help argument positions
ambiguous, and allowed a catalog test that could not actually detect an
uncataloged hand-written match arm.

The coordinator preserved each command family's exact stream and explicitly
called out the reference exception; limited informational behavior to five
exact argv shapes; required all other help placements to retain usage errors;
and made the catalog the single source of command-path recognition rather than
an independently checked documentation list.

Revised verdict: **ACCEPT**. The reviewer confirmed that the command inventory
is complete, the compatibility and help-position contracts are precise, the
single-owned catalog is feasible without a framework, and this is the correct
bounded prerequisite for derived-runtime asset packaging.

## Implementation Evidence

Developer: Codex `/root/ticket029_implementation`

- Added one 21-leaf checked catalog that owns path recognition, help synopsis,
  summaries, namespaces, and the exhaustive internal dispatch enum. The
  operational adapters now receive that enum instead of matching a second list
  of command strings.
- Added exact root/version, namespace, and leaf informational parsing. All
  cataloged help paths are stdout-only and return before any filesystem,
  process, model, release, or network adapter can run; misplaced and extended
  help remains operational input.
- Preserved the pre-ticket operational oracle, including the four-line legacy
  root usage, compact JSON exits, and reference usage/operational JSON on
  stdout. `spec/build-cli.md` exercises the complete catalog, every help path in
  an empty directory, strict help placement, and representative legacy
  failures.
- Updated README, FAQ, and frontier to point maintainers to the checked
  catalog, and closed the recorded maintainer-interface drift with its
  executable resolution.
- Focused binary unit tests: 6 passed. Full repository gate:
  `make lint` passed; `make test` passed; `make spec` passed with 209 passed and
  3 skipped.

Remaining concern: none. This changes discoverability only and adds no
dependency or maintenance operation.

Code-review remediation removed the separate namespace-name array: namespace
recognition, help, and display order now derive from first occurrence in the
leaf catalog, with tests proving every catalog namespace is discoverable. The
executable version spec now derives its oracle from the bounded local workspace
`Cargo.toml` instead of repeating `0.1.0`.

## Adversarial Code Review

Reviewer: Codex `/root/ticket029_code_review`

Initial verdict: **REJECT**. The reviewer found a second independently
maintained namespace list beside the leaf catalog and a hardcoded `0.1.0`
version oracle in the executable spec.

Remediation removed the namespace list so namespace recognition, help, and
display order derive from the leaf entries themselves. Tests now prove every
entry namespace is discoverable. The executable version oracle now derives
from the bounded local workspace `Cargo.toml`.

Revised verdict: **ACCEPT**. The reviewer confirmed both findings are resolved
and found no regression in command resolution, exact help placement,
side-effect ordering, legacy error bytes/streams, or documentation scope.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: Codex `/root`

The coordinator inspected the final catalog, dispatcher, executable spec,
issue resolution, and current-state documentation, then reran `make lint`,
`make test`, `make spec` (`209 passed, 3 skipped`), and `git diff --check`; all
passed. No operational command, dependency, asset read, process, network
request, or external effect was added.
