# 049 — Compact first-use README

Status: ready

## Why

PangoPup's core lookup, model fallback, asset synchronization, CLI, HTTP
service, Docker image, and Linux installer are shipped, but the 1,019-line
README leads with implementation history and makes ordinary installation and
use difficult to discover. The Apple ARM64 investigation is also complete:
production will retain the already-qualified ONNX Runtime rather than maintain
a custom native runtime solely to suppress a harmless warning.

## Scope

- Replace `README.md` with a compact user-first guide covering what PangoPup
  does, lookup-first/model-fallback behavior, prerequisites, Linux installation,
  asset synchronization and disk use, CLI scoring, forced model scoring, the
  foreground HTTP service, Docker on AMD64 and ARM64, updates, and safe manual
  uninstall.
- Include complete copy/paste HTTP request examples and explain response
  provenance in plain English.
- Clearly state that Apple Silicon Docker uses native Linux ARM64 CPU inference,
  not MPS/Metal, and may print the known harmless ONNX Runtime CPU-vendor
  warning. Record that the project will wait for an upstream runtime carrying
  Apple-aware cpuinfo instead of shipping a custom runtime.
- Preserve required GPL and dataset attribution links and link maintainers to
  detailed architecture and planning documents instead of repeating their
  history in the README.
- Add executable documentation checks for commands, endpoint examples, XDG
  paths, Docker volume behavior, update, and uninstall guidance in
  `spec/readme-first-use.md`. The spec performs local/static checks only: no
  network, Docker build, asset synchronization, large download, public effect,
  or destructive uninstall command is executed.
- Update `planning/frontier.md` so the accepted runtime decision and completed
  documentation are current.
- Do not change scoring, assets, cache behavior, CLI/API contracts, installer
  behavior, Docker build behavior, dependencies, or public releases.
- Do not add an automated uninstaller. Removal stays explicit so replacing the
  executable cannot accidentally delete roughly 15 GB of reusable assets or
  the separate result cache.

## Success Checklist

- A new user can find copy/paste paths for Linux install, `sync --progress`,
  `status`, an SNV lookup, `--model-only`, `serve`, and HTTP scoring without
  consulting another document.
- The README distinguishes the executable, durable XDG data, disposable
  download/model-result cache, Docker image, and Docker volumes, with separate
  update and removal commands.
- The README accurately describes supported platforms, disk requirements,
  CPU-only Apple Docker inference, security boundary, asset sources, licenses,
  and the absence of a published registry image at this ticket's boundary. It
  states that the service has no built-in authentication or TLS, uses loopback
  by default, and requires an authenticated TLS proxy before non-loopback
  exposure.
- Disk guidance distinguishes installed data from the resumable-download and
  model-result cache. It reports the retained Mac observations (about 14.8 GB
  durable data and 2.4 GB cache after first sync) and recommends at least 25 GB
  free for provisioning rather than implying 15 GB covers the whole operation.
- Executable checks fail if the named first-use commands/endpoints, update and
  uninstall guidance, or important platform statements disappear or drift.
- User-facing examples agree with current `--help`, CLI JSONL, HTTP request,
  and status behavior.
- `README.md` is no more than 450 lines and 3,000 words. Installation and quick
  start precede maintainer internals; engineering history is linked rather than
  embedded.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Runtime maintenance:** custom Apple-aware ONNX Runtime, suppressing stderr,
   or waiting upstream. Wait for an upstream release while documenting the
   harmless warning; a private native-runtime fork would add recurring security,
   packaging, correctness, and multi-platform qualification work for no scoring
   benefit.
2. **Uninstall:** automated deletion or explicit commands. Document explicit,
   independently selectable removal of executable, data, cache, image, and
   volumes so an upgrade cannot destroy expensive reusable downloads.
3. **README depth:** retain the engineering ledger or optimize for first use.
   Optimize for first use and link `architecture/` and `planning/` for maintainers.
4. **Behavior changes:** repair examples by changing the product or document the
   shipped contract. This ticket documents and tests the shipped contract only;
   any discovered product defect returns to design rather than being hidden in
   prose.

## Dependencies

None. Tickets 043–045 fixed and requalified the setup friction, and Ticket 048
established the Apple-warning cause and informed the accepted wait-upstream
decision.

## Notes

- The immutable `v0.1.0` executable release predates Tickets 043–045. The README
  must present two unmistakable tracks: (1) the exact capabilities of published
  `v0.1.0`, and (2) building current `main` for `sync --progress`, `--model-only`,
  `serve`, and the other newer behavior. No example may imply the curl-installed
  public release already contains commands it does not. Publishing a replacement
  executable is the next separate external-effect ticket, after which its own
  documentation update can collapse these tracks.
- No GHCR image exists yet. Docker instructions build locally from an exact
  checkout until the later container-publication ticket completes.
- The curl installer is `install.sh`; it installs only the executable and does
  not synchronize assets or change `PATH`.
- Default Linux paths follow XDG: data under `XDG_DATA_HOME` (normally
  `~/.local/share/pangopup`) and cache under `XDG_CACHE_HOME` (normally
  `~/.cache/pangopup`). Verify exact current behavior before writing commands.

## Coordinator Authorship

Coordinator: Codex. Drafted from the completed Ticket 048 result, live product
behavior, public release inventory, and rolling frontier.

## Independent Ticket Review

Reviewer: independent design reviewer `ticket049_design_review` — ACCEPT.

The first review rejected an ambiguous installer/current-main story, unnamed
test ownership, a subjective size target, incomplete disk guidance, and an
implicit service-security boundary. The coordinator revised the ticket to add
two honest delivery tracks, the bounded static spec, objective size limits,
retained disk observations plus 25 GB provisioning guidance, and the explicit
loopback/authenticated-proxy rule. Re-review accepted all findings as resolved.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
