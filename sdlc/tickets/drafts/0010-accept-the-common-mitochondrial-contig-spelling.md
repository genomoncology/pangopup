---
flow: build
priority: 3
---
# The mitochondrial contig is accepted by the spelling callers hold

`M` and `chrM` are accepted. `MT` is refused as an invalid contig spelling.

`MT` is how Ensembl names the mitochondrial contig, and how it appears in much
of the variant data a caller will already be holding. A caller converting a file
meets a refusal on one contig out of twenty-five, for a spelling difference that
names the same sequence.

`README.md` documents the accepted set correctly, so nothing is inconsistent
today. This is about which spellings a caller can submit without rewriting their
input first.

The accepted set otherwise stays closed. This ticket adds `MT` and its `chr`
form and no other alias. Accepting both follows the optional-prefix pattern
every other contig already uses. `chrMT` is the uncommon half of that pair and
is accepted for consistency rather than because callers ask for it.

Every user-input adapter that already delegates to the shared contig parser accepts both aliases, including the `pangopup-build reference window` maintenance command. A separate rejection rule in that command would create two public alias policies. The accepted cost is a slightly wider maintenance-command interface. Source ingestion remains stricter because stored source identities require canonical contig names.

Issue: `sdlc/issues/2026-09-04-accept-the-mt-contig-spelling.md`

Done, observably:

- A variant submitted as `MT` is answered exactly as the same variant submitted
  with the spelling accepted today.
- A variant submitted as `chrMT` is answered the same way.
- Results report the contig exactly as they do today, whichever spelling was
  submitted: all four accepted mitochondrial inputs normalize to `chrM`.
- Every contig spelling refused today, other than `MT` and `chrMT`, is still
  refused.
- `README.md` and `spec/snv-lookup.md` document and prove the accepted spellings through the CLI. HTTP route tests prove that the shared parser accepts the same aliases and emits canonical `chrM` output.
- `spec/reference.md` proves the maintenance reference-window adapter accepts both aliases and emits the same bytes as `chrM`.
- Source-ingestion tests prove that source rows still reject `MT` and `chrMT` before the shared caller adapter runs.
- Core parser tests keep every previously refused spelling refused except `MT` and `chrMT`.

Boundary: do not change canonical source-data or stored contig names, how any contig is reported, the accession forms already accepted, the assembly check, or any other part of variant parsing. Do not add aliases for the other contigs. Do not change lookup, scoring, or the HTTP surface beyond what accepting the spelling requires.
