#!/usr/bin/env python3
"""Check each intentional application, publication, and evidence version claim."""

from __future__ import annotations

import json
import hashlib
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parent.parent
PUBLIC_EXECUTABLE_VERSION = "0.4.1"
PUBLIC_EXECUTABLE_RELEASE_ID = "383676522"
PUBLIC_EXECUTABLE_COMMIT = "ba8b62180ecd5750a575944d2070f83ca585f4ed"
PUBLIC_CONTAINER_VERSION = "0.4.1"
PUBLIC_CONTAINER_INDEX = "sha256:2177c02fc045136a2ef066dbbfa669f59d56dc15e44765e7b7bfbbc9969a6eb8"
CANDIDATE_RELEASE_DATE = "2026-09-06"
FRONTIER_UPDATED_DATE = "2026-09-06"
V040_RELEASE_NOTES_SHA256 = "729fa6ed9ddb641501f2abdf5e63cd2fd9861154a46f02967bea7ff408ce4aa9"
V041_RELEASE_NOTES_SHA256 = "a2e481810f3e9095c5a06437fc47b96162c79f6c66147d08b3c0f2711e5e1abe"
CURRENT_STATE_DOCUMENTS = (
    "architecture/delivery.md",
    "architecture/service.md",
    "planning/faq.md",
    "planning/frontier.md",
)
FORBIDDEN_CURRENT_STATE_CLAIMS = (
    "public container remains v0.3.0",
    "prepares v0.4.1",
)
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
RESPONSE_SHAPE_INVENTORY = (
    ("Status response root", "adds", "`scoring_identity` and `request_contract`"),
    (
        "Status `model` object",
        "adds",
        "`work_unit`, `planning_millis_per_unit`, and `full_capacity_planning_seconds`",
    ),
    ("Every score item", "adds", "`input` and `scoring_identity`"),
    ("Score item status", "adds", "`\"rejected\"`"),
    ("Rejected score item", "carries", "`error` and `reason`"),
    ("Every structured score record", "adds", "`stable_gene`"),
)
REJECTED_ITEM_SHAPES = (
    "- Invalid-input rejected item: carries `input`, `status`, empty `records`, empty `source_reference_ambiguities`, `error`, `reason`, and `scoring_identity`; it has no normalized genomic fields or `provenance`.",
    "- Normalized model-rejected item: carries `input`, normalized `assembly`, `contig`, `position`, `ref`, and `alt`, `status`, empty `records`, empty `source_reference_ambiguities`, `error`, `reason`, and `scoring_identity`; it has no `provenance`.",
)
FIRST_INVENTORY_CONTAINER_NOTE = (
    "PangoPup published no v0.4.0 container."
    " The first published container carrying this inventory is v0.4.1."
)
COMPATIBILITY_DOCUMENTS = (
    (
        "architecture/compatibility.md",
        "../spec/http-service.md#request-contract-schema",
    ),
    (
        "planning/artifacts/057-release-notes.md",
        "../../spec/http-service.md#request-contract-schema",
    ),
)
V050_COMPATIBILITY_DOCUMENT = "architecture/compatibility.md"
V050_RESPONSE_SHAPE_HEADING = "## v0.5.0 response-shape inventory"
V050_RESPONSE_SHAPE_INVENTORY = (
    ("Every structured score record", "adds", "`gene_names`"),
    ("Every source-reference ambiguity", "adds", "`gene_names`"),
    (
        "Score-record `gene_names` object",
        "carries",
        "`symbol`, `source`, `hgnc_id`, `ncbi_gene_id`, `prev_symbols`, and `alias_symbols`",
    ),
    ("Score-record `gene_names.source`", "reports", "`hgnc` or `ncbi`"),
    (
        "Status response root",
        "adds",
        "`data_set_version`, `runtime_profile_id`, and `scoring_semantics`",
    ),
    ("Every score item", "adds", "`data_set_version`"),
    # Ticket 0054. The HTTP item names the deployment it came from; the
    # command-line line names the software that printed it. Both belong in the
    # one inventory a strict consumer reads before it accepts either surface.
    (
        "Every command-line score line",
        "adds",
        "`provenance.software_version`",
    ),
)
V050_UNNAMED_GENE_SHAPE = (
    "- Unnamed gene: the score record and the source-reference ambiguity carry no"
    " `gene_names` object, and a named gene omits `hgnc_id`, `ncbi_gene_id`,"
    " `prev_symbols`, and `alias_symbols` where its naming source supplies none."
)
V050_NAMING_NOTE = (
    "PangoPup ships one gene-name index with the build. No status field reports an"
    " installed naming vintage. A deployment cannot vary the names PangoPup returns."
)
V050_DEPLOYMENT_ORDER = (
    "Deploy strict consumer support for the complete response-shape inventory"
    " before deploying PangoPup v0.5.0."
)
AMBIGUOUS_SYMBOL_GUIDANCE = (
    "`prev_symbols` and `alias_symbols` are ambiguous. One symbol can point at"
    " several genes, and one can be another gene's approved symbol. Never match on"
    " them alone. `stable_gene` remains the only key."
)
ATTRIBUTION_DOCUMENT = "NOTICE"
NAMING_SOURCE_CLAIMS = (
    "HUGO Gene Nomenclature Committee",
    "hgnc_complete_set_2026-09-04.tsv",
    "16903161",
    "6f43d6ff43aa9fdfa5fb2f20a20a7cace66e6e02e2a0dcf19d9b726e2e248d20",
    "Creative Commons Public Domain (CC0)",
)
NCBI_SOURCE_CLAIMS = (
    "National Center for Biotechnology Information",
    "Homo_sapiens.gene_info.gz",
    "https://ftp.ncbi.nlm.nih.gov/gene/DATA/GENE_INFO/Mammalia/Homo_sapiens.gene_info.gz",
    "NCBI replaces this file at a fixed URL and publishes no dated archive of it.",
)
GENCODE_ANNOTATION_CLAIMS = (
    "gencode.v38.annotation.gtf.gz",
    "https://www.ebi.ac.uk/about/terms-of-use",
)
REQUEST_CONTRACT_SCHEMA_MEMBERS = (
    "request_contract: object",
    "|-- api_version: string",
    "|-- route: string",
    "|-- content_type: string",
    "|-- max_body_bytes: integer",
    "|-- variants: object",
    "|   |-- min_items: integer",
    "|   |-- max_items: integer",
    "|   |-- max_uncached_model_items: integer",
    "|   |-- model_work_unit: string",
    "|   |-- assembly: string",
    "|   |-- max_model_allele_bases: integer",
    "|   |-- max_exact_edit_sequence_bases: integer",
    "|   |-- forms: array of strings",
    "|   `-- contigs: array of objects",
    "|       |-- canonical: string",
    "|       `-- accepted: array of strings",
    "|-- gene_filter: object",
    "|   |-- accepted_forms: array of strings",
    "|   |-- version_minimum: integer",
    "|   |-- version_maximum: integer",
    "|   `-- version_allows_leading_zero: boolean",
    "`-- model_only: object",
    "    |-- type: string",
    "    `-- optional: boolean",
)


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


