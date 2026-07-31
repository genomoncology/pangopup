# Ticket 038 public Linux release operation and evidence

State: **PREPUBLICATION — two package attempts stopped before artifact upload;
no Actions artifact, tag, release, or executable release asset has been
created.**

This is a credential-free operation record. Never paste tokens, authentication
headers, raw `gh auth` output, environment dumps, or signed URL query strings
into this file.

## Preserved stopped-run evidence

The original Ubuntu 22.04/GLIBC 2.35 attempt targeted preparation commit
`a82968cfc9c29c4e95f647ecaac7452e6ef78da2`. Its exact remote `ci` run
`30648083892` passed, but package run `30648307402` failed while linking the
selected static ONNX Runtime archive. The archive imports C23 conversion
symbols that GLIBC 2.35 does not provide. The run produced no Actions artifact,
draft, tag, release, or uploaded release asset and was not retried.

The corrected-baseline attempt targeted commit
`dc9cf1ccb3b0313042bed333463758da24feb184`; exact remote `ci` run
`30649639343` passed. Package run `30649914623` passed the full gate, build,
deterministic SBOM, exact-six preparation, and GLIBC/dependency/inventory
qualification. It then stopped in the clean-container smoke because the outer
shell removed the quotation marks from the intended JSON `grep` pattern.
Actions artifact upload was skipped, no release state changed, and the run was
not retried.

Those two dispatch authorizations are consumed. After independent acceptance
of the executed shared-script container smoke regression and a new exact
commit's green remote gate, exactly one smoke-corrected dispatch is permitted.
It must use the
same Ubuntu 24.04/GLIBC 2.39 baseline and must not alter the static runtime,
release inventory, or semantic gates.

## Fixed release contract

- repository: `genomoncology/pangopup`
- tag: `v0.1.0`
- title: `Pangopup v0.1.0`
- version: `0.1.0`
- target commit: `<PENDING_40_LOWERCASE_COMMIT>`
- release body: `planning/artifacts/038-release-notes.md`, byte-for-byte
- build workflow: `.github/workflows/package-linux.yml`
- container smoke: `scripts/smoke-linux-release.sh`, invoked with the exact
  executable, source tree, absent data directory, and model-cache paths
- build runner: GitHub-hosted Ubuntu 24.04
- admitted maximum imported GLIBC version: `2.39`
- supported runtime: Linux x86_64/amd64 with GLIBC 2.39 or newer
- pinned qualification container:
  `ubuntu@sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90`
- upload attempts permitted: one per member, without replacement

The admitted release inventory is exactly:

```text
LICENSE
NOTICE
pangopup-linux-x86_64
pangopup-linux-x86_64.cdx.json
pangopup-linux-x86_64.sha256
release-manifest.json
```

## Fresh stop-before-effect audit

Record pass/fail and non-sensitive evidence for every item before dispatching
the smoke-corrected workflow. Any failure stops the operation. Earlier audits
belong to their consumed attempts and cannot authorize this dispatch.

- [ ] repository visibility is public
- [ ] immutable releases are enabled
- [ ] default Actions token is read-only
- [ ] Actions pull-request approval is disabled
- [ ] Dependabot security updates are enabled
- [ ] secret scanning, push protection, and non-provider-pattern controls are enabled
- [ ] open secret alert count is zero
- [ ] both reviewed `main` rulesets are active and unchanged
- [ ] Cargo/workspace version is exactly `0.1.0`
- [ ] neither tag nor release `v0.1.0` exists
- [ ] target commit is reachable from public `main`
- [ ] target commit's `ci` run has exactly one successful job named `gate`

Smoke-corrected audit timestamp: `<PENDING_UTC>`
Smoke-corrected audit operator: `<PENDING>`
Target `ci` run ID/URL: `<PENDING>`  
Security/ruleset evidence summary: `<PENDING_CREDENTIAL_FREE_SUMMARY>`

## Exact-commit build and local admission

Only the coordinator performs these effects, using the authenticated official
`gh` executable. `COMMIT` must equal the fixed target above.

```bash
REPO=genomoncology/pangopup
COMMIT=<40_LOWERCASE_COMMIT>
gh workflow run package-linux.yml --repo "$REPO" --ref main -f commit="$COMMIT"
```

