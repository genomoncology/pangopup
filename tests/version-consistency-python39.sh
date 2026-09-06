#!/usr/bin/env bash
set -euo pipefail

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

python3 - "$repository/scripts/check-version-consistency.py" <<'PY'
import builtins
import contextlib
import io
import runpy
import sys

checker = sys.argv[1]
original_import = builtins.__import__


def import_without_tomllib(name, globals=None, locals=None, fromlist=(), level=0):
    if name == "tomllib" or name.startswith("tomllib."):
        raise ModuleNotFoundError("No module named 'tomllib'", name="tomllib")
    return original_import(name, globals, locals, fromlist, level)


builtins.__import__ = import_without_tomllib
try:
    namespace = runpy.run_path(checker)
    main = namespace["main"]
    checker_globals = main.__globals__
    original_read = checker_globals["read"]
    required_public_executable_version = "0.4.1"
    required_public_executable_release_id = "383676522"
    required_public_executable_commit = "ba8b62180ecd5750a575944d2070f83ca585f4ed"
    required_public_container_version = "0.4.1"
    required_public_container_index = "sha256:2177c02fc045136a2ef066dbbfa669f59d56dc15e44765e7b7bfbbc9969a6eb8"
    required_v041_release_notes_sha256 = "a2e481810f3e9095c5a06437fc47b96162c79f6c66147d08b3c0f2711e5e1abe"
    required_frontier_updated_date = "2026-09-06"
    if checker_globals.get("PUBLIC_EXECUTABLE_VERSION") != required_public_executable_version:
        raise AssertionError("checker does not pin the public executable version")
    if checker_globals.get("PUBLIC_EXECUTABLE_RELEASE_ID") != required_public_executable_release_id:
        raise AssertionError("checker does not pin the public executable release")
    if checker_globals.get("PUBLIC_EXECUTABLE_COMMIT") != required_public_executable_commit:
        raise AssertionError("checker does not pin the public executable commit")
    if checker_globals.get("PUBLIC_CONTAINER_VERSION") != required_public_container_version:
        raise AssertionError("checker does not pin the public container version")
    if checker_globals.get("PUBLIC_CONTAINER_INDEX") != required_public_container_index:
        raise AssertionError("checker does not pin the public container index")
    if checker_globals.get("FRONTIER_UPDATED_DATE") != required_frontier_updated_date:
        raise AssertionError("checker does not pin the current frontier update date")
    if checker_globals.get("V041_RELEASE_NOTES_SHA256") != required_v041_release_notes_sha256:
        raise AssertionError("checker does not pin the immutable v0.4.1 release body")
    required_current_state_documents = (
        "architecture/delivery.md",
        "architecture/service.md",
        "planning/faq.md",
        "planning/frontier.md",
    )
    required_forbidden_current_claims = (
        "public container remains v0.3.0",
        "prepares v0.4.1",
    )
    if tuple(checker_globals.get("CURRENT_STATE_DOCUMENTS", ())) != required_current_state_documents:
        raise AssertionError("checker current-state documents differ from the independent required set")
    if tuple(checker_globals.get("FORBIDDEN_CURRENT_STATE_CLAIMS", ())) != required_forbidden_current_claims:
        raise AssertionError("checker stale current-state claims differ from the independent required set")
    required_documents = (
        (
            "architecture/compatibility.md",
            "../spec/http-service.md#request-contract-schema",
        ),
        (
            "planning/artifacts/057-release-notes.md",
            "../../spec/http-service.md#request-contract-schema",
        ),
    )
    required_inventory = (
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
    required_rejected_shapes = (
        "- Invalid-input rejected item: carries `input`, `status`, empty `records`, empty `source_reference_ambiguities`, `error`, `reason`, and `scoring_identity`; it has no normalized genomic fields or `provenance`.",
        "- Normalized model-rejected item: carries `input`, normalized `assembly`, `contig`, `position`, `ref`, and `alt`, `status`, empty `records`, empty `source_reference_ambiguities`, `error`, `reason`, and `scoring_identity`; it has no `provenance`.",
    )
    required_schema_members = (
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
    if tuple(checker_globals["COMPATIBILITY_DOCUMENTS"]) != required_documents:
        raise AssertionError("checker compatibility documents differ from the independent required set")
    if tuple(checker_globals["RESPONSE_SHAPE_INVENTORY"]) != required_inventory:
        raise AssertionError("checker response-shape inventory differs from the independent required set")
    if tuple(checker_globals["REJECTED_ITEM_SHAPES"]) != required_rejected_shapes:
        raise AssertionError("checker rejected-item shapes differ from the independent required set")
    if tuple(checker_globals["REQUEST_CONTRACT_SCHEMA_MEMBERS"]) != required_schema_members:
        raise AssertionError("checker request-contract schema differs from the independent required set")
    for scope, action, fields in required_inventory:
        expected_line = f"- {scope}: {action} {fields}."
        if checker_globals["inventory_line"](scope, action, fields) != expected_line:
            raise AssertionError("checker inventory-line format differs from the independent required format")
    cargo = original_read("Cargo.toml")
    lock = original_read("Cargo.lock")
    compatibility_text = {
        path: original_read(path) for path, _schema_target in required_documents
    }
    request_contract_schema = original_read("spec/http-service.md")
    citation = original_read("CITATION.cff")
    citation_test = original_read("crates/pangopup-cli/tests/citation.rs")
    delivery = original_read("architecture/delivery.md")
    service = original_read("architecture/service.md")
    frontier = original_read("planning/frontier.md")
    v040_release_notes = original_read("planning/artifacts/057-release-notes.md")
    v040_publication_record = original_read("planning/artifacts/058-public-v0.4.0.md")
    v041_publication_record = original_read("planning/artifacts/060-public-v0.4.1.md")
    v041_release_notes = original_read("planning/artifacts/059-release-notes.md")
    candidate = namespace["workspace_version"]()
    required_candidate_release_date = "2026-09-06"
    if checker_globals["CANDIDATE_RELEASE_DATE"] != required_candidate_release_date:
        raise AssertionError("checker candidate release date differs from the independent required date")

    main()

    def replace_once(text, old, new):
        if text.count(old) != 1:
            raise AssertionError(f"expected one occurrence of {old!r}")
        return text.replace(old, new, 1)

    def expect_rejected(label, overrides):
        def read_with_mutation(path):
            if path in overrides:
                return overrides[path]
            return original_read(path)

        checker_globals["read"] = read_with_mutation
        try:
            with contextlib.redirect_stderr(io.StringIO()):
                main()
        except SystemExit as error:
            if error.code != 1:
                raise AssertionError(f"{label} exited with {error.code!r}") from error
        else:
            raise AssertionError(f"checker accepted {label}")
        finally:
            checker_globals["read"] = original_read

    expect_rejected(
        "a changed workspace version",
        {
            "Cargo.toml": replace_once(
                cargo, f'version = "{candidate}"', 'version = "9.9.9"'
            )
        },
    )
    expect_rejected(
        "a changed PangoPup lockfile package version",
        {
            "Cargo.lock": replace_once(
                lock,
                f'name = "pangopup-assets"\nversion = "{candidate}"',
                'name = "pangopup-assets"\nversion = "9.9.9"',
            )
        },
    )
    expect_rejected(
        "a changed candidate citation release date",
        {
            "CITATION.cff": replace_once(
                citation,
                f"date-released: {required_candidate_release_date}",
                "date-released: 2099-01-01",
            )
        },
    )
    expect_rejected(
        "a changed candidate citation release-date fixture",
        {
            "crates/pangopup-cli/tests/citation.rs": replace_once(
                citation_test,
                f'("date-released", "{required_candidate_release_date}"),',
                '("date-released", "2099-01-01"),',
            )
        },
    )
    executable_claim = f"[`v{required_public_executable_version}`](https://github.com/genomoncology/pangopup/releases/tag/v{required_public_executable_version})"
    expect_rejected(
        "a stale public executable claim",
        {
            "architecture/delivery.md": replace_once(
                delivery, executable_claim, "[`v9.9.9`](https://example.invalid/v9.9.9)"
            )
        },
    )
    container_claim = f"The current public set is `{required_public_container_version}`/`v{required_public_container_version}`/`latest`, all resolving to index `{required_public_container_index}`"
    expect_rejected(
        "a stale public container claim",
        {
            "architecture/delivery.md": replace_once(
                delivery, container_claim, container_claim.replace(required_public_container_index, "sha256:" + "0" * 64)
            )
        },
    )
    expect_rejected(
        "changed immutable v0.4.0 release-note bytes",
        {
            "planning/artifacts/057-release-notes.md": replace_once(
                v040_release_notes, "# PangoPup v0.4.0 release notes", "# Changed v0.4.0 release notes"
            )
        },
    )
    expect_rejected(
        "an incomplete v0.4.0 partial record",
        {
            "planning/artifacts/058-public-v0.4.0.md": replace_once(
                v040_publication_record,
                "State: **PARTIAL — immutable v0.4.0 executable public; v0.4.0 container aliases absent.**",
                "",
            )
        },
    )
    expect_rejected(
        "changed immutable v0.4.1 release-note bytes",
        {"planning/artifacts/059-release-notes.md": v041_release_notes + "\nmutation\n"},
    )
    expect_rejected(
        "a stale current frontier update date",
        {
            "planning/frontier.md": replace_once(
                frontier,
                f"Updated: {required_frontier_updated_date}",
                "Updated: 2026-08-05",
            )
        },
    )
    current_transition = "The immutable public executable and native container identify application v0.4.1 from one exact source commit"
    expect_rejected(
        "a stale candidate transition in current service architecture",
        {
            "architecture/service.md": replace_once(
                service,
                current_transition,
                "The public executable identifies application v0.4.0; the public container remains v0.3.0; the repository prepares v0.4.1 as one coherent executable/container candidate",
            )
        },
    )
    current_frontier = f"GitHub Latest is immutable executable v{required_public_executable_version} release ID `{required_public_executable_release_id}` at commit `{required_public_executable_commit}`."
    expect_rejected(
        "a stale split predecessor in the current frontier",
        {
            "planning/frontier.md": replace_once(
                frontier,
                current_frontier,
                "GitHub Latest is immutable executable v0.4.0 release ID `383614742` at commit `ea4438e50762e32f09052b364060c89201ed78bc`.",
            )
        },
    )
    expect_rejected(
        "a prepared v0.4.1 publication record after publication",
        {
            "planning/artifacts/060-public-v0.4.1.md": replace_once(
                v041_publication_record,
                "State: **COMPLETE — immutable v0.4.1 executable and native container are public and qualified.**",
                "State: **PREPARED — no v0.4.1 tag, release, or container alias exists.**",
            )
        },
    )
    for path in required_current_state_documents:
        current_text = original_read(path)
        expect_rejected(
            f"an appended stale v0.4.0/v0.3.0 split in {path}",
            {path: current_text + "\nThe public executable is v0.4.0 and the public container remains v0.3.0.\n"},
        )
        expect_rejected(
            f"an appended stale v0.4.1 preparation claim in {path}",
            {path: current_text + "\nThe repository prepares v0.4.1.\n"},
        )
    for path, schema_target in required_documents:
        for scope, action, fields in required_inventory:
            line = f"- {scope}: {action} {fields}."
            expect_rejected(
                f"a removed {scope} inventory entry in {path}",
                {path: replace_once(compatibility_text[path], line, "")},
            )
            expect_rejected(
                f"a changed {scope} object scope in {path}",
                {
                    path: replace_once(
                        compatibility_text[path],
                        line,
                        line.replace(scope, "Unspecified response object", 1),
                    )
                },
            )
        for shape in required_rejected_shapes:
            expect_rejected(
                f"a removed rejected-item shape in {path}",
                {path: replace_once(compatibility_text[path], shape, "")},
            )
        schema_link = f"The complete [`request_contract` nested schema]({schema_target}) is part of the public HTTP contract."
        expect_rejected(
            f"a removed request-contract schema link in {path}",
            {path: replace_once(compatibility_text[path], schema_link, "")},
        )
        order = "Deploy strict consumer support for the complete response-shape inventory before deploying PangoPup v0.4.0."
        expect_rejected(
            f"a removed consumer-first deployment order in {path}",
            {path: replace_once(compatibility_text[path], order, "")},
        )
    for member in required_schema_members:
        expect_rejected(
            f"a removed request-contract schema member {member!r}",
            {
                "spec/http-service.md": replace_once(
                    request_contract_schema, member, ""
                )
            },
        )
finally:
    builtins.__import__ = original_import
PY
