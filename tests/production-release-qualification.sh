#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
root=$repo/target/production-release-qualification-test
chmod -R u+w "$root" 2>/dev/null || true
rm -rf "$root"
install -d -m 700 "$root/bin"

cat >"$root/bin/pangopup" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
command=$1
shift
if [[ "${1:-}" == --help ]]; then
  case "$command" in
    sync) printf '%s\n' 'Usage: pangopup sync [--offline] [--progress | --quiet] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]' ;;
    status) printf '%s\n' 'Usage: pangopup status [--data-dir <ABSOLUTE_PATH>]' ;;
    lookup) printf '%s\n' 'Usage: pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] [--model-only] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]' ;;
    serve) printf '%s\n' 'Usage: pangopup serve [--listen <ADDRESS>] [--data-dir <ABSOLUTE_PATH>] [--model-workers <1..8>] [--model-threads <1..8>] [--model-queue-capacity <1..1024>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]' ;;
    *) exit 2 ;;
  esac
  exit
fi
case "$command" in
  sync)
    if [[ " $* " == *' --offline '* ]]; then
      if [[ " $* " == *' --progress '* ]]; then
        printf '%s\n' \
          'sync: checking snv assets' \
          'sync: reusing installed snv assets' \
          'sync: reusing installed runtime assets' \
          'sync: ready (0 downloaded, 0 resumed)' >&2
      fi
      printf '%s\n' '{"status":"ready","snv":{"status":"reused"},"runtime":{"status":"reused"},"downloaded_bytes":0,"resumed_bytes":0}'
    else
      [[ $HOME == "$QUALIFICATION_EXPECTED_HOME" ]]
      [[ $XDG_DATA_HOME == "$QUALIFICATION_EXPECTED_DATA" ]]
      [[ $XDG_CACHE_HOME == "$QUALIFICATION_EXPECTED_CACHE" ]]
      install -d -m 700 "$XDG_DATA_HOME/pangopup" "$XDG_DATA_HOME/pangopup/bundles"
      case ${QUALIFICATION_SNV_LAYOUT:-safe} in
        safe)
          install -d -m 700 "$QUALIFICATION_EXPECTED_SNV_BUNDLE"
          chmod 555 "${QUALIFICATION_EXPECTED_SNV_BUNDLE%/bundle}" "$QUALIFICATION_EXPECTED_SNV_BUNDLE"
          ;;
        zero) ;;
        multiple)
          install -d -m 700 \
            "$XDG_DATA_HOME/pangopup/bundles/first/bundle" \
            "$XDG_DATA_HOME/pangopup/bundles/second/bundle"
          chmod 555 \
            "$XDG_DATA_HOME/pangopup/bundles/first" \
            "$XDG_DATA_HOME/pangopup/bundles/first/bundle" \
            "$XDG_DATA_HOME/pangopup/bundles/second" \
            "$XDG_DATA_HOME/pangopup/bundles/second/bundle"
          ;;
        symlink)
          install -d -m 555 "$XDG_DATA_HOME/outside-bundle/bundle"
          ln -s "$XDG_DATA_HOME/outside-bundle" "$XDG_DATA_HOME/pangopup/bundles/linked"
          ;;
        unsafe)
          install -d -m 755 "$XDG_DATA_HOME/pangopup/bundles/unsafe"
          install -d -m 555 "$XDG_DATA_HOME/pangopup/bundles/unsafe/bundle"
          ;;
        *) exit 2 ;;
      esac
      if [[ " $* " == *' --progress '* ]]; then
        printf '%s\n' \
          'sync: checking snv assets' \
          'sync: snv payload fresh attempt 1/4 10/10 bytes (10 downloaded, 0 resumed)' \
          'sync: ready (10 downloaded, 0 resumed)' >&2
      fi
      printf '%s\n' '{"status":"ready","snv":{"status":"installed"},"runtime":{"status":"installed"},"downloaded_bytes":10,"resumed_bytes":0}'
    fi
    ;;
  status)
    printf '%s\n' '{"status":"ready","snv":{"status":"ready"},"runtime":{"status":"ready"}}'
    ;;
  lookup)
    if [[ " $* " == *' --model-only '* && " $* " == *' GRCh38:chr12:6801301:G:A '* ]]; then
      printf 'model\n' >>"$QUALIFICATION_LOOKUP_LOG"
      cat "$QUALIFICATION_SOURCE/tests/fixtures/executable-release/model-only-snv.jsonl"
      exit
    fi
    if [[ " $* " == *' GRCh38:chr12:6801303:G:GA '* ]]; then
      [[ " $* " != *' --bundle '* ]] || exit 2
      printf 'model\n' >>"$QUALIFICATION_LOOKUP_LOG"
      cat "$QUALIFICATION_SOURCE/tests/fixtures/executable-release/m09.jsonl"
      exit
    fi
    group=unfiltered
    bundle=
    variants=()
    while (( $# )); do
      case $1 in
        --bundle) bundle=$2; shift 2 ;;
        --gene) group=$2; shift 2 ;;
        --variant) variants+=("$2"); shift 2 ;;
        --format)
          [[ $2 == jsonl ]] || exit 2
          shift 2
          ;;
        *) exit 2 ;;
      esac
    done
    [[ $bundle == "$QUALIFICATION_EXPECTED_SNV_BUNDLE" ]] || exit 2
    printf 'snv\t%s\n' "$bundle" >>"$QUALIFICATION_LOOKUP_LOG"
    mapfile -t expected_variants < <(awk -F '\t' -v group="$group" 'NR > 1 && $2 == group { print $4 }' "$QUALIFICATION_SOURCE/tests/fixtures/snv-regression/requests.tsv")
    [[ "${variants[*]}" == "${expected_variants[*]}" ]] || exit 2
    cat "$QUALIFICATION_SOURCE/tests/fixtures/snv-regression/expected/$group.jsonl"
    ;;
  serve)
    exec python3 - "$QUALIFICATION_SOURCE" <<'PY'
