#!/usr/bin/env bash
set -euo pipefail

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
wrapper="$repository/scripts/run-linux-tests-with-public-failure.sh"
[[ -x "$wrapper" ]]
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
fake_bin="$fixture/bin"
mkdir "$fake_bin"
system_tee=$(command -v tee)

cat >"$fake_bin/make" <<'SH'
#!/usr/bin/env bash
[[ "${1-}" == test ]]
printf '%s' "${MAKE_OUTPUT-}"
printf x >>"${MAKE_CALLS:?}"
exit "${MAKE_STATUS:?}"
SH

cat >"$fake_bin/tee" <<SH
#!/usr/bin/env bash
"$system_tee" "\$@"
exit "\${TEE_STATUS:?}"
SH
chmod +x "$fake_bin/make" "$fake_bin/tee"

run_wrapper() {
    local expected=$1
    local make_status=$2
    local tee_status=$3
    local content=$4
    local run_root
    local calls
    local status
    run_root=$(mktemp -d "$fixture/run.XXXXXX")
    calls="$run_root/make-calls"
    set +e
    WRAPPER_OUTPUT=$(PATH="$fake_bin:$PATH" RUNNER_TEMP="$run_root" MAKE_STATUS="$make_status" TEE_STATUS="$tee_status" MAKE_OUTPUT="$content" MAKE_CALLS="$calls" bash "$wrapper" 2>&1)
    status=$?
    set -e
    [[ "$(wc -c <"$calls" | tr -d ' ')" == 1 ]]
    if [[ "$status" != "$expected" ]]; then
        printf 'wrapper returned %s instead of %s\n%s\n' "$status" "$expected" "$WRAPPER_OUTPUT" >&2
        exit 1
    fi
}

run_wrapper 0 0 0 'success'
[[ "$WRAPPER_OUTPUT" == success ]]

run_wrapper 2 2 0 $'percent%\r\nline two\n'
annotation=${WRAPPER_OUTPUT##*$'\n'}
[[ "$annotation" == '::error file=Makefile,line=30,title=Linux make test failure::percent%25%0D%0Aline two' ]]

run_wrapper 7 0 7 'tee failed'
[[ "$WRAPPER_OUTPUT" == *'::error file=Makefile,line=30,title=Linux make test failure::tee failed' ]]

run_wrapper 2 2 7 'make wins'

bounded='discarded-line'
for index in $(seq 2 130); do
    printf -v line 'line-%03d-%0200d' "$index" 0
    bounded+=$'\n'"$line"
done
run_wrapper 2 2 0 "$bounded"
annotation=${WRAPPER_OUTPUT##*$'\n'}
summary=${annotation#*::error file=Makefile,line=30,title=Linux make test failure::}
decoded=${summary//'%0A'/$'\n'}
[[ "$decoded" != *discarded-line* ]]
[[ "$decoded" == *line-130-* ]]
[[ "$(printf '%s' "$decoded" | wc -c | tr -d ' ')" -le 16000 ]]
[[ "$(printf '%s' "$decoded" | awk 'END { print NR }')" -le 120 ]]
