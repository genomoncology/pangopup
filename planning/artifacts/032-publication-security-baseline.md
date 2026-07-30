# Ticket 032 publication-security operation plan

Captured: 2026-07-30

This is a sanitized, reviewable plan for repository administration. It contains
no token, credential, authorization header, cookie, secret value, or release
operation. The developer performed only read requests. The coordinator is the
only actor authorized to perform the mutations below, and only after code
review, commit/push, and a green `gate` run for that exact commit.

All REST requests and verification reads use:

```text
Accept: application/vnd.github+json
X-GitHub-Api-Version: 2026-03-10
repository: genomoncology/pangopup
```

The payload shapes were checked against GitHub's official 2026-03-10 OpenAPI
description at commit
`5e28810649ba41b5483753ba74f976f83856a504`. Target-repository acceptance
remains deliberately unproven until the coordinator's authorized external
effect; GitHub exposes no read-only create-ruleset validation endpoint.

## Sanitized captured pre-state

Read-only API observations:

| Surface | Endpoint | Captured value |
|---|---|---|
| Repository | `GET /repos/genomoncology/pangopup` | public; default branch `main` |
| Actions defaults | `GET /repos/genomoncology/pangopup/actions/permissions/workflow` | `default_workflow_permissions=write`; `can_approve_pull_request_reviews=true` |
| Vulnerability alerts | `GET /repos/genomoncology/pangopup/vulnerability-alerts` | HTTP 204, enabled |
| Dependabot security updates | `GET /repos/genomoncology/pangopup/automated-security-fixes` | `enabled=false`; `paused=false` |
| Secret scanning | repository `security_and_analysis` | `disabled` |
| Push protection | repository `security_and_analysis` | `disabled` |
| Non-provider patterns | repository `security_and_analysis` | `disabled` |
| Validity checks | repository `security_and_analysis` | `disabled` |
| Attached code-security configuration | `GET /repos/genomoncology/pangopup/code-security-configuration` | HTTP 204, none |
| Repository rulesets | `GET /repos/genomoncology/pangopup/rulesets` | `[]` |
| Main branch protection | `GET /repos/genomoncology/pangopup/branches/main/protection` | HTTP 404, not protected |

The coordinator must repeat every read above immediately before mutation and
compare the semantic values, not volatile response headers. Any difference is
drift: abort without mutation and return the plan for review.

## Known residual API limitation

The 2026-03-10 `PATCH /repos/{owner}/{repo}` schema accepts repository-level
writes for `secret_scanning`, `secret_scanning_push_protection`, and
`secret_scanning_non_provider_patterns`. It does **not** accept
`security_and_analysis.secret_scanning_validity_checks`, even though the
repository read returns that field. The repository also has no attached
code-security configuration that could establish a bounded repository-local
alternative.

GitHub does expose validity checks through organization code-security
configurations, and an organization configuration can be attached to selected
repository IDs. The existing GitHub-recommended configuration enables
additional out-of-scope features. Creating a custom configuration would expand
authority beyond this repository, requires `write:org` rather than the
inspected credential's `read:org`, applies asynchronously, and has
non-equivalent rollback because detaching retains repository settings. That is
a real REST route, but not a reviewed or authorized Ticket 032 fallback. It
requires a separate design of authority, exact feature scope, asynchronous
verification, and rollback.

The independently reviewed design revision authorizes the other writable,
independent protections rather than withholding them. The coordinator may
execute the exact sequence below after all lifecycle prerequisites. Validity
checks are not mutated: they are expected to remain `disabled`, the
publication-security issue remains open, and asset publication remains
blocked. No undocumented field is guessed here.

## Exact intended mutation order and payloads

### 1. Actions defaults

```text
PUT /repos/genomoncology/pangopup/actions/permissions/workflow
```

```json
{
  "default_workflow_permissions": "read",
  "can_approve_pull_request_reviews": false
}
```

Expected response: HTTP 204. Verify by repeating the GET and comparing both
fields.

### 2. Dependabot security updates

```text
PUT /repos/genomoncology/pangopup/automated-security-fixes
```

No request body.

Expected response: HTTP 204. Verify the GET reports `enabled=true`.

### 3. Repository-level secret-scanning controls

This payload covers only the three controls accepted by the current official
repository-update schema:

```text
PATCH /repos/genomoncology/pangopup
```

```json
{
  "security_and_analysis": {
    "secret_scanning": {
      "status": "enabled"
    },
    "secret_scanning_push_protection": {
      "status": "enabled"
    },
    "secret_scanning_non_provider_patterns": {
      "status": "enabled"
    }
  }
}
```

Expected response: HTTP 200. Verify all three fields are `enabled` with
`GET /repos/genomoncology/pangopup`. Verify validity checks still report the
captured `disabled` state, record that residual limitation, and continue to
step 4.

### 4. Unbypassed main-history ruleset

```text
POST /repos/genomoncology/pangopup/rulesets
```

```json
{
  "name": "pangopup-main-history",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": {
    "ref_name": {
      "include": [
        "refs/heads/main"
      ],
      "exclude": []
    }
  },
  "rules": [
    {
      "type": "deletion"
    },
    {
      "type": "non_fast_forward"
    }
  ]
}
```

