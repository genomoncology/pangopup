---
flow: build
priority: 1
deps: []
---
# Shell lint scanners cover real command context

## Outcome

PangoPup's shell lint gates inspect every repository shell program and distinguish executable commands from comments, quoted text, assignments, and heredoc bodies. They catch the missed negative assertions, false build evidence, unscanned pipelines, and early-reader failures recorded in drafts 0098, 0110, 0114, and 0115.

## What is true today

All current gates pass. `negative-assertion-strength.sh` only recognizes a leading `! ` shape. `built-executable-currency.sh` can credit heredoc text or an unused assignment as a build. `pipeline-match-integrity.sh` scans only `*.sh` even though executable shell programs also live under `sdlc/scripts/` and `sdlc/project/`. Two scanners pipe potentially long producer output into `head -n 1`, which can fail with SIGPIPE under `set -euo pipefail`.

## Done, observably

- Define one shared, deterministic shell-source discovery rule. It includes tracked `*.sh` files, executable tracked files whose first line selects a supported shell, and sourced `*.sh` helpers without shebangs. It excludes generated output and build directories. Every shell scanner either uses that rule or proves why its narrower input set is required.
- Replace the negative-assertion scanner's line-prefix heuristic with bounded command-context analysis. It detects unasserted failure for prefix negation, grouped negation, negation after `&&` or `||`, and negation inside function bodies. It does not treat comments, single- or double-quoted text, assignments, case patterns, arithmetic, or heredoc bodies as commands. Existing named exemptions remain explicit and closed.
- Make build-currency evidence require an actual executable command invocation in command context. A variable assignment, function declaration without invocation, comment, quoted string, or heredoc body containing `scripts/require-built-commands.sh` cannot satisfy the gate. Existing real build calls remain accepted.
- Scan shell-shebang programs as well as `.sh` files for unsafe match pipelines. The existing `sdlc/project/health` pipeline is either made safe or rejected by the gate. Source-only helpers remain covered where another shell program can execute them.
- Remove early-closing `head -n 1` pipelines from scanners that read unbounded producer output. A fixture with at least 20,000 matching lines completes under `set -euo pipefail`, returns the intended answer, and leaves no partial or nondeterministic result.
- Each old false negative and false positive has a red fixture that fails on the parent commit and passes after the change. Mutation checks reintroduce each defect independently and make the owning gate fail for the named reason. Scanner output reports counted files and command contexts rather than claiming source coverage from filename counts alone.
- Archive drafts 0098, 0110, 0114, and 0115 with one durable completion record. Update scanner documentation and the roadmap. Independent review finds no bypass, accidental widening, platform-only utility assumption, or quadratic scan over repository text. `make lint`, `make test`, and `make spec` pass.

## Boundary

Do not change product behavior, release assets, runtime qualification, score precision, biological claims, or public APIs. Do not add a general shell interpreter or a network dependency. This ticket hardens repository gates only.
