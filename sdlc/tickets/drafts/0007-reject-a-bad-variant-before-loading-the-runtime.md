---
flow: build
priority: 4
---
# A variant rejected on its own terms fails before the runtime is loaded

Refusing bad input costs as much as scoring good input, because the model and
reference are opened before the submitted variant is examined.

Measured on the retained Ryzen host, with the assets installed and warm:

```
5.41s  allele longer than 100 bases   (refused)
5.55s  REF does not match GRCh38      (refused)
5.40s  a variant that is scored        (full inference)
0.01s  a variant answered from cache
0.01s  a variant answered from the precomputed index
```

An allele longer than 100 bases is decided by the submitted variant alone. It
needs no model, no reference and no mask. A caller waits five seconds to be told
a length.

The lookup path is the fastest thing this project has, and its first impression
for anyone trying the command with a typo is that the tool is slow. The service
opens the profile once and does not show this; the command pays it every run.

Checks that need the reference, the REF comparison among them, are not in scope
here and stay where they are.

Issue: `sdlc/issues/2026-09-04-reject-cheap-input-errors-before-loading-the-model.md`

Done, observably:

- A variant that can be refused from the submitted request alone is refused
  without the runtime assets being opened.
- Such a refusal reports the same code and message it reports today, and the
  command exits as it does today.
- A variant that requires the reference to judge is still refused exactly as it
  is now.
- A batch containing one refusable variant still yields no partial results.
- The suite pins the earlier refusal with a case that fails before the change.

Boundary: do not change which variants are refused, what any refusal says, or
the order in which a batch reports. Do not move the reference-dependent checks.
Do not change the precomputed, cached or scoring paths, the batch atomicity
guarantee, or anything the HTTP service does — it opens the profile once at
startup and is unaffected.