Record the single resulting run, require its head SHA and workflow input to be
`$COMMIT`, wait for success, and download its artifact once into a new private
directory. Then run the shared local admission boundary:

```bash
scripts/qualify-linux-release.sh <EXACT_SIX_DIR> 0.1.0 "$COMMIT"
```

Workflow run ID/URL: `<PENDING>`  
Actions artifact ID/name: `<PENDING>`  
Download timestamp: `<PENDING_UTC>`  
Local qualifier result: `<PENDING>`

Record name, byte size, and SHA-256 for all six files:

| Name | Bytes | SHA-256 |
|---|---:|---|
| `LICENSE` | pending | pending |
| `NOTICE` | pending | pending |
| `pangopup-linux-x86_64` | pending | pending |
| `pangopup-linux-x86_64.cdx.json` | pending | pending |
| `pangopup-linux-x86_64.sha256` | pending | pending |
| `release-manifest.json` | pending | pending |

## Prepublication production qualification

In a pinned Linux x86_64 container, mount only the requalified executable,
this exact source tree, and new private XDG data/cache/output volumes. Do not
mount the host's installed Pangopup data. The container runs the shell runner;
the trusted build host then runs the checker through the repository-standard
`uv` script environment:

```bash
scripts/run-production-qualification.sh \
  <PANGOPUP_EXECUTABLE> <EXACT_SOURCE_TREE> \
  <ABSOLUTE_XDG_DATA_HOME> <ABSOLUTE_XDG_CACHE_HOME> <ABSENT_OUTPUT_DIR>
scripts/check-production-qualification.py <OUTPUT_DIR> <EXACT_SOURCE_TREE>
```

The runner performs one online sync, one offline reuse, combined status, the
seven ordered 1,000-SNV batches, and exact M09 model inference. The checker
removes only `provenance.bundle_id` from both SNV sides, compares every other
JSON value and order, and compares M09 byte-for-byte. It reads only output and
checked-in oracle files; it does not scan or hash installed production assets.
The container needs no Python runtime; the host checker requires `uv`.

The container invocation is fixed to the same Ubuntu digest used by the
packaging smoke. `QUALIFICATION_ROOT` is a new empty private host directory;
`RELEASE_DIR` is the locally requalified exact-six directory; and
`SOURCE_TREE` is the exact publication-ready checkout:

```bash
IMAGE=ubuntu@sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90
docker run --rm --read-only --tmpfs /tmp:rw,noexec,nosuid,size=64m \
  -v "$RELEASE_DIR:/release:ro" -v "$SOURCE_TREE:/source:ro" \
  -v "$QUALIFICATION_ROOT:/qualification:rw" "$IMAGE" bash -ceu '
    /source/scripts/run-production-qualification.sh \
      /release/pangopup-linux-x86_64 /source \
      /qualification/data /qualification/cache /qualification/output
  '
scripts/check-production-qualification.py \
  "$QUALIFICATION_ROOT/output" "$SOURCE_TREE"
```

Container image/digest: `<PENDING>`  
Requests TSV SHA-256: `<PENDING>`  
Canonical SNV actual SHA-256: `<PENDING>`  
Canonical SNV expected SHA-256: `<PENDING>`  
M09 oracle SHA-256: `<PENDING>`  
M09 actual SHA-256: `<PENDING>`  
Online/offline/status result: `<PENDING>`  
Qualification result: `<PENDING>`

## Draft, upload, and publication

Create one private draft targeting `$COMMIT` with the fixed title and exact
checked-in body. Upload each admitted file once without `--clobber`. After
every upload, compare the complete remote name/size/GitHub SHA-256 inventory
with the local inventory above. Record every attempt; any mismatch stops the
operation.

Before publication recheck the exact release ID, draft state, absent tag,
target, title, byte-exact body, and closed six-file inventory. Publish once,
mark it Latest, then require `draft=false`, `immutable=true`, the public tag ref
to resolve directly to `$COMMIT`, and `/releases/latest` to resolve to
`v0.1.0`.

