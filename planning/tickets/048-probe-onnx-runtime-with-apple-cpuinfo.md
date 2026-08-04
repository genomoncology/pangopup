# 048 — Probe ONNX Runtime 1.28 with Apple-aware cpuinfo

Status: publication-ready

## Why

Ticket 047 proved that the stock `ort` 2.0.0-rc.13 / ONNX Runtime 1.28.0
binary still prints an unknown-CPU-vendor warning before PangoPup handles
`--version` in an Apple Silicon Docker container. The failure is now explained:
ONNX Runtime 1.28 pins cpuinfo commit
`4628dc060ce4e82345dc166bbac875609db4ff69`, while Linux recognition of Apple
ARM implementer `0x61` landed six days later in cpuinfo commit
`0c0ab15cb0a8bafbbf71c2ae6f84128a4c2a8da6`.

This ticket tests that causal explanation directly. It does not select or ship
a custom ONNX Runtime. It creates matched baseline and patched maintainer
probes from the same source-build recipe. The baseline must reproduce the
warning before the otherwise identical cpuinfo-patched build can say whether
that pin caused it.

## Scope

- Add a maintainer-only, containerized A/B build recipe for ONNX Runtime
  v1.28.0. Build A retains cpuinfo commit
  `4628dc060ce4e82345dc166bbac875609db4ff69`. Build B advances it to
  `0c0ab15cb0a8bafbbf71c2ae6f84128a4c2a8da6`. Source, toolchain, flags, static
  linking, and PangoPup source are otherwise byte-for-byte the same inputs.
- Authenticate the exact commit behind ONNX Runtime v1.28.0 and its source
  archive SHA-256; both cpuinfo commit IDs and archive digests; every build and
  final-image base by digest; and relevant compiler, CMake, and Rust versions.
  Fail if the expected old dependency fields are absent or if the A/B source
  trees differ anywhere except the exact reviewed cpuinfo URL/revision/hash
  fields in `cmake/deps.txt`.
- Build both probes natively as Linux/ARM64 using `ort`/`ort-sys`
  2.0.0-rc.13, explicit ONNX Runtime API 27, and their respective CPU-only
  static runtimes through `ORT_LIB_LOCATION`. Prove that cpuinfo is enabled and
  linked into both final PangoPup executables, rather than merely proving that
  the environment variable was set. The production `Dockerfile`, accepted
  dependency lock, runtime assets, and released images remain unchanged on
  `main`.
- Add cheap local checks that prove the recipe is pinned, the one-change patch
  is exact, the candidate uses the custom runtime rather than the downloaded
  stock archive, and the probe image carries its exact source revision label.
- On a temporary `qualification/ticket-048-cpuinfo-probe` branch, run the
  baseline command first, capturing streams separately:

  ```text
  docker run --rm pangopup:ticket-048-cpuinfo-baseline --version
  ```

- Require native `linux/arm64`, an image revision label equal to the tested
  40-character commit, exit code zero, stdout exactly `pangopup 0.1.0\n`, and
  stderr exactly equal to Ticket 047's 76-byte warning. If that baseline does
  not reproduce, stop as inconclusive.
- Only after that baseline passes, run the identically built patched image and
  require the same architecture, revision, exit code, and stdout, with
  completely empty stderr.
- Retain a compact result under `planning/artifacts/` and update
  `planning/frontier.md` after the external probe. Do not retain raw build
  trees, downloaded source archives, static libraries, Docker images, or Mac
  evidence in git.
- Explicitly exclude warning filtering, stderr suppression, CPU spoofing,
  model/reference/mask changes, biological scoring, performance comparison,
  package/release publication, and production adoption of the custom runtime.

## Success Checklist

- The recipe starts from the exact authenticated commit behind ONNX Runtime
  v1.28.0 and applies exactly the reviewed cpuinfo dependency-field
  replacements; any upstream drift fails closed.
- The custom runtime is built reproducibly enough to record exact source,
  dependency, toolchain/base-image, command, library, and final-image
  identities, with applicable ONNX Runtime and cpuinfo license notices
  preserved for the probe.
- Focused automated checks fail if A and B differ beyond the expected cpuinfo
  dependency fields, if either PangoPup candidate silently falls back to the
  stock `ort-sys` download, or if cpuinfo is not present in both linked
  executables.
- The exact-commit Apple ARM64 probe records architecture, revision, exit code,
  exact stdout, and exact stderr for both A and B, plus both static-runtime and
  final-image hashes.
- A baseline mismatch is **inconclusive**. The exact baseline warning plus a
  nonempty patched warning **rejects** the candidate. The exact baseline warning
  plus empty patched stderr **confirms this causal hypothesis**.
