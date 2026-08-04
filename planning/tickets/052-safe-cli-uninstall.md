# 052 — Safely uninstall code or the complete local installation

Status: ready

## Why

Pangopup documents several manual `rm` commands but has no checked removal
workflow. Users must currently discover the executable, installed assets, and
cache roots themselves, and a copied command can remove the wrong directory.
The next bounded outcome is one Linux CLI command that inspects and displays
the paths it would affect, then removes either only the running Pangopup
executable or that executable plus Pangopup's managed data and cache roots.

## Scope

- Add `pangopup uninstall [--full] [--yes]` to the checked runtime command and
  focused-help catalog. These are its only options; duplicates, values, and
  unknown options are usage errors.
- Resolve the executable with the operating system and resolve data/cache roots
  through the same `PANGOPUP_*`, XDG, and `HOME` precedence used by runtime
  commands. Inspect every displayed root before prompting or deleting: paths
  must be absolute, roots must not be symlinks, present roots must be real
  user-owned directories named `pangopup`, and the executable must be the
  current single-link regular user-owned Pangopup file in a removable real
  parent. A missing data or cache root is valid and is reported as absent.
- Prove each present removal root is PangoPup-managed before offering full
  removal. Accept an empty `pangopup` root or a root whose complete top-level
  entry set is drawn from the installed/synchronization/cache layouts owned by
  this release; reject unknown top-level entries. Always reject `/`, `HOME`, an
  XDG parent, the executable parent, or a root containing the executable.
  Normalize and resolve existing paths, then reject data/cache equality,
  ancestry, `..` aliases, and symlink-mediated aliases.
- With no flags, require an interactive terminal, display the checked
  executable/data/cache paths, and offer exactly: code only, code and all
  managed data, or cancel. `--full` preselects complete removal but still asks
  for confirmation. `--yes` skips interaction and selects code only unless
  combined with `--full`; `--full --yes` selects complete removal.
- Code-only removal preserves data and cache. Complete removal deletes the
  resolved data and cache roots before unlinking the executable. Failure leaves
  the executable available and reports which earlier phase failed; removal is
  not transactional and does not promise rollback of already removed data.
- Interactive operation requires both stdin and stderr to be terminals. With
  neither flag, accept one exact `1`, `2`, or `3` response; EOF or any other
  response fails without mutation. With `--full` alone, accept one exact
  yes/no confirmation. `--yes` requires no terminal. Emit the path plan and
  prompts on stderr and one stable JSON result on stdout. Cancellation succeeds
  without mutation. Noninteractive invocation without `--yes` fails before
  mutation with an actionable typed error.
- Successful removal emits exactly
  `{"status":"removed","scope":"code_only|full","executable":{"path":...,"state":"removed"},"data":{"path":...,"state":"preserved|absent|removed"},"cache":{"path":...,"state":"preserved|absent|removed"}}`.
  Cancellation emits the same three path objects with `status:"cancelled"`,
  `scope:"none"`, and only `preserved|absent` states. Paths must be UTF-8 or
  preflight fails. Failures emit no success JSON and use the typed codes
  `UNINSTALL_NONINTERACTIVE`, `UNINSTALL_UNSAFE`, `UNINSTALL_BUSY`, or
  `UNINSTALL_IO` through the established stderr error envelope.
- After the user confirms full removal, acquire and retain the existing
  nonblocking installation and provisioning authorities, covering concurrent
  sync and install, then revalidate all identities immediately before mutation.
  Cancellation returns before lock acquisition so it does not create lock
  files or alter directory metadata. Document that a foreground service must be
  stopped first because Pangopup deliberately has no process registry or
  supervisor.
- Perform removal through held directory descriptors with no-follow,
  descriptor-relative traversal and inode/device revalidation. Unlink nested
  symlinks without following them; reject foreign-owner descendants,
  cross-device descendants, sockets/FIFOs/devices, and unexpected hard-linked
  regular files. Revalidate the executable inode and single-link shape
  immediately before unlinking it. The safety walk reads metadata only, never
  file contents.
- Limit the command to the direct Linux executable installation. A container
  whose executable or parent is not removable fails safely and directs users
  to the documented host-side Docker image/volume commands. Do not add process
  supervision, privilege escalation, package-manager integration, arbitrary
  caller-supplied deletion paths, recursive byte counting, or an installer
  uninstall mode.
- Update `README.md`, `spec/cli.md`, `spec/readme-first-use.md`, and
  `architecture/delivery.md` in the same implementation. Replace the manual
  direct-install removal guidance with the interactive command, both flag
  combinations, preservation semantics, and separate Docker removal commands.

## Success Checklist

