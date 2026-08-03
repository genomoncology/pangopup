# 044 — Focused runtime CLI help

Status: ready

## Why

The shipped runtime CLI has conventional root help but inconsistent command
help. `pangopup sync --help`, `status --help`, `serve --help`, `assets install
--help`, and `assets runtime install --help` exit 2 with `CLI_USAGE` errors.
`lookup --help` exits successfully but prints the entire root catalog instead
of focused lookup guidance. The `assets` and `assets runtime` namespaces have
no successful help at all.

This was the clearest remaining usability failure in independent Apple Silicon
container validation. It is the next frontier outcome because users should be
able to discover exact command grammar before downloading assets, choosing
paths, or starting the server.

## Scope

- Add successful focused `-h` and `--help` for the six public runtime leaves:
  `sync`, `status`, `serve`, `lookup`, `assets install`, and `assets runtime
  install`.
- Add successful namespace help for `assets` and `assets runtime`. Namespace
  help lists only descendant commands; leaf help contains exactly one usage
  synopsis plus one concise description.
- Keep root/no-argument help byte-compatible, including its title, ordered
  command catalog, and `--help`/`--version` lines. Preserve existing root and
  lookup version behavior.
- Recognize information only in the exact closed forms `<path> -h` and `<path>
  --help`. A help flag mixed with operands, followed by extra arguments, placed
  before a path, or attached to an unknown path remains ordinary operational
  input and returns the existing typed `CLI_USAGE` failure.
- Handle help before `serve` dispatch or operational parsing. Successful help
  must not resolve data/cache paths, inspect or download assets, bind a socket,
  open SQLite, initialize ONNX, or start model workers.
- Introduce one small runtime help catalog containing the six leaf paths,
  synopses, and summaries. Use it as the sole source for root, namespace, and
  leaf help rendering. Do not replace the operational parser, add a dependency
  or CLI framework, or refactor scoring/service code.
- Use these exact catalog summaries and the already-shipped synopses, in this
  order:

  | Path | Summary |
  | --- | --- |
  | `sync` | `Synchronize the pinned SNV lookup and model-side runtime assets.` |
  | `status` | `Report the combined installed SNV and model-side runtime state.` |
  | `serve` | `Run the foreground HTTP scoring service.` |
  | `assets install` | `Install a caller-supplied SNV transport into the local asset store.` |
  | `assets runtime install` | `Install a caller-supplied compatible model-side runtime profile.` |
  | `lookup` | `Score one or more GRCh38 variants with lookup-first model fallback.` |

  Leaf bytes are exactly `Usage: pangopup <SYNOPSIS>`, one empty line, the
  summary, and one final line feed.
- Render namespace help exactly as follows, including order and final line
  feed:

  ```text
  Usage: pangopup assets <ACTION>

  Actions:
    pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]
    pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON> --model-bundle <DIR> --reference-bundle <DIR> --mask <FILE> [--data-dir <ABSOLUTE_PATH>]
  ```

  ```text
  Usage: pangopup assets runtime <ACTION>

  Actions:
    pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON> --model-bundle <DIR> --reference-bundle <DIR> --mask <FILE> [--data-dir <ABSOLUTE_PATH>]
  ```
- Add executable regressions to `spec/cli.md`; change `spec/http-service.md` to
  prove focused `serve --help`; extend `spec/container-image.md`,
  `scripts/qualify-container.sh`, and `tests/container-delivery.sh` so the
  stripped final image executes every help path network-disabled, read-only,
  non-root, and without mounted assets.
- Update `README.md`, `architecture/delivery.md`, and `planning/frontier.md` so
  current help behavior and the next undrafted synchronization-progress slot
  are accurate. Do not add an ADR for this local CLI correction.
- Exclude a `help` command, shell completions, man pages, option-by-option prose,
  changes to operational errors, removal or expansion of subcommand version
  aliases, ONNX CPU-vendor warning suppression, sync progress/retry behavior,
  and any scoring, HTTP, asset, cache, release, or publication change.

## Success Checklist

- Each exact root, namespace, and leaf `-h`/`--help` form exits 0 and writes its
  rendered help to stdout. No PangoPup JSON error is written. Leaf output starts
  with `Usage: pangopup <leaf synopsis>` and does not list unrelated commands.
- `pangopup assets --help` lists the two descendant install paths;
  `pangopup assets runtime --help` lists only runtime install. Root/no-argument
  help is byte-identical to the pre-ticket output.
- A table-driven unit test proves the catalog has exactly six unique leaves,
  both namespaces, one synopsis/summary per leaf, deterministic ordering, and
  focused renderings. Exact-position tests cover both help flags and reject
  misplaced, mixed, extended, and unknown forms without intercepting them.
- Check in a literal pre-ticket root-help oracle at
  `tests/fixtures/runtime-cli/root-help.txt`, including its single final line
  feed. It must not be generated from the new catalog. Assert no-argument,
  root `-h`, and root `--help` output byte-for-byte against that independent
  fixture while actual root rendering comes from the catalog.