import http.server, json, pathlib, sys
source = pathlib.Path(sys.argv[1])
model = json.loads((source / "tests/fixtures/executable-release/m09.jsonl").read_bytes())
model_only_snv = json.loads((source / "tests/fixtures/executable-release/model-only-snv.jsonl").read_bytes())
automatic_snv = json.loads((source / "tests/fixtures/snv-regression/expected/ENSG00000010610.jsonl").read_text().splitlines()[0])
class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def emit(self, value):
        body = json.dumps(value, separators=(",", ":")).encode() + b"\n"
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_GET(self):
        values = {
            "/livez": {"status":"live"},
            "/readyz": {"status":"ready"},
            "/v1/status": {"version":"0.4.0","readiness":"ready"},
        }
        self.emit(values[self.path])
    def do_POST(self):
        body = self.rfile.read(int(self.headers["content-length"]))
        request = json.loads(body)
        if request.get("model_only"):
            result = model_only_snv
        elif request["variants"][0].endswith(":G:A"):
            result = automatic_snv
        else:
            result = model
        self.emit({"results":[result]})
    def log_message(self, *_): pass
http.server.ThreadingHTTPServer(("127.0.0.1", 18080), Handler).serve_forever()
PY
    ;;
  *) exit 2 ;;
esac
SH
chmod 755 "$root/bin/pangopup"

jq -S -c '.results[0] + {provenance:.provenance}' \
  "$repo/tests/fixtures/container-qualification/production-model-oracle.json" \
  >"$root/derived-model-only-snv.json"
jq -S -c . "$repo/tests/fixtures/executable-release/model-only-snv.jsonl" \
  >"$root/checked-model-only-snv.json"
cmp "$root/derived-model-only-snv.json" "$root/checked-model-only-snv.json"