def simple_quoted_value(line: str, key: str) -> str | None:
    match = re.fullmatch(rf'{re.escape(key)}\s*=\s*"([^"\\]*)"', line.strip())
    return None if match is None else match.group(1)


def workspace_version() -> str:
    in_workspace_package = False
    versions = []
    for line in read("Cargo.toml").splitlines():
        stripped = line.strip()
        if stripped == "[workspace.package]":
            in_workspace_package = True
            continue
        if stripped.startswith("["):
            in_workspace_package = False
            continue
        if in_workspace_package:
            version = simple_quoted_value(stripped, "version")
            if version is not None:
                versions.append(version)
    if len(versions) != 1:
        fail("candidate", "Cargo.toml", "one simple quoted workspace package version")
    return versions[0]


def locked_package_versions() -> dict[str, str]:
    locked = {}
    package = None

    def record_package() -> None:
        if package is None:
            return
        name = package.get("name")
        version = package.get("version")
        if name not in PACKAGES:
            return
        if version is None or name in locked:
            fail("candidate", "Cargo.lock", f"one simple quoted version for {name}")
        locked[name] = version

    for line in read("Cargo.lock").splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            record_package()
            package = {} if stripped == "[[package]]" else None
            continue
        if package is None:
            continue
        for key in ("name", "version"):
            value = simple_quoted_value(stripped, key)
            if value is not None:
                if key in package:
                    fail("candidate", "Cargo.lock", f"one simple quoted {key} per package")
                package[key] = value
                break
    record_package()
    return locked