Expected response: HTTP 201. Capture the returned opaque ruleset ID as
`HISTORY_RULESET_ID`; never derive or guess it. Verify the named active ruleset
targets only `refs/heads/main`, contains exactly deletion and non-fast-forward
rules, and has an empty bypass array.

### 5. Administrator-bypassed contribution ruleset

```text
POST /repos/genomoncology/pangopup/rulesets
```

```json
{
  "name": "pangopup-main-contributions",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [
    {
      "actor_type": "RepositoryRole",
      "actor_id": 5,
      "bypass_mode": "always"
    }
  ],
  "conditions": {
    "ref_name": {
      "include": [
        "refs/heads/main"
      ],
      "exclude": []
    }
  },
  "rules": [
    {
      "type": "pull_request",
      "parameters": {
        "dismiss_stale_reviews_on_push": false,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_approving_review_count": 0,
        "required_review_thread_resolution": false
      }
    },
    {
      "type": "required_status_checks",
      "parameters": {
        "do_not_enforce_on_create": false,
        "required_status_checks": [
          {
            "context": "gate"
          }
        ],
        "strict_required_status_checks_policy": false
      }
    }
  ]
}
```

Expected response: HTTP 201. Capture the returned opaque ruleset ID as
`CONTRIBUTIONS_RULESET_ID`; never derive or guess it. Verify the active ruleset
targets only `refs/heads/main`, has exactly the pull-request and required-status
rules above, requires only context `gate`, and has exactly this bypass tuple:

```text
actor_type=RepositoryRole
actor_id=5
bypass_mode=always
```

There is no user, team, deploy-key, integration, or organization-admin bypass.

## Expected Ticket 032 after-state

This ticket's bounded external effect requires all of the following at once:

- Actions default workflow permissions are `read` and Actions cannot approve
  pull requests.
- Dependabot security updates report enabled; vulnerability alerts remain
  enabled.
- Secret scanning, push protection, and non-provider patterns report enabled.
  Validity checks remain disabled because no reviewed repository-local write
  exists.
- Exactly one active repository ruleset named `pangopup-main-history` targets
  `refs/heads/main`, forbids deletion and non-fast-forward updates, and has no
  bypass actor.
- Exactly one active repository ruleset named
  `pangopup-main-contributions` targets `refs/heads/main`, requires pull
  requests and status context `gate`, and has only the reviewed
  `RepositoryRole/5/always` bypass.
- No release, release asset, immutable-release setting, branch ref, workflow
  run, or other repository setting changed as part of this operation.

This after-state completes only the independently writable controls. It does
not close the publication-security issue or make any asset publication-ready.
The issue remains narrowed to the validity-check API limitation plus the
release-specific inventory, SBOM, provenance, and publication evidence named
in the ticket.

After creation, record the two observed opaque ruleset IDs in this artifact and
append sanitized GET responses reduced to the fields above. Do not record
headers or credentials.

The coordinator must then make an ordinary reviewed fast-forward evidence
commit through the administrator bypass and confirm its `gate` run is green.
Deletion and force push are verified from the unbypassed rule configuration;
they are never exercised destructively.

## Per-operation rollback

Rollback applies after any failed mutation or failed verification. Walk only
the successful operations in exact reverse order. Stop and report if rollback
itself fails, but continue safe read-only verification.

| Reverse order | Operation to undo | Exact rollback | Verification |
|---:|---|---|---|
| 1 | Contribution ruleset created by this ticket | `DELETE /repos/genomoncology/pangopup/rulesets/{CONTRIBUTIONS_RULESET_ID}` using only the ID returned by step 5 | GET that exact ID is 404 and the list contains no `pangopup-main-contributions` ruleset |
| 2 | History ruleset created by this ticket | `DELETE /repos/genomoncology/pangopup/rulesets/{HISTORY_RULESET_ID}` using only the ID returned by step 4 | GET that exact ID is 404 and the list contains no `pangopup-main-history` ruleset |
| 3 | Three supported secret controls enabled | `PATCH /repos/genomoncology/pangopup` with the rollback JSON below | Repository GET shows all three `disabled`; validity checks must equal their captured value |
| 4 | Dependabot security updates enabled | `DELETE /repos/genomoncology/pangopup/automated-security-fixes` | GET reports `enabled=false` and `paused=false` |
| 5 | Actions defaults hardened | `PUT /repos/genomoncology/pangopup/actions/permissions/workflow` with the captured rollback JSON below | GET reports `write` and PR approval `true` |

Secret-control rollback payload:

```json
{
  "security_and_analysis": {
    "secret_scanning": {
      "status": "disabled"
    },
    "secret_scanning_push_protection": {
      "status": "disabled"
    },
    "secret_scanning_non_provider_patterns": {
      "status": "disabled"
    }
  }
}
```

Actions rollback payload:

```json
{
  "default_workflow_permissions": "write",
  "can_approve_pull_request_reviews": true
}
```

Delete no pre-existing ruleset and never select rollback targets by name alone.
The captured pre-state had no rulesets, so only IDs returned by this ticket's
successful POST responses are eligible. After rollback, repeat the complete
pre-state read set and prove semantic equality before reporting failure.

## Observed after-state

Pending coordinator external effect. No repository setting, rule, release, or
asset was mutated during development. The future observed record must show the
three writable secret-scanning controls enabled and validity checks still
disabled; it must not describe this bounded result as publication readiness.