export QUALIFICATION_SOURCE=$repo
export QUALIFICATION_EXPECTED_HOME=$root/output/home
export QUALIFICATION_EXPECTED_DATA=$root/data
export QUALIFICATION_EXPECTED_CACHE=$root/cache
export QUALIFICATION_EXPECTED_SNV_BUNDLE=$root/data/pangopup/bundles/qualified/bundle
export QUALIFICATION_LOOKUP_LOG=$root/lookups.log
"$repo/scripts/run-production-qualification.sh" \
  "$root/bin/pangopup" "$repo" "$root/data" "$root/cache" "$root/output"
"$repo/scripts/check-production-qualification.py" "$root/output" "$repo" >"$root/check.out"
grep -Fxq 'production qualification passed' "$root/check.out"
[[ $(grep -Fc $'snv\t' "$root/lookups.log") == 7 ]]
[[ $(grep -Fxc $'snv\t'"$QUALIFICATION_EXPECTED_SNV_BUNDLE" "$root/lookups.log") == 7 ]]
[[ $(grep -Fxc 'model' "$root/lookups.log") == 2 ]]

reuse_output=$root/reuse-output
QUALIFICATION_EXPECTED_HOME=$reuse_output/home \
  "$repo/scripts/run-production-qualification.sh" \
    "$root/bin/pangopup" "$repo" "$root/data" "$root/cache" "$reuse_output" \
    --reuse-installed
"$repo/scripts/check-production-qualification.py" \
  "$reuse_output" "$repo" --reuse-installed >"$root/reuse-check.out"
grep -Fxq 'production qualification passed' "$root/reuse-check.out"

for layout in zero multiple symlink unsafe; do
  bad=$root/bad-$layout
  if QUALIFICATION_SNV_LAYOUT=$layout \
    QUALIFICATION_EXPECTED_HOME=$bad/output/home \
    QUALIFICATION_EXPECTED_DATA=$bad/data \
    QUALIFICATION_EXPECTED_CACHE=$bad/cache \
    QUALIFICATION_EXPECTED_SNV_BUNDLE=$bad/data/pangopup/bundles/qualified/bundle \
    QUALIFICATION_LOOKUP_LOG=$bad/lookups.log \
    "$repo/scripts/run-production-qualification.sh" \
      "$root/bin/pangopup" "$repo" "$bad/data" "$bad/cache" "$bad/output" \
      >"$root/$layout.out" 2>"$root/$layout.err"; then
    printf 'runner accepted %s installed SNV layout\n' "$layout" >&2
    exit 1
  fi
done
grep -Fxq 'expected exactly one installed SNV bundle, found 0' "$root/zero.err"
grep -Fxq 'expected exactly one installed SNV bundle, found 2' "$root/multiple.err"
grep -Fxq 'installed SNV bundle is unsafe' "$root/symlink.err"
grep -Fxq 'installed SNV bundle is unsafe' "$root/unsafe.err"

if "$repo/scripts/run-production-qualification.sh" \
  "$root/bin/pangopup" "$repo" "$root/data" "$root/cache" "$root/second-output" \
  >"$root/fresh.out" 2>"$root/fresh.err"; then
  printf 'runner accepted existing XDG directories\n' >&2
  exit 1
fi
grep -Fxq 'qualification directories must be absent' "$root/fresh.err"

cp -a "$root/output" "$root/reused-online-output"
sed -i 's/"installed"/"reused"/g' "$root/reused-online-output/sync-online.json"
if "$repo/scripts/check-production-qualification.py" "$root/reused-online-output" "$repo" >"$root/reused.out" 2>"$root/reused.err"; then
  printf 'checker accepted reused first online sync\n' >&2
  exit 1
fi
grep -Fxq 'unexpected snv state: sync-online.json' "$root/reused.err"

cp -a "$root/output" "$root/format-output"
sed -i '1s/,/, /' "$root/format-output/snv-ENSG00000010610.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/format-output" "$repo" >"$root/format.out" 2>"$root/format.err"; then
  printf 'checker accepted formatting drift\n' >&2
  exit 1
