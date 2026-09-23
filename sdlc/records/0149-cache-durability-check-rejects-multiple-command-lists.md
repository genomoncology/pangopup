# Cache durability check rejects multiple command lists

The downloaded-library cache check now refuses an unquoted, unescaped shell command separator on a cache-deciding Make recipe line. It recognizes semicolons, ampersands, and pipes, including `&&` and `||`, before reading the static environment prefix. The refusal names the Makefile and physical line number. Quoted and escaped separators remain ordinary argument text.

The focused fixture puts each separator before a later `env ORT_CACHE_DIR=... cargo build` command. The former checker accepted the semicolon case. The fixture also accepts quoted and escaped separators in arguments. The checker still does not execute recipes or parse command lists. Exact cache-variable matching, lexical path normalization, and the relative `ORT_CACHE_DIR` exception retain the Ticket 0142 rules.

Ticket 0149 is archived. The focused durability check, adjacent recipe cache coverage, Bash syntax check, `git diff --check`, `make lint`, `make test`, and `make spec` passed on macOS. The specification gate reported 207 passed. The macOS test gate excludes `pangopup-build` by repository policy.
