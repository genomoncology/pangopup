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
case "$command" in
  sync)
    if [[ " $* " == *' --offline '* ]]; then
      printf '%s\n' '{"status":"ready","snv":{"status":"reused"},"runtime":{"status":"reused"}}'
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
      printf '%s\n' '{"status":"ready","snv":{"status":"installed"},"runtime":{"status":"installed"}}'
    fi
    ;;
  status)
    printf '%s\n' '{"status":"ready","snv":{"status":"ready"},"runtime":{"status":"ready"}}'
    ;;
  lookup)
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
  *) exit 2 ;;
esac
SH
chmod 755 "$root/bin/pangopup"

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
[[ $(grep -Fxc 'model' "$root/lookups.log") == 1 ]]

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
image=ubuntu@sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90
[[ "$(grep -Fc "$image" "$repo/planning/artifacts/038-public-linux-release.md")" == 3 ]]
grep -Fq 'build runner: GitHub-hosted Ubuntu 24.04' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'admitted maximum imported GLIBC version: `2.39`' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'package run `30648307402` failed while linking' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Package run `30651619497` passed the full gate' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Package run `30652858960` then redundantly reran the full' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'Package run `30653836700` built the release executables' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'exactly one corrected dispatch is permitted' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'SNV-oracle-corrected audit timestamp: `<PENDING_UTC>`' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'passes that exact bundle path explicitly to each of the seven ordered' "$repo/planning/artifacts/038-public-linux-release.md"
grep -Fq 'M09 model request deliberately has no `--bundle`' "$repo/planning/artifacts/038-public-linux-release.md"

printf 'production release qualification tests passed\n'
