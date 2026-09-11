---
base: 65d93b316945126512292b5beb222ae948c1b43e
head: 27b82a841b40db86f65f944c93cdea462bcec039
---
# The gate ladder does not check or report what it claims

Three checks that reported success while checking nothing now check. Six
`spec/` bash blocks that mustmatch skipped whole assert and can fail;
`tests/spec-block-execution.sh` refuses any fence mustmatch would skip, by name
and line. `scripts/spec-refutes.sh --fails` refuses status 126 with its own
sentence, and 127 keeps the message it had, byte for byte, because a missing
command and one that lost its execute bit want different repairs.
`tests/rust-literal-continuity.sh` refuses the trace a hand-wrapped Rust string
literal leaves when its trailing backslash goes missing, exempting exactly one
declaration by path and opening together. `tests/built-executable-currency.sh`
reads a code line carrying an earlier `#` as code, sees a build after the first
use, does not read a cargo command inside a `grep` argument as a build, and
floors on the four harnesses the tree actually holds. Every assertion in
`tests/executable-delivery.sh` names what it wanted, through the four forms in
`tests/support/expected-text.sh`.

## The delivered scope is smaller than the ticket text

The ticket names six sections. Two were already closed before this flight
began, by tickets 0082 and 0092 -- the inheritance check in
`tests/shell-spawn-cache-isolation.sh` reads a hand-off rather than a mention
since 0092, with fixtures both ways -- and nothing was authored for either.

Section 4b, "a build after the first use is invisible", was judged not a defect
in this file. The scan compares the first build with the first use, which is
exactly the sentence it holds: a harness must build before it first reaches
into `target/debug`. A later build says nothing about that first reach. The
re-measurement of each section against the file as it stands today, rather than
against the ticket's 2026-09-10 reading, is the reason the delivered scope is
the smaller one. Three residues of that file that are real are filed as
`sdlc/tickets/drafts/0102`, and the 69 assertions of the same silent shape in
`tests/production-release-qualification.sh` as `sdlc/tickets/drafts/0100`.

## The container harness stays in `make spec`

`spec/container-image.md:9` was kept, with the reason written at the top of
`tests/container-delivery.sh`. That harness does execute
`scripts/qualify-container.sh`, whose EXIT trap runs `docker rm --force` twice
and `docker volume rm --force` once. It reaches nowhere all the same:
`qualify-container.sh` exits 2 on an invalid expected registry digest before
its first `docker image inspect`, so nothing is pulled, built or started, and
all three removals are redirected under `|| true`. Verified with logging stubs
in place of `docker`, and again against a live daemon holding 6 real
containers: containers, images, volumes and networks were byte-identical before
and after the whole ladder. The block changes directory to the repository root
first, because mustmatch runs a block with the working directory set to the
markdown file's own directory and that harness reads `Dockerfile` and
`.github/workflows/` by relative path.

## What the blocks found when they finally ran

The inspect transcript in `spec/model-kernel.md` was missing
`"representation":"singleton"`. That field arrived in commit `c815901` on
2026-07-26: 47 days and 516 commits of silent staleness in a block that had
never run a line. The pinned text is now the command's own output.

The two `text` fences that carried those transcripts are restored in this
repository's own idiom, `run id=` on the command and `text expect=<id> exact`
on the transcript, which pins the bytes and stays readable. `exact` also
refuses a reordered key, which the inline canonical-JSON argument did not:
swapping `"cases"` and `"channel_arrays"` in the qualify pin fails the block.

Plain `mustmatch` compares complete canonical JSON, so a field added to the
output is caught. `mustmatch like` is the subset form and tolerates added
fields; where a whole record is pinned that way, a new field is absorbed
silently, which is filed as `sdlc/tickets/drafts/0101`.

## Two defects of the ticket's own families landed inside its own gates

`tests/negative-assertion-strength.sh` exempted any statement carrying `&&`.
Outside `[[ ]]` that consumes nothing -- `grep -Fq x file && found=1` neither
names what it wanted nor stops the harness -- so the exemption blessed a worse
shape than the bare one it refused. Its heredoc detector also read `<<<` as
opening a heredoc, which swallowed the statements after a here-string. Both are
repaired, each with the fixture that proves it.

At verify a third of the same family was found and repaired here. The
collapsed-literal scan refused a whole comment line: a five-line `//` table
copied out of `tests/spec-block-execution.sh` into a crate source passed
`cargo fmt --all --check` and was refused, with a message telling its author to
restore a continuation that was never there. The scan now reads past a line
beginning `//`, which loses nothing, because a collapsed literal never begins
one. The two-line form of the same accident -- the backslash gone, the two
source lines left where they are -- is caught by nothing, and is filed as
`sdlc/tickets/drafts/0103` because catching it means telling a line that
continues a literal from one that begins it.

## Exercised

Base against candidate, on copies. Deleting the `mustmatch` call from
`spec/runtime-profile.md:46` made mustmatch report `14 passed, 1 skipped` and
exit 0, naming nothing; the candidate gate refused with
`spec/runtime-profile.md:45 (1 line(s) never run)`. A `--fails` pin on a shell
script at mode 644 exited 0 on the base and is now refused with `the command is
there and could not be run, so its failure proves nothing`; a directory behaves
the same way, and 127 keeps its own sentence. Collapsing the literal at
`crates/pangopup-cache/src/lib.rs:703` left `cargo fmt --all --check` green on
both, and the candidate gate names the line.

A maintainer is not obstructed. A new asserting spec block was accepted and
moved the gate's counts from 315 to 316 fenced blocks and 204 to 205 shell
blocks, so the count is measured rather than written down. Four new assertions
added to `tests/executable-delivery.sh` through the new helpers were accepted,
and one bare `grep -Fq` in their place was refused by file and line. `make
spec` reports `314 passed` and no skips.

`pangopup-build model inspect` and `model qualify` were run against the
fixture bundle and their output compared with the two pins: identical by
`diff`, by `cmp` and by md5.

`make lint`, `make test`, `make spec`, `scripts/run-service-fixture-tests.sh`,
each of the 22 portable and 3 shell qualification harnesses run on its own, and
`bash sdlc/scripts/lint` all pass on this candidate. `make test` took 103.9 s.