Draft/release ID: `<PENDING>`  
Upload attempts by member: `<PENDING>`  
Published timestamp: `<PENDING_UTC>`  
Public release URL: `<PENDING>`  
Tag ref/object SHA: `<PENDING>`  
Immutable state: `<PENDING>`  
Latest endpoint result: `<PENDING>`

Before publication only, a failed draft may be deleted by its exact ID after
reauthentication if and only if the tag remains absent. Stop after deletion;
do not retry. Never delete or modify a published release or tag.

## Bounded public verification

Unauthenticated checks must verify release/tag metadata and byte-exact
downloads of the five small assets. Bind the executable through GitHub's
reported size and SHA-256 rather than downloading it again. Then use a second
fresh pinned container and isolated install directory to fetch the exact
tagged `install.sh`, install through its checksum path, reuse the already
qualified XDG volumes offline, run status, one retained SNV, and exact M09.

Public metadata result: `<PENDING>`  
Five-small-asset result: `<PENDING>`  
Executable reported size/digest result: `<PENDING>`  
Tagged installer container/result: `<PENDING>`  
Offline reuse/status result: `<PENDING>`  
Retained SNV result: `<PENDING>`  
Repeated M09 result: `<PENDING>`

## Exact syntax-checked coordinator runbook

The following is one executable Bash program, not pseudocode. Before use,
replace the three angle-bracket values with the independently reviewed commit,
its successful CI run ID, and a new absent qualification root. Run it from
that exact clean checkout. It deliberately stops for no interactive decisions.
The only release mutation client is the official `gh` executable. The normal
test gate extracts this block and passes it to `bash -n`.

