#!/usr/bin/env python3
"""Check each intentional application, publication, and evidence version claim."""

from __future__ import annotations

import json
import pathlib
import re
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parent.parent
PUBLIC_VERSION = "0.3.0"
PACKAGES = {
    "pangopup-assets",
    "pangopup-build",
    "pangopup-cache",
    "pangopup-cli",
    "pangopup-core",
    "pangopup-engine",
    "pangopup-index",
    "pangopup-model",
}


def fail(category: str, path: str, claim: str) -> None:
    print(
        f"version consistency: {category}: {path}: expected {claim}",
        file=sys.stderr,
    )
    raise SystemExit(1)


def read(path: str) -> str:
    return (ROOT / path).read_text()


def require(category: str, path: str, claim: str, pattern: str) -> None:
    if re.search(pattern, read(path), re.MULTILINE) is None:
        fail(category, path, claim)


def markdown_section(path: str, heading: str) -> str:
    text = read(path)
    match = re.search(rf"^{re.escape(heading)}\n", text, re.MULTILINE)
    if match is None:
        fail("document structure", path, f"section {heading!r}")
    level = len(heading) - len(heading.lstrip("#"))
    following = re.search(rf"^#{{1,{level}}} ", text[match.end() :], re.MULTILINE)
    end = len(text) if following is None else match.end() + following.start()
    return text[match.end() : end]


def require_in_section(category: str, path: str, heading: str, claim: str) -> None:
    if claim not in markdown_section(path, heading):
        fail(category, path, f"{claim!r} in {heading}")


def check_candidate(candidate: str) -> None:
    lock = tomllib.loads(read("Cargo.lock"))
    locked = {
        package["name"]: package["version"]
        for package in lock["package"]
        if package["name"] in PACKAGES
    }
    if set(locked) != PACKAGES:
        fail("candidate", "Cargo.lock", f"the eight PangoPup packages {sorted(PACKAGES)}")
    for package in sorted(PACKAGES):
        if locked[package] != candidate:
            fail("candidate", "Cargo.lock", f"{package} version {candidate}")

    claims = (
        (
            ".github/workflows/publish-container.yml",
            "container staging VERSION",
            rf"^  VERSION: {re.escape(candidate)}$",
        ),
        (
            "scripts/check-production-qualification.py",
            "qualified HTTP status version",
            rf'^    if not isinstance\(status, dict\) or status\.get\("version"\) != "{re.escape(candidate)}"',
        ),
        ("spec/cli.md", "root --version output", rf'^pangopup --version \| mustmatch like "pangopup {re.escape(candidate)}"$'),
        ("spec/cli.md", "root -V output", rf'^pangopup -V \| mustmatch like "pangopup {re.escape(candidate)}"$'),
        ("spec/cli.md", "lookup --version output", rf'^pangopup lookup --version \| mustmatch like "pangopup {re.escape(candidate)}"$'),
        (
            "crates/pangopup-cli/src/service_tests.rs",
            "HTTP status version fixture",
            rf'^            "version": "{re.escape(candidate)}",$',
        ),
        (
            "tests/executable-delivery.sh",
            "candidate smoke executable output",
            rf"^  printf 'pangopup {re.escape(candidate)}\\n'$",
        ),
        (
            "tests/production-release-qualification.sh",
            "candidate qualification server status",
            rf'^            "/v1/status": {{"version":"{re.escape(candidate)}","readiness":"ready"}},$',
        ),
        (
            "tests/production-release-qualification.sh",
            "candidate status mutation test",
            rf'^sed -i \'s/"version":"{re.escape(candidate)}"/"version":"9\.9\.9"/\'',
        ),
    )
    for path, claim, pattern in claims:
        require("candidate", path, claim, pattern)

    service = re.sub(r"\s+", " ", markdown_section("architecture/service.md", "# Service Boundary"))
    transition = (
        f"currently identifies application v{PUBLIC_VERSION}; the repository prepares "
        f"v{candidate} as one coherent executable/container candidate"
    )
    if transition not in service:
        fail("candidate/public transition", "architecture/service.md", transition)


