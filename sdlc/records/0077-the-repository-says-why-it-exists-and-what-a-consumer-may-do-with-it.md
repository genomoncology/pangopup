---
base: bc428d362e6d22b89d4c02fe927c8792e3e942ee
head: db636c6d7b6cc610745b65af878651243a466493
---
# The repository says why it exists and what a consumer may do with it

`architecture/motivation.md` is new. It carries what each outside party says
about its own software or service as a dated quotation attributed to that
party, states which licence this repository is under, names the three ways
software reaches this project, and quotes the three passages of this
repository's own LICENSE that bear on a caller's question. It draws no
conclusion. `architecture/README.md` opens with how the system is arranged and
keeps the build history below, under a heading of its own. `README.md` carries
one link to the new page from the section that already states this project's
licence.

## What verify changed

The shipped page named its own section "What calling this software means for
the caller" and then declined to answer, sending the reader to a 674-line
licence with no indication of which part bears. The ticket asks for two things
together: no legal conclusion, and a reader who learns what calling this
software means for the caller's own licence. Only the first held.

Three passages of `LICENSE` now stand quoted, each with the section that
carries it and an attributing verb, so the licence speaks and this repository
does not: section 0 on what a covered work is and what "based on" means,
section 2 on the output of a run, and section 5 on what an aggregate is. Each
quotation was checked character by character against `LICENSE`. The lead-in to
each names the subject of the passage rather than one of the three
arrangements, so nothing is paired by adjacency. Which passage governs which
arrangement is left to the reader, and the two denials that follow are each a
sentence of their own.

Two sentences were repaired. The opening said the public service "answers for
it answers a few queries at a time", and called the predictor best-known in one
sentence and better-known nine lines later. The README link sentence said "the
public lookup service limits", which reads as a verb until the eye reaches the
end.

## The facts, audited again from the sources

Every dated claim in `architecture/motivation.md` was checked against the
retained captures of 2026-08-28, with no URL fetched. The three quotations from
the SpliceAI LICENSE file and README, the GitHub licence classification and the
2026-04-20 archive date, the lookup service's stated per-minute limit, its
batch guidance, its statement about Illumina's precomputed tables and GENCODE,
its maintainer, and both paper records including PMID 35449021 and
DOI 10.1186/s13059-022-02664-4 all match. The page does not repeat the
out-of-date GPLv3 line that the service's own page still carries;
`tests/repository-sourcing.sh` refuses it by name.

The NOTICE claim holds. `NOTICE:16` records the Pangolin project as
`License: GNU General Public License version 3`, which is what the page says
and all it says.

Every claim in the new `architecture/README.md` opening was checked against
`crates/`. The 11-byte record is `encode_fixed_locus` in
`crates/pangopup-index/src/snv.rs`, packing a three-bit reference base and
three 28-bit alternate scores, one record to one position of one gene.
Lookup-first is `LookupFirstRouter::inspect`; the CPU model route is
`ModelFallback::complete` over `ort` with no GPU feature; the SQLite cache is
`pangopup-cache`; `lookup` and `serve` are the two query paths; and `sync`,
`status` and `uninstall` are subcommands of the same binary. Nothing in that
opening is overstated.

No sentence in either new page crosses the standing ruling. Neither file
contains any of the eight interpretation terms the gate refuses, and neither
says what a score is evidence for.

## The refusals, proved by hand

Each mutation was applied to `architecture/motivation.md`,
`tests/repository-sourcing.sh` was run, and the file was restored from a copy
taken beforehand rather than by `git checkout`.

| mutation | exit | what the refusal named |
| --- | --- | --- |
| opening denial reversed | 1 | `names legal advice only to claim it`, quoting the sentence |
| closing denial reversed | 1 | `names legal advice only to claim it`, quoting the sentence |
| capture date dropped from the lookup-service unit | 1 | `cites a source without the date it was read`, quoting the unit |
| the Pangolin paper unit removed | 1 | `carries no single unit stating the upstream model paper`, naming the DOI |

Draft 0118 records that a denial can be satisfied by an unrelated negation
earlier in the same sentence. The shipped prose does not rely on that hole. The
first mutation above reversed the denial in a sentence whose neighbour carries
`draws no conclusion of its own`, and the gate still refused it, because each
denial is a sentence of its own.

## The README trade

Exact. `wc -l` 271, `wc -w` 1,829, the figures in `spec/readme-first-use.md`
stating those numbers, and the presentation section at md5
`b857365241c863dbfd2f0c6929acad4e`. The verify repair changed one word to its
possessive and moved neither figure.

## The ladder

`make lint` 4 s, exit 0. `make test` 169 s, exit 0. `make spec` 23 s,
`316 passed`. `scripts/run-service-fixture-tests.sh` 5 s, exit 0.
`bash sdlc/scripts/lint` 5 s, exit 0. All 31 `PORTABLE_QUALIFICATION` and 3
`SHELL_QUALIFICATION` gates were run one at a time: 34 of 34 exit 0.
`tests/production-release-qualification.sh` was not blocked on port 18080; it
ran in 11 s and passed. `tests/repository-sourcing.sh` costs 3 s.
`tests/shell-spawn-cache-isolation.sh` was run 20 times on its own: 0 failures.

Run with `CARGO_HOME` and `RUSTUP_HOME` at the operator's own, `HOME` moved to a
fresh private root, the four `PANGOPUP_*` names unset, and `ORT_CACHE_DIR` never
exported. `~/.cache/pangopup/model-results.sqlite3` is byte-identical after all
of it: md5 `0ce402e99f91b4d16ed3aaddc88160fc`, 303,104 bytes, mtime unchanged.
`~/.cache/ort.pyke.io` holds 288,252,072 bytes, the figure it held beforehand.
No `pangopup` or `pangopup-build` process is left running.

Verify filed no draft. Drafts 0117 and 0118 came from code review and stand.