def check_candidate(candidate: str) -> None:
    locked = locked_package_versions()
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
        # The claim is the candidate version the stub reports, so the pattern
        # ends at the last field it needs to have read past. What else the
        # status response publishes beside it is the release checker's subject
        # and not this file's, and pinning the whole field list here made a
        # published value the checker gained a version-consistency failure.
        # It still ends on the delimiter that closes the value, because a
        # pattern stopping mid-token matches any longer name starting with it:
        # without the delimiter, `data_set_version_old` satisfies this claim.
        (
            "tests/production-release-qualification.sh",
            "candidate qualification server status",
            rf'^            "/v1/status": {{"version":"{re.escape(candidate)}","readiness":"ready","scoring_identity":scoring_identity,"data_set_version":data_set_version[,}}]',
        ),
        (
            "tests/production-release-qualification.sh",
            "candidate status mutation test",
            rf'^sed -i \'s/"version":"{re.escape(candidate)}"/"version":"9\.9\.9"/\'',
        ),
        ("CITATION.cff", "citation version", rf"^version: {re.escape(candidate)}$"),
        (
            "CITATION.cff",
            "candidate release date",
            rf"^date-released: {re.escape(CANDIDATE_RELEASE_DATE)}$",
        ),
        (
            "CITATION.cff",
            "citation release URL",
            rf'^repository-artifact: "https://github\.com/genomoncology/pangopup/releases/tag/v{re.escape(candidate)}"$',
        ),
        (
            "crates/pangopup-cli/tests/citation.rs",
            "citation release fixture",
            rf'^const RELEASE: &str = "https://github\.com/genomoncology/pangopup/releases/tag/v{re.escape(candidate)}";$',
        ),
        (
            "crates/pangopup-cli/tests/citation.rs",
            "citation version fixture",
            rf'^        \("version", "{re.escape(candidate)}"\),$',
        ),
        (
            "crates/pangopup-cli/tests/citation.rs",
            "candidate release date fixture",
            rf'^        \("date-released", "{re.escape(CANDIDATE_RELEASE_DATE)}"\),$',
        ),
        (
            "spec/container-image.md",
            "publication-target container tags",
            rf'^`{re.escape(candidate)}` and `v{re.escape(candidate)}`; the manifest digest is the immutable deployment identity,$',
        ),
    )
    for path, claim, pattern in claims:
        require("candidate", path, claim, pattern)

    require_in_section("candidate", "README.md", "## Quick start", f"raw.githubusercontent.com/genomoncology/pangopup/v{candidate}/install.sh")
    require_in_section("candidate", "README.md", "## Quick start", f"bash -s -- --version {candidate}")
    require_in_section("candidate", "README.md", "## Docker", f"export PANGOPUP_IMAGE=ghcr.io/genomoncology/pangopup:{candidate}")
    require_in_section("candidate", "README.md", "## Storage and operations", f"VERSION={candidate}")
    require_in_section("candidate", "README.md", "## Storage and operations", f"export PANGOPUP_IMAGE=ghcr.io/genomoncology/pangopup:{candidate}")
    require_in_section("candidate", "README.md", "## Storage and operations", f"docker image rm ghcr.io/genomoncology/pangopup:{candidate}")
    require_in_section("candidate", "spec/readme-first-use.md", "# README first-use contract", f"raw.githubusercontent.com/genomoncology/pangopup/v{candidate}/install.sh")
    require_in_section("candidate", "spec/readme-first-use.md", "# README first-use contract", f"bash -s -- --version {candidate}")
    require_in_section("candidate", "spec/readme-first-use.md", "# README first-use contract", f"ghcr.io/genomoncology/pangopup:{candidate}")
    require_in_section("candidate", "spec/readme-first-use.md", "# README first-use contract", f"VERSION={candidate}")

    # The transition sentence names what is published, not what the workspace
    # builds. The two coincided while the candidate and the public release were
    # both v0.4.1. They separate whenever a candidate runs ahead of publication.
    service = re.sub(r"\s+", " ", markdown_section("architecture/service.md", "# Service Boundary"))
    transition = (
        "immutable public executable and native container identify application "
        f"v{PUBLIC_EXECUTABLE_VERSION} from one exact source commit"
    )
    if transition not in service:
        fail("candidate/public transition", "architecture/service.md", transition)


