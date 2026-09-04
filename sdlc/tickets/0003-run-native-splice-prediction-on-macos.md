---
flow: build
priority: 6
deps: ["0002"]
---
# Run asset-backed splice prediction natively on macOS

## Outcome

A developer can install or synchronize the production asset tuple, inspect its status, and run the native lookup-first splice service on macOS. The same inputs produce the same observable results on Linux and macOS. Continuous integration protects both platforms.

## Current facts

The Rust workspace builds a native Apple Silicon executable. Existing qualification covers Linux ARM64 inside Docker Desktop on a Mac. Native macOS model fallback remains unqualified. Main still returns `UNSUPPORTED_PLATFORM` from `status`, `assets install`, `sync`, and `serve`. The Makefile excludes the asset-backed specifications on macOS. Two of those specifications use GNU-only `find -printf`.

Ticket 0002 supplies the descriptor-relative asset boundary this behavior requires.

## Acceptance status on 2026-09-04

Candidate `2d0e93861bd379f5c650db859ea47a6fcc56f6e8` passed independent code review, the complete macOS `make lint`, `make test`, and `make spec` gates, and two complete native Linux `make test` runs from a byte-verified archive of that exact commit. Hosted runs 33846843073 and 33847656728 passed the macOS job, Linux lint, the Linux ARM64 cross-check, and the portable service fixture. Both failed the Linux `make test` step with exit code 2. Hosted container runs 33846843016 and 33847656772 passed both AMD64 and ARM64 smoke jobs. An exact Ubuntu 24.04 x86_64 reproduction of the hosted command sequence exposed `reference_reader::tests::identified_reference_rejects_same_size_corruption_symlink_and_hash_races`. Its 6,648-byte fixture fit in one 65,536-byte read. The after-chunk mutation changed bytes that the hasher had already consumed, and the assertion then depended on the filesystem advancing its metadata timestamp between adjacent operations. `architecture/design.md` defines concurrent in-place mutation or truncation of the identified inode as outside the supported threat model. The remediation renames the test and moves its admitted mutation after descriptor admission and before the first read. Digest or metadata validation now detects it without depending on timestamp resolution. The first complete remediated Linux `make spec` run then exposed three descriptor-relative SNV transport message regressions. Linux returned its raw symbolic-link error or an internal held-entry message where the established specification requires `MANIFEST_INVALID: required input is not a regular file` and `PART_SET_INVALID: transport entries must be regular files`. The remediation preserves both error kinds and normalizes only held non-regular-entry failures at the SNV transport boundary. Focused tests now pin both messages. The ticket remains open until the remediated candidate passes independent review and hosted Linux CI.

Hosted run 33853360764 at candidate `8b9524fc9f8fe3f3d68a812d504903d224c35f4d` passed macOS check 100960957014. Linux check 100960956749 passed lint, the Linux ARM64 cross-check, and the portable service fixture before `make test` failed after 47 seconds with exit code 2. Container run 33853360730 passed both Ubuntu 24.04 AMD64 and ARM64 smoke jobs. The anonymous GitHub check API exposes only that exit code, and the anonymous job-log endpoint returns HTTP 403. The exact candidate passed an Ubuntu 24.04 x86_64 `make test` run under the hosted environment variables. It also passed 300 repetitions of the repaired reference test. A concurrent four-group stress run on the same four CPUs passed 200 repetitions of the signal test, 20 repetitions of each asset timeout test, four repetitions of the transport resource test, and three repetitions of the SNV regression fixture. These probes did not reproduce a product or test defect. The Linux workflow now preserves the test gate exit status and emits the final bounded test log through a public check annotation when the gate fails. It does not retry the gate or change production behavior. The ticket remains open until that evidence identifies the remaining failure or hosted Linux CI passes.

Hosted run 33862030637 at candidate `e7831288b792ecfcd48940cd43a9c0f5e015b87e` passed macOS check 100988425400. Linux check 100988425657 failed only in `Run Linux tests with public failure evidence`. The public check annotation existed. GitHub's API reported a 4,096-character message retained from the start of the submitted 16,000-byte summary. The retained text ended before the final failure. The job-log endpoint remained HTTP 403 without admin rights. This remediation caps the raw final log tail at 1,300 bytes before workflow-command escaping. Escaping can expand every byte to three characters. The annotation message therefore stays at or below 3,900 characters and keeps the final output. A Bash 3.2-compatible causal test simulates the first 4,096 retained characters. The ticket remains open until hosted Linux CI identifies the failure or passes.

Hosted run 33863790902 used the bounded public annotation to expose the remaining Linux failure. Linux job 100993919570 passed lint, the Linux ARM64 command build, and the portable service fixture before `local_install_is_atomic_reusable_and_lock_safe` failed under the non-root runner. Transport inspection opened a mode-000 payload part even though reuse needs only its descriptor-relative no-follow metadata. The correction checks each payload part's file type, filesystem, and size through the held directory without opening it. A fresh installation still opens and authenticates every payload byte before publication. The exact mode-000 reuse test now passes as a non-root Linux user, and a separate corruption test proves that a fresh install rejects changed payload bytes without publishing a bundle. The ticket remains open until the corrected candidate passes hosted Linux CI.

## Done, observably

- `pangopup sync`, `pangopup assets install`, and `pangopup status` complete on macOS against a local transport and match Linux for the same input.
- `pangopup serve` starts on macOS from an activated asset profile. Lookup hits and model fallback return the same scored results as Linux.
- The local-assets, remote-assets, runtime-install, runtime-release, runtime-transport, and HTTP-service specifications run on macOS. Their commands use tools available on the stock CI host.
- A macOS CI job runs the focused portable asset tests and every specification restored by this ticket. It runs all three full gates when the required repository tools can be installed without secrets. Any narrower gate states and enforces its remaining exclusions.
- Existing Linux CI stays green and continues to run the full gates.
- `README.md`, `architecture/delivery.md`, and `architecture/runtime-data.md` describe the supported native macOS path and its remaining limits.
- `make lint`, `make test`, and `make spec` pass on macOS and Linux.

## Boundary

Do not add a native macOS release archive or installer. Do not make `uninstall` portable. Do not change model math, lookup-first routing, asset identity, or scoring output. Do not suppress the retained Linux ARM64 warning from ONNX Runtime.
