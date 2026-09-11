---
---
# The inheritance check reads a mention and never reads order

`tests/shell-spawn-cache-isolation.sh` covers `scripts/smoke-linux-release.sh`,
which runs an executable handed to it and names none, by requiring every shell
file that names that script to be one of the harnesses the scan accepted. That
rule is weaker than the sentence it stands for, in two directions.

It never reads order. `inheritors_hold` asks whether the caller takes a cache
home anywhere in the file, while `examine` compares line numbers for a named
run. Measured in this checkout on 2026-09-10:
`tests/executable-delivery.sh` hands the smoke script an executable at lines 47
and 71 and takes its cache home at line 83. Those two calls pass a stub the
harness writes itself and an explicit `SMOKE_CACHE`, so nothing is reached
today, and the one call that hands the real executable is at line 102, after
the cache home. The check would read the same either way.

It reads a mention rather than a hand-off. The scan is `grep -lF` over whole
files, so a path named in a comment is a caller. The gate exempts itself by
name for exactly this reason: it names the script in a variable assignment and
would otherwise refuse itself.

Separating a stub from the shipped executable means reading what each call
passes, which is the argument-shape form ticket 0071 refuses to take, so the
repair is not obvious and is not a matter of tightening a pattern.

Done, observably:

- A file that hands an executable to the smoke script before taking a cache
  home is refused, or the rule states in one sentence what it does hold and a
  check proves that sentence.
- A file that only mentions the smoke script's path in commentary is not read
  as a caller.

Boundary: no product behaviour, cache location, or default changes, and the
three harnesses keep the source points they have.