fi
grep -Fxq 'SNV oracle mismatch: ENSG00000010610' "$root/format.err"

cp -a "$root/output" "$root/progress-output"
sed -i 's/(10 downloaded, 0 resumed)/(9 downloaded, 0 resumed)/' \
  "$root/progress-output/sync-online.progress"
if "$repo/scripts/check-production-qualification.py" "$root/progress-output" "$repo" >"$root/progress.out" 2>"$root/progress.err"; then
  printf 'checker accepted progress/final mismatch\n' >&2
  exit 1
fi
grep -Fxq 'online sync progress totals do not match final JSON' "$root/progress.err"

cp -a "$root/output" "$root/progress-decrease-output"
sed -i '0,/(10 downloaded, 0 resumed)/s//(11 downloaded, 0 resumed)/' \
  "$root/progress-decrease-output/sync-online.progress"
if "$repo/scripts/check-production-qualification.py" "$root/progress-decrease-output" "$repo" >"$root/progress-decrease.out" 2>"$root/progress-decrease.err"; then
  printf 'checker accepted transfer counters above completion\n' >&2
  exit 1
fi
grep -Fxq 'online sync progress counters decreased' "$root/progress-decrease.err"

cp -a "$root/output" "$root/progress-duplicate-output"
tail -1 "$root/progress-duplicate-output/sync-online.progress" \
  >>"$root/progress-duplicate-output/sync-online.progress"
if "$repo/scripts/check-production-qualification.py" "$root/progress-duplicate-output" "$repo" >"$root/progress-duplicate.out" 2>"$root/progress-duplicate.err"; then
  printf 'checker accepted duplicate completion records\n' >&2
  exit 1
fi
grep -Fxq 'online sync progress lacks transfer or completion evidence' \
  "$root/progress-duplicate.err"

cp -a "$root/output" "$root/model-only-output"
sed -i 's/"gain_score":"0.00"/"gain_score":"0.01"/' \
  "$root/model-only-output/model-only-SNV.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/model-only-output" "$repo" >"$root/model-only.out" 2>"$root/model-only.err"; then
  printf 'checker accepted changed model-only result\n' >&2
  exit 1
fi
grep -Fxq 'model-only SNV oracle mismatch' "$root/model-only.err"

cp -a "$root/output" "$root/http-output"
sed -i 's/"version":"0.4.0"/"version":"9.9.9"/' "$root/http-output/http-status.txt"
if "$repo/scripts/check-production-qualification.py" "$root/http-output" "$repo" >"$root/http.out" 2>"$root/http.err"; then
  printf 'checker accepted changed HTTP status version\n' >&2
  exit 1
fi
grep -Fxq 'HTTP status response mismatch' "$root/http.err"

cp -a "$root/output" "$root/http-model-only-output"
sed -i 's/"gain_score":"0.00"/"gain_score":"0.01"/' \
  "$root/http-model-only-output/http-model-only.txt"
if "$repo/scripts/check-production-qualification.py" "$root/http-model-only-output" "$repo" >"$root/http-model-only.out" 2>"$root/http-model-only.err"; then
  printf 'checker accepted changed HTTP model-only SNV\n' >&2
  exit 1
fi
grep -Fxq 'HTTP model-only SNV response mismatch' "$root/http-model-only.err"

cp -a "$root/output" "$root/truncated-output"
sed -i '$d' "$root/truncated-output/snv-unfiltered.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/truncated-output" "$repo" >"$root/truncated.out" 2>"$root/truncated.err"; then
  printf 'checker accepted truncated output\n' >&2
  exit 1
fi
grep -Fxq 'SNV oracle mismatch: unfiltered' "$root/truncated.err"