- Confirmation does not authorize
  custom-runtime adoption; model parity, cache compatibility, performance,
  packaging, maintenance cost, and full Mac qualification remain a separately
  reviewed decision and ticket.
- `make lint`, `make test`, and `make spec` pass for the reviewed probe
  preparation.

## Decisions

1. **Change one dependency pin, not logging behavior.** Options were to hide the
   warning, spoof CPU identity, use an arbitrary ONNX Runtime development
   snapshot, or rebuild v1.28.0 with the first cpuinfo commit that recognizes
   Apple on Linux. The last option isolates the proposed cause and cannot make
   a false success by discarding diagnostics.
2. **Require a matched source-built baseline.** Comparing a prebuilt stock
   archive to a custom source build would confound the dependency pin with
   compiler flags, linkage, dead stripping, and toolchain changes. A and B use
   one recipe and differ only in the authenticated cpuinfo dependency fields;
   A must first reproduce the known warning.
3. **Use a maintainer-only temporary candidate.** Committing a custom native
   runtime to the production Dockerfile before qualification would create a
   supply-chain and maintenance commitment. The probe recipe and candidate live
   only on a temporary qualification branch; `main` keeps the accepted rc.12
   runtime until a later adoption decision.
4. **Stop after the A/B `--version` probes regardless of outcome.** A baseline
   mismatch is inconclusive; a remaining patched warning rejects the fix; clean
   patched stderr after the exact baseline warning confirms only this cause. No
   outcome says anything about biological equivalence or performance.
5. **Build natively on the qualifying architecture.** Cross-compilation could
   prove linkage but not the Apple-Docker startup behavior. The decisive image
   is built and run as Linux/ARM64 on the Mac, with the exact commit embedded
   and checked.

## Dependencies

- Ticket 047 rejection evidence:
  `planning/artifacts/047-onnx-runtime-1.28-apple-probe.md`.
- ONNX Runtime v1.28.0 dependency pin:
  `https://github.com/microsoft/onnxruntime/blob/v1.28.0/cmake/deps.txt`.
- cpuinfo Apple Linux support:
  `https://github.com/pytorch/cpuinfo/commit/0c0ab15cb0a8bafbbf71c2ae6f84128a4c2a8da6`.

## Notes

- Apple Docker reports implementer `0x61` and CPU part `0x000`. The selected
  cpuinfo change assigns the Apple vendor before part decoding. The unknown part
  may still have its own diagnostic, so the only acceptance criterion is
  observed empty stderr, not source inspection.
- ONNX Runtime's PR #28344 makes unknown ARM CPU identifiers safe but does not
  add Apple recognition. Do not treat it as this probe's fix.
- The implementation may use a dedicated probe Dockerfile and small scripts
  under a clearly named maintainer path. It must not make ordinary gates fetch
  or compile ONNX Runtime from source.
- The reviewed implementation is committed and pushed only to
  `qualification/ticket-048-cpuinfo-probe` after code review marks it
  `publication-ready`. The coordinator runs `make lint`, `make test`, and
  `make spec` on that exact branch before pushing it. The Mac authenticates
  that exact commit. After the result is retained on `main`, the coordinator
  deletes the local and remote temporary branch; no probe recipe or candidate
  dependency change remains on `main`.
- GitHub workflows triggered by the temporary branch are not substitutes for
  the Apple ARM64 probe and may be cancelled after the exact commit is
  available. No GitHub release, package, or container is published.

## Coordinator Authorship

Coordinator: Codex `/root`, 2026-08-03.

Drafted from Ticket 047's rejected stock-runtime result and the newly verified
upstream cpuinfo chronology.

## Independent Ticket Review

Reviewer: `/root/ticket048_design_review`, 2026-08-03.

The first review rejected three ambiguities: a stock-prebuilt-versus-custom
comparison could not isolate cpuinfo; source, toolchain, archive, and linked
detector identities were underspecified; and the temporary-branch lifecycle did
not define gates or the three possible outcomes. The coordinator revised the
ticket to require matched source-built A/B images, fail-closed exact identities
and linked-cpuinfo proof, and an explicit publication-ready temporary-branch
lifecycle with inconclusive/rejected/confirmed outcomes. The same reviewer
reread the complete ticket and accepted it with no remaining finding.

## Implementation Evidence

Developer: `/root/ticket048_implementation`, 2026-08-03.

Implemented a disposable maintainer-only probe under
`maintainers/ticket-048/`; neither the production `Dockerfile` nor the accepted
rc.12/API-24 Cargo dependency changed. The native ARM64 recipe resolves the
live official ONNX Runtime tag to the expected commit, authenticates that tag
archive and both cpuinfo archives, constructs and compares baseline/patched
source trees, uses the SHA-256-authenticated cpuinfo extraction as the actual
CMake FetchContent source, and builds both CPU-only static runtimes,
removes `download-binaries` only inside the candidate build context, and proves
the custom `libcpuinfo.a` and ONNX Runtime archives entered the executable via
the linker map and retained symbol. It preserves applicable licenses and emits
ONNX Runtime's complete third-party notices, plus the exact source, toolchain,
and static-library identities for the Mac runner to copy into external
evidence.

