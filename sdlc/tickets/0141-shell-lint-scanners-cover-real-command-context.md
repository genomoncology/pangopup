---
flow: build
priority: 1
deps: []
---
# Shell lint scanners cover their supported command context

## Outcome

PangoPup's shell lint gates inspect every repository shell program through one documented bounded grammar. They distinguish literal text from executable command words and substitutions. They catch the missed negative assertions, false build evidence, unscanned pipelines, and repository-wide early-reader failures recorded in drafts 0098, 0110, 0114, and 0115.

## What is true today

All current gates pass. `negative-assertion-strength.sh` only recognizes a leading `! ` shape. `built-executable-currency.sh` can credit heredoc text or an unused assignment as a build. `pipeline-match-integrity.sh` scans only `*.sh` even though executable shell programs also live under `sdlc/scripts/` and `sdlc/project/`. Two scanners pipe potentially long producer output into `head -n 1`, which can fail with SIGPIPE under `set -euo pipefail`.

## Done, observably

- Define one shared deterministic discovery rule for `negative-assertion-strength.sh`, `built-executable-currency.sh`, `pipeline-match-integrity.sh`, and their coverage gate. It reads the exact tracked path set from Git and includes every `*.sh` path plus extensionless files whose first line is one of the repository's admitted POSIX `sh` or Bash shebang forms, regardless of executable bit. It includes sourced `.sh` helpers without shebangs and excludes checked build/output paths explicitly. Exact miniature path-set fixtures cover an extensionless program, sourced helper, excluded output, non-executable shebang file, and a path containing spaces. Miniature repositories become real Git repositories before testing discovery.
- Document the supported lexical grammar. It handles comments, single and double quotes across lines, escaped newlines, command substitutions, arithmetic regions, functions, grouped commands, `case`, `if`/`elif`, `while`/`until`, `&&`/`||`, and quoted, unquoted, and tab-stripping `<<-` heredocs. Literal text inside quotes, assignments, and heredoc bodies earns no command evidence, while quoted command words and executable substitutions remain visible. Unsupported syntax that could change a relevant answer produces a named diagnostic instead of silently passing.
- Replace the negative-assertion scanner's line-prefix heuristic with that bounded command-context analysis. It detects unasserted failure for prefix and grouped negation, final negation after `&&` or `||`, and negation inside function bodies. It preserves valid condition-consuming negation in `if`, `elif`, `while`, and `until` and distinguishes nonfinal boolean-list conditions from an unasserted final status. Existing named exemptions remain explicit and closed.
- Preserve the conservative executable-use rule in the build-currency gate. Credit a builder variable only at an actual command invocation, with a recognized preceding binding that has not changed. Declarations and assignments alone earn no credit. Pin refusal for reassignment, invocation after the first built-executable use, heredoc text, and an uncalled function. Preserve the existing rule that a `target/debug` mention in an executable command argument counts as use. Enumerate the accepted static binding and invocation forms; unknown build-evidence forms fail with a diagnostic rather than claiming arbitrary shell control-flow proof.
- Apply the same lexical distinction to pipeline matching. Scan admitted shell-shebang programs as well as `.sh` files. Literal heredoc or quoted pipeline text is ignored, while pipelines inside executable substitutions are checked.
- Add one bounded repository rule for pipelines whose producer status can be replaced by an early-closing reader. Cover the reader forms named by draft 0115: `head`, `sed q`, exit-on-first-match `awk`, `grep -m`, `grep -l`, and a bare `read`. Repair every live finding or prove its producer cannot outlive the reader through a narrow checked exception. A fixture with at least 20,000 matching lines completes under `set -euo pipefail`, returns the intended answer, and leaves no partial or nondeterministic result.
- Do not hand-edit `sdlc/project/health`. Its current canonical `grep -qx` pipeline must be corrected in the owning `repos/sdlc` repository through that repository's reviewed work, then copied byte-for-byte into PangoPup through the recorded canonical update mechanism. If that dependency does not land, keep draft 0114 open and do not claim completion for the full discovery rule.
- Each old false negative and false positive has a red fixture that fails on the parent commit and passes after the change. Mutation checks reintroduce each defect independently and make the owning gate fail for the named reason. Positive controls pass before and after. Scanner output reports counted files, command contexts, supported grammar cases, and refused unsupported cases rather than claiming full shell interpretation.
- Archive drafts 0098, 0110, 0114, and 0115 with one durable completion record. Update scanner documentation and the roadmap. Independent review finds no bypass, accidental widening, platform-only utility assumption, or quadratic scan over repository text. `make lint`, `make test`, and `make spec` pass.

## Boundary

Do not change product behavior, release assets, runtime qualification, score precision, biological claims, or public APIs. Do not add a general shell interpreter or a network dependency. PangoPup changes harden repository gates only; the required canonical health correction remains owned and reviewed by `repos/sdlc`.
