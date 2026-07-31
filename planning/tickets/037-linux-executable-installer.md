# 037 — Package and install the Linux x86_64 executable

Status: ready

## Why

Pangopup now exposes the complete `sync`, `status`, and `lookup` CLI, but users
still have to compile it. The immutable SNV and runtime data releases are
already downloaded by `pangopup sync`; the missing prerequisite is a reviewed
Linux executable, checksum-verifying binary-only installer, and exact-commit
GitHub build path.

This ticket prepares and tests those pieces without triggering a workflow,
creating a tag/release/attestation, or publishing an artifact. Those external
effects remain Ticket 038.

## Scope

- Add Bash `install.sh` with grammar
  `install.sh [--version latest|MAJOR.MINOR.PATCH] [--install-dir <ABSOLUTE_PATH>]`.
  Defaults are `latest` and
  `${PANGOPUP_INSTALL_DIR:-$HOME/.local/bin}`. Explicit directory wins over the
  environment. Reject duplicates, unknown arguments, missing/relative/empty
  selected directories, malformed versions, unsupported OS, and unsupported
  architecture before downloading.
- Support only Linux `x86_64`/`amd64`. Download the direct executable
  `pangopup-linux-x86_64` and one-record
  `pangopup-linux-x86_64.sha256` from:
  - latest:
    `https://github.com/genomoncology/pangopup/releases/latest/download/<NAME>`;
  - pinned:
    `https://github.com/genomoncology/pangopup/releases/download/v<VERSION>/<NAME>`.
  Do not query release JSON, accept URL/repository overrides, or select any
  SNV/runtime data asset.
- Require Bash, either `curl` or `wget`, and one of `sha256sum`, `shasum`, or
  `openssl`. Use fail-closed HTTPS flags and a private temporary directory,
  cleaned on every exit. No tar/gzip dependency exists.
- Accept exactly one checksum record: 64 hexadecimal digits optionally
  followed by two spaces or ` *` plus the exact binary basename. Compare the
  lowercase computed digest before execution. Reject empty/multiple/wrong-name
  or otherwise malformed records.
- Require the downloaded object to be a regular singly linked file. Copy it to
  a fresh private temporary file inside the destination, set mode `0755`, and
  execute that temporary file with `--version` before atomically renaming it to
  `<install-dir>/pangopup`. A failed download, checksum, file-shape check,
  smoke test, or rename preserves any existing executable.
- Create an absent destination and parents without `sudo`. After creation or
  resolution, require the destination itself to be a real directory and not a
  symlink; reject an existing non-directory. Create and clean the replacement
  temporary file inside that directory on every exit so the final rename is
  same-filesystem. Never remove or truncate an existing `pangopup` before the
  successful final rename.
- Pinned installation requires exact stdout `pangopup <requested-version>`.
  Latest installation accepts strict stdout `pangopup MAJOR.MINOR.PATCH`,
  reports that resolved version, and relies on the checksum beside GitHub's
  selected Latest asset rather than pretending to resolve the tag separately.
- Install only the executable. Do not invoke `pangopup sync`, use `sudo`, edit
  shell startup files, install packages, or write runtime data. Print the
  installed path/version, immutable release/source/license/notice URLs built
  from the strict resolved executable version (including after `latest`), and
  next commands `pangopup sync` and `pangopup status`. If the destination is
  absent from `PATH`, print an export command rather than changing a file.
- Add a deterministic local release-preparation entry point. It accepts the
  direct workflow-built executable, already-generated CycloneDX SBOM, exact
  version, exact lowercase 40-character commit, repository root, and absent
  output directory. It:
  - requires repository `HEAD` to equal the commit and the tracked tree/index
    to be clean;
  - requires workspace version and exact executable `--version` output to
    equal the supplied version;
  - copies and strips the executable without changing the caller's file;
  - validates the SBOM as JSON and copies it without rewriting;
  - copies exact `LICENSE` and `NOTICE` bytes;
  - emits exactly six files: `pangopup-linux-x86_64`, its `.sha256`,
    `pangopup-linux-x86_64.cdx.json`, `release-manifest.json`, `LICENSE`, and
    `NOTICE`;
  - computes `DT_NEEDED` and maximum imported GLIBC version from the final
    stripped copy, then writes a canonical manifest binding schema, version,
    commit, target
    `x86_64-unknown-linux-gnu`, Rust 1.93.1, measured binary size, maximum GLIBC
    version, ordered dynamic dependency names, and every output member's
    size/SHA-256 except the manifest itself.
  It performs no network operation and does not claim that arbitrary local
  input proves binary provenance; the exact-checkout workflow and later GitHub
  attestation provide that evidence.
- Add a manually dispatched GitHub packaging workflow accepting one required
  lowercase 40-character commit. It uses only full-commit action pins and
  `contents: read`, checks out and verifies that exact commit, requires it to
  be reachable from `origin/main`, and proves the tracked tree/index clean.
- On `ubuntu-22.04`, install the repository-pinned Rust toolchain and exact
  `cargo-cyclonedx` 0.5.9, run `make lint`, `make test`, and `make spec`, then
  build `cargo build --locked --release --package pangopup-cli`.