def check_current_public() -> None:
    claims = (
        ("CITATION.cff", "citation version", rf"^version: {re.escape(PUBLIC_VERSION)}$"),
        (
            "CITATION.cff",
            "citation release URL",
            rf'^repository-artifact: "https://github\.com/genomoncology/pangopup/releases/tag/v{re.escape(PUBLIC_VERSION)}"$',
        ),
        (
            "crates/pangopup-cli/tests/citation.rs",
            "citation release fixture",
            rf'^const RELEASE: &str = "https://github\.com/genomoncology/pangopup/releases/tag/v{re.escape(PUBLIC_VERSION)}";$',
        ),
        (
            "crates/pangopup-cli/tests/citation.rs",
            "citation version fixture",
            rf'^        \("version", "{re.escape(PUBLIC_VERSION)}"\),$',
        ),
        (
            "spec/container-image.md",
            "published container tags",
            rf'^`{re.escape(PUBLIC_VERSION)}` and `v{re.escape(PUBLIC_VERSION)}`; the manifest digest is the immutable deployment identity,$',
        ),
    )
    for path, claim, pattern in claims:
        require("current-public", path, claim, pattern)

    require_in_section("current-public", "README.md", "## Quick start", f"raw.githubusercontent.com/genomoncology/pangopup/v{PUBLIC_VERSION}/install.sh")
    require_in_section("current-public", "README.md", "## Quick start", f"bash -s -- --version {PUBLIC_VERSION}")
    require_in_section("current-public", "README.md", "## Docker", f"export PANGOPUP_IMAGE=ghcr.io/genomoncology/pangopup:{PUBLIC_VERSION}")
    require_in_section("current-public", "README.md", "## Storage and operations", f"VERSION={PUBLIC_VERSION}")
    require_in_section("current-public", "README.md", "## Storage and operations", f"export PANGOPUP_IMAGE=ghcr.io/genomoncology/pangopup:{PUBLIC_VERSION}")
    require_in_section("current-public", "README.md", "## Storage and operations", f"docker image rm ghcr.io/genomoncology/pangopup:{PUBLIC_VERSION}")
    require_in_section("current-public", "spec/readme-first-use.md", "# README first-use contract", f"raw.githubusercontent.com/genomoncology/pangopup/v{PUBLIC_VERSION}/install.sh")
    require_in_section("current-public", "spec/readme-first-use.md", "# README first-use contract", f"bash -s -- --version {PUBLIC_VERSION}")
    require_in_section("current-public", "spec/readme-first-use.md", "# README first-use contract", f"ghcr.io/genomoncology/pangopup:{PUBLIC_VERSION}")
    require_in_section("current-public", "spec/readme-first-use.md", "# README first-use contract", f"VERSION={PUBLIC_VERSION}")
    require_in_section("current-public", "architecture/delivery.md", "## Thin container delivery", f"The current public set is `{PUBLIC_VERSION}`/`v{PUBLIC_VERSION}`/`latest`")
    require_in_section("current-public", "architecture/delivery.md", "## GitHub Releases", f"[`v{PUBLIC_VERSION}`](https://github.com/genomoncology/pangopup/releases/tag/v{PUBLIC_VERSION})")
    require_in_section("current-public", "planning/faq.md", "### How will users install the executable?", f"tagged `v{PUBLIC_VERSION}` script with\n`--version {PUBLIC_VERSION}`")
    require_in_section("current-public", "planning/faq.md", "### How will users install the executable?", f"[`v{PUBLIC_VERSION}`](https://github.com/genomoncology/pangopup/releases/tag/v{PUBLIC_VERSION})")
    require_in_section("current-public", "planning/faq.md", "### How will users install the executable?", f"The public v{PUBLIC_VERSION} release passed a clean isolated Linux run")


