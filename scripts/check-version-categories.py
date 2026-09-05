#!/usr/bin/env python3
"""Keep candidate, public, historical, and fixture versions in their categories."""

from __future__ import annotations

import pathlib
import re
import sys


PUBLIC_VERSION = "0.3.0"
APPLICATION_PACKAGES = (
    "pangopup-assets",
    "pangopup-build",
    "pangopup-cache",
    "pangopup-cli",
    "pangopup-core",
    "pangopup-engine",
    "pangopup-index",
    "pangopup-model",
)

CURRENT_PUBLIC_COUNTS = {
    "README.md": {PUBLIC_VERSION: 6},
    "CITATION.cff": {PUBLIC_VERSION: 2},
    "architecture/delivery.md": {PUBLIC_VERSION: 4},
    "planning/faq.md": {PUBLIC_VERSION: 5},
    "spec/container-image.md": {PUBLIC_VERSION: 2},
    "spec/readme-first-use.md": {PUBLIC_VERSION: 4},
    "crates/pangopup-cli/tests/citation.rs": {PUBLIC_VERSION: 2},
}

HISTORICAL_COUNTS = {
    "planning/frontier.md": {PUBLIC_VERSION: 6},
    "planning/artifacts/054-release-notes.md": {PUBLIC_VERSION: 5},
    "planning/artifacts/055-public-v0.3.0.md": {PUBLIC_VERSION: 27},
    "planning/artifacts/056-independent-public-v0.3.0.md": {PUBLIC_VERSION: 7},
    "tests/executable-delivery.sh": {PUBLIC_VERSION: 6},
}

FIXED_FIXTURE_COUNTS = {
    "crates/pangopup-assets/src/active_identity.rs": {
        PUBLIC_VERSION: 5,
        "0.3.1": 1,
    },
    "tests/container-tag-absence.sh": {PUBLIC_VERSION: 6},
}

CANDIDATE_PATHS = (
    ".github/workflows/publish-container.yml",
    "Cargo.lock",
    "Cargo.toml",
    "architecture/service.md",
    "crates/pangopup-cli/src/main.rs",
    "crates/pangopup-cli/src/service.rs",
    "crates/pangopup-cli/src/service_tests.rs",
    "scripts/check-production-qualification.py",
    "spec/cli.md",
    "tests/production-release-qualification.sh",
)

CHECKED_PATHS = tuple(
    sorted(
        set(CANDIDATE_PATHS)
        | set(CURRENT_PUBLIC_COUNTS)
        | set(HISTORICAL_COUNTS)
        | set(FIXED_FIXTURE_COUNTS)
    )
)


def read_text(root: pathlib.Path, relative: str) -> str:
    try:
        return (root / relative).read_text(encoding="utf-8")
    except OSError as error:
        raise SystemExit(f"{relative}: cannot read version category input: {error}")


def workspace_version(root: pathlib.Path) -> str:
    content = read_text(root, "Cargo.toml")
    section = re.search(
        r"(?ms)^\[workspace\.package\]\s*$\n(.*?)(?=^\[|\Z)", content
    )
    matches = [] if section is None else re.findall(
        r'(?m)^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"\s*$', section.group(1)
    )
    if len(matches) != 1:
        raise SystemExit(
            "Cargo.toml: application-candidate expected one workspace package version; "
            f"observed {len(matches)}"
        )
    return matches[0]


def package_versions(lockfile: str) -> dict[str, list[str]]:
    versions = {name: [] for name in APPLICATION_PACKAGES}
    for package in re.findall(
        r"(?ms)^\[\[package\]\]\s*$\n(.*?)(?=^\[\[package\]\]|\Z)", lockfile
    ):
        name = re.search(r'(?m)^name\s*=\s*"([^"]+)"\s*$', package)
        version = re.search(r'(?m)^version\s*=\s*"([^"]+)"\s*$', package)
        if name is not None and version is not None and name.group(1) in versions:
            versions[name.group(1)].append(version.group(1))
    return versions


def token_count(content: str, version: str) -> int:
    return len(
        re.findall(
            rf"(?<![0-9.]){re.escape(version)}(?![0-9.])",
            content,
        )
    )


def add_count_errors(
    errors: list[str],
    root: pathlib.Path,
    category: str,
    expectations: dict[str, dict[str, int]],
) -> None:
    for relative, versions in expectations.items():
        content = read_text(root, relative)
        for version, expected in versions.items():
            observed = token_count(content, version)
            if observed != expected:
                errors.append(
                    f"{relative}: {category} expected {expected} occurrences of "
                    f"{version}; observed {observed}"
                )


