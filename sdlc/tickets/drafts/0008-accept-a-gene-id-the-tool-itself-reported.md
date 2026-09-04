---
flow: build
priority: 4
---
# A gene ID the tool reported can be submitted back to it

The value the tool prints is not a value the tool accepts.

A scored result from the model path reports its gene as `ENSG00000010610.10`.
Passing that back is refused:

```
$ pangopup lookup --variant GRCh38:chr12:6801303:G:GA --gene ENSG00000010610.10
{"code":"INVALID_GENE","message":"Ensembl gene ID must be ENSG followed by 11 digits, got ENSG00000010610.10"}
```

The precomputed path reports the same gene as `ENSG00000010610`, which is
accepted, and filtering on that form correctly matches results from both paths.
The matching is right. Only the round trip fails.

Reading an identifier out of one answer and putting it into the next command is
what a person does first and what a script written against the output does
always. Both meet an error naming a rule the tool's own output appears to break.

`architecture/runtime-data.md` already records why the two sources spell the
gene differently. Whether the two paths should agree on how they report it is a
separate question and is not part of this ticket: the versioned form carries
real information, and changing what is reported would change the output contract.
This ticket changes only what the filter accepts.

Issue: `sdlc/issues/2026-09-04-let-a-reported-gene-id-be-submitted-back.md`

Done, observably:

- A gene identifier taken from any result this tool produces is accepted by the
  gene filter.
- Submitting the reported form and submitting the unversioned form select the
  same records.
- A gene identifier that is genuinely malformed is still refused, and says so.
- The suite pins the round trip with a case that fails before the change.

Boundary: do not change what either scoring path reports as the gene, the output
shape, or which records a filter matches today. Do not change the HTTP surface,
which exposes no gene filter. This ticket widens what the filter accepts and
changes nothing about what it returns.
