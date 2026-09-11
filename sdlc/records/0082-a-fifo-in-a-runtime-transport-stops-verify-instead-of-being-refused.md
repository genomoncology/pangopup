---
base: ea99ccebb6e0102c8fee5b29c6dc92d9d17b8d22
head: d53ed69f6bbf129a4d4503d9826e3abfc13de9d3
---
# A FIFO in a runtime transport stops verify instead of being refused

`pangopup-build runtime-transport verify` judged a member only after it had
opened it, so an open of a writerless FIFO waited and the command never
returned. The open now carries `O_NONBLOCK`, which cannot wait, and the kind
check runs on what came back. A member that is not a regular file is refused as
`PART_SET_INVALID` with `<name> is not a regular file`. The refusal names the
member, so an operator reads which one is wrong and what is wrong with it.

The rule covers every kind at once. A FIFO and a directory fail the kind check
after the open. A socket fails the open with `ENXIO` and a symlink with
`ELOOP`, and both errno are read as a member kind the open refuses rather than
as an IO fault. Every other errno stays `INPUT_IO` with the system's own text,
so a regular member that cannot be read for an ordinary reason still reports
the IO refusal it always did.

The spec pin ticket 0080 removed with its reason is back in
`spec/runtime-transport.md`, live, as an exact-output pin rather than a
`--fails` refutation: `--fails` reads a command killed by `timeout` as a
satisfied refutation, so it would go green on a command that still waits.
`timeout` bounds the block. `tests/spec-refutation-evidence.sh` no longer
refuses `mkfifo` in a spec block; it now refuses an unbounded `pangopup-build`
or `pangopup` command inside a block that builds one, which is the shape that
hangs the gate instead of failing it.

Exercised beyond the suite under a private `HOME`, `XDG_CACHE_HOME` and
`TMPDIR`, on a transport packed by the shipped command. With no bound but a
120-second safety net, verify on a transport whose `model-NOTICE` is a FIFO
returned on its own in 5 milliseconds with
`{"status":"error","code":"PART_SET_INVALID","message":"model-NOTICE is not a
regular file","details":null}`. A FIFO in the stored member `model.onnx.zst`
was refused by that name as well, so the raw and stored routes agree. A socket
at `reference-NOTICE`, a directory at `mask-NOTICE` and a symlink at
`mask-NOTICE` were each refused by name in the same time. A regular member
under mode `000` still reported `INPUT_IO` with `Permission denied (os error
13)`. The intact transport verified, unpacked, and its model, reference and
mask each compared byte-identical to the fixture they came from. Unpack on the
FIFO transport gave the same named refusal and published nothing. The corrupt,
substituted and extra-member neighbours kept their own messages. Removing
`timeout` from the restored spec block was refused by the evidence gate, naming
the line.

Four costs are accepted rather than carried as open work. `EIO`, `ENOSPC`,
`ENOMEM` and `ENFILE` were not exercised; they share the untouched fall-through
arm with `EMFILE`, which was exercised live. A cross-device member still gets
the unnamed same-filesystem message, deliberately kept separate, and falls
outside this ticket's boundary. `held_regular_size` keeps a combined message,
but its single caller overwrites it, so the difference is unobservable.

`make lint`, `make test`, `make spec`, `scripts/run-service-fixture-tests.sh`,
`bash tests/spec-refutation-evidence.sh` and `bash sdlc/scripts/lint` all pass
on this candidate. `make spec` reports `306 passed, 6 skipped` against the
base's `304 passed, 6 skipped`; the two new blocks are the restored pin and its
expected output. The refutation floor still reads 27, the FIFO pin being an
exact-output pin the count does not read. No digest, manifest or member-set
claim moved.
