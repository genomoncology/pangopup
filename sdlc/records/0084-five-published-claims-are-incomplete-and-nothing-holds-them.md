---
base: 34a1560782a01b37a681c10f230b7b8b5b4a8970
head: e86b46b11a22e3820249e2a610b10efc9b711c78
---
# Five published claims are incomplete and nothing holds them

Five sentences a consumer reads were each incomplete or wrong, and no gate
read any of them. Each is corrected in the document that carries it, and
three new checks read the claim back out of the evidence the repository
already keeps.

`architecture/service.md` and `README.md` now say that every returned score
item carries `data_set_version`, in the section that already says it of
`scoring_identity`. `architecture/compatibility.md` stays the one place the
response shape is enumerated and is unchanged. `architecture/compatibility.md`
and `spec/score-value.md` now carry, beside the disagreement figures, the fact
that the measured set holds transversions and no transitions and that a
transition-bearing set could move either figure by an amount nothing measured.
`architecture/compatibility.md` states that a gene's score can depend on which
other same-strand genes overlap the same variant, names
`P01-same-strand-order`, and names the one record of snv-stride-500k-v1 where
the two routes render a different value at a variant more than one gene
overlaps. `architecture/index.md` says that 15,030,604,105 bytes is
1,366,418,555 gene loci multiplied by 11, carries the measured
15,030,603,775-byte payload beside it, and accounts for the 330-byte
difference as the 30 `REF=N` loci held in the exception section.

`tests/published-claim-evidence.sh` holds the first, third and fourth.
`tests/route-disagreement-rate.sh` grew two fields and holds the second.
`tests/spec-record-pin-completeness.sh` holds every `spec/` JSON-object pin to
comparing a complete record unless the block declares itself partial and says
why. Both new harnesses joined `PORTABLE_QUALIFICATION`.

## The four documents, read as a consumer reads them

The `data_set_version` sentence lands where a reader needs it in both
documents. In `architecture/service.md` it stands directly after the sentence
that puts the same value on the status route, and its "too" reads correctly
there. In `README.md` it stood after the instruction to store the value, where
"too" had nothing before it to attach to; verify swapped the two sentences.
The README section is a dense run of telegraphic statements, and the corrected
sentence reads as one of them rather than as a bolt-on.

`architecture/compatibility.md` holds together. Neither addition duplicates
anything already in that file. The two substitution sentences stand inside the
`## Score values` paragraph, immediately after the last percentage and before
the sentence saying the rate was measured once, which is where a reader meets
the figures. The same-strand paragraph is new and stands on its own. It did
read badly at its close: it states the mask carry-over, then names a record
where the two routes disagree, and a reader takes the second as evidence for
the first. The committed measurement does not say that. Verify added two
sentences saying what the measurement shows and what it does not. The
mechanism is also stated in `architecture/design.md:233` and
`architecture/runtime-data.md:260`, both about the implementation; the new
paragraph is the first statement of it in the consumer contract.

A reader can follow the arithmetic in `architecture/index.md` from the numbers
on the page: 1,366,418,555 times 11 is 15,030,604,105, less the measured
15,030,603,775 is 330, which is 30 loci at 11 bytes. The link to the build
artifact is there for a reader who wants the source, not for the arithmetic.
One sentence beside it read wrong. The paragraph above called the figure
"before directories and exceptions" while the new paragraph explains that the
figure charges the 30 exception loci at the full 11 bytes each. The figure
excludes the exception section's own cost, not the loci in it, so verify made
the phrase say so.

The three `# partial:` declarations in `spec/full-bundle.md` are correct as
code-review left them. Measured on this checkout, `pangopup-build build`
prints fourteen fields. All four invocations print the same fourteen with the
same count values: the plain build, the gzip-reference build, the second
destination and the already-present rebuild, whose only difference is
`"status":"already_present"`. Each declaration names three of the fourteen, so
eleven counts are left out, and all eleven are pinned in full by the strict
pin above them. "Eleven counts" is the right number and "counts" is the right
word.

## Exercised

Under a private `HOME`, `XDG_CACHE_HOME`, `XDG_DATA_HOME` and `TMPDIR`, with
`CARGO_HOME` and `RUSTUP_HOME` pinned to the real ones, with the four
`PANGOPUP_*` cache variables unset.

`pangopup --version` prints `pangopup 0.5.0`. The two transcripts that gained
`"software_version":"0.5.0"` were run by hand and compared byte for byte with
the pins beside them. `pangopup lookup --bundle <fixture> --variant
GRCh38:17:7686072:G:T`, with the digest substituted the way the block
substitutes it, is identical to `spec/snv-lookup.md:18`. `pangopup lookup
--bundle tests/fixtures/snv-regression/bundle --variant GRCh38:chr10:1:A:C` is
identical to `spec/model-routing.md:104`.

A real service was started through `scripts/run-service-fixture-tests.sh`
under a hard `timeout`. It reported `20 tests matched installed_success`. The
served score item was read out of one of them: it carries
`"data_set_version":"sha256:017e2cce..."`, and the status response of the same
service carries the same value. The two corrected sentences are true of the
running product.

Every one of the nine `spec/` files carrying a JSON-object pin was run on its
own: `runtime-install.md` 10, `full-bundle.md` 15, `runtime-transport.md` 13,
`local-assets.md` 12, `snv-transport.md` 17, `snv-lookup.md` 38,
`model-routing.md` 28, `upstream-compatibility.md` 7, `reference.md` 20. All
passed and none skipped.

Each new gate was proved red by hand, with the tree confirmed clean after
every reversal. Reversing the `architecture/service.md` sentence to "No
returned score item carries `data_set_version`" was refused with `names a
score item and data_set_version in the same sentence only to deny it`, naming
the file and the section; the same reversal in `README.md` was refused the
same way. Reversing the same-strand sentence to "does not depend" was refused
with `names the dependence only to deny it`. Replacing
`architecture/index.md`'s derivation sentence with "That corpus size was read
off the built file" was refused with `states 15,030,604,105 bytes for the
complete corpus and presents it as a measurement; it is 1,366,418,555 loci
multiplied by 11`. Deleting one `# partial:` line was refused by file and
line, and weakening the repaired `spec/snv-lookup.md` pin back to `like` was
refused by file and line. Deleting the transversion sentence from
`architecture/compatibility.md` was refused with the artifact field it no
longer matches.

`make lint`, `make test`, `make spec`,
`scripts/run-service-fixture-tests.sh`, all 24 `PORTABLE_QUALIFICATION` and 3
`SHELL_QUALIFICATION` harnesses run one at a time, and `bash sdlc/scripts/lint`
pass on this candidate. `make test` took 133.65 seconds. `make spec` reports
`314 passed` and no skips. `tests/spec-record-pin-completeness.sh` reports 24
JSON-object pins in 24 spec files, 21 comparing the complete record and 3
declared partial in `spec/full-bundle.md`.

One failure was seen and is not this ticket's. An earlier `make spec` reported
`313 passed, 1 failed`: `installed_success::pinned_values_hold_across_cpu_policies_and_move_with_the_scored_inputs`
panicked on `assert!(child.wait().expect("service exit").success())` after
sending SIGTERM. It did not reproduce in 27 further runs, 12 of them with
every core held busy, and `make spec` passed three more times. The failing
test and the service it starts carry the same git blobs on `origin/main` as on
this candidate, so the change cannot explain it. Filed as
`sdlc/tickets/drafts/0106`.

Nothing under `crates/` changed. No number was re-measured and no new number
is published.
