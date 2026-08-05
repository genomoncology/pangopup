# 058 — Explain the prediction and add a CLI-only quick start

Status: ready

## Why

The README explains installation, synchronization, and CLI scoring, but it
does not plainly explain what the reported gain/loss values predict, and a new
CLI user must currently assemble first-use steps from separate sections. Add a
short scientific explanation and one copyable path near the top before
storage, server, and Docker details.

## Scope

- Use the exact top-level order `Introduction` → `What it predicts` →
  `Quick start: CLI` → `Storage and memory`.
- Add a compact `What it predicts` section before the quick start. In plain
  English, explain that Pangolin predicts splice-site usage/strength from DNA
  sequence and that PangoPup reports the predicted change caused by a variant:
  gain/loss scores and the relative positions of the strongest changes within
  its 50-nucleotide scoring window, with separate records for overlapping
  genes where applicable.
- State the direction and coordinate convention exactly: gain is an increase
  and signed loss is a decrease in predicted splice-site strength/usage. A
  reported position is a genomic-coordinate offset from the submitted variant;
  positive means a higher genomic coordinate even for minus-strand genes. It
  is not a transcript-oriented distance to an exon boundary.
- Cite Tony Zeng and Yang I. Li's Pangolin paper directly. Prefer paraphrase;
  if a direct quote is useful, keep it short and visibly attributed rather
  than reproducing the supplied abstract paragraph.
- Explicitly distinguish the score from a pathogenicity classification,
  clinical diagnosis, or prediction of the exact RNA transcript/protein
  consequence. Do not claim that PangoPup exposes tissue-specific scores when
  its public result contract does not.
- Show the immutable v0.3.0 Linux installer, `PATH` update, first asset sync,
  ready status, one precomputed SNV lookup, and one automatic model-fallback
  non-SNV lookup.
- Briefly state the Linux x86-64/GLIBC prerequisite, first-sync disk/download
  cost, network-free behavior after sync, default JSON Lines output, and how
  `provenance.kind` identifies the selected route.
- Link readers to the existing detailed Linux installation and CLI sections.
- Keep this path CLI-only: no HTTP, Docker, source build, custom data paths,
  forced-model mode, table formatting, or uninstall instructions in the quick
  start. Existing detailed sections remain authoritative for those topics.
- Add or extend a static offline README contract test that extracts only the
  text bounded by `## Quick start: CLI` and `## Storage and memory`. Within
  that bounded section, require the installer, `PATH`, sync, status, both
  lookup examples, JSON Lines/provenance explanation, and absence of HTTP or
  Docker commands. Separately bound and check `## What it predicts` so required
  score meaning, limitations, and paper link cannot be satisfied accidentally
  by the existing prior-art or later CLI sections.

## Success Checklist

- A Linux CLI user can copy the section in order and reach both lookup and
  model-backed output without consulting another section.
- A reader can understand what gain/loss scores and positions mean, and what
  the result does not establish clinically, before installing the software.
- The scientific description links Tony Zeng and Yang I. Li's paper at
  `https://doi.org/10.1186/s13059-022-02664-4` and does not overstate
  PangoPup's public output contract.
- Commands use public v0.3.0 syntax and known supported GRCh38 examples.
- The section accurately warns that initial sync downloads about 2.44 GiB,
  installs about 14.76 GiB, and needs at least 25 GB free.
- The quick start appears before `Storage and memory`; HTTP and Docker commands
  do not appear inside it.
- An offline test checks the stable command/content and ordering contract.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **CLI-only first path.** The user explicitly asked for CLI only, so service
   and Docker setup remain in their existing sections.
2. **Show automatic routing.** Use one known SNV and one known insertion so the
   two ordinary paths are visible without introducing `--model-only` in the
   first-use flow.
3. **Repeat only essential facts.** The quick start repeats the immutable
   installer and minimum capacity warning because they affect successful first
   use; detailed explanations remain below to keep the top compact.
4. **Describe the public result, not the training headline.** The Pangolin
   paper describes a multi-tissue model. PangoPup's public score records expose
   variant-level gain/loss and positions, not a tissue-by-tissue result or a
   clinical classification, so the top-level explanation stays within that
   observable contract.

## Dependencies

Public PangoPup v0.3.0 and the existing README installation/CLI contracts.

## Notes

- Known SNV: `GRCh38:chr12:6801301:G:A`.
- Known insertion/model fallback: `GRCh38:chr12:6801303:G:GA`.
- The installer does not install assets; `pangopup sync --progress` does.
- Do not claim cold-cache latency or repeat the detailed resource table.
- Paper: Tony Zeng and Yang I. Li, “Predicting RNA splicing from DNA sequence
  using Pangolin,” Genome Biology 23, 103 (2022), DOI
  `10.1186/s13059-022-02664-4`. Ian supplied the abstract language as context;
  do not reproduce the full paragraph.

## Coordinator Authorship

Coordinator: Codex (`/root`), 2026-08-05.

## Independent Ticket Review

Reviewer: Codex subagent `/root/ticket058_quickstart_design_review`, 2026-08-05.

Initial verdict: REJECT. The original acceptance test could have searched the
whole README and passed using commands already present in later sections.

Resolution: require the test to extract and validate the exact quick-start
section between its heading and `Storage and memory`, including negative HTTP/
Docker assertions, and separately bound the scientific explanation. The
materially expanded prediction-description scope is returned to the same
reviewer with this correction.

Second verdict: REJECT. The revised ticket contradicted its section order and
did not define the score-position coordinate convention precisely enough.

Second resolution: fix the exact top-level order and require the README to
define gain, signed loss, and genomic offsets—including minus-strand behavior
and the non-transcript-oriented boundary. Returned to the same reviewer.

Final verdict: ACCEPT. The ticket now fixes the section order, observable score
semantics and limitations, direct paper attribution, compact CLI-only flow,
and section-bounded tests. No material design finding remains.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
