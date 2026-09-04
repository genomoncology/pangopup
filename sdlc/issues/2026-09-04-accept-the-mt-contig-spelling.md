# The MT contig spelling is refused

`M` and `chrM` are accepted. `MT` is refused as an invalid contig spelling.

```
$ pangopup lookup --variant GRCh38:MT:100:G:A
{"code":"INVALID_VARIANT","message":"invalid contig spelling"}
$ pangopup lookup --variant GRCh38:M:100:G:A
{"code":"ASSETS_MISSING", ...}     # spelling accepted, failed later
```

## Why this matters

`MT` is how Ensembl names the mitochondrial contig and how it appears in much of
the variant data a caller already holds. Converting a file meets a refusal on one
contig out of twenty-five, over a spelling that names the same sequence.

`README.md` documents the accepted set correctly, so nothing is inconsistent
today. This is about which spellings a caller can submit without rewriting their
input first.

## Suggested direction

Accept the mitochondrial spelling and no other alias. Whether the `chr`-prefixed
form is accepted alongside it follows the existing optional-prefix pattern and
should be settled in the ticket.

Ticket: `sdlc/tickets/drafts/0010-accept-the-common-mitochondrial-contig-spelling.md`
