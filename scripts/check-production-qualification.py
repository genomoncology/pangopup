#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""Check production release outputs without opening production asset members."""

from __future__ import annotations

import hashlib
import json
import pathlib
import re
import sys


GROUPS = (
    "ENSG00000010610",
    "ENSG00000141499",
    "ENSG00000141510",
    "ENSG00000169129",
    "ENSG00000175727",
    "ENSG00000185974",
    "unfiltered",
)
GROUP_COUNTS = {
    "ENSG00000010610": 318,
    "ENSG00000141499": 320,
    "ENSG00000141510": 320,
    "ENSG00000169129": 6,
    "ENSG00000175727": 6,
    "ENSG00000185974": 6,
    "unfiltered": 24,
}
REQUESTS_SHA256 = "042fcc0e550f7dfccad742a6a2e6a89b0c4e245673b0222bcefb7d42b1ffe52d"
M09_SHA256 = "16bbc2256a07104b576fa7c5cd81378b900dd0920e20c8f1cb53c286414a91e9"
EXPECTED_SHA256 = {
    "ENSG00000010610": "3a7ac3b091ef94249ddd7a95e040cee3a44405ff9310b1715bd0135cda3684c0",
    "ENSG00000141499": "88c50558919f1504d18d6e215aef3bb0df9f6755130c10f7e32d879b238a4ab1",
    "ENSG00000141510": "2b6010b8ff8c08213850ba7626f4f5a84f8ccc48c7321302fa8e026fbd57a518",
    "ENSG00000169129": "a702a4ff5c4c81696697efc0695076477737b1fe9f5c9ff7cfcdeb4ceba23438",
    "ENSG00000175727": "ae74708584f0f166004219503a5829501b3e60c581f164d31447b3bb84b5f472",
    "ENSG00000185974": "4038e311c672562022a32a43c44e67190e22507f24d03d8e129e4e2272240fb7",
    "unfiltered": "65b7996ebef2f6c976b1492913777177614876079cf28960b8deccf394436633",
}
BUNDLE_FIELD = re.compile(br',"bundle_id":"sha256:[0-9a-f]{64}"')


def fail(message: str) -> None:
    raise SystemExit(message)


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_json(path: pathlib.Path) -> object:
    try:
        return json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        fail(f"invalid JSON: {path.name}: {error}")


def closed_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate key: {key}")
        value[key] = item
    return value


def canonical_snv(path: pathlib.Path) -> bytes:
    try:
        content = path.read_bytes()
    except OSError as error:
        fail(f"cannot read SNV output: {path.name}: {error}")
    if not content or not content.endswith(b"\n") or b"\r" in content:
        fail(f"invalid SNV JSONL framing: {path.name}")
    output = bytearray()
    for number, line in enumerate(content[:-1].split(b"\n"), 1):
        try:
            value = json.loads(line, object_pairs_hook=closed_object)
            provenance = value["provenance"]
            bundle_id = provenance["bundle_id"]
            if not isinstance(bundle_id, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", bundle_id):
                raise ValueError("invalid bundle identity")
        except (json.JSONDecodeError, KeyError, TypeError, ValueError) as error:
            fail(f"invalid SNV JSONL: {path.name}:{number}: {error}")
        matches = list(BUNDLE_FIELD.finditer(line))
        if len(matches) != 1:
            fail(f"bundle identity is not one exact removable field: {path.name}:{number}")
        match = matches[0]
        output.extend(line[: match.start()])
        output.extend(line[match.end() :])
        output.extend(b"\n")
    return bytes(output)


def require_fixture_identities(source: pathlib.Path) -> None:
    requests = source / "tests/fixtures/snv-regression/requests.tsv"
    if sha256(requests) != REQUESTS_SHA256:
        fail("request fixture identity mismatch")
    try:
        lines = requests.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        fail(f"cannot read request fixture: {error}")
    if not lines or lines[0] != "order\tgroup\tgroup_order\tvariant\tgene":
        fail("request fixture header mismatch")
    observed = {group: 0 for group in GROUPS}
    for expected_order, line in enumerate(lines[1:]):
        fields = line.split("\t")
        if len(fields) != 5 or fields[0] != str(expected_order):
            fail("request fixture order mismatch")
        group = fields[1]
        if group not in observed or fields[2] != str(observed[group]):
            fail("request fixture group/order mismatch")
        if (group == "unfiltered" and fields[4] != ".") or (
            group != "unfiltered" and fields[4] != group
        ):
            fail("request fixture gene mismatch")
        observed[group] += 1
    if len(lines) - 1 != 1_000 or observed != GROUP_COUNTS:
        fail("request fixture count mismatch")

    for group in GROUPS:
        expected = source / "tests/fixtures/snv-regression/expected" / f"{group}.jsonl"
        if sha256(expected) != EXPECTED_SHA256[group]:
            fail(f"expected oracle identity mismatch: {group}")
        if len(expected.read_bytes().splitlines()) != GROUP_COUNTS[group]:
            fail(f"expected oracle count mismatch: {group}")
    model = source / "tests/fixtures/executable-release/m09.jsonl"
    if sha256(model) != M09_SHA256:
        fail("model oracle identity mismatch: M09-insertion-short-plus")


def require_ready(path: pathlib.Path, component_statuses: set[str]) -> None:
    value = read_json(path)
    if not isinstance(value, dict) or value.get("status") != "ready":
        fail(f"qualification state is not ready: {path.name}")
    for component in ("snv", "runtime"):
        state = value.get(component)
        if not isinstance(state, dict) or state.get("status") not in component_statuses:
            fail(f"unexpected {component} state: {path.name}")


def main() -> None:
    if len(sys.argv) != 3:
        fail("usage: check-production-qualification.py <OUTPUT_DIR> <SOURCE_TREE>")
    output = pathlib.Path(sys.argv[1])
    source = pathlib.Path(sys.argv[2])
    if not output.is_dir() or output.is_symlink() or not source.is_dir() or source.is_symlink():
        fail("qualification directories are unsafe")

    require_fixture_identities(source)

    require_ready(output / "sync-online.json", {"installed"})
    require_ready(output / "sync-offline.json", {"reused"})
    require_ready(output / "status.json", {"ready"})

    combined_actual = bytearray()
    combined_expected = bytearray()
    for group in GROUPS:
        actual_path = output / f"snv-{group}.jsonl"
        expected_path = source / "tests/fixtures/snv-regression/expected" / f"{group}.jsonl"
        actual = canonical_snv(actual_path)
        expected = canonical_snv(expected_path)
        if actual != expected:
            fail(f"SNV oracle mismatch: {group}")
        combined_actual.extend(actual)
        combined_expected.extend(expected)

    model_actual = output / "model-M09.jsonl"
    model_expected = source / "tests/fixtures/executable-release/m09.jsonl"
    if model_actual.read_bytes() != model_expected.read_bytes():
        fail("model oracle mismatch: M09-insertion-short-plus")

    print(f"requests_sha256={REQUESTS_SHA256}")
    print(f"snv_canonical_actual_sha256={hashlib.sha256(combined_actual).hexdigest()}")
    print(f"snv_canonical_expected_sha256={hashlib.sha256(combined_expected).hexdigest()}")
    print(f"model_oracle_sha256={sha256(model_expected)}")
    print(f"model_actual_sha256={sha256(model_actual)}")
    print("production qualification passed")


if __name__ == "__main__":
    main()
