# Ticket 048 maintainer probe

This directory exists only on the temporary
`qualification/ticket-048-cpuinfo-probe` branch. It builds two native ARM64
images from ONNX Runtime v1.28.0. The builds differ only in the authenticated
cpuinfo dependency line shown in `expected-cpuinfo.patch`.

Pinned inputs:

- ONNX Runtime tag `v1.28.0` resolves to commit
  `da9b5e364c465de65c49d91e696cd6485270757f`; the GitHub tag archive has
  SHA-256
  `9616cbdbbfcb1420b3261cd280a047d74ab0a249825e577b0e2dd310e22f6b83`.
- Baseline cpuinfo commit
  `4628dc060ce4e82345dc166bbac875609db4ff69` uses upstream archive SHA-1
  `e58d4b47c16a982111c897e669ae4f1821a393d7` and SHA-256
  `2ed3ebc6c2656cc0aafc7af319e5cb0f97cc9b415eae180f566def84f1ca6a29`.
- Patched cpuinfo commit
  `0c0ab15cb0a8bafbbf71c2ae6f84128a4c2a8da6` uses upstream archive SHA-1
  `44419f8b0fda75bb0d2fbe3dd0629493c98ad905` and SHA-256
  `d40ed2e6134c6b8103ac7916de080a67964f48a8f61c5e72ecca18e9c492f884`.

The Dockerfile verifies the live official ONNX Runtime tag, archives, exact
one-line patch, custom static link map, retained cpuinfo symbol, and absence of
the stock download feature. The SHA-256-authenticated cpuinfo extraction is
passed to CMake as the actual FetchContent source. The image retains ONNX
Runtime's `LICENSE` and `ThirdPartyNotices.txt` plus cpuinfo's `LICENSE`.
The Mac runner preserves the compiler/CMake/Rust versions and hashes of the
linked cpuinfo library, complete static-library manifest, and final images; it
also rejects identical baseline/patched cpuinfo library identities.

The ordinary repository gates do not invoke this build. Before pushing the
temporary branch, run the cheap checks:

```text
maintainers/ticket-048/check_probe.py
maintainers/ticket-048/test_check_probe.py
make lint
make test
make spec
```

On the Apple Silicon Mac, fetch and check out the exact temporary branch, then
run from the clean checkout:

```text
git fetch origin \
  qualification/ticket-048-cpuinfo-probe:refs/remotes/origin/qualification/ticket-048-cpuinfo-probe
git switch -C qualification/ticket-048-cpuinfo-probe \
  origin/qualification/ticket-048-cpuinfo-probe
maintainers/ticket-048/run-mac-probe.sh \
  "$HOME/Desktop/pangopup-mac-evidence-ticket048"
```

The runner stops before building the patched image unless the matched
source-built baseline reproduces Ticket 047's exact warning. It stops with a
failure unless the patched image writes completely empty stderr. It does not
run biological, cache, performance, service, package, or release tests.

Do not push either image or any build/evidence output. Send the coordinator the
small report and the recorded image IDs, source revision, stdout, and stderr.
The coordinator will retain a compact artifact on `main`, then delete this
temporary branch.