sed -i '0,/"gain_score":"0.00"/s//"gain_score":"0.99"/' "$root/output/snv-ENSG00000010610.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/output" "$repo" >"$root/tamper.out" 2>"$root/tamper.err"; then
  printf 'checker accepted changed score\n' >&2
  exit 1
fi
grep -Fxq 'SNV oracle mismatch: ENSG00000010610' "$root/tamper.err"

fixture=$root/substituted-source/tests/fixtures
install -d -m 700 "$fixture/snv-regression/expected" "$fixture/executable-release"
cp "$repo/tests/fixtures/snv-regression/requests.tsv" "$fixture/snv-regression/requests.tsv"
cp "$repo/tests/fixtures/snv-regression/expected/"*.jsonl "$fixture/snv-regression/expected/"
cp "$repo/tests/fixtures/executable-release/m09.jsonl" "$fixture/executable-release/m09.jsonl"
cp "$repo/tests/fixtures/executable-release/model-only-snv.jsonl" \
  "$fixture/executable-release/model-only-snv.jsonl"
cp "$fixture/snv-regression/expected/ENSG00000169129.jsonl" "$fixture/snv-regression/expected/unfiltered.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/format-output" "$root/substituted-source" >"$root/substitution.out" 2>"$root/substitution.err"; then
  printf 'checker accepted substituted oracle\n' >&2
  exit 1
fi
grep -Fxq 'expected oracle identity mismatch: unfiltered' "$root/substitution.err"

cp "$repo/tests/fixtures/snv-regression/requests.tsv" "$fixture/snv-regression/requests.tsv"
sed -i '$d' "$fixture/snv-regression/requests.tsv"
if "$repo/scripts/check-production-qualification.py" "$root/format-output" "$root/substituted-source" >"$root/request.out" 2>"$root/request.err"; then
  printf 'checker accepted truncated requests\n' >&2
  exit 1
fi
grep -Fxq 'request fixture identity mismatch' "$root/request.err"

cp "$repo/tests/fixtures/snv-regression/requests.tsv" "$fixture/snv-regression/requests.tsv"
cp "$repo/tests/fixtures/snv-regression/expected/unfiltered.jsonl" "$fixture/snv-regression/expected/unfiltered.jsonl"
sed -i 's/"gain_score":"0.00"/"gain_score":"0.01"/' "$fixture/executable-release/m09.jsonl"
if "$repo/scripts/check-production-qualification.py" "$root/format-output" "$root/substituted-source" >"$root/model-substitution.out" 2>"$root/model-substitution.err"; then
  printf 'checker accepted substituted model oracle\n' >&2
  exit 1
fi
grep -Fxq 'model oracle identity mismatch: M09-insertion-short-plus' "$root/model-substitution.err"

awk '
  $0 == "<!-- BEGIN TICKET 038 COORDINATOR SCRIPT -->" { inside=1; next }
  $0 == "<!-- END TICKET 038 COORDINATOR SCRIPT -->" { inside=0; found=1; next }
  inside && $0 != "```bash" && $0 != "```" { print }
  END { if (!found) exit 1 }
' "$repo/planning/artifacts/038-public-linux-release.md" >"$root/coordinator-runbook.sh"
bash -n "$root/coordinator-runbook.sh"

awk '
  $0 == "<!-- BEGIN TICKET 050 COORDINATOR SCRIPT -->" { inside=1; next }
  $0 == "<!-- END TICKET 050 COORDINATOR SCRIPT -->" { inside=0; found=1; next }
  inside && $0 != "```bash" && $0 != "```" { print }
  END { if (!found) exit 1 }
