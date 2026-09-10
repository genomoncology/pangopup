---
---
# A spec bash block that asserts nothing is skipped whole

mustmatch runs a plain ```bash block only when the block calls a `mustmatch`
command. A block that ends without one is reported as `SKIP` and none of its
lines run. The summary line counts it as a skip beside the passes, so nothing
in a gate log says which claim went unchecked.

Measured in this checkout on 2026-09-10 while running the design stage of
ticket 0080. `mustmatch test -v spec/runtime-transport.md` reported
`SKIP Derived runtime-asset local transport (line 79)`. That block was the one
holding the four checks that a corrupt, substituted, symlinked or FIFO runtime
transport is refused. Ticket 0080 takes that
block; these six are outside it:

- `spec/container-image.md:9` runs `bash tests/container-delivery.sh`.
- `spec/model-kernel.md:7` runs `pangopup-build model inspect`.
- `spec/model-kernel.md:20` runs `pangopup-build model qualify`.
- `spec/runtime-profile.md:45` checks that no profile was written.
- `spec/runtime-release.md:58` checks that no release directory was created.
- `spec/runtime-release.md:77` checks that no staging directory was left behind.

The two `model-kernel.md` blocks are each followed by a ```text block with no
`expect=` attribute, so the transcript beneath them pins nothing either.

Done, observably:

- A spec bash block that mustmatch would skip fails a gate, naming the block.
- Every block listed above either asserts something or is removed with a
  reason.

Boundary: this states no new claim about what the software does. It makes the
blocks that already carry claims run. `spec/container-image.md:9` invokes a
container harness; whether that harness belongs in `make spec` is part of the
decision, not a foregone one.
