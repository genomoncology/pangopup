#!/bin/sh
set -eu

expected_branch=qualification/ticket-048-cpuinfo-probe
public_repository=https://github.com/genomoncology/pangopup.git
onnxruntime_commit=da9b5e364c465de65c49d91e696cd6485270757f
onnxruntime_source_sha256=9616cbdbbfcb1420b3261cd280a047d74ab0a249825e577b0e2dd310e22f6b83
cpuinfo_baseline_commit=4628dc060ce4e82345dc166bbac875609db4ff69
cpuinfo_baseline_sha256=2ed3ebc6c2656cc0aafc7af319e5cb0f97cc9b415eae180f566def84f1ca6a29
cpuinfo_patched_commit=0c0ab15cb0a8bafbbf71c2ae6f84128a4c2a8da6
cpuinfo_patched_sha256=d40ed2e6134c6b8103ac7916de080a67964f48a8f61c5e72ecca18e9c492f884
expected_warning='onnxruntime cpuid_info warning: Unknown CPU vendor. cpuinfo_vendor value: 0'
evidence_dir=${1:?usage: run-mac-probe.sh ABSOLUTE-EVIDENCE-DIRECTORY}

case "$evidence_dir" in /*) ;; *) echo "evidence directory must be absolute" >&2; exit 64 ;; esac
test ! -e "$evidence_dir"
mkdir -m 700 "$evidence_dir"

root=$(git rev-parse --show-toplevel)
cd "$root"
test "$(git branch --show-current)" = "$expected_branch"
test -z "$(git status --short)"
revision=$(git rev-parse HEAD)
printf '%s\n' "$revision" | grep -Eq '^[0-9a-f]{40}$'
printf '%s\n' "$revision" >"${evidence_dir}/revision.txt"
git ls-remote "$public_repository" "refs/heads/${expected_branch}" >"${evidence_dir}/live-remote-ref.txt"
awk 'END { exit NR == 1 ? 0 : 1 }' "${evidence_dir}/live-remote-ref.txt"
read -r live_revision live_ref <"${evidence_dir}/live-remote-ref.txt"
test "$live_ref" = "refs/heads/${expected_branch}"
git fetch --no-tags "$public_repository" "refs/heads/${expected_branch}"
fetched_revision=$(git rev-parse FETCH_HEAD)
test "$revision" = "$live_revision"
test "$revision" = "$fetched_revision"
test "$(docker info --format '{{.OSType}}/{{.Architecture}}')" = linux/aarch64
maintainers/ticket-048/check_probe.py

build_probe() {
    variant=$1
    image="pangopup:ticket-048-cpuinfo-${variant}"
    docker build --pull --platform linux/arm64 \
        --file maintainers/ticket-048/Dockerfile \
        --target probe \
        --build-arg "CPUINFO_VARIANT=${variant}" \
        --build-arg "PANGOPUP_REVISION=${revision}" \
        --tag "$image" . \
        >"${evidence_dir}/${variant}-build.txt" 2>&1
    docker image inspect "$image" >"${evidence_dir}/${variant}-image-inspect.json"
    test "$(docker image inspect --format '{{.Os}}/{{.Architecture}}' "$image")" = linux/arm64
    test "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.revision"}}' "$image")" = "$revision"
    test "$(docker image inspect --format '{{index .Config.Labels "org.pangopup.probe.cpuinfo-variant"}}' "$image")" = "$variant"
    test "$(docker image inspect --format '{{index .Config.Labels "org.pangopup.probe.onnxruntime-commit"}}' "$image")" = "$onnxruntime_commit"
    test "$(docker image inspect --format '{{index .Config.Labels "org.pangopup.probe.onnxruntime-source-sha256"}}' "$image")" = "$onnxruntime_source_sha256"
    test "$(docker image inspect --format '{{index .Config.Labels "org.pangopup.probe.cpuinfo-baseline-commit"}}' "$image")" = "$cpuinfo_baseline_commit"
    test "$(docker image inspect --format '{{index .Config.Labels "org.pangopup.probe.cpuinfo-baseline-sha256"}}' "$image")" = "$cpuinfo_baseline_sha256"
    test "$(docker image inspect --format '{{index .Config.Labels "org.pangopup.probe.cpuinfo-patched-commit"}}' "$image")" = "$cpuinfo_patched_commit"
    test "$(docker image inspect --format '{{index .Config.Labels "org.pangopup.probe.cpuinfo-patched-sha256"}}' "$image")" = "$cpuinfo_patched_sha256"
    docker image inspect --format '{{.Id}}' "$image" >"${evidence_dir}/${variant}-image-id.txt"
    evidence_container=$(docker create "$image")
    docker cp "${evidence_container}:/usr/share/doc/pangopup/ticket-048-build-versions.txt" "${evidence_dir}/${variant}-build-versions.txt"
    docker cp "${evidence_container}:/usr/share/doc/pangopup/ticket-048-ort-features.txt" "${evidence_dir}/${variant}-ort-features.txt"
    docker cp "${evidence_container}:/usr/share/doc/pangopup/ticket-048-ort-static-manifest.sha256" "${evidence_dir}/${variant}-ort-static-manifest.sha256"
    docker cp "${evidence_container}:/usr/share/doc/pangopup/ticket-048-cpuinfo-library.sha256" "${evidence_dir}/${variant}-cpuinfo-library.sha256"
    docker cp "${evidence_container}:/usr/share/doc/pangopup/ticket-048-cpuinfo-source.txt" "${evidence_dir}/${variant}-cpuinfo-source.txt"
    docker rm "$evidence_container" > /dev/null
    case "$variant" in
        baseline) expected_cpuinfo_commit=$cpuinfo_baseline_commit; expected_cpuinfo_sha256=$cpuinfo_baseline_sha256 ;;
        patched) expected_cpuinfo_commit=$cpuinfo_patched_commit; expected_cpuinfo_sha256=$cpuinfo_patched_sha256 ;;
    esac
    grep -Fx "commit=${expected_cpuinfo_commit}" "${evidence_dir}/${variant}-cpuinfo-source.txt"
    grep -Fx "archive_sha256=${expected_cpuinfo_sha256}" "${evidence_dir}/${variant}-cpuinfo-source.txt"
    set +e
    docker run --rm "$image" --version \
        >"${evidence_dir}/${variant}-version.stdout" \
        2>"${evidence_dir}/${variant}-version.stderr"
    version_exit=$?
    set -e
    printf '%s\n' "$version_exit" >"${evidence_dir}/${variant}-version.exit"
    test "$version_exit" = 0
    printf 'pangopup 0.1.0\n' >"${evidence_dir}/expected-version.stdout"
    cmp "${evidence_dir}/expected-version.stdout" "${evidence_dir}/${variant}-version.stdout"
}

build_probe baseline
printf '%s\n' "$expected_warning" >"${evidence_dir}/expected-baseline.stderr"
if ! cmp "${evidence_dir}/expected-baseline.stderr" "${evidence_dir}/baseline-version.stderr"; then
    printf '%s\n' "inconclusive" >"${evidence_dir}/outcome.txt"
    exit 1
fi

build_probe patched
if cmp -s "${evidence_dir}/baseline-cpuinfo-library.sha256" "${evidence_dir}/patched-cpuinfo-library.sha256"; then
    printf '%s\n' "invalid-identical-cpuinfo-libraries" >"${evidence_dir}/outcome.txt"
    exit 1
fi
if test -s "${evidence_dir}/patched-version.stderr"; then
    printf '%s\n' "rejected" >"${evidence_dir}/outcome.txt"
    exit 1
fi

printf '%s\n' "confirmed" >"${evidence_dir}/outcome.txt"
printf 'Ticket 048 confirmed the cpuinfo-pin hypothesis. Raw evidence: %s\n' "$evidence_dir"