' "$repo/planning/artifacts/050-public-linux-release.md" >"$root/ticket050-runbook.sh"
bash -n "$root/ticket050-runbook.sh"
grep -Fq 'readonly TAG=v0.2.0' "$root/ticket050-runbook.sh"
grep -Fq '"$SOURCE_TREE/scripts/qualify-linux-release.sh" "$PRIVATE/release" "$VERSION" "$COMMIT"' "$root/ticket050-runbook.sh"
[[ $(grep -Fc 'run-production-qualification.sh' "$root/ticket050-runbook.sh") == 3 ]]
[[ $(grep -Fc -- '--reuse-installed' "$root/ticket050-runbook.sh") == 2 ]]
grep -Fq 'gh api --method PATCH "repos/$REPO/releases/$RELEASE_ID" -F draft=false' "$root/ticket050-runbook.sh"
grep -Fq 'PUBLISHED=1' "$root/ticket050-runbook.sh"
grep -Fq 'https://raw.githubusercontent.com/genomoncology/pangopup/v0.2.0/install.sh' "$root/ticket050-runbook.sh"
grep -Fq 'repos/$REPO/actions/workflows/package-linux.yml' "$root/ticket050-runbook.sh"
grep -Fq 'ARTIFACT_ID=$(jq -r .artifacts[0].id "$PRIVATE/artifacts.json")' "$root/ticket050-runbook.sh"
grep -Fq 'exec {UPLOAD_FD}<"$source_path"' "$root/ticket050-runbook.sh"
grep -Fq 'sha256sum "/proc/self/fd/$UPLOAD_FD"' "$root/ticket050-runbook.sh"
grep -Fq 'https://github.com/$REPO/releases/download/$TAG/$name' "$root/ticket050-runbook.sh"
[[ $(grep -Fc 'curl -q -fsSL' "$root/ticket050-runbook.sh") == 5 ]]
! grep -Fq 'curl -fsSL' "$root/ticket050-runbook.sh"
! grep -Eq 'GH_TOKEN=|GITHUB_TOKEN=|Authorization:' "$repo/planning/artifacts/050-public-linux-release.md"

assert_source_authority() {
  local runbook=$1
  grep -Fxq 'readonly CHECKOUT=$PWD' "$runbook" || return 1
  grep -Fxq 'test -z "$(git -C "$CHECKOUT" replace -l)"' "$runbook" || return 1
  grep -Fxq 'GIT_NO_REPLACE_OBJECTS=1 git -C "$CHECKOUT" cat-file -e "$COMMIT^{commit}"' "$runbook" || return 1
  grep -Fxq 'GIT_NO_REPLACE_OBJECTS=1 git -C "$CHECKOUT" archive --format=tar "$COMMIT" \' "$runbook" || return 1
  [[ $(grep -Fc 'GIT_NO_REPLACE_OBJECTS=1' "$runbook") == 2 ]] || return 1
  grep -Fxq 'readonly SOURCE_TREE=$PRIVATE/source' "$runbook" || return 1
  grep -Fxq 'readonly BODY=$SOURCE_TREE/planning/artifacts/050-release-notes.md' "$runbook" || return 1
  grep -Fq '"$SOURCE_TREE/scripts/qualify-linux-release.sh"' "$runbook" || return 1
  [[ $(grep -Fc '"$SOURCE_TREE/scripts/check-production-qualification.py"' "$runbook") == 2 ]] || return 1
  ! grep -Eq '^scripts/(qualify-linux-release\.sh|check-production-qualification\.py)' "$runbook" || return 1
}
assert_source_authority "$root/ticket050-runbook.sh"

cp "$root/ticket050-runbook.sh" "$root/mutable-source-runbook.sh"
sed -i 's|readonly SOURCE_TREE=\$PRIVATE/source|readonly SOURCE_TREE=$CHECKOUT|' \
  "$root/mutable-source-runbook.sh"
if assert_source_authority "$root/mutable-source-runbook.sh"; then
  printf 'source authority accepted mutable checkout\n' >&2
  exit 1
fi

cp "$root/ticket050-runbook.sh" "$root/replace-enabled-runbook.sh"
sed -i 's/GIT_NO_REPLACE_OBJECTS=1 git -C "$CHECKOUT" archive/git -C "$CHECKOUT" archive/' \
  "$root/replace-enabled-runbook.sh"
