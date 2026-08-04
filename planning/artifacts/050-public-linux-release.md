# Ticket 050 public Linux release record

State: **PUBLICATION-READY — no GitHub release, tag, workflow dispatch, or
upload has been performed by the developer.**

The reviewed target is one immutable `v0.2.0` Linux x86-64 release containing
exactly `LICENSE`, `NOTICE`, `pangopup-linux-x86_64`,
`pangopup-linux-x86_64.cdx.json`, `pangopup-linux-x86_64.sha256`, and
`release-manifest.json`. The immutable `snv-grch38-v1` and
`runtime-grch38-v1` releases are inputs, not members. No raw upstream data,
container image, ARM64 executable, changed model, or rebuilt index is in scope.

The coordinator must replace the three `REPLACE_...` values below only after
the publication-ready implementation is committed and pushed. The script
authenticates that exact checkout, materializes a private `git archive` of the
exact commit, and uses only that immutable snapshot for release notes, scripts,
oracles, and qualification. It also authenticates one successful `ci/gate`, repository
security controls, tag absence, one read-only packaging run, the six local
members, clean production scoring, one private draft, every uploaded prefix,
and the complete draft before its single irreversible publish. A failure before
publication deletes only the authenticated unpublished draft with the absent
tag. A failure after publication records the failure and stops; it never edits
or deletes the immutable release.

The qualification runner checks observable online progress, quiet/offline
reuse, focused help, the retained 1,000-SNV oracle, automatic M09 inference,
explicit model-only scoring of an indexed SNV, and
foreground HTTP health/status/SNV/model scoring. The public tagged installer
repeats the same non-download checks against the already qualified XDG assets.

## Exact syntax-checked coordinator runbook