def check_current_public() -> None:
    require("current-state", "planning/frontier.md", "current update date", rf"^Updated: {re.escape(FRONTIER_UPDATED_DATE)}$")
    require_in_section("current-public-container", "architecture/delivery.md", "## Thin container delivery", f"The current public set is `{PUBLIC_CONTAINER_VERSION}`/`v{PUBLIC_CONTAINER_VERSION}`/`latest`, all resolving to index `{PUBLIC_CONTAINER_INDEX}`")
    require_in_section("current-public-executable", "architecture/delivery.md", "## GitHub Releases", f"[`v{PUBLIC_EXECUTABLE_VERSION}`](https://github.com/genomoncology/pangopup/releases/tag/v{PUBLIC_EXECUTABLE_VERSION})")
    require_in_section("current-public-executable", "planning/faq.md", "### How will users install the executable?", f"tagged `v{PUBLIC_EXECUTABLE_VERSION}` script with\n`--version {PUBLIC_EXECUTABLE_VERSION}`")
    require_in_section("current-public-executable", "planning/faq.md", "### How will users install the executable?", f"[`v{PUBLIC_EXECUTABLE_VERSION}`](https://github.com/genomoncology/pangopup/releases/tag/v{PUBLIC_EXECUTABLE_VERSION})")
    require_in_section("current-public-executable", "planning/faq.md", "### How will users install the executable?", f"The public v{PUBLIC_EXECUTABLE_VERSION} release passed a clean isolated Linux run")
    require_in_section("current-state", "planning/frontier.md", "## Current release state", f"GitHub Latest is immutable executable v{PUBLIC_EXECUTABLE_VERSION} release ID `{PUBLIC_EXECUTABLE_RELEASE_ID}` at commit `{PUBLIC_EXECUTABLE_COMMIT}`.")
    require_in_section("current-state", "planning/frontier.md", "## Current release state", f"GHCR `latest`, `{PUBLIC_CONTAINER_VERSION}`, and `v{PUBLIC_CONTAINER_VERSION}` resolve to native AMD64/ARM64 OCI index `{PUBLIC_CONTAINER_INDEX}` from the same source commit.")
    require_in_section("current-state", "planning/artifacts/060-public-v0.4.1.md", "# PangoPup v0.4.1 publication record", "State: **COMPLETE — immutable v0.4.1 executable and native container are public and qualified.**")
    for path in CURRENT_STATE_DOCUMENTS:
        normalized = re.sub(r"\s+", " ", read(path)).lower()
        for stale_claim in FORBIDDEN_CURRENT_STATE_CLAIMS:
            if stale_claim in normalized:
                fail("current-state", path, f"absence of stale claim {stale_claim!r}")


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
            r"^require_text \"\$root/ticket050-runbook\.sh\" 'readonly TAG=v0\.2\.0'$",
        ),
        (
            "tests/production-release-qualification.sh",
            "v0.2.0 runbook installer fixture",
            r"^require_text \"\$root/ticket050-runbook\.sh\" 'https://raw\.githubusercontent\.com/genomoncology/pangopup/v0\.2\.0/install\.sh'$",
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
        ("planning/artifacts/057-release-notes.md", "v0.4.0 release-note title", r"^# PangoPup v0\.4\.0 release notes$"),
        (
            "planning/artifacts/058-public-v0.4.0.md",
            "v0.4.0 executable-only publication state",
            r"^State: \*\*PARTIAL — immutable v0\.4\.0 executable public; v0\.4\.0 container aliases absent\.\*\*$",
        ),
        (
            "planning/artifacts/060-public-v0.4.1.md",
            "v0.4.1 complete publication state",
            r"^State: \*\*COMPLETE — immutable v0\.4\.1 executable and native container are public and qualified\.\*\*$",
        ),
    )
    for path, claim, pattern in claims:
        require("history", path, claim, pattern)

    release_notes_digest = hashlib.sha256(
        (ROOT / "planning/artifacts/057-release-notes.md").read_bytes()
    ).hexdigest()
    if release_notes_digest != V040_RELEASE_NOTES_SHA256:
        fail("history", "planning/artifacts/057-release-notes.md", "the immutable v0.4.0 release body")

    v041_release_notes_digest = hashlib.sha256(
        read("planning/artifacts/059-release-notes.md").encode()
    ).hexdigest()
    if v041_release_notes_digest != V041_RELEASE_NOTES_SHA256:
        fail("history", "planning/artifacts/059-release-notes.md", "the immutable v0.4.1 release body")

    resources = [
        json.loads(line)
        for line in read("planning/artifacts/053-current-runtime-resources.jsonl").splitlines()
    ]
    metadata = [record for record in resources if record.get("kind") == "metadata"]
    if len(metadata) != 1 or metadata[0].get("version") != "0.2.0":
        fail("history", "planning/artifacts/053-current-runtime-resources.jsonl", "measured version 0.2.0")


