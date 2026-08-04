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
one-line patch, retained cpuinfo symbol, absence of a dynamic ONNX Runtime
dependency, and absence of the stock download feature. It passes ONNX
Runtime's required container-build root opt-in, uses the actual Release static
library directory, and explicitly supplies ONNX Runtime 1.28's
`libmodel_package.a`, which `ort-sys` rc.13 omits from its full-static library
list. The SHA-256-authenticated cpuinfo extraction is passed to CMake as the
actual FetchContent source. The image retains ONNX Runtime's `LICENSE` and
`ThirdPartyNotices.txt` plus cpuinfo's `LICENSE`.
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
maintainers/ticket-048/run-mac-probe.sh ABSOLUTE-FRESH-EVIDENCE-DIRECTORY
```

## Apple result

The A/B experiment was confirmed on the Apple M5 Max Docker host using
PangoPup source revision
`c3ca22c4187127c8e1d8646d24f8eb07c788d739`. That is the source revision in
the retained Mac evidence; it is intentionally not rewritten to the later
commit that incorporates the independently discovered harness amendments.

The baseline image
`sha256:7f152b61e3faf636964d9701ab12d5c014bcf131ba0f21fa0430896c11786fe8`
reproduced the exact 76-byte warning. The Apple-aware image
`sha256:047246f47d305a294650fc8795a97136c6085215c3ab54848a5b7b1bee3b64f1`
kept byte-identical stdout and produced empty stderr. Both were native
`linux/arm64`, exited zero, and were not published.

Two earlier attempts were inconclusive harness failures rather than product
results. The first exposed BSD `wc` padding; the second exposed ONNX Runtime's
explicit refusal to build as root without opt-in. The independent successful
run then found the Release-library path, missing `libmodel_package.a`, and two
unreliable concurrent-link-map greps. Its retained effective Dockerfile is now
the repository recipe byte-for-byte. Executable `nm` and `readelf` checks are
the final static-link evidence.

The runner stops before building the patched image unless the matched
source-built baseline reproduces Ticket 047's exact warning. It stops with a
failure unless the patched image writes completely empty stderr. It does not
run biological, cache, performance, service, package, or release tests. The
confirmed causal result does not approve this custom runtime for production.

Do not push either image or any build/evidence output. Send the coordinator the
small report and the recorded image IDs, source revision, stdout, and stderr.
The coordinator will retain a compact artifact on `main`, then delete this
temporary branch.
