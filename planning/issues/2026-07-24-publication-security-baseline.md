# Public repository and publication security baseline

Status: open
Found by: 2026-07-24 adversarial project review
Priority: block the next public model, compiled GRCh38 sequence index, mask,
executable, or container release

## Observation

On 2026-07-24 the public GitHub repository reported no branch protection and no
rulesets. Dependabot security updates, secret scanning, non-provider scanning,
push protection, and validity checks were disabled. CI used mutable
`actions/checkout@v4`, had no explicit least-privilege `permissions` block, and
installed `mustmatch==0.1.0` from the package index without a content hash.

Local dependency probes found no RustSec advisory in the current lockfile.
`cargo deny check advisories bans sources` passed; a complete license policy is
not configured. This is a release-governance gap, not evidence of a currently
compromised dependency.

## Required resolution

- Pin every GitHub Action to a reviewed full commit and set explicit minimum
  workflow permissions.
- Enable the repository's available secret-scanning/push-protection and
  dependency-security facilities.
- Adopt a main-branch status-check/ruleset policy compatible with the project's
  coordinator-owned direct-push lifecycle.
- Make maintenance-tool installation content-authenticated or deliver it from
  another pinned reviewed boundary.
- Before distributing executable/model/container artifacts, enforce dependency
  advisory and license policy and produce the planned SBOM/provenance evidence.

This does not block local Ticket 012 work and should not be mixed into its mask
format implementation.

## Ticket 032 implementation state

The checked-code side is prepared: CI declares read-only contents permission,
pins both actions to full commits, authenticates the exact mustmatch 0.1.0 and
cargo-deny 0.19.4 release bytes before installation, and runs the reviewed
advisory/license/source policy through `make lint`. The locked graph currently
passes. Cargo-deny 0.19.4 cannot exempt Pangopup's versionless local path edges
while denying registry wildcards for publishable workspace crates; independent
design re-review accepted preserving those builder-causal manifests and
allowing the wildcard lint, with registry/Git sources still denied except for
canonical crates.io.

The issue remains open, but Ticket 032 applied every independently writable
repository-local control on 2026-07-30. Actions now default to read-only and
cannot approve pull requests; Dependabot security updates, secret scanning,
push protection, and non-provider-pattern scanning are enabled; and two active
`main` rulesets protect history and require pull requests plus the `gate`
status, with only the reviewed contribution-policy administrator bypass.
GitHub's current repository-update schema
accepts writes for secret scanning, push protection, and non-provider patterns
but exposes validity checks only in the repository read model, not as a
repository-local write. Independent design re-review authorizes applying every
other writable control while validity checks remain disabled. That bounded
hardening is complete, but the issue and asset-publication block remain open. The
reviewed operation and rollback plan is retained in
[`planning/artifacts/032-publication-security-baseline.md`](../artifacts/032-publication-security-baseline.md).
Organization code-security configurations do expose a validity-check setting,
but the existing recommended configuration enables broader features. A custom
configuration requires `write:org`, applies asynchronously, and cannot be
rolled back by detach alone because detach retains applied repository
settings. That route needs separate authority and lifecycle review.

Release-specific dependency inventory, SBOM, provenance, controlled
stable-source upload, remote digest comparison, immutable finalization,
model-side runtime sync, and clean-machine inference remain future publication
work.