def inventory_line(scope: str, action: str, fields: str) -> str:
    return f"- {scope}: {action} {fields}."


def check_response_shape_compatibility() -> None:
    heading = "## v0.4.0 response-shape inventory"
    for path, schema_target in COMPATIBILITY_DOCUMENTS:
        for scope, action, fields in RESPONSE_SHAPE_INVENTORY:
            require_in_section(
                "response-shape compatibility",
                path,
                heading,
                inventory_line(scope, action, fields),
            )
        for shape in REJECTED_ITEM_SHAPES:
            require_in_section(
                "response-shape compatibility",
                path,
                heading,
                shape,
            )
        require_in_section(
            "response-shape compatibility",
            path,
            heading,
            f"The complete [`request_contract` nested schema]({schema_target}) is part of the public HTTP contract.",
        )
        require_in_section(
            "response-shape compatibility",
            path,
            heading,
            "Permissive JSON readers that ignore unknown properties remain compatible.",
        )
        require_in_section(
            "response-shape compatibility",
            path,
            heading,
            "Deploy strict consumer support for the complete response-shape inventory before deploying PangoPup v0.4.0.",
        )
        require_in_section(
            "response-shape compatibility",
            path,
            heading,
            "`stable_gene` is the stable Ensembl grouping and filter key. `gene` remains the source-reported identity. Consumers must retain `gene` when exact version or PAR identity matters.",
        )
    require_in_section(
        "response-shape compatibility",
        "architecture/compatibility.md",
        heading,
        FIRST_INVENTORY_CONTAINER_NOTE,
    )
    schema_heading = "## Request contract schema"
    for member in REQUEST_CONTRACT_SCHEMA_MEMBERS:
        require_in_section(
            "request-contract schema",
            "spec/http-service.md",
            schema_heading,
            member,
        )


def check_gene_naming_compatibility() -> None:
    # The v0.5 section must sit below the v0.4 section. markdown_section ends a
    # section at the next heading of the same or a higher level, so a v0.5
    # heading placed above would truncate every v0.4 claim to nothing.
    for scope, action, fields in V050_RESPONSE_SHAPE_INVENTORY:
        require_in_section(
            "response-shape compatibility",
            V050_COMPATIBILITY_DOCUMENT,
            V050_RESPONSE_SHAPE_HEADING,
            inventory_line(scope, action, fields),
        )
    for claim in (
        V050_UNNAMED_GENE_SHAPE,
        V050_DEPLOYMENT_ORDER,
        AMBIGUOUS_SYMBOL_GUIDANCE,
        V050_NAMING_NOTE,
    ):
        require_in_section(
            "response-shape compatibility",
            V050_COMPATIBILITY_DOCUMENT,
            V050_RESPONSE_SHAPE_HEADING,
            claim,
        )
    attribution = read(ATTRIBUTION_DOCUMENT)
    for claim in NAMING_SOURCE_CLAIMS + NCBI_SOURCE_CLAIMS + GENCODE_ANNOTATION_CLAIMS:
        if claim not in attribution:
            fail("attribution", ATTRIBUTION_DOCUMENT, claim)


def main() -> None:
    candidate = workspace_version()
    check_candidate(candidate)
    check_current_public()
    check_fixed_fixtures()
    check_history()
    check_response_shape_compatibility()
    check_gene_naming_compatibility()
    print(
        f"version consistency: current {candidate}; public executable "
        f"{PUBLIC_EXECUTABLE_VERSION}; public container {PUBLIC_CONTAINER_VERSION}"
    )


if __name__ == "__main__":
    main()
