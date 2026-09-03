# sdlc/scripts/

Copied into a repository at onboarding. These scripts are how *this* project
installs itself and judges its own work — edit them freely; nothing above
reads them except by name.

Worktrees, landing and teardown are **not** here. Those are mechanism, and
they live one level up in `sdlc/project/` as the five scripts the factory
calls (`tasks`, `before`, `success`, `failure`, `health`), copied verbatim
from the sdlc repo. This folder is only what is genuinely this project's.

| Script    | Contract                                             |
| --------- | ---------------------------------------------------- |
| `install` | optional project preparation, run by `project/before` in the prepared tree before `lint` and `test`. It receives the live checkout as `SDLC_REPO`; no file skips preparation |
| `lint`    | first rung of the gate ladder. Exit 0 when the code is clean |
| `test`    | second rung. Exit 0 when the tests pass              |
| `spec`    | top rung: does the work match what was asked?        |
| `health`  | optional project health extension. When executable, canonical health runs it from the registered repository root after its own checks. Write findings to stdout and return nonzero to mark the repository unfit; absent or non-executable files are skipped |
| `ratchet.mjs` | the size ceiling, read from `sdlc/ratchet.json`. Called by `lint`; quiet in a repo that has adopted no ceiling |
| `answer-ratchet.mjs` | optional exact answer budgets, read from `sdlc/answer-ratchet.json`. Called by `lint`; quiet in a repo that has adopted none |

`ratchet.mjs` is the one file here that is copied rather than written: it
applies the same rule the sealed verify gate applies — the ceiling must equal
the measured total — so lint and the gate can never disagree about the number.
The number itself lives in `sdlc/ratchet.json` and nowhere else.

`answer-ratchet.mjs` reads the optional `sdlc/answer-ratchet.json`. Its
`fixture` is a committed repository-relative directory. Each item in
`questions` supplies a reader-facing `question`, a non-empty `command` argv
array, optional `env` mapping, and exact `exit`, `stdout`, and `stderr`
budgets. The command runs directly from the fixture without a shell. Standard
output and standard error are captured separately and must each equal their
byte budget, including multibyte output. Lowering a number records a cheaper
answer; increasing one is a deliberate ratchet change.

Cheapest rung first. `project/before` runs `install`, then `lint` and `test`
as the green-main gate; the assembly's stage gates run the ladder again inside
the flight, resolving each script from `origin/main` so a flight cannot rewrite
the rule that judges it.

The queue-era `prepare`, `integrate`, `discard` and `doctor` were removed on
2026-08-18. `project/before` absorbed `prepare`, `project/success` absorbed
`integrate` and the landing, teardown moved into `success` and `failure`, and
the factory's own doctor replaced the script.
