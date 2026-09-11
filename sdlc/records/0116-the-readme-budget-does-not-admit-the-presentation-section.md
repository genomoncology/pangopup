---
base: 3edd406b7d25ca9f0ddb3b58089f43206f2346c4
head: 4464c5b88ce35751c5537145b1a1ac7541a52315
---
# The README budget does not admit the presentation section

The README keeps the presentation section exactly as commit `e14e822` wrote it.
`git diff e14e822 -- README.md` is empty at the end of this work. The pinned
figures moved to the file instead.

## What was red

The ticket named four failures. This checkout carried six, in two gates.

`make spec` reported `311 passed, 3 failed` on `3edd406`. Three blocks of
`spec/readme-first-use.md` refused the file: the size block on `wc -l` 271
against `-le 270` and `wc -w` 1829 against `-le 1770`, and on a heading list
missing `## Watch a presentation on it`; the opening-space block on an
image-mark count of 2 against a pin of 1; and the third block on `supported
non-SNV` and `` explicit `--model-only` request ``. Those two phrases live in
the text equivalent, and its window stopped at the first `## ` heading, which
the new section supplied above them.

`make test` was red too, which the ticket did not record.
`tests/readme-branding.sh` exited 1 on `README image check failed: README must
contain no Markdown image other than the exact hero`.
`scripts/check-readme-images.sh` pins the same count a second time.

## What landed

The figures are the size of the file as it stands: 271 lines, 1,829 words, two
image marks, and a heading list led by `## Watch a presentation on it`. They
were 270 lines and 1,770 words against a README of 260 lines and 1,766 words,
so ten lines and four words stood unclaimed. `spec/readme-first-use.md` records
both pairs, the commit that moved them, and the rule that a later addition is
paid for out of the guide rather than added to it. That rule paragraph stays and
still governs the next sentence.

The two phrases move to the block that already governs the space before Quick
start, where they stand. `scripts/check-readme-images.sh` pins the thumbnail
line whole, the way it pins the hero, and takes the thumbnail into its asset
inventory at 1280x720 and 200,000 bytes.

`tests/readme-budget-exactness.sh` is the new gate. It reads every pinned figure
out of both files and requires each to equal what `README.md` measures, so no
ceiling can stand above the tree again. It then runs the same check against a
README grown by one line, one word, one renamed heading and one added image, and
against a contract whose line cap was raised one above the file, and requires
each to be refused by name. A figure it cannot find is refused rather than
skipped.

## The refusals, measured through `make spec`

Each mutation was applied to `README.md`, `make spec` was run, and the file was
restored from a copy taken beforehand.

| mutation | file | `make spec` |
| --- | --- | --- |
| one blank line appended | 272 lines, 1,829 words | `313 passed, 1 failed` |
| one word after the final newline | 271 lines, 1,830 words | `313 passed, 1 failed` |
| one sentence appended | 273 lines, 1,837 words | `313 passed, 1 failed` |
| `## Docker` renamed `## Containers` | 271 lines, 1,829 words | `312 passed, 2 failed` |
| a third image mark, same size | 271 lines, 1,829 words | `313 passed, 1 failed` |

Each assertion was also run alone to name which one answered: the line cap on
the first, the word cap on the second, both on the third, the heading list on
the fourth, the image-mark count on the fifth. The third-image mutation was put
to `tests/readme-branding.sh` as well, which refused it with `README must
contain no Markdown image other than the hero and the presentation thumbnail`.
`make spec` reported `314 passed` after each restore.

Neighbouring behaviour was probed the same way. Deleting the thumbnail line,
deleting the maker attribution and deleting `supported non-SNV` from the text
equivalent each turned the opening-space block red and `make spec` to
`313 passed, 1 failed`.

## The ladder

`make lint` 2.21 s, exit 0. `make test` 141.62 s, exit 0. `make spec` 15.76 s,
`314 passed`, 0 skipped, 0 failed. `scripts/run-service-fixture-tests.sh`
2.26 s, 20 tests matched. `bash sdlc/scripts/lint` exit 0. All 30
`PORTABLE_QUALIFICATION` and 3 `SHELL_QUALIFICATION` gates were run one at a
time, 33 of 33 exit 0. The new gate costs 0.10 s.

Run with `CARGO_HOME` and `RUSTUP_HOME` at the operator's own, `HOME` moved to a
private root, the four `PANGOPUP_*` names unset, and `ORT_CACHE_DIR` never
exported. `~/.cache/pangopup/model-results.sqlite3` is byte-identical after all
of it: md5 `0ce402e99f91b4d16ed3aaddc88160fc`, 303,104 bytes, mtime unchanged.
`~/.cache/ort.pyke.io` holds 288,252,072 bytes, the figure it held beforehand.
No `pangopup` or `pangopup-build` process is left running.

Verify filed no draft.