def check_fixed_fixtures() -> None:
    claims = (
        (
            "crates/pangopup-assets/src/active_identity.rs",
            "pinned 0.3.0 canonical identity input",
            r'^        let preimage = ActiveScoringIdentityPreimage::new\("0\.3\.0", &runtime_id\(\), policy\(1\)\);$',
        ),
        (
            "crates/pangopup-assets/src/active_identity.rs",
            "version-change identity comparison",
            r'^            ActiveScoringIdentityPreimage::new\("0\.3\.1", &runtime_id, policy\(1\)\)\.identity\(\);$',
        ),
        (
            "crates/pangopup-assets/src/active_identity.rs",
            "pinned canonical software version text",
            r'^                "\\"software_version\\":\\"0\.3\.0\\"}"$',
        ),
        (
            "crates/pangopup-assets/src/active_identity.rs",
            "pinned 0.3.0 identity digest",
            r'^            "sha256:c0e2e1fd77821555a868b5f70514769d144a15aeb160e71aea17d6099839328f"$',
        ),
        (
            "crates/pangopup-assets/src/active_identity.rs",
            "baseline version input",
            r'^            ActiveScoringIdentityPreimage::new\("0\.3\.0", &runtime_id, policy\(1\)\)\.identity\(\);$',
        ),
        (
            "crates/pangopup-assets/src/active_identity.rs",
            "policy comparison version input",
            r'^            ActiveScoringIdentityPreimage::new\("0\.3\.0", &runtime_id, policy\(2\)\)\.identity\(\);$',
        ),
        (
            "crates/pangopup-assets/src/active_identity.rs",
            "runtime comparison version input",
            r'^            ActiveScoringIdentityPreimage::new\("0\.3\.0", &changed_runtime_id, policy\(1\)\)\.identity\(\);$',
        ),
        (
            "tests/container-tag-absence.sh",
            "absent-tag success fixture",
            r'^"\$helper" 0\.3\.0 404 "\$root/missing\.json"$',
        ),
        (
            "tests/container-tag-absence.sh",
            "existing-tag rejection fixture",
            r'^expect_rejected existing "\$helper" 0\.3\.0 200 "\$root/missing\.json"$',
        ),
        (
            "tests/container-tag-absence.sh",
            "unauthorized-tag rejection fixture",
            r'^expect_rejected unauthorized "\$helper" 0\.3\.0 401 "\$root/missing\.json"$',
        ),
        (
            "tests/container-tag-absence.sh",
            "server-error rejection fixture",
            r'^expect_rejected server-error "\$helper" 0\.3\.0 500 "\$root/missing\.json"$',
        ),
        (
            "tests/container-tag-absence.sh",
            "disguised-denial rejection fixture",
            r'^expect_rejected disguised-denial "\$helper" 0\.3\.0 404 "\$root/denied\.json"$',
        ),
        (
            "tests/container-tag-absence.sh",
            "malformed-404 rejection fixture",
            r'^expect_rejected malformed-404 "\$helper" 0\.3\.0 404 "\$root/not-json"$',
        ),
        (
            "spec/executable-install.md",
            "prior-release installer fixture",
            r'^\.\./install\.sh --version v0\.2\.0$',
        ),
        (
            "tests/production-release-qualification.sh",
            "v0.2.0 runbook tag fixture",
            r"^grep -Fq 'readonly TAG=v0\.2\.0' ",
        ),
        (
            "tests/production-release-qualification.sh",
            "v0.2.0 runbook installer fixture",
            r"^grep -Fq 'https://raw\.githubusercontent\.com/genomoncology/pangopup/v0\.2\.0/install\.sh' ",
        ),
        (
            "tests/executable-delivery.sh",
            "v0.3.0 publication record fixture",
            r'^publication_record="\$repo/planning/artifacts/055-public-v0\.3\.0\.md"$',
        ),
    )
    for path, claim, pattern in claims:
        require("fixed fixture", path, claim, pattern)


def check_history() -> None:
    claims = (
        ("planning/artifacts/050-release-notes.md", "v0.2.0 release-note title", r"^# PangoPup v0\.2\.0$"),
        (
            "planning/artifacts/050-public-linux-release.md",
            "v0.2.0 executable publication state",
            r"^State: \*\*COMPLETE — immutable `v0\.2\.0` is public and qualified\.\*\*$",
        ),
        (
            "planning/artifacts/051-public-container.md",
            "v0.2.0 container publication record",
            r"^State: \*\*COMPLETE — the public `0\.2\.0`, `v0\.2\.0`, and `latest` tags resolve to$",
        ),
        (
            "planning/artifacts/053-current-runtime-resources.md",
            "measured v0.2.0 binary",
            r"^- Binary version: `0\.2\.0`$",
        ),
        ("planning/artifacts/054-release-notes.md", "v0.3.0 release-note title", r"^# PangoPup v0\.3\.0 release notes$"),
        (
            "planning/artifacts/055-public-v0.3.0.md",
            "v0.3.0 publication record title",
            r"^# Ticket 055 v0\.3\.0 publication record$",
        ),
        (
            "planning/artifacts/056-independent-public-v0.3.0.md",
            "independent v0.3.0 qualification title",
            r"^# Independent public v0\.3\.0 qualification$",
        ),
    )
    for path, claim, pattern in claims:
        require("history", path, claim, pattern)

    resources = [
        json.loads(line)
        for line in read("planning/artifacts/053-current-runtime-resources.jsonl").splitlines()
    ]
    metadata = [record for record in resources if record.get("kind") == "metadata"]
    if len(metadata) != 1 or metadata[0].get("version") != "0.2.0":
        fail("history", "planning/artifacts/053-current-runtime-resources.jsonl", "measured version 0.2.0")


def main() -> None:
    cargo = tomllib.loads(read("Cargo.toml"))
    candidate = cargo["workspace"]["package"]["version"]
    check_candidate(candidate)
    check_current_public()
    check_fixed_fixtures()
    check_history()
    print(f"version consistency: candidate {candidate}; public {PUBLIC_VERSION}")


if __name__ == "__main__":
    main()
