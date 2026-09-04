#!/usr/bin/env bash
set -euo pipefail

log="$RUNNER_TEMP/make-test.log"
set +e
make test 2>&1 | tee "$log"
statuses=("${PIPESTATUS[@]}")
set -e
make_status=${statuses[0]}
tee_status=${statuses[1]}
if [[ "$make_status" == 0 && "$tee_status" == 0 ]]; then
    exit 0
fi
# GitHub retains the first 4,096 annotation characters. Each raw byte can
# expand to three characters below, so keep a final 1,300-byte tail.
if [[ -f "$log" ]] && summary=$(tail -n 120 "$log" | tail -c 1300); then
    :
else
    summary='Linux test log is unavailable'
fi
summary=${summary//'%'/'%25'}
summary=${summary//$'\r'/'%0D'}
summary=${summary//$'\n'/'%0A'}
printf '::error file=Makefile,line=30,title=Linux make test failure::%s\n' "$summary" || true
if [[ "$make_status" != 0 ]]; then
    exit "$make_status"
fi
exit "$tee_status"