- Executable specs prove focused help; interactive code-only, full, and cancel
  choices; `--yes`; `--full`; `--full --yes`; nonterminal refusal; stable JSON
  and stream separation; and unchanged Docker guidance without touching real
  user paths.
- Inside-out tests use isolated executable copies and XDG roots to prove exact
  option grammar, precedence, absent roots, code-only preservation, deletion
  order, cancellation, permission/ownership rejection where the platform can
  exercise it, held-lock rejection, and that root or nested symlinks cannot
  delete an outside sentinel. Destructive negative cases cover `/`, `HOME`, XDG
  parents, arbitrary owned directories, unknown entries, executable ancestry,
  equal/nested roots, `..` and symlink aliases, cross-device/special entries,
  foreign ownership, and unexpected hard links. Tests never uninstall the
  build-tree executable. Cancellation also proves an empty root's names and
  metadata are unchanged because no lock authority was acquired.
- Help and removal run before asset/model/cache initialization and do not scan
  or mmap installed scoring assets. A representative tree with large sparse
  files proves the preflight is metadata-bounded rather than proportional to
  asset bytes.
- README remains within its existing size bounds and explains that `--full`
  removes installed assets, resumable downloads, and the default SQLite cache
  inside the resolved cache root, while separately configured files outside
  those roots are not discoverable and are not removed.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Two flags only.** Separate `--assets`, `--cache`, `--dry-run`, or path
   flags are more expressive but make the ordinary choice harder to explain.
   Use only `--full` for removal scope and `--yes` for confirmation. The
   unflagged prompt remains the human-readable dry run because it displays all
   checked paths before any choice.
2. **Managed roots, not arbitrary discovery.** Searching the filesystem or
   deleting every custom model-cache path would be incomplete and unsafe.
   Remove only the current executable and the resolved Pangopup data/cache
   roots. State clearly that an explicitly relocated model cache outside the
   cache root must be removed separately.
3. **Metadata-only safety walk.** Reading 17 GB to calculate size would make
   uninstall slow without preventing path escape. Walk directory metadata
   through held descriptors to enforce ownership, device, type, link, and
   no-follow rules, but never read payload bytes. Prove outside sentinels
   survive and a large sparse payload does not increase bytes read.
4. **Executable last.** Removing code first makes recovery and diagnostics
   harder if data deletion fails. Complete removal deletes selected managed
   roots first and unlinks the running Linux executable only after success.

## Dependencies

None.

## Notes

- The installer writes one regular executable, normally
  `$HOME/.local/bin/pangopup`; it does not currently write an install receipt.
- Runtime data precedence is `PANGOPUP_DATA_DIR`, `XDG_DATA_HOME/pangopup`, then
  `$HOME/.local/share/pangopup`. Transport-cache precedence is
  `PANGOPUP_CACHE_DIR`, `XDG_CACHE_HOME/pangopup`, then
  `$HOME/.cache/pangopup`.
- The default SQLite result cache is inside the resolved XDG cache root, but an
  explicit `PANGOPUP_MODEL_CACHE` or CLI `--model-cache` may be elsewhere and
  cannot be reliably rediscovered by a later uninstall process.
- “Direct installation” means the currently running user-owned Pangopup
  executable. Without an installer receipt, an explicitly copied or build-tree
  executable named `pangopup` can intentionally remove itself; tests always use
  isolated copies.

## Coordinator Authorship

Coordinator: Codex. Drafted from the shipped `v0.2.0` installer, current XDG
resolvers, checked help catalog, README removal guidance, and Ian's approved
two-flag interaction.

## Independent Ticket Review

Reviewer: Huygens. Initial verdict: REJECT. The draft allowed an explicit
environment root such as `$HOME` to pass ownership checks, omitted alias and
ancestry rejection, checked only stderr terminal state, did not hold existing
sync/install authorities, underspecified JSON/errors, and overstated direct
installation identity without a receipt. The coordinator resolved those
findings with positive managed-root admission, relationship rejection, dual
terminal requirements, held locks and revalidation, exact output contracts,
descriptor-relative metadata-only deletion, and a narrower current-executable
claim. First re-review found one remaining lock-order contradiction: acquiring
lock authorities before prompting would mutate an otherwise untouched root on
cancellation. The coordinator moved acquisition after confirmation and added
an exact no-mutation cancellation proof. Final verdict: ACCEPT. The reviewer
confirmed the full contract is safely bounded and implementable.

## Implementation Evidence

Developer: pending.

## Adversarial Code Review

Reviewer: pending.

## External Effect Evidence

Coordinator: not applicable.

## Coordinator Final Check

Coordinator: pending.