The runner refetches the live branch from the public HTTPS repository and
requires its commit to equal the clean checkout and fresh `FETCH_HEAD`. It
records that revision before any outcome, then authenticates native Docker
architecture, exact image revision/variant/source labels, extracted cpuinfo
source identity, differing baseline/patched cpuinfo libraries, exit codes,
stdout, and stderr. It builds the patched candidate only after the source-built
baseline reproduces Ticket 047 exactly and records `inconclusive`, `rejected`,
or `confirmed` without running later qualification.

Focused evidence:

- `maintainers/ticket-048/check_probe.py` — pass.
- `maintainers/ticket-048/test_check_probe.py` — 10 mutation tests passed.
- Direct AWK smoke check accepted one record and rejected two records without
  parsing formatted numeric output.
- `sh -n maintainers/ticket-048/run-mac-probe.sh` — pass.
- `ruff check` over both Python files — pass.
- `docker build --check --platform linux/arm64` for the probe Dockerfile — pass
  with no warnings; no ONNX Runtime build or download was performed.
- `git diff --check` — pass.

Upstream identities independently resolved while preparing the recipe:

- ONNX Runtime `v1.28.0` ->
  `da9b5e364c465de65c49d91e696cd6485270757f`; tag archive SHA-256
  `9616cbdbbfcb1420b3261cd280a047d74ab0a249825e577b0e2dd310e22f6b83`.
- cpuinfo baseline zip SHA-1/SHA-256:
  `e58d4b47c16a982111c897e669ae4f1821a393d7` /
  `2ed3ebc6c2656cc0aafc7af319e5cb0f97cc9b415eae180f566def84f1ca6a29`.
- cpuinfo patched zip SHA-1/SHA-256:
  `44419f8b0fda75bb0d2fbe3dd0629493c98ad905` /
  `d40ed2e6134c6b8103ac7916de080a67964f48a8f61c5e72ecca18e9c492f884`.

The decisive source builds and two `--version` probes remain intentionally
unrun pending independent code review and coordinator publication of the exact
temporary branch.

The first Apple preflight authenticated the clean checkout and live public ref
at commit `86d98e5fd3a91281680f00c12ec2cb0b34145e92`, then stopped before
reaching `check_probe.py` or either build because BSD `wc` renders the one-line
count with leading padding. The tester subsequently reran only the read-only
checker independently and it passed. No image, build log, or outcome was
created. The runner now uses output-free AWK
record-count validation, and a mutation test forbids restoring `wc -l` parsing.
The original evidence remains at
`/Users/ian/Desktop/pangopup-mac-evidence-ticket048`; the corrected probe must
use the fresh
`/Users/ian/Desktop/pangopup-mac-evidence-ticket048-rerun1` directory.

## Adversarial Code Review

Reviewer: `/root/ticket048_code_review`, 2026-08-03.

The first review found four material authentication gaps: the declared ONNX
Runtime tag commit was not mechanically verified; CMake could refetch cpuinfo
instead of consuming the independently SHA-256-authenticated bytes; the Mac
runner trusted a stale local remote-tracking ref and did not fully authenticate
image labels; and the statically linked runtime's third-party notices were
missing. The same developer fixed all four, added mutation coverage, and
returned the complete diff to the same reviewer.

The reviewer reran the source checker, all ten mutation tests, shell syntax,
Ruff, `git diff --check`, and Docker's native-ARM64 static validation. It
verified the live ORT ref and archive identities, actual authenticated cpuinfo
FetchContent source, CMake/static/link-map/symbol evidence, A/B library
inequality check, fresh public qualification-ref authentication, exact
fail-fast order, source/variant labels, and complete notices. It accepted the
diff for the temporary qualification branch only, with no remaining finding.
This is not approval to adopt or publish the custom runtime.

## External Effect Evidence

Coordinator: pending

The only external effect is a temporary qualification branch used to let the
Apple ARM64 tester authenticate and build the exact reviewed A/B candidates.
Code review advances the ticket to `publication-ready`; the coordinator runs
all gates, commits and pushes the exact preparation, and confirms the remote
branch before the Mac probe. After an inconclusive, rejected, or confirmed
result is recorded as a compact artifact on `main`, the coordinator deletes
the local and remote branch. No probe source, release asset, package, or image
is retained or published.

## Coordinator Final Check

Coordinator: pending
