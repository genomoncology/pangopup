# Ticket 048 Apple cpuinfo A/B probe

Ticket 048 tested whether ONNX Runtime 1.28's cpuinfo pin causes its Apple
Docker Desktop unknown-vendor warning. The matched source-build experiment
confirmed that narrow causal hypothesis. It does not approve a custom runtime
for production.

## Exact experiment

- PangoPup source revision tested:
  `c3ca22c4187127c8e1d8646d24f8eb07c788d739`.
- ONNX Runtime: v1.28.0 commit
  `da9b5e364c465de65c49d91e696cd6485270757f`.
- Baseline cpuinfo:
  `4628dc060ce4e82345dc166bbac875609db4ff69`.
- Apple-aware cpuinfo:
  `0c0ab15cb0a8bafbbf71c2ae6f84128a4c2a8da6`.
- Both images: native `linux/arm64`; neither was published.

The two source trees differed only in the authenticated cpuinfo dependency
fields. Both final executables retained `cpuinfo_initialize` and had no dynamic
ONNX Runtime dependency.

## Result

Both `pangopup --version` commands exited zero and wrote byte-identical
15-byte stdout (`pangopup 0.1.0\n`). Baseline stderr was the exact expected
76-byte unknown-vendor warning. Apple-aware stderr was empty.

- Baseline image:
  `sha256:7f152b61e3faf636964d9701ab12d5c014bcf131ba0f21fa0430896c11786fe8`.
- Apple-aware image:
  `sha256:047246f47d305a294650fc8795a97136c6085215c3ab54848a5b7b1bee3b64f1`.

This confirms that advancing cpuinfo to its first Apple Linux MIDR-aware
revision removes the observed warning. It does not establish biological
equivalence, old-cache reuse, performance, packaging, maintenance cost, or
full Mac qualification; those remain required before any production adoption.

## Harness findings and provenance

Two earlier attempts were inconclusive. BSD `wc` padding broke the original
remote-ref record check, and ONNX Runtime's build script then rejected Docker's
root build user without explicit opt-in. The independent successful run also
corrected the static Release-library path, explicitly linked ONNX Runtime
1.28's `libmodel_package.a` omitted by `ort-sys` rc.13, and removed two
concurrent-link-map greps that were not reliable final-binary evidence. The
repository probe recipe was subsequently amended to match that effective
recipe byte-for-byte.

Raw evidence remains outside git on the qualifying Mac at
`/Users/ian/Desktop/pangopup-mac-evidence-ticket048-rerun2-independent`.
The original and first-rerun evidence directories were preserved unchanged.
