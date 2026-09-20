---
---
# The quiet-grep scan reads only files named `.sh`

`tests/pipeline-match-integrity.sh` collects the files it reads with

    find "$repository" -type f -name '*.sh' -not -path '*/target/*'

Fifty-six files answer to that. Eight more in this checkout are bash and are
not named `.sh`: `sdlc/scripts/install`, `sdlc/scripts/spec`,
`sdlc/scripts/test`, `sdlc/scripts/lint`, and `sdlc/project/success`,
`sdlc/project/failure`, `sdlc/project/before`, `sdlc/project/health`. Each
opens with a `bash` shebang and the four under `sdlc/project/` run under
`set -euo pipefail`.

One of them holds the shape the scan exists to refuse. `sdlc/project/health`
line 154:

    if printf '%s\n' $legacy_issue_numbers | grep -qx "$number"; then

`grep -qx` stops at its first match and closes the pipe, `printf` can still be
writing, and `pipefail` reports 141 instead of the match. The writer is short,
so the race is narrow -- but it is the same race, and a longer list makes it
wider. Nothing reads that file for this shape today, and nothing would read a
ninth file added beside it.

Ticket 0113 asked for the shape to be refused "wherever it stands in this
repository". It is refused wherever it stands in a file named `.sh`.

Done, observably: every shell file in the checkout is read for this shape,
whatever it is named, and the count of files read is held to what the tree
holds so a file dropping out of the set is refused. A file is a shell file
when its first line names a shell.
