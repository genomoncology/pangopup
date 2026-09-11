---
flow: build
priority: 1
---
# The repository has no changelog

Five releases stand tagged — `v0.1.0` 2026-07-31, `v0.2.0` 2026-08-04, `v0.3.0`
2026-08-05, `v0.4.0` and `v0.4.1` both 2026-09-06 — and no file in this
repository tells a reader what changed between any two of them. There is no
`CHANGELOG.md`. A consumer deciding whether to upgrade reads the git log or asks.

The sixth release is about to be cut. `Cargo.toml` says 0.5.0, `v0.5.0` is not
yet tagged, and 272 commits stand between `v0.4.1` and `HEAD`, touching 216
files. One of those commits changes the shape of published output. A consumer
who upgrades without being told will find five new fields, one of them inserted
between two existing keys.

Settled: the changelog is a plain list, newest first, one section per released
version, describing what a consumer sees. It is not a commit log and does not
enumerate internal work. Tickets, records and the planning folder already carry
that.

Settled: the entry for a version states any change a consumer must act on before
it states anything else, and says plainly when a change can break a reader.

Settled: entries for `v0.1.0` through `v0.4.1` are written from what those tags
actually published. Where the history does not support a specific claim, the
entry says less rather than guessing.

Done, observably:

- `CHANGELOG.md` stands at the repository root and carries a section for every
  released version, newest first, with the release date of each tag.
- The section for the version `Cargo.toml` names describes the output-shape
  change, names the five added fields, and says which readers it breaks.
- The section for the version `Cargo.toml` names records that no published asset
  digest moved, so a consumer holding assets from the previous release re-syncs
  nothing.
- The section for the version `Cargo.toml` names records that the model cache is
  emptied once on first run after upgrade, and what the operator sees when it
  happens.
- A gate refuses a changelog whose newest section does not name the version
  `Cargo.toml` states, and refuses a released tag that no section names. The
  refusal names the version it could not find.
- The README points a reader at the changelog.
- `make lint`, `make test` and `make spec` pass.

Boundary: this ticket changes no product source under `crates/`, alters no
score, position, status, reason or provenance field, and adds no route, flag or
output. It publishes no new measurement and re-measures nothing. It does not
tag, publish or release anything, and it does not touch any workflow under
`.github/`. It does not restate the response shape, which
`architecture/compatibility.md` enumerates. It names no private consumer of this
software, and it does not say what a score is evidence for.
