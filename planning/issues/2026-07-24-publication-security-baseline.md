# Public repository and publication security baseline

Status: open
Found by: 2026-07-24 adversarial project review
Priority: block the next public model, reference, executable, or container release

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
