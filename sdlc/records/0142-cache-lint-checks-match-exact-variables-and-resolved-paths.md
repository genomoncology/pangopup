# Cache lint checks match exact variables and resolved paths

The cache-isolation checks now compare complete static `env` unset operands. Exact names pass. Longer names, quoted fragments, comments, command arguments, dynamic operands, unsupported escapes, wildcard spellings, invalid assignment names, and nested `env` prefixes cannot stand in for them.

The download-cache durability check no longer evaluates recipe prefixes with `bash -c`. It admits a bounded static prefix and path grammar, expands only the supported Make and shell variables, and normalizes paths lexically without reading the filesystem. Absolute cache paths and repository-relative removal paths now classify dot segments, parent segments, equal paths, sibling prefixes, trailing separators, and equivalent spellings consistently. A relative `ORT_CACHE_DIR` remains explicitly unclassified because Cargo runs the dependency build script from a package root this repository does not know.

Inside-out fixtures preserve the accepted recipes and cover unsafe command substitution, backticks, process substitution, evaluation, wildcards, unknown or extended variables, quoting and escaping, macOS and simulated Linux fallback, and sentinels that remain untouched. The separate case of multiple commands on one recipe line remains in draft 0149.

Independent design review accepted the ticket. Independent code review rejected ten candidates before accepting the final implementation. The remediation rounds covered complete option operands; quote-aware tokenization; bounded variable expansion; validation of every prefix assignment and unset operand; path quote, escape, and normalization rules; Linux fallback; literal prefix identifiers; and nested `env` handling. The final candidate passed `make lint`, `make test`, `make spec`, and `git diff --check` on macOS.

Drafts 0107 and 0108 are archived with this ticket.
