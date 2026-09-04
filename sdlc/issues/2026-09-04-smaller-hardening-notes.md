# Held observations, and what held up under testing

Two observations from the 2026-09-04 adversarial session that are deliberately
NOT ticketed, and a record of the surface that survived the same session. Kept so
the ground is not covered twice.

The two findings from that session that did become tickets have their own issue
files: `2026-09-04-require-a-json-content-type.md` and
`2026-09-04-accept-the-mt-contig-spelling.md`. This file is not the issue for
any ticket, and no repair should delete it.

## Held: a large body gets no HTTP reply

Bodies over the 64 KiB limit answer 413 `REQUEST_TOO_LARGE` correctly up to a few
hundred kilobytes. At 5 MB the client sees a broken pipe and no response, because
the server replies and closes while the client is still writing.

Held deliberately. This is ordinary behavior for this server shape, the limit
itself works, and the fix would mean reading a body the service has already
decided to reject. Worth a documented note so an operator meeting a client-side
connection reset knows what it means. Revisit if a real caller reports it.

## Held: leading zeros are accepted in a position

`GRCh38:chr12:0000006801301:G:A` parses and returns output byte-identical to the
plain form. The cache key uses the parsed integer, so no duplicate entry is
created.

Held deliberately, and recorded so it is a decision rather than an accident. The
parser admits any ASCII digit string fitting a nonzero u32. Nothing observable is
wrong; tightening it would refuse input that currently produces correct answers.

## What was tried and held up

Recorded so the same ground is not covered twice. Variant parsing rejected every
malformed position, allele, contig, assembly, separator, and non-UTF-8 argument
tested, and separated input errors (exit 2) from state errors (exit 1).
`install.sh` rejected shell metacharacters, path traversal, relative and empty
install directories, repeated and valueless flags, and handled a path containing
spaces and a nonexistent version. Argument validation rejected every out-of-range
and repeated option tested. The sync, install, and uninstall locks refused
concurrent operations correctly and `status` reported `syncing` accurately. The
JSON body parser rejected unknown fields, duplicate keys, wrong types and
10,000-deep nesting without a stack overflow. Eight parallel processes writing
the same uncached variant produced identical output and left the SQLite cache
passing `integrity_check`; a read-only cache file and a deliberately corrupted
one both produced correct behavior.
