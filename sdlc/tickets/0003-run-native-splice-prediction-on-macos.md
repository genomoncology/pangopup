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

Candidate `2d0e93861bd379f5c650db859ea47a6fcc56f6e8` passed independent code review, the complete macOS `make lint`, `make test`, and `make spec` gates, and two complete native Linux `make test` runs from a byte-verified archive of that exact commit. Fresh hosted run 33846843073 passed the macOS job but failed the Linux `make test` step with exit code 2. This session lacks authenticated access to the job log, so the failing test is not known. Fresh hosted container run 33846843016 passed both AMD64 and ARM64 smoke jobs. The ticket remains open until a later hosted Linux run passes or exposes a reproducible defect.

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
