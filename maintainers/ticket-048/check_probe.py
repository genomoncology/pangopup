#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""Cheap fail-closed checks for the disposable Ticket 048 probe."""

from __future__ import annotations

import argparse
from pathlib import Path


REQUIRED_DOCKER_TEXT = (
    "da9b5e364c465de65c49d91e696cd6485270757f",
    "9616cbdbbfcb1420b3261cd280a047d74ab0a249825e577b0e2dd310e22f6b83",
    "4628dc060ce4e82345dc166bbac875609db4ff69",
    "e58d4b47c16a982111c897e669ae4f1821a393d7",
    "2ed3ebc6c2656cc0aafc7af319e5cb0f97cc9b415eae180f566def84f1ca6a29",
    "0c0ab15cb0a8bafbbf71c2ae6f84128a4c2a8da6",
    "44419f8b0fda75bb0d2fbe3dd0629493c98ad905",
    "d40ed2e6134c6b8103ac7916de080a67964f48a8f61c5e72ecca18e9c492f884",
    "onnxruntime_BUILD_SHARED_LIB=OFF",
    "onnxruntime_ENABLE_CPUINFO=ON",
    "git ls-remote https://github.com/microsoft/onnxruntime.git refs/tags/v1.28.0",
    "FETCHCONTENT_SOURCE_DIR_PYTORCH_CPUINFO=/opt/cpuinfo",
    ".pangopup-source-identity",
    "ORT_LIB_LOCATION=/opt/onnxruntime/build/Linux",
    "! grep -F 'download-binaries' /tmp/ort-features.txt",
    "pytorch_cpuinfo-build/libcpuinfo.a",
    "grep -F 'cpuinfo_initialize'",
    "/opt/onnxruntime/ThirdPartyNotices.txt",
    "org.opencontainers.image.revision=\"${PANGOPUP_REVISION}\"",
)

REQUIRED_RUNNER_TEXT = (
    "https://github.com/genomoncology/pangopup.git",
    'git ls-remote "$public_repository" "refs/heads/${expected_branch}"',
    'git fetch --no-tags "$public_repository" "refs/heads/${expected_branch}"',
    "awk 'END { exit NR == 1 ? 0 : 1 }'",
    'printf \'%s\\n\' "$revision" >"${evidence_dir}/revision.txt"',
    'org.pangopup.probe.cpuinfo-variant',
    'org.pangopup.probe.onnxruntime-commit',
    'org.pangopup.probe.onnxruntime-source-sha256',
    'org.pangopup.probe.cpuinfo-baseline-commit',
    'org.pangopup.probe.cpuinfo-patched-commit',
    'baseline-cpuinfo-library.sha256',
    'patched-cpuinfo-library.sha256',
    'invalid-identical-cpuinfo-libraries',
)

FORBIDDEN_RUNNER_TEXT = ("wc -l",)

FORBIDDEN_DOCKER_TEXT = (
    "2>/dev/null",
    "RUST_LOG=off",
    "ORT_LOG",
    "cpuinfo_vendor value",
    "CPU implementer",
)


def check(root: Path) -> list[str]:
    errors: list[str] = []
    probe = root / "maintainers/ticket-048/Dockerfile"
    patch = root / "maintainers/ticket-048/expected-cpuinfo.patch"
    model_manifest = root / "crates/pangopup-model/Cargo.toml"
    production_docker = root / "Dockerfile"
    runner = root / "maintainers/ticket-048/run-mac-probe.sh"

    for path in (probe, patch, model_manifest, production_docker, runner):
        if not path.is_file():
            errors.append(f"missing {path.relative_to(root)}")
    if errors:
        return errors

    probe_text = probe.read_text(encoding="utf-8")
    patch_text = patch.read_text(encoding="utf-8")
    model_text = model_manifest.read_text(encoding="utf-8")
    production_text = production_docker.read_text(encoding="utf-8")
    runner_text = runner.read_text(encoding="utf-8")

    for expected in REQUIRED_DOCKER_TEXT:
        if expected not in probe_text:
            errors.append(f"probe Dockerfile lost required evidence: {expected}")
    for forbidden in FORBIDDEN_DOCKER_TEXT:
        if forbidden in probe_text:
            errors.append(f"probe Dockerfile contains forbidden suppression/spoofing: {forbidden}")
    for expected in REQUIRED_RUNNER_TEXT:
        if expected not in runner_text:
            errors.append(f"Mac runner lost required authentication/evidence: {expected}")
    for forbidden in FORBIDDEN_RUNNER_TEXT:
        if forbidden in runner_text:
            errors.append(f"Mac runner uses nonportable structural parsing: {forbidden}")

    removed = "-pytorch_cpuinfo;https://github.com/pytorch/cpuinfo/archive/4628dc060ce4e82345dc166bbac875609db4ff69.zip;e58d4b47c16a982111c897e669ae4f1821a393d7"
    added = "+pytorch_cpuinfo;https://github.com/pytorch/cpuinfo/archive/0c0ab15cb0a8bafbbf71c2ae6f84128a4c2a8da6.zip;44419f8b0fda75bb0d2fbe3dd0629493c98ad905"
    if patch_text.count(removed) != 1 or patch_text.count(added) != 1:
        errors.append("expected patch is not the one cpuinfo dependency replacement")
    changed_lines = [line for line in patch_text.splitlines() if line.startswith(("+", "-")) and not line.startswith(("+++", "---"))]
    if changed_lines != [removed, added]:
        errors.append("expected patch changes fields beyond the reviewed cpuinfo line")

    for expected in ('=2.0.0-rc.12', '"api-24"', '"download-binaries"'):
        if expected not in model_text:
            errors.append(f"accepted production model dependency changed: missing {expected}")
    for candidate in ('=2.0.0-rc.13', '"api-27"'):
        if candidate in model_text:
            errors.append(f"temporary candidate leaked into production manifest: {candidate}")

    for digest in (
        "ecbe59a8408895edd02d9ef422504b8501dd9fa1526de27a45b73406d734d659",
        "d97bc0a941b8d4be647dc0ee75b264ddbb772f1ac5ba690a4309c00723b23775",
    ):
        if digest not in production_text or digest not in probe_text:
            errors.append(f"probe and production do not share pinned base digest {digest}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    errors = check(args.root.resolve())
    for error in errors:
        print(error)
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