- Generate the CLI binary SBOM before packaging with
  `CARGO_NET_OFFLINE=true`, target `x86_64-unknown-linux-gnu`, JSON output,
  binary-only description, the CLI manifest, and
  `SOURCE_DATE_EPOCH=$(git show -s --format=%ct HEAD)`. cargo-cyclonedx embeds
  its absolute source path in graph identifiers, so use one fixed absolute
  scratch pathname `/tmp/pangopup-cyclonedx-source-v1`. Require it absent,
  create it mode `0700`, extract `git archive <EXACT_COMMIT>`, generate and
  retain the first SBOM outside that tree, remove the tree, then repeat a fresh
  extraction/generation at the identical pathname. Trap cleanup and reject a
  pre-existing file, directory, or symlink rather than reusing/deleting it.
  Run exact `cargo-cyclonedx` 0.5.9 installed by `cargo install --locked` in
  both rounds with the existing Cargo cache available offline and the explicit
  CLI manifest/target/JSON/binary-description arguments. Compare the two
  generated files byte-for-byte and pass one to the preparer. Never run the
  generator against the tracked checkout or rewrite its identifiers. If
  cargo-cyclonedx 0.5.9 does not honor deterministic timestamp/serial inputs,
  stop and return the ticket to design review rather than normalize away
  signed semantics.
- Run the preparer after build and SBOM generation, then qualify the exact
  final output file `pangopup-linux-x86_64`—not the unstripped build input—with:
  - `readelf -d` `DT_NEEDED` restricted to `libstdc++.so.6`,
    `libgcc_s.so.1`, `libm.so.6`, and `libc.so.6`;
  - maximum imported `GLIBC_X.Y` derived by `readelf --version-info`, compared
    numerically and required to be at most 2.35;
  - a separate digest-pinned `ubuntu:22.04` container executing `--version`,
    `--help`, top-level missing `status`, one miniature SNV hit, and one
    miniature explicit model-fallback request from a read-only mounted checkout.
- Verify that the manifest's binary size/digest, ordered `DT_NEEDED` list, and
  GLIBC maximum describe that same qualified final file, verify the exact
  six-file inventory/digests, and only then upload that directory as one
  private GitHub Actions artifact. The workflow contains no release,
  package-registry, attestation, or other write permission/action. Ticket 038
  will add or invoke the separately reviewed attestation/publication effect.
- Add focused offline tests for the preparer and installer. Installer tests
  mock `uname`, downloader, and checksum tools through a private `PATH`; the
  production repository, asset names, and URL shapes remain literal and cannot
  be overridden. Keep Mustmatch to user-visible grammar/results; test manifest,
  determinism, and preservation inside-out.
- Update `AGENTS.md`, `README.md`, `architecture/README.md`,
  `architecture/delivery.md`, `planning/frontier.md`, and `planning/faq.md`.
  Document prerequisites, version-pinned/latest curl commands, direct-binary
  choice, no automatic data sync/PATH mutation, Linux/GLIBC baseline, exact
  future release inventory, and that no executable release exists until
  Ticket 038.
- Exclude workflow triggering, attestation, public tag/release/assets, data
  release changes, runtime-data download, Homebrew/APT/RPM, ARM64,
  macOS/Windows, musl/static work, update/uninstall commands, HTTP,
  Docker/systemd, and any runtime asset rebuild.

## Success Checklist

- Preparing the same checked fixture executable/SBOM/metadata twice produces
  byte-identical six-file outputs with exact manifest/checksum identities; the
  input executable/SBOM/source tree remain unchanged.
- Installer tests prove latest and pinned URLs, curl/wget selection, the three
  checksum-tool paths, successful isolated initial/replacement installation,
  exact version validation, PATH guidance, stable license/source links, and
  printed `sync/status` next steps without network or runtime-asset access.
- A compact negative matrix covers grammar/platform/prerequisite rejection,
  downloader failure, checksum-record errors/mismatch, unsafe downloaded file,
  smoke-test failure preserving an existing binary, and destination failure.
- Workflow/static tests prove full action SHAs, exact-commit/main/clean checks,
  locked gates/build/SBOM, two fresh same-path deterministic SBOM rounds with
  hostile-preexisting-path rejection, the explicit dynamic
  library allowlist, numeric GLIBC 2.35 ceiling, separate pinned-container
  smoke, exact artifact inventory, and `contents: read` only.
- The ordinary gate never dispatches the workflow, contacts GitHub, generates
  an attestation, or opens/downloads production runtime assets.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Direct binary or compressed archive.** The measured stripped binary is
   about 26 MB versus roughly 10 MB compressed, while runtime data is measured
   in gigabytes. Prefer the direct binary and remove tar parsing/extraction and
   its disproportionate attack/test surface.
2. **Installer downloads data too.** Install only the executable; runtime
   assets retain their explicit resumable `pangopup sync` consent boundary.
3. **One platform or premature matrix.** Ship the currently qualified Linux
   x86_64 target. ARM/macOS require separate evidence.
