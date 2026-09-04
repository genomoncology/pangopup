# Let a reported gene ID be submitted back

## Observation

The two scoring paths spell the same gene differently, and `--gene` accepts only one spelling.

```
precomputed: "gene":"ENSG00000010610"
model      : "gene":"ENSG00000010610.10"

$ pangopup lookup --variant GRCh38:chr12:6801303:G:GA --gene ENSG00000010610.10
{"status":"error","code":"INVALID_GENE","message":"Ensembl gene ID must be ENSG followed by 11 digits, got ENSG00000010610.10"}
```

Filtering on the unversioned stem works against both paths, so the matching is correct. Only the round trip fails: the value the tool prints is not a value the tool accepts.

## Why this matters

Copying an identifier out of one result and into the next command is the obvious next step for a first-time user, and for a script built by reading the output. Both hit an error that names a rule the output appears to break.

## Suggested direction

Accept an optional `.VERSION` suffix on `--gene` and match on the stem. Whether the two paths should agree on how they report the gene is a separate question worth deciding on its own: the versioned form carries real information, and `architecture/runtime-data.md` already records why the sources differ.
