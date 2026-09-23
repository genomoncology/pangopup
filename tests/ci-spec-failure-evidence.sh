#!/usr/bin/env bash
set -euo pipefail

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
wrapper="$repository/scripts/run-macos-spec-with-public-failure.sh"
workflow="$repository/.github/workflows/ci.yml"
[[ -x "$wrapper" ]]
[[ "$(sed -n '115p' "$repository/Makefile")" == spec:* ]]
macos_steps=$(sed -n '/^  macos:$/,$p' "$workflow")
[[ "$macos_steps" == *'run: scripts/run-macos-spec-with-public-failure.sh'* ]]
[[ "$macos_steps" != *'run: make spec'* ]]

fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
mkdir "$fixture/bin"
cat >"$fixture/bin/make" <<'SH'
#!/usr/bin/env bash
[[ "${1-}" == spec ]]
printf '%s' "${MAKE_OUTPUT-}"
exit "${MAKE_STATUS:?}"
SH
chmod +x "$fixture/bin/make"

run_wrapper() {
    local expected=$1
    local result
    local status
    set +e
    result=$(PATH="$fixture/bin:$PATH" RUNNER_TEMP="$fixture" MAKE_OUTPUT="$2" MAKE_STATUS="$3" bash "$wrapper" 2>&1)
    status=$?
    set -e
    [[ "$status" == "$expected" ]] || {
        printf 'spec wrapper returned %s instead of %s\n%s\n' "$status" "$expected" "$result" >&2
        exit 1
    }
    WRAPPER_OUTPUT=$result
}

run_wrapper 0 'passing spec' 0
[[ "$WRAPPER_OUTPUT" == 'passing spec' ]]
run_wrapper 2 $'failed example%\r\nfailed assertion\n' 2
[[ "$WRAPPER_OUTPUT" == *'::error file=Makefile,line=115,title=macOS make spec failure::failed example%25%0D%0Afailed assertion' ]]
