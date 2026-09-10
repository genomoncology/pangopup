---
---
# A FIFO in a runtime transport stops verify instead of being refused

`pangopup-build runtime-transport verify` opens each member of the transport
directory and reads it. A FIFO named as a member has no writer, so the open
blocks and the command never returns. It neither accepts the transport nor
refuses it.

Measured in this checkout on 2026-09-10 while running the design stage of
ticket 0080. `spec/runtime-transport.md` builds a transport whose `model-NOTICE`
member is a FIFO and claims that verify refuses it. That claim had never run:
it was written in the inert `!` form, inside a block that mustmatch skipped.
Made live, the block failed with `bash block timed out after 30 seconds`, and
the verify process was left in `wait_for_partner` on
`target/spec/runtime-transport/fifo/model-NOTICE` for more than five minutes
after mustmatch had given up on the block. mustmatch's block timeout does not
reach the grandchild.

A corrupt, substituted and symlinked member are each refused, so the FIFO is
the one member kind that stops the command rather than being judged.

Done, observably:

- A transport whose member is not a regular file is refused, with the member
  named, and verify returns.
- `spec/runtime-transport.md` carries that claim as a live refutation.

Boundary: this is about what verify does with a member that is not a regular
file. It states no new claim about the digests or the manifest.