- A regression in `spec/cli.md` fails against the pre-ticket binary because
  `sync --help` exits 2; another proves the previously successful but global
  `lookup --help` is now focused. Executable specs cover all eight non-root
  paths and representative invalid placements.
- Help succeeds with invalid/missing data and cache environment values and
  performs no service or model work. The final-image helper invokes all eight
  non-root help paths with `--network none`, a read-only root filesystem, no
  mounted assets, and the image's numeric non-root user.
- The final-image check asserts successful exit and the exact expected leading
  usage line for every path. Third-party startup warnings on stderr are not
  reclassified as a help failure in this ticket; PangoPup must not emit a JSON
  error envelope.
- `make lint`, `make test`, and `make spec` pass. The existing native container
  workflow inherits the updated bounded helper; no image is published.

## Decisions

### Every public path versus only the four Mac-reported leaves

Fixing only `sync`, `status`, `serve`, and `lookup` would leave the two install
commands and both namespaces with the same broken discovery experience.
Options were those four commands, all leaf commands, or all leaves plus
namespaces. Cover all six leaves and both namespaces so “every runtime
subcommand” is literal and the hierarchy is discoverable.

### Small catalog versus more parser conditionals

Adding eight special cases in `main` would be quick but would preserve the
current duplicated global synopsis and drift risk. Replacing the parser with a
CLI framework would be disproportionate and could change operational errors.
Use a small checked help-only catalog, modeled on the established maintainer
catalog, while leaving operational parsing untouched.

### Exact trailing information versus help anywhere

Treating `--help` anywhere as success could hide malformed real invocations,
such as a partially supplied sync or lookup request. Accept only the exact path
followed by one help flag. Preserve all mixed, extended, misplaced, and unknown
forms as operational input and retain their existing errors.

### Preserve root bytes versus redesign all help

The root help is already documented and consumed by specs. Reformatting it is
not necessary to fix focused command discovery. Generate it from the catalog
but prove its complete bytes against a literal checked pre-ticket fixture that
the catalog does not generate. New namespace and leaf renderings use the exact
bytes specified above and the concise style already established by
`pangopup-build`.

### Host spec plus final-image proof

Host specs prove the public grammar but do not prove that the distroless
entrypoint intercepts `serve --help` before service startup. Run the same
closed help matrix in the actual final image with no network or assets. This is
bounded and enters the already-native AMD64/ARM64 qualification helper rather
than adding a separate deployment harness.

## Dependencies

- Ticket 043's completed read-only status correction and qualified minimal
  Docker image.

## Notes

- The observed pre-ticket behavior is: `lookup --help` exits 0 with root help;
  the other five leaf paths and both namespaces exit 2. Retain these as
  regression facts, not as desired compatibility.
- The root-help fixture is copied once from the current `a0ab089` executable
  output before implementation and thereafter remains an independent oracle;
  no test or build step regenerates it.
- The harmless ONNX Runtime CPU-vendor warning observed on Apple Silicon may be
  emitted before Rust `main`; suppressing it would require a separate runtime-
  initialization decision. This ticket owns PangoPup help bytes and exit
  status, not third-party pre-main stderr.
- Normal tests and specs are offline and asset-independent. Container help
  qualification must use no data/cache volumes and must clean only resources
  the helper already owns.
- Do not copy Mac evidence, absolute local user paths, downloaded assets, or
  generated container output into the public repository.
- The gate is `make lint`, `make test`, and `make spec`; there is no `make
  check`. Evidence in this ticket is illustrative unless recorded as final
  implementation evidence.

## Coordinator Authorship

Coordinator: Codex

The coordinator authored this ticket from the completed Ticket 043 outcome,
the rolling frontier, and the reproduced runtime help failures. It does not
implement product code or approve its own ticket.

## Independent Ticket Review

Reviewer: `ticket044_design_review` (independent read-only sub-agent)

The first review rejected two decision gaps: the developer would have had to
invent the six public summaries and namespace bytes, and a catalog-derived
root expectation could tautologically approve its own output. The coordinator
froze every summary, ordering, leaf/namespace byte shape, and final line feed,
then required a literal pre-ticket root-help fixture copied from the shipped
`a0ab089` executable rather than generated from the new catalog.

Re-review: accepted. The reviewer confirmed the public bytes, exact-position
semantics, parser-preservation approach, pre-fix regressions, no-volume final-
image proof, third-party stderr boundary, named documentation, and exclusions
are decision-complete and feasible. No material finding remains.

## Implementation Evidence

Developer: pending

Record focused tests, final-image evidence, documentation changes, and any
scope-relevant deviation, then set status to `review`. The developer does not
commit or push.

## Adversarial Code Review

Reviewer: pending

Record findings and disposition before completion. The reviewer is read-only
and distinct from the ticket reviewer and developer. Material fixes return to
the same developer and then this reviewer.

## External Effect Evidence

Coordinator: not applicable

This ticket performs no public or irreversible external effect.

## Coordinator Final Check

Coordinator: pending

Record final `make lint`, `make test`, and `make spec`, the focused final-image
qualification, and a shipped/future documentation scan before committing and
pushing the completed outcome.