<!-- BEGIN TICKET 050 COORDINATOR SCRIPT -->
```bash
set -euo pipefail
umask 077

readonly REPO=genomoncology/pangopup
readonly TAG=v0.2.0
readonly VERSION=0.2.0
readonly TITLE='PangoPup v0.2.0'
readonly COMMIT=REPLACE_WITH_40_LOWERCASE_PUBLICATION_READY_COMMIT
readonly CI_RUN_ID=REPLACE_WITH_SUCCESSFUL_CI_RUN_ID
readonly QUALIFICATION_ROOT=REPLACE_WITH_ABSENT_ABSOLUTE_PRIVATE_DIRECTORY
readonly IMAGE=ubuntu@sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90
readonly CHECKOUT=$PWD
readonly HOST_UID=$(id -u)
readonly HOST_GID=$(id -g)
readonly HISTORY_RULESET_ID=20071950
readonly CONTRIBUTIONS_RULESET_ID=20071963

[[ "$COMMIT" =~ ^[0-9a-f]{40}$ && "$CI_RUN_ID" =~ ^[1-9][0-9]*$ ]]
[[ "$QUALIFICATION_ROOT" == /* && ! -e "$QUALIFICATION_ROOT" && ! -L "$QUALIFICATION_ROOT" ]]
test "$(git remote get-url origin)" = git@github.com:genomoncology/pangopup.git
test "$(git rev-parse HEAD)" = "$COMMIT"
git diff --quiet --
git diff --cached --quiet --
git fetch --force --prune origin main
test "$(git rev-parse origin/main)" = "$COMMIT"
test -z "$(git -C "$CHECKOUT" replace -l)"
GIT_NO_REPLACE_OBJECTS=1 git -C "$CHECKOUT" cat-file -e "$COMMIT^{commit}"
for tool in gh jq docker curl sha256sum tar; do command -v "$tool" >/dev/null; done

PRIVATE=$(mktemp -d)
chmod 0700 "$PRIVATE"
install -d -m 0700 "$QUALIFICATION_ROOT"
GIT_NO_REPLACE_OBJECTS=1 git -C "$CHECKOUT" archive --format=tar "$COMMIT" \
  >"$PRIVATE/source.tar"
install -d -m 0700 "$PRIVATE/source"
tar -xf "$PRIVATE/source.tar" -C "$PRIVATE/source"
readonly SOURCE_TREE=$PRIVATE/source
readonly BODY=$SOURCE_TREE/planning/artifacts/050-release-notes.md
test -f "$BODY" && test ! -L "$BODY"
test "$(sed -nE 's/^version = "([^"]+)"$/\1/p' "$SOURCE_TREE/Cargo.toml" | head -1)" = "$VERSION"
for path in scripts/qualify-linux-release.sh scripts/run-production-qualification.sh \
  scripts/check-production-qualification.py tests/fixtures/snv-regression/requests.tsv \
  tests/fixtures/executable-release/m09.jsonl \
  tests/fixtures/executable-release/model-only-snv.jsonl; do
  test -f "$SOURCE_TREE/$path" && test ! -L "$SOURCE_TREE/$path"
done
RELEASE_ID=
PUBLISHED=0
cleanup_failed_draft() {
  status=$?
  trap - EXIT
  if ((status != 0 && PUBLISHED == 0)) && [[ -n "$RELEASE_ID" ]]; then
    draft=$(gh api "repos/$REPO/releases/$RELEASE_ID" --jq .draft || true)
    tag_count=$(gh api "repos/$REPO/git/matching-refs/tags/$TAG" --jq length || true)
    if [[ "$draft" == true && "$tag_count" == 0 ]]; then
      gh api --method DELETE "repos/$REPO/releases/$RELEASE_ID" --silent
    fi
  fi
  exit "$status"
}
trap cleanup_failed_draft EXIT

test "$(gh api "repos/$REPO" --jq .visibility)" = public
test "$(gh api "repos/$REPO/immutable-releases" --jq .enabled)" = true
gh api "repos/$REPO/actions/permissions/workflow" >"$PRIVATE/actions.json"
jq -e '.default_workflow_permissions == "read" and .can_approve_pull_request_reviews == false' "$PRIVATE/actions.json" >/dev/null
gh api "repos/$REPO/automated-security-fixes" >"$PRIVATE/dependabot.json"
jq -e '.enabled == true and .paused == false' "$PRIVATE/dependabot.json" >/dev/null
gh api "repos/$REPO" >"$PRIVATE/repository.json"
jq -e '.security_and_analysis.secret_scanning.status == "enabled" and
  .security_and_analysis.secret_scanning_push_protection.status == "enabled" and
  .security_and_analysis.secret_scanning_non_provider_patterns.status == "enabled"' "$PRIVATE/repository.json" >/dev/null
gh api --method GET --paginate "repos/$REPO/secret-scanning/alerts" -f state=open \
  --jq '.[].number' >"$PRIVATE/open-secret-alerts.txt"
test ! -s "$PRIVATE/open-secret-alerts.txt"
gh api "repos/$REPO/rulesets" >"$PRIVATE/rulesets.json"
jq -e 'length == 2 and
  ([.[].name] | sort) == ["pangopup-main-contributions","pangopup-main-history"] and
  all(.[]; .enforcement == "active" and .target == "branch")' \
  "$PRIVATE/rulesets.json" >/dev/null
gh api "repos/$REPO/rulesets/$HISTORY_RULESET_ID" >"$PRIVATE/history.json"
jq -e '.name == "pangopup-main-history" and .enforcement == "active" and
  .target == "branch" and .bypass_actors == [] and
  .conditions.ref_name.include == ["refs/heads/main"] and
  .conditions.ref_name.exclude == [] and
  ([.rules[].type] | sort) == ["deletion","non_fast_forward"]' \
  "$PRIVATE/history.json" >/dev/null
gh api "repos/$REPO/rulesets/$CONTRIBUTIONS_RULESET_ID" >"$PRIVATE/contributions.json"
jq -e '.name == "pangopup-main-contributions" and .enforcement == "active" and
  .target == "branch" and
  .conditions.ref_name.include == ["refs/heads/main"] and
  .conditions.ref_name.exclude == [] and
  .bypass_actors == [{"actor_id":5,"actor_type":"RepositoryRole","bypass_mode":"always"}] and
  ([.rules[].type] | sort) == ["pull_request","required_status_checks"] and
  ([.rules[] | select(.type == "required_status_checks") |
    .parameters.required_status_checks[].context]) == ["gate"] and
  ([.rules[] | select(.type == "required_status_checks") |
    .parameters.strict_required_status_checks_policy]) == [false] and
  ([.rules[] | select(.type == "pull_request") |
    .parameters.required_approving_review_count]) == [0]' \
  "$PRIVATE/contributions.json" >/dev/null

gh run view "$CI_RUN_ID" --repo "$REPO" --json headSha,name,status,conclusion,jobs \
  >"$PRIVATE/ci.json"
jq -e --arg commit "$COMMIT" '.headSha == $commit and .name == "ci" and
  .status == "completed" and .conclusion == "success" and (.jobs | length) == 1 and
  .jobs[0].name == "gate" and .jobs[0].conclusion == "success"' "$PRIVATE/ci.json" >/dev/null
test -z "$(gh api --paginate "repos/$REPO/releases" --jq '.[].tag_name' | grep -Fx "$TAG" || true)"
test "$(gh api "repos/$REPO/git/matching-refs/tags/$TAG" --jq length)" -eq 0

gh api "repos/$REPO/actions/workflows/package-linux.yml" >"$PRIVATE/workflow.json"
jq -e '.path == ".github/workflows/package-linux.yml" and
  .name == "package-linux" and .state == "active"' "$PRIVATE/workflow.json" >/dev/null
gh run list --repo "$REPO" --workflow package-linux.yml --event workflow_dispatch \
  --limit 100 --json databaseId | jq -r '.[].databaseId' | sort -n >"$PRIVATE/runs-before.txt"
gh workflow run package-linux.yml --repo "$REPO" --ref main -f commit="$COMMIT"
PACKAGE_RUN_ID=
for _ in $(seq 1 60); do
  gh run list --repo "$REPO" --workflow package-linux.yml --event workflow_dispatch \
    --limit 100 --json databaseId | jq -r '.[].databaseId' | sort -n >"$PRIVATE/runs-after.txt"
  mapfile -t new_runs < <(comm -13 "$PRIVATE/runs-before.txt" "$PRIVATE/runs-after.txt")
  if (( ${#new_runs[@]} == 1 )); then PACKAGE_RUN_ID=${new_runs[0]}; break; fi
  (( ${#new_runs[@]} == 0 )) || { printf 'ambiguous package runs\n' >&2; exit 1; }
  sleep 5
done
[[ "$PACKAGE_RUN_ID" =~ ^[1-9][0-9]*$ ]]
gh run watch "$PACKAGE_RUN_ID" --repo "$REPO" --exit-status
gh run view "$PACKAGE_RUN_ID" --repo "$REPO" --json headSha,name,event,status,conclusion,jobs \
  >"$PRIVATE/package-run.json"
jq -e --arg commit "$COMMIT" '.headSha == $commit and .name == "package-linux" and
  .event == "workflow_dispatch" and .status == "completed" and
  .conclusion == "success" and (.jobs | length) == 1 and
  .jobs[0].name == "package" and .jobs[0].conclusion == "success"' \
  "$PRIVATE/package-run.json" >/dev/null
gh api "repos/$REPO/actions/runs/$PACKAGE_RUN_ID/artifacts" >"$PRIVATE/artifacts.json"
jq -e --arg name "pangopup-linux-$COMMIT" '.total_count == 1 and
  (.artifacts | length) == 1 and .artifacts[0].name == $name and
  .artifacts[0].expired == false and .artifacts[0].size_in_bytes > 0' \
  "$PRIVATE/artifacts.json" >/dev/null
ARTIFACT_ID=$(jq -r .artifacts[0].id "$PRIVATE/artifacts.json")
[[ "$ARTIFACT_ID" =~ ^[1-9][0-9]*$ ]]
install -d -m 0700 "$PRIVATE/release"
gh run download "$PACKAGE_RUN_ID" --repo "$REPO" --name "pangopup-linux-$COMMIT" \
  --dir "$PRIVATE/release"
"$SOURCE_TREE/scripts/qualify-linux-release.sh" "$PRIVATE/release" "$VERSION" "$COMMIT"

docker run --rm --read-only --tmpfs /tmp:rw,noexec,nosuid,size=64m \
  --user "$HOST_UID:$HOST_GID" -v "$PRIVATE/release:/release:ro" \
  -v "$SOURCE_TREE:/source:ro" -v "$QUALIFICATION_ROOT:/qualification:rw" \
  "$IMAGE" bash -ceu '
    /source/scripts/run-production-qualification.sh /release/pangopup-linux-x86_64 \
      /source /qualification/data /qualification/cache /qualification/output
  '
"$SOURCE_TREE/scripts/check-production-qualification.py" "$QUALIFICATION_ROOT/output" "$SOURCE_TREE" \
  >"$PRIVATE/production-qualification.txt"

members=(LICENSE NOTICE pangopup-linux-x86_64 pangopup-linux-x86_64.cdx.json pangopup-linux-x86_64.sha256 release-manifest.json)
: >"$PRIVATE/local.tsv"
for name in "${members[@]}"; do
  path=$PRIVATE/release/$name
  test -f "$path" && test ! -L "$path" && test "$(stat -c %h "$path")" -eq 1
  printf '%s\t%s\tsha256:%s\n' "$name" "$(stat -c %s "$path")" \
    "$(sha256sum "$path" | cut -d' ' -f1)" >>"$PRIVATE/local.tsv"
done
test "$(wc -l <"$PRIVATE/local.tsv")" -eq 6
jq -n --arg tag "$TAG" --arg target "$COMMIT" --arg name "$TITLE" --rawfile body "$BODY" \
  '{tag_name:$tag,target_commitish:$target,name:$name,body:$body,draft:true,prerelease:false}' \
  >"$PRIVATE/create.json"
gh api --method POST "repos/$REPO/releases" --input "$PRIVATE/create.json" >"$PRIVATE/draft.json"
RELEASE_ID=$(jq -r .id "$PRIVATE/draft.json")
[[ "$RELEASE_ID" =~ ^[1-9][0-9]*$ ]]
jq -jr .body "$PRIVATE/draft.json" >"$PRIVATE/body"
cmp "$BODY" "$PRIVATE/body"

install -d -m 0700 "$PRIVATE/upload"
: >"$PRIVATE/uploaded.tsv"
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
  read -r expected_name expected_size expected_digest < <(
    awk -F '\t' -v n="$name" '$1==n {print $1, $2, $3}' "$PRIVATE/local.tsv"
  )
  test "$expected_name" = "$name" && test "$expected_size" = "$size"
  test "$expected_digest" = "sha256:$digest"
  cp "/proc/self/fd/$UPLOAD_FD" "$PRIVATE/upload/$name"
  chmod 0400 "$PRIVATE/upload/$name"
  test "$(sha256sum "$PRIVATE/upload/$name" | cut -d' ' -f1)" = "$digest"
  gh release upload "$TAG" "$PRIVATE/upload/$name" --repo "$REPO"
  test "$(stat -Lc '%d %i %F %u %h %s' "$source_path")" = "$path_state"
  test "$(sha256sum "/proc/self/fd/$UPLOAD_FD" | cut -d' ' -f1)" = "$digest"
  exec {UPLOAD_FD}<&-
  printf '%s\t%s\tsha256:%s\n' "$name" "$size" "$digest" >>"$PRIVATE/uploaded.tsv"
  gh api --paginate "repos/$REPO/releases/$RELEASE_ID/assets" \
    --jq '.[] | [.name, (.size|tostring), .digest] | @tsv' | sort >"$PRIVATE/remote.tsv"
  sort "$PRIVATE/uploaded.tsv" >"$PRIVATE/expected.tsv"
  cmp "$PRIVATE/expected.tsv" "$PRIVATE/remote.tsv"
done

gh api "repos/$REPO/releases/$RELEASE_ID" >"$PRIVATE/prepublish.json"
jq -e --arg tag "$TAG" --arg target "$COMMIT" '.tag_name == $tag and
  .target_commitish == $target and .draft == true and .prerelease == false and
  (.assets | length) == 6' "$PRIVATE/prepublish.json" >/dev/null
jq -jr .body "$PRIVATE/prepublish.json" >"$PRIVATE/prepublish-body"
cmp "$BODY" "$PRIVATE/prepublish-body"
sort "$PRIVATE/local.tsv" >"$PRIVATE/local-sorted.tsv"
cmp "$PRIVATE/local-sorted.tsv" "$PRIVATE/remote.tsv"

gh api --method PATCH "repos/$REPO/releases/$RELEASE_ID" -F draft=false \
  -f make_latest=true >"$PRIVATE/published.json"
PUBLISHED=1
jq -e '.draft == false and .prerelease == false and .immutable == true' \
  "$PRIVATE/published.json" >/dev/null
test "$(gh api "repos/$REPO/git/ref/tags/$TAG" --jq .object.sha)" = "$COMMIT"
test "$(gh api "repos/$REPO/releases/latest" --jq .tag_name)" = "$TAG"

env -u GH_TOKEN -u GITHUB_TOKEN curl -q -fsSL \
  "https://api.github.com/repos/$REPO/releases/tags/$TAG" >"$PRIVATE/public.json"
jq -e --arg tag "$TAG" --arg target "$COMMIT" --arg title "$TITLE" \
  '.tag_name == $tag and .target_commitish == $target and .name == $title and
  .draft == false and .prerelease == false and .immutable == true and
  (.assets | length) == 6' "$PRIVATE/public.json" >/dev/null
env -u GH_TOKEN -u GITHUB_TOKEN curl -q -fsSL \
  "https://api.github.com/repos/$REPO/git/ref/tags/$TAG" >"$PRIVATE/public-tag.json"
env -u GH_TOKEN -u GITHUB_TOKEN curl -q -fsSL \
  "https://api.github.com/repos/$REPO/releases/latest" >"$PRIVATE/public-latest.json"
jq -e --arg commit "$COMMIT" '.object.type == "commit" and .object.sha == $commit' \
  "$PRIVATE/public-tag.json" >/dev/null
jq -e --arg tag "$TAG" '.tag_name == $tag' "$PRIVATE/public-latest.json" >/dev/null
jq -r '.assets[] | [.name, (.size|tostring), .digest] | @tsv' "$PRIVATE/public.json" \
  | sort >"$PRIVATE/public.tsv"
cmp "$PRIVATE/local-sorted.tsv" "$PRIVATE/public.tsv"
jq -jr .body "$PRIVATE/public.json" >"$PRIVATE/public-body"
cmp "$BODY" "$PRIVATE/public-body"
for name in LICENSE NOTICE pangopup-linux-x86_64.cdx.json pangopup-linux-x86_64.sha256 release-manifest.json; do
  env -u GH_TOKEN -u GITHUB_TOKEN curl -q -fsSL \
    "https://github.com/$REPO/releases/download/$TAG/$name" -o "$PRIVATE/public-$name"
  expected=$(awk -F '\t' -v n="$name" '$1==n {print $3}' "$PRIVATE/local.tsv")
  test "sha256:$(sha256sum "$PRIVATE/public-$name" | cut -d' ' -f1)" = "$expected"
done

docker run --rm --tmpfs /tmp:rw,nosuid,size=256m \
  --env "HOST_UID=$HOST_UID" --env "HOST_GID=$HOST_GID" \
  -v "$QUALIFICATION_ROOT:/qualification:rw" -v "$SOURCE_TREE:/source:ro" \
  "$IMAGE" bash -ceu '
    apt-get update >/dev/null
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates curl >/dev/null
    install -d -m 755 /qualification/install
    curl -q -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.2.0/install.sh -o /tmp/install.sh
    PANGOPUP_INSTALL_DIR=/qualification/install bash /tmp/install.sh --version 0.2.0
    chown "$HOST_UID:$HOST_GID" /qualification
    /usr/bin/setpriv --reuid="$HOST_UID" --regid="$HOST_GID" --clear-groups \
      env -i HOME=/qualification/home PATH=/usr/bin:/bin \
      /source/scripts/run-production-qualification.sh /qualification/install/pangopup \
        /source /qualification/data /qualification/cache /qualification/post --reuse-installed
  '
"$SOURCE_TREE/scripts/check-production-qualification.py" "$QUALIFICATION_ROOT/post" "$SOURCE_TREE" \
  --reuse-installed >"$PRIVATE/public-qualification.txt"

trap - EXIT
printf 'Ticket 050 publication passed: release_id=%s package_run_id=%s\n' \
  "$RELEASE_ID" "$PACKAGE_RUN_ID"
printf 'Ticket 050 artifact admitted: artifact_id=%s\n' "$ARTIFACT_ID"
```
<!-- END TICKET 050 COORDINATOR SCRIPT -->

## External effect evidence

Coordinator: pending. After publication, record the exact commit, CI/package
run URLs, artifact ID, release ID/URL, six names/sizes/digests, public
qualification result, and tagged installer result here without credentials,
headers, signed URLs, environment dumps, or private paths.