<!-- BEGIN TICKET 038 COORDINATOR SCRIPT -->
```bash
set -euo pipefail
umask 077

readonly REPO=genomoncology/pangopup
readonly TAG=v0.1.0
readonly TITLE='Pangopup v0.1.0'
readonly COMMIT=REPLACE_WITH_40_LOWERCASE_PUBLICATION_READY_COMMIT
readonly CI_RUN_ID=REPLACE_WITH_SUCCESSFUL_CI_RUN_ID
readonly QUALIFICATION_ROOT=REPLACE_WITH_ABSENT_ABSOLUTE_PRIVATE_DIRECTORY
readonly IMAGE=ubuntu@sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90
readonly SOURCE_TREE=$PWD
readonly BODY=$SOURCE_TREE/planning/artifacts/038-release-notes.md
readonly HISTORY_RULESET_ID=20071950
readonly CONTRIBUTIONS_RULESET_ID=20071963

[[ "$COMMIT" =~ ^[0-9a-f]{40}$ ]]
[[ "$CI_RUN_ID" =~ ^[1-9][0-9]*$ ]]
[[ "$QUALIFICATION_ROOT" == /* && ! -e "$QUALIFICATION_ROOT" && ! -L "$QUALIFICATION_ROOT" ]]
test "$(git remote get-url origin)" = git@github.com:genomoncology/pangopup.git
test "$(git rev-parse HEAD)" = "$COMMIT"
git diff --quiet --
git diff --cached --quiet --
git fetch --force --prune origin main
test "$(git rev-parse origin/main)" = "$COMMIT"
git merge-base --is-ancestor "$COMMIT" origin/main
test "$(sed -nE 's/^version = "([^"]+)"$/\1/p' Cargo.toml | head -1)" = 0.1.0
test -f "$BODY"

command -v gh >/dev/null
command -v jq >/dev/null
command -v docker >/dev/null
command -v curl >/dev/null
command -v sha256sum >/dev/null
gh --version >/dev/null

PRIVATE=$(mktemp -d)
chmod 0700 "$PRIVATE"
install -d -m 0700 "$QUALIFICATION_ROOT"
RELEASE_ID=
PUBLISHED=0
cleanup_failed_draft() {
  status=$?
  trap - EXIT
  if (( status != 0 && PUBLISHED == 0 )) && [[ -n "$RELEASE_ID" ]]; then
    draft=$(gh api "repos/$REPO/releases/$RELEASE_ID" --jq .draft || true)
    tag_count=$(gh api "repos/$REPO/git/matching-refs/tags/$TAG" --jq length || true)
    if [[ "$draft" == true && "$tag_count" == 0 ]]; then
      gh api --method DELETE "repos/$REPO/releases/$RELEASE_ID" --silent
      if gh api "repos/$REPO/releases/$RELEASE_ID" --silent 2>/dev/null; then
        printf 'failed draft still exists\n' >&2
      fi
    fi
  fi
  exit "$status"
}
trap cleanup_failed_draft EXIT

# Recheck every live publication-security control before the first effect.
test "$(gh api "repos/$REPO" --jq .visibility)" = public
test "$(gh api "repos/$REPO/immutable-releases" --jq .enabled)" = true
gh api "repos/$REPO/actions/permissions/workflow" >"$PRIVATE/actions.json"
jq -e '.default_workflow_permissions == "read" and .can_approve_pull_request_reviews == false' "$PRIVATE/actions.json" >/dev/null
gh api "repos/$REPO/automated-security-fixes" >"$PRIVATE/dependabot.json"
jq -e '.enabled == true and .paused == false' "$PRIVATE/dependabot.json" >/dev/null
gh api "repos/$REPO" >"$PRIVATE/repository.json"
jq -e '
  .security_and_analysis.secret_scanning.status == "enabled" and
  .security_and_analysis.secret_scanning_push_protection.status == "enabled" and
  .security_and_analysis.secret_scanning_non_provider_patterns.status == "enabled"
' "$PRIVATE/repository.json" >/dev/null
gh api --method GET --paginate "repos/$REPO/secret-scanning/alerts" \
  -f state=open --jq '.[].number' >"$PRIVATE/open-secret-alerts.txt"
test ! -s "$PRIVATE/open-secret-alerts.txt"

gh api "repos/$REPO/rulesets" >"$PRIVATE/rulesets.json"
jq -e '
  length == 2 and
  ([.[].name] | sort) == ["pangopup-main-contributions", "pangopup-main-history"] and
  all(.[]; .enforcement == "active" and .target == "branch")
' "$PRIVATE/rulesets.json" >/dev/null
gh api "repos/$REPO/rulesets/$HISTORY_RULESET_ID" >"$PRIVATE/history.json"
jq -e '
  .name == "pangopup-main-history" and .enforcement == "active" and
  .target == "branch" and .bypass_actors == [] and
  .conditions.ref_name.include == ["refs/heads/main"] and
  .conditions.ref_name.exclude == [] and
  ([.rules[].type] | sort) == ["deletion", "non_fast_forward"]
' "$PRIVATE/history.json" >/dev/null
gh api "repos/$REPO/rulesets/$CONTRIBUTIONS_RULESET_ID" >"$PRIVATE/contributions.json"
jq -e '
  .name == "pangopup-main-contributions" and .enforcement == "active" and
  .target == "branch" and
  .conditions.ref_name.include == ["refs/heads/main"] and
  .conditions.ref_name.exclude == [] and
  .bypass_actors == [{"actor_id":5,"actor_type":"RepositoryRole","bypass_mode":"always"}] and
  ([.rules[].type] | sort) == ["pull_request", "required_status_checks"] and
  ([.rules[] | select(.type == "required_status_checks") |
    .parameters.required_status_checks[].context]) == ["gate"] and
  ([.rules[] | select(.type == "required_status_checks") |
    .parameters.strict_required_status_checks_policy]) == [false] and
  ([.rules[] | select(.type == "pull_request") |
    .parameters.required_approving_review_count]) == [0]
' "$PRIVATE/contributions.json" >/dev/null

# Bind the publication-ready commit to its one successful CI gate.
gh run view "$CI_RUN_ID" --repo "$REPO" \
  --json headSha,name,status,conclusion,jobs >"$PRIVATE/ci.json"
jq -e --arg commit "$COMMIT" '
  .headSha == $commit and .name == "ci" and .status == "completed" and
  .conclusion == "success" and (.jobs | length) == 1 and
  .jobs[0].name == "gate" and .jobs[0].status == "completed" and
  .jobs[0].conclusion == "success"
' "$PRIVATE/ci.json" >/dev/null
test -z "$(gh api --paginate "repos/$REPO/releases" --jq '.[].tag_name' | grep -Fx "$TAG" || true)"
test "$(gh api "repos/$REPO/git/matching-refs/tags/$TAG" --jq length)" -eq 0

# Dispatch the reviewed read-only workflow once and identify exactly one new run.
gh api "repos/$REPO/actions/workflows/package-linux.yml" >"$PRIVATE/workflow.json"
jq -e '.path == ".github/workflows/package-linux.yml" and .name == "package-linux" and .state == "active"' "$PRIVATE/workflow.json" >/dev/null
gh run list --repo "$REPO" --workflow package-linux.yml --event workflow_dispatch \
  --limit 100 --json databaseId >"$PRIVATE/runs-before.json"
jq -r '.[].databaseId' "$PRIVATE/runs-before.json" | sort -n >"$PRIVATE/runs-before.txt"
gh workflow run package-linux.yml --repo "$REPO" --ref main -f commit="$COMMIT"
PACKAGE_RUN_ID=
for _ in $(seq 1 60); do
  gh run list --repo "$REPO" --workflow package-linux.yml --event workflow_dispatch \
    --limit 100 --json databaseId >"$PRIVATE/runs-after.json"
  jq -r '.[].databaseId' "$PRIVATE/runs-after.json" | sort -n >"$PRIVATE/runs-after.txt"
  mapfile -t new_runs < <(comm -13 "$PRIVATE/runs-before.txt" "$PRIVATE/runs-after.txt")
  if (( ${#new_runs[@]} == 1 )); then PACKAGE_RUN_ID=${new_runs[0]}; break; fi
  if (( ${#new_runs[@]} > 1 )); then printf 'ambiguous package runs\n' >&2; exit 1; fi
  sleep 5
done
[[ "$PACKAGE_RUN_ID" =~ ^[1-9][0-9]*$ ]]
gh run watch "$PACKAGE_RUN_ID" --repo "$REPO" --exit-status
gh run view "$PACKAGE_RUN_ID" --repo "$REPO" \
  --json headSha,name,event,status,conclusion,jobs >"$PRIVATE/package-run.json"
jq -e --arg commit "$COMMIT" '
  .headSha == $commit and .name == "package-linux" and
  .event == "workflow_dispatch" and .status == "completed" and
  .conclusion == "success" and (.jobs | length) == 1 and
  .jobs[0].name == "package" and .jobs[0].conclusion == "success"
' "$PRIVATE/package-run.json" >/dev/null
gh api "repos/$REPO/actions/runs/$PACKAGE_RUN_ID/artifacts" >"$PRIVATE/artifacts.json"
jq -e --arg name "pangopup-linux-$COMMIT" '
  .total_count == 1 and (.artifacts | length) == 1 and
  .artifacts[0].name == $name and .artifacts[0].expired == false and
  .artifacts[0].size_in_bytes > 0
' "$PRIVATE/artifacts.json" >/dev/null
ARTIFACT_ID=$(jq -r .artifacts[0].id "$PRIVATE/artifacts.json")
install -d -m 0700 "$PRIVATE/release"
gh run download "$PACKAGE_RUN_ID" --repo "$REPO" \
  --name "pangopup-linux-$COMMIT" --dir "$PRIVATE/release"
scripts/qualify-linux-release.sh "$PRIVATE/release" 0.1.0 "$COMMIT"

# Prove fresh XDG installation, offline reuse, 1,000 SNVs, and exact M09.
docker run --rm --read-only --tmpfs /tmp:rw,noexec,nosuid,size=64m \
  -v "$PRIVATE/release:/release:ro" -v "$SOURCE_TREE:/source:ro" \
  -v "$QUALIFICATION_ROOT:/qualification:rw" "$IMAGE" bash -ceu '
    /source/scripts/run-production-qualification.sh \
      /release/pangopup-linux-x86_64 /source \
      /qualification/data /qualification/cache /qualification/output
  '
scripts/check-production-qualification.py "$QUALIFICATION_ROOT/output" "$SOURCE_TREE" \
  >"$PRIVATE/production-qualification.txt"

# Freeze the six local upload identities and create one private draft.
members=(LICENSE NOTICE pangopup-linux-x86_64 pangopup-linux-x86_64.cdx.json pangopup-linux-x86_64.sha256 release-manifest.json)
: >"$PRIVATE/local-inventory.tsv"
for name in "${members[@]}"; do
  path=$PRIVATE/release/$name
  test -f "$path" && test ! -L "$path" && test "$(stat -c %h "$path")" -eq 1
  printf '%s\t%s\tsha256:%s\n' "$name" "$(stat -c %s "$path")" \
    "$(sha256sum "$path" | cut -d' ' -f1)" >>"$PRIVATE/local-inventory.tsv"
done
test "$(wc -l <"$PRIVATE/local-inventory.tsv")" -eq 6
jq -n --arg tag "$TAG" --arg target "$COMMIT" --arg name "$TITLE" \
  --rawfile body "$BODY" \
  '{tag_name:$tag,target_commitish:$target,name:$name,body:$body,draft:true,prerelease:false}' \
  >"$PRIVATE/create-release.json"
gh api --method POST "repos/$REPO/releases" --input "$PRIVATE/create-release.json" \
  >"$PRIVATE/draft.json"
RELEASE_ID=$(jq -r .id "$PRIVATE/draft.json")
[[ "$RELEASE_ID" =~ ^[1-9][0-9]*$ ]]
jq -e --arg tag "$TAG" --arg target "$COMMIT" --arg title "$TITLE" '
  .tag_name == $tag and .target_commitish == $target and .name == $title and
  .draft == true and .prerelease == false
' "$PRIVATE/draft.json" >/dev/null
jq -jr .body "$PRIVATE/draft.json" >"$PRIVATE/remote-body"
cmp "$BODY" "$PRIVATE/remote-body"
test "$(gh api "repos/$REPO/git/matching-refs/tags/$TAG" --jq length)" -eq 0

# Copy each held, reauthenticated descriptor into the private staging root,
# upload it once, then require the exact remote prefix inventory.
install -d -m 0700 "$PRIVATE/upload"
: >"$PRIVATE/uploaded-prefix.tsv"
for name in "${members[@]}"; do
  source_path=$PRIVATE/release/$name
  exec {UPLOAD_FD}<"$source_path"
  path_state=$(stat -Lc '%d %i %F %u %h %s' "$source_path")
  fd_state=$(stat -Lc '%d %i %F %u %h %s' "/proc/self/fd/$UPLOAD_FD")
  test "$path_state" = "$fd_state"
  test "$(stat -Lc %F "/proc/self/fd/$UPLOAD_FD")" = 'regular file'
  test "$(stat -Lc %u "/proc/self/fd/$UPLOAD_FD")" -eq "$(id -u)"
  test "$(stat -Lc %h "/proc/self/fd/$UPLOAD_FD")" -eq 1
  digest=$(sha256sum "/proc/self/fd/$UPLOAD_FD" | cut -d' ' -f1)
  size=$(stat -Lc %s "/proc/self/fd/$UPLOAD_FD")
  read -r expected_name expected_size expected_digest < <(awk -F '\t' -v n="$name" '$1==n {print $1, $2, $3}' "$PRIVATE/local-inventory.tsv")
  test "$expected_name" = "$name" && test "$expected_size" = "$size" && test "$expected_digest" = "sha256:$digest"
  cp "/proc/self/fd/$UPLOAD_FD" "$PRIVATE/upload/$name"
  chmod 0400 "$PRIVATE/upload/$name"
  test "$(sha256sum "$PRIVATE/upload/$name" | cut -d' ' -f1)" = "$digest"
  gh release upload "$TAG" "$PRIVATE/upload/$name" --repo "$REPO"
  test "$(stat -Lc '%d %i %F %u %h %s' "$source_path")" = "$path_state"
  test "$(sha256sum "/proc/self/fd/$UPLOAD_FD" | cut -d' ' -f1)" = "$digest"
  exec {UPLOAD_FD}<&-
  printf '%s\t%s\tsha256:%s\n' "$name" "$size" "$digest" >>"$PRIVATE/uploaded-prefix.tsv"
  gh api --paginate "repos/$REPO/releases/$RELEASE_ID/assets" \
    --jq '.[] | [.name, (.size|tostring), .digest] | @tsv' | sort \
    >"$PRIVATE/remote-prefix.tsv"
  sort "$PRIVATE/uploaded-prefix.tsv" >"$PRIVATE/expected-prefix.tsv"
  cmp "$PRIVATE/expected-prefix.tsv" "$PRIVATE/remote-prefix.tsv"
done

# Recheck the complete private draft immediately before irreversible publish.
gh api "repos/$REPO/releases/$RELEASE_ID" >"$PRIVATE/prepublish.json"
jq -e --arg tag "$TAG" --arg target "$COMMIT" --arg title "$TITLE" '
  .id > 0 and .tag_name == $tag and .target_commitish == $target and
  .name == $title and .draft == true and .prerelease == false and
  (.assets | length) == 6
' "$PRIVATE/prepublish.json" >/dev/null
test "$(jq -r .id "$PRIVATE/prepublish.json")" = "$RELEASE_ID"
jq -jr .body "$PRIVATE/prepublish.json" >"$PRIVATE/prepublish-body"
cmp "$BODY" "$PRIVATE/prepublish-body"
test "$(gh api "repos/$REPO/git/matching-refs/tags/$TAG" --jq length)" -eq 0
gh api --paginate "repos/$REPO/releases/$RELEASE_ID/assets" \
  --jq '.[] | [.name, (.size|tostring), .digest] | @tsv' | sort \
  >"$PRIVATE/remote-final.tsv"
sort "$PRIVATE/local-inventory.tsv" >"$PRIVATE/local-final.tsv"
cmp "$PRIVATE/local-final.tsv" "$PRIVATE/remote-final.tsv"

gh api --method PATCH "repos/$REPO/releases/$RELEASE_ID" \
  -F draft=false -f make_latest=true >"$PRIVATE/published.json"
PUBLISHED=1
jq -e --arg tag "$TAG" --arg target "$COMMIT" --arg title "$TITLE" '
  .tag_name == $tag and .target_commitish == $target and .name == $title and
  .draft == false and .prerelease == false and .immutable == true
' "$PRIVATE/published.json" >/dev/null
test "$(gh api "repos/$REPO/git/ref/tags/$TAG" --jq .object.type)" = commit
test "$(gh api "repos/$REPO/git/ref/tags/$TAG" --jq .object.sha)" = "$COMMIT"
test "$(gh api "repos/$REPO/releases/latest" --jq .tag_name)" = "$TAG"

# Use no GitHub credential for bounded public metadata and five small assets.
env -u GH_TOKEN -u GITHUB_TOKEN curl -fsSL \
  "https://api.github.com/repos/$REPO/releases/tags/$TAG" >"$PRIVATE/public-release.json"
env -u GH_TOKEN -u GITHUB_TOKEN curl -fsSL \
  "https://api.github.com/repos/$REPO/git/ref/tags/$TAG" >"$PRIVATE/public-tag.json"
env -u GH_TOKEN -u GITHUB_TOKEN curl -fsSL \
  "https://api.github.com/repos/$REPO/releases/latest" >"$PRIVATE/public-latest.json"
jq -e --arg tag "$TAG" --arg target "$COMMIT" --arg title "$TITLE" '
  .id > 0 and .tag_name == $tag and .target_commitish == $target and
  .name == $title and .draft == false and .prerelease == false and
  .immutable == true and (.assets | length) == 6
' "$PRIVATE/public-release.json" >/dev/null
jq -e --arg tag "$TAG" '.tag_name == $tag' "$PRIVATE/public-latest.json" >/dev/null
jq -e --arg commit "$COMMIT" '.object.type == "commit" and .object.sha == $commit' "$PRIVATE/public-tag.json" >/dev/null
jq -jr .body "$PRIVATE/public-release.json" >"$PRIVATE/public-body"
cmp "$BODY" "$PRIVATE/public-body"
jq -r '.assets[] | [.name, (.size|tostring), .digest] | @tsv' \
  "$PRIVATE/public-release.json" | sort >"$PRIVATE/public-inventory.tsv"
cmp "$PRIVATE/local-final.tsv" "$PRIVATE/public-inventory.tsv"
for name in LICENSE NOTICE pangopup-linux-x86_64.cdx.json pangopup-linux-x86_64.sha256 release-manifest.json; do
  env -u GH_TOKEN -u GITHUB_TOKEN curl -fsSL \
    "https://github.com/$REPO/releases/download/$TAG/$name" -o "$PRIVATE/public-$name"
  expected=$(awk -F '\t' -v n="$name" '$1==n {print $3}' "$PRIVATE/local-inventory.tsv")
  test "sha256:$(sha256sum "$PRIVATE/public-$name" | cut -d' ' -f1)" = "$expected"
done
jq -e --argjson size "$(stat -c %s "$PRIVATE/release/pangopup-linux-x86_64")" \
  --arg digest "sha256:$(sha256sum "$PRIVATE/release/pangopup-linux-x86_64" | cut -d' ' -f1)" '
  [.assets[] | select(.name == "pangopup-linux-x86_64") |
    select(.size == $size and .digest == $digest)] | length == 1
' "$PRIVATE/public-release.json" >/dev/null

# A second fresh container downloads the tagged installer through its normal
# public checksum path, reuses only the already-qualified XDG assets offline,
# and emits one retained SNV plus exact M09 for host comparison.
test ! -e "$QUALIFICATION_ROOT/install" && test ! -e "$QUALIFICATION_ROOT/post"
docker run --rm --tmpfs /tmp:rw,nosuid,size=256m \
  -v "$QUALIFICATION_ROOT:/qualification:rw" -v "$SOURCE_TREE:/source:ro" \
  "$IMAGE" bash -ceu '
    apt-get update >/dev/null
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates curl >/dev/null
    install -d -m 700 /qualification/install /qualification/post /qualification/home
    curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.1.0/install.sh -o /tmp/install.sh
    PANGOPUP_INSTALL_DIR=/qualification/install bash /tmp/install.sh --version 0.1.0
    export HOME=/qualification/home XDG_DATA_HOME=/qualification/data XDG_CACHE_HOME=/qualification/cache
    /qualification/install/pangopup sync --offline >/qualification/post/sync.json
    /qualification/install/pangopup status >/qualification/post/status.json
    /qualification/install/pangopup lookup --format jsonl --gene ENSG00000010610 \
      --variant GRCh38:chr12:6801301:G:A >/qualification/post/snv.jsonl
    /qualification/install/pangopup lookup --format jsonl \
      --variant GRCh38:chr12:6801303:G:GA >/qualification/post/m09.jsonl
  '
jq -e '.status == "ready" and .snv.status == "reused" and .runtime.status == "reused"' "$QUALIFICATION_ROOT/post/sync.json" >/dev/null
jq -e '.status == "ready" and .snv.status == "ready" and .runtime.status == "ready"' "$QUALIFICATION_ROOT/post/status.json" >/dev/null
head -1 tests/fixtures/snv-regression/expected/ENSG00000010610.jsonl \
  | sed -E 's/,"bundle_id":"sha256:[0-9a-f]{64}"//' >"$PRIVATE/one-snv-expected.jsonl"
sed -E 's/,"bundle_id":"sha256:[0-9a-f]{64}"//' "$QUALIFICATION_ROOT/post/snv.jsonl" \
  >"$PRIVATE/one-snv-actual.jsonl"
cmp "$PRIVATE/one-snv-expected.jsonl" "$PRIVATE/one-snv-actual.jsonl"
cmp tests/fixtures/executable-release/m09.jsonl "$QUALIFICATION_ROOT/post/m09.jsonl"

trap - EXIT
printf 'Ticket 038 public operation passed: release_id=%s package_run_id=%s artifact_id=%s\n' \
  "$RELEASE_ID" "$PACKAGE_RUN_ID" "$ARTIFACT_ID"
```
<!-- END TICKET 038 COORDINATOR SCRIPT -->

## Completion-only documentation transition

After all public checks pass, replace PREPUBLICATION/current-future wording in
this record, `README.md`, `architecture/delivery.md`, `planning/frontier.md`,
and `planning/faq.md` with the exact public URL and completed state. The same
developer makes that bounded diff and the same code reviewer accepts it before
the coordinator marks Ticket 038 complete and pushes the completion commit.