if assert_source_authority "$root/replace-enabled-runbook.sh"; then
  printf 'source authority accepted replacement-enabled archive\n' >&2
  exit 1
fi

assert_ownership_contract() {
  local runbook=$1
  grep -Fxq 'readonly HOST_UID=$(id -u)' "$runbook" || return 1
  grep -Fxq 'readonly HOST_GID=$(id -g)' "$runbook" || return 1
  [[ $(grep -Fxc -- '  --user "$HOST_UID:$HOST_GID" \' "$runbook") == 1 ]] || return 1
  [[ $(grep -Fxc -- '  --env "HOST_UID=$HOST_UID" --env "HOST_GID=$HOST_GID" \' "$runbook") == 1 ]] || return 1
  [[ $(grep -Fc '/usr/bin/setpriv --reuid="$HOST_UID" --regid="$HOST_GID" --clear-groups' "$runbook") == 4 ]] || return 1
  [[ $(grep -Fc 'env -i HOME=/qualification/home XDG_DATA_HOME=/qualification/data XDG_CACHE_HOME=/qualification/cache PATH=/usr/bin:/bin' "$runbook") == 4 ]] || return 1
  grep -Fxq '    chown "$HOST_UID:$HOST_GID" /qualification/install /qualification/home /qualification/post' "$runbook" || return 1
  ! grep -Eq 'chown[[:space:]].*/qualification/(data|cache)|chown[[:space:]]+-R' "$runbook" || return 1
}
assert_ownership_contract "$root/coordinator-runbook.sh"

cp "$root/coordinator-runbook.sh" "$root/missing-user-runbook.sh"
sed -i '0,/--user "\$HOST_UID:\$HOST_GID"/s///' "$root/missing-user-runbook.sh"
if assert_ownership_contract "$root/missing-user-runbook.sh"; then
  printf 'ownership contract accepted missing prepublication user mapping\n' >&2
  exit 1
fi

cp "$root/coordinator-runbook.sh" "$root/drifted-setpriv-runbook.sh"
sed -i '0,/--clear-groups/s//--keep-groups/' "$root/drifted-setpriv-runbook.sh"
if assert_ownership_contract "$root/drifted-setpriv-runbook.sh"; then
  printf 'ownership contract accepted drifted privilege drop\n' >&2
  exit 1
fi

cp "$root/coordinator-runbook.sh" "$root/unsafe-chown-runbook.sh"
printf '\nchown -R "$HOST_UID:$HOST_GID" /qualification/data /qualification/cache\n' >>"$root/unsafe-chown-runbook.sh"
if assert_ownership_contract "$root/unsafe-chown-runbook.sh"; then
  printf 'ownership contract accepted qualified-data chown\n' >&2
  exit 1
fi

image=ubuntu@sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90
[[ "$(grep -Fc "$image" "$repo/planning/artifacts/038-public-linux-release.md")" == 4 ]]
grep -Fq 'build runner: GitHub-hosted Ubuntu 24.04' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'admitted maximum imported GLIBC version: `2.39`' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'package run `30648307402` failed while linking' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Package run `30651619497` passed the full gate' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Package run `30652858960` then redundantly reran the full' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Package run `30653836700` built the release executables' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'exactly one corrected dispatch is permitted' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'State: **COMPLETE — immutable public release `v0.1.0` targets reviewed commit' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Target `ci` run ID/URL: `30657770808`' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Workflow run ID/URL: `30657987617`' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Draft/release ID: `363278563`' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'checksum-verifying tagged installer are shipped' "$repo/AGENTS.md"
! grep -Fq 'public executable publication remains a separate ticket' "$repo/AGENTS.md"
grep -Fq 'passes that exact bundle path explicitly to each of the seven ordered' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'M09 model request deliberately has no `--bundle`' "$repo/planning/artifacts/038-public-linux-release.md"

printf 'production release qualification tests passed\n'