4. **Latest discovery API or redirect.** Existing data releases are frozen and
   the next release will be the executable. Use GitHub's Latest asset redirect
   plus pinned-version URLs without adding JSON/jq discovery.
5. **PATH mutation or guidance.** Do not edit shell configuration; print exact
   guidance.
6. **Attestation here or at publication.** A callable attestation job is an
   external write, not inert preparation. Keep this workflow read-only and put
   the exact subjects/predicate and invocation in Ticket 038.
7. **SBOM generation inside packaging or as authenticated input.** Generate it
   reproducibly in the exact-checkout workflow, then make the offline preparer
   validate and bind those bytes. Because cargo-cyclonedx makes absolute source
   paths part of its graph identifiers, compare two fresh sequential
   extractions at one fixed safe path rather than rewriting output.

## Dependencies

- Ticket 036, complete: stable top-level `sync`, `status`, and lookup CLI.
- Ticket 032, complete: repository dependency/license and workflow security.
- Existing public immutable SNV/runtime data releases.

## Notes

- Current workspace version is `0.1.0`; this ticket prepares but does not claim
  future release `v0.1.0`.
- `LICENSE` and `NOTICE` are adjacent immutable release assets and their stable
  URLs are printed by the installer. GitHub's generated source archives become
  the executable source distribution only after Ticket 038 creates the exact
  tag.

## Coordinator Authorship

Coordinator: `/root`

Revised after review replaced the archive with direct-binary delivery, moved
all attestation effects to Ticket 038, and separated deterministic SBOM
generation from offline packaging. The coordinator does not implement or
approve this ticket.

## Independent Ticket Review

Reviewer: pending

First review: REJECT.

- SBOM generation conflicted with a non-mutating/network-free packager and was
  not deterministic without fixed time/serial inputs.
- Tar-member validation was both underspecified and disproportionate to the
  small download saving.
- A callable attestation job contradicted the no-external-effect boundary.
- Commit provenance, clean-container ABI qualification, latest-version
  checking, retained license notice, and test layering were incomplete.

The coordinator selected direct-binary delivery, made SBOM generation a
deterministic exact-checkout workflow step and validated packager input, moved
attestation entirely to Ticket 038, defined commit/ABI/container checks,
clarified latest semantics and stable license links, and reduced the test
matrix. The same reviewer must re-review.

Second review: REJECT.

- Qualification targeted the pre-strip build rather than the final published
  bytes.
- Clean destination creation, symlink rejection, same-directory staging, and
  existing-binary preservation were not explicit.
- cargo-cyclonedx's beside-manifest output behavior made the scratch-generation
  requirement infeasible as written, and latest links were not explicitly
  version-resolved.

The coordinator moved every ABI/container/smoke check after packaging onto the
exact final binary, bound those results in the manifest, defined clean and
atomic destination behavior, prescribed two independent exact-commit
`git archive` SBOM scratch trees in offline mode, and made all printed links
use the resolved executable version. The same reviewer must re-review.

Final re-review: ACCEPT. The reviewer confirmed that the exact final binary is
now the measured and qualified object, destination behavior is safe and
complete, deterministic scratch SBOM generation is feasible, immutable links
use the resolved version, the workflow remains read-only, and the simplified
direct-binary outcome fits the frontier.

Implementation prerequisite result: the developer correctly stopped after
proving two different absolute scratch paths produce different cargo-cyclonedx
graph identifiers despite identical source/time/options. The coordinator
returned the ticket to `proposed` and revised the design to use two fresh
sequential extractions at the same fixed, safely created absolute path, with no
SBOM rewriting. The same reviewer must approve this material change before
development resumes.

Material-change re-review: ACCEPT. The reviewer confirmed that identical-path
sequential generation removes only the observed path-dependent variance while
preserving two fresh exact-commit inputs and untouched output comparison. The
fixed path is created atomically mode `0700`, rejected if pre-existing, and
removed only when the workflow recorded ownership. Offline/tool/time/target
controls and the no-public-effect boundary remain intact.

## Implementation Evidence

Developer: `/root/ticket037_implementation`

Stopped before implementation as required by the reviewed ticket. The pinned
`cargo-cyclonedx` 0.5.9 prerequisite does not produce byte-identical SBOMs in
two independent `git archive HEAD` scratch trees, even with the same
`SOURCE_DATE_EPOCH`, offline Cargo metadata, target, format, and binary
description. The CLI SBOM digests were:

- scratch `one`: `bd128c49539995c87bea5a678b1134428f3e4c04d8dc5a99bcb94610642f217b`
- scratch `two`: `c36a57a7587a07bebb9750ed823a921197f4f1ae54f6c38cc8fff7543e746826`

The first semantic difference is the absolute scratch path embedded in the
root `bom-ref`; the same path-dependent difference appears in local workspace
component and dependency references. This is not timestamp or serial drift.
No generated SBOM or product change was committed. Per Scope, do not normalize
the generated signed semantics: the ticket must return to coordinator design
and independent ticket review.

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