def add_fragment_error(
    errors: list[str],
    root: pathlib.Path,
    relative: str,
    fragment: str,
    description: str,
) -> None:
    if fragment not in read_text(root, relative):
        errors.append(
            f"{relative}: application-candidate expected {description}; observed binding missing"
        )


def check(root: pathlib.Path) -> None:
    candidate = workspace_version(root)
    errors: list[str] = []

    lock_versions = package_versions(read_text(root, "Cargo.lock"))
    for package, observed in lock_versions.items():
        if observed != [candidate]:
            errors.append(
                "Cargo.lock: application-candidate expected "
                f"{package}={candidate}; observed {observed}"
            )

    workflow = read_text(root, ".github/workflows/publish-container.yml")
    workflow_versions = re.findall(
        r"(?m)^  VERSION:\s*([0-9]+\.[0-9]+\.[0-9]+)\s*$", workflow
    )
    if workflow_versions != [candidate]:
        errors.append(
            ".github/workflows/publish-container.yml: application-candidate expected "
            f"VERSION {candidate}; observed {workflow_versions}"
        )

    cli_versions = re.findall(
        r'mustmatch like "pangopup ([0-9]+\.[0-9]+\.[0-9]+)"',
        read_text(root, "spec/cli.md"),
    )
    if cli_versions != [candidate, candidate, candidate]:
        errors.append(
            f"spec/cli.md: application-candidate expected three CLI claims at {candidate}; "
            f"observed {cli_versions}"
        )

    service = read_text(root, "architecture/service.md")
    transition = re.search(
        r"public index\s+currently identifies application v([0-9]+\.[0-9]+\.[0-9]+)\.\s+"
        r"The repository prepares v([0-9]+\.[0-9]+\.[0-9]+)",
        service,
    )
    observed_transition = None if transition is None else transition.groups()
    expected_transition = (PUBLIC_VERSION, candidate)
    if observed_transition != expected_transition:
        errors.append(
            "architecture/service.md: application-candidate expected public/candidate "
            f"transition {expected_transition}; observed {observed_transition}"
        )

    fragments = (
        (
            "crates/pangopup-cli/src/main.rs",
            'println!("pangopup {}", env!("CARGO_PKG_VERSION"));',
            "CLI output bound to the package version",
        ),
        (
            "crates/pangopup-cli/src/service.rs",
            'version: env!("CARGO_PKG_VERSION"),',
            "HTTP status bound to the package version",
        ),
        (
            "crates/pangopup-cli/src/service.rs",
            'ActiveScoringIdentityPreimage::new(env!("CARGO_PKG_VERSION"), &profile_id, policy)',
            "scoring identity bound to the package version",
        ),
        (
            "scripts/check-production-qualification.py",
            "expected_version = workspace_version(source)",
            "qualification bound to the workspace version",
        ),
        (
            "scripts/check-production-qualification.py",
            'status.get("version") != expected_version',
            "HTTP qualification bound to the workspace version",
        ),
        (
            "tests/production-release-qualification.sh",
            "export QUALIFICATION_APPLICATION_VERSION=$version",
            "qualification fixture bound to the workspace version",
        ),
        (
            "tests/executable-delivery.sh",
            'SMOKE_APPLICATION_VERSION="$version"',
            "delivery smoke fixture bound to the workspace version",
        ),
    )
    for relative, fragment, description in fragments:
        add_fragment_error(errors, root, relative, fragment, description)

    add_count_errors(errors, root, "current-public", CURRENT_PUBLIC_COUNTS)
    add_count_errors(errors, root, "historical-record", HISTORICAL_COUNTS)
    add_count_errors(errors, root, "fixed-fixture", FIXED_FIXTURE_COUNTS)

    if errors:
        raise SystemExit("\n".join(errors))
    print(
        f"version categories passed: candidate {candidate}; public {PUBLIC_VERSION}; "
        f"{len(HISTORICAL_COUNTS)} historical paths; "
        f"{len(FIXED_FIXTURE_COUNTS)} fixed fixture paths"
    )


def main() -> None:
    arguments = sys.argv[1:]
    if arguments and arguments[0] == "--paths":
        if len(arguments) > 2:
            raise SystemExit("usage: check-version-categories.py --paths [ROOT]")
        print("\n".join(CHECKED_PATHS))
        return
    if len(arguments) > 1:
        raise SystemExit("usage: check-version-categories.py [ROOT]")
    root = (
        pathlib.Path(arguments[0]).resolve()
        if arguments
        else pathlib.Path(__file__).resolve().parent.parent
    )
    if not root.is_dir() or root.is_symlink():
        raise SystemExit(f"unsafe repository root: {root}")
    check(root)


if __name__ == "__main__":
    main()
