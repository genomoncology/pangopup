# Ticket 047 ONNX Runtime 1.28 Apple probe

Ticket 047 tested one narrow hypothesis: whether moving from `ort`/`ort-sys`
2.0.0-rc.12 and ONNX Runtime 1.24.2 to rc.13 and ONNX Runtime 1.28.0 would stop
the Apple CPU-vendor warning without hiding it. The fail-fast probe rejected
the candidate, so the accepted runtime on `main` remains unchanged.

## Exact candidate

- Commit: `12ea6aa1e486ef5cfd17fc01e522fad5eab37748`.
- Candidate: exact `ort` and `ort-sys` 2.0.0-rc.13, ONNX Runtime 1.28.0,
  explicit API 27, with no warning filtering or loading-policy change.
- Image: native `linux/arm64`, 24,284,560 bytes.
- The image revision label matched the exact candidate commit.

## Fail-fast result

On the Apple M5 Max Docker host, `pangopup --version` exited zero and wrote the
expected 15 stdout bytes:

```text
pangopup 0.1.0
```

Stderr was 76 bytes and contained exactly:

```text
onnxruntime cpuid_info warning: Unknown CPU vendor. cpuinfo_vendor value: 0
```

The candidate therefore did not solve the problem it was intended to solve.
The tester stopped immediately. No biological, cache-interoperability,
performance, full command, HTTP, production-model, package, or release
qualification was run, and no result from those deferred stages is claimed.

## Evidence provenance and cleanup

This Linux checkout did not open the Mac's raw evidence. The tester supplied a
report whose raw copy remains on that Mac at:

`/Users/ian/Desktop/pangopup-mac-evidence-ticket047/REPORT.md`

The report retains one tester-command mistake: wrapper `01-checkout-verify`
ran from the workspace parent instead of the fresh clone. Wrapper
`02-checkout-verify-corrected` reran from the fresh clone, followed by
`03-exact-commit-assertions`. Those corrected checks authenticated the branch,
`FETCH_HEAD`, and detached `HEAD` as the exact candidate commit and showed a
clean checkout. The initial wrapper mistake did not affect the image build or
probe.

After the evidence was supplied, the coordinator deleted both the remote and
local `qualification/ticket-047-ort-probe` branches. `main` was never advanced
to the rejected candidate and remains on the accepted rc.12 / ONNX Runtime
1.24.2 implementation.
