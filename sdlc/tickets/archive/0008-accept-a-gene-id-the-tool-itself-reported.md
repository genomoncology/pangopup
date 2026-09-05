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

Reading an identifier out of one answer and putting it into the next request is what a person does first and what a script written against the output does always. Both the CLI and HTTP scoring route accept a gene filter, and both currently meet an error naming a rule the tool's own output appears to break.

PangoPup results use three accepted forms that identify the same stable gene: `ENSG` followed by 11 digits, that stable identifier followed by a nonzero decimal `u32` version without a leading zero, and the versioned form followed by `_PAR_Y`. The filter accepts all three and matches on the stable component. It rejects a zero or leading-zero version, a missing version, an overflowing version, `_PAR_X`, a repeated suffix, extra separators, and every other string outside those existing output grammars.

`architecture/runtime-data.md` already records why the two sources spell the
gene differently. Whether the two paths should agree on how they report it is a
separate question and is not part of this ticket: the versioned form carries
real information, and changing what is reported would change the output contract.
This ticket changes only what the filter accepts.

Issue: `sdlc/issues/2026-09-04-let-a-reported-gene-id-be-submitted-back.md`

Done, observably:

- Stable, versioned, and versioned `_PAR_Y` gene identifiers accepted by the existing result types normalize to the same stable filter identity.
- Submitting a reported form and submitting its unversioned form select the same records through both the CLI and HTTP scoring route.
- The CLI round trip takes the versioned gene emitted by the checked miniature model result and submits it through `--gene`. The HTTP route proves the equivalent request behavior. Existing renderer tests prove that output remains unchanged.
- `.0`, `.01`, a missing version, an overflowing version, `_PAR_X`, a repeated suffix, extra separators, and other malformed identifiers remain rejected. The CLI keeps exit 2 and `INVALID_GENE`. HTTP keeps status 400 and `INVALID_REQUEST`.
- Inside-out tests pin accepted normalization and malformed grammar boundaries. Observable tests pin both adapters with cases that fail before the change.
- `README.md`, `architecture/runtime-data.md`, `spec/model-routing.md`, and `spec/http-service.md` describe or prove the accepted filter forms without changing the reported gene contract.

Boundary: do not change what either scoring path reports as the gene, the output shape, the engine's stable filter identity, or which records an existing stable filter matches today. Do not add gene-symbol lookup or reinterpret `_PAR_Y` as a distinct stable filter identity. This ticket widens what the CLI and HTTP filters accept and changes nothing about what they return.
