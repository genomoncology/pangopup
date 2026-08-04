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
M09_SHA256 = "f7e2d7f207ff28d2dff32a033754d395eb9e9fd1bcbb9c5b56b85ce27a8720c9"
MODEL_ONLY_SNV_SHA256 = "49664b7eddbcefe34d1d3035ee964837318666e44db655295116c0bc7309d20a"
EXPECTED_SHA256 = {
    "ENSG00000010610": "83e3aac1fe5feaefddeb3d4419e7dbb36cbb1566e0e8b6d49327e2dcdfccf183",
    "ENSG00000141499": "4ec3696e837b56c9fa1f7711c7ec7ed4bddbe9574c464f93b6677eb0cd6948ed",
    "ENSG00000141510": "a36503fd551ee6421c2384bca6edd82dc614cf0844016cb4cd94c7ed5ada9dc0",
    "ENSG00000169129": "9b54aaf6de9f9bb0c70bb9f9acb1ad5f7d5c46f3e666e1a6e10c9c27915c0bca",
    "ENSG00000175727": "c8e4625fc7253f36844bde07796dbf3adb0934a7e36bc607dc73920e3e2d9055",
    "ENSG00000185974": "65f5cfe6740d501d767597e9d9a3f560d34cabf08a7789918ccd274bb0cc39ec",
    "unfiltered": "f70408edb9503e39788274cadc32a3b6a17cf29dc2950b6af25ca6dd8e1c2e1d",
}
BUNDLE_FIELD = re.compile(br',"bundle_id":"sha256:[0-9a-f]{64}"')
TRANSFER = re.compile(
    r"^sync: (?:snv|runtime) \S+ (?:cached|fresh|resume|restart) attempt [1-4]/4 "
    r"\d+/\d+ bytes \((\d+) downloaded, (\d+) resumed\)$"
)
COMPLETE = re.compile(r"^sync: ready \((\d+) downloaded, (\d+) resumed\)$")


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


def without_bundle_identity(value: object) -> object:
    if not isinstance(value, dict):
        fail("SNV response is not an object")
    copied = json.loads(json.dumps(value))
    provenance = copied.get("provenance")
    if not isinstance(provenance, dict):
        fail("SNV response provenance is missing")
    bundle_id = provenance.pop("bundle_id", None)
    if not isinstance(bundle_id, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", bundle_id):
        fail("SNV response bundle identity is invalid")
    return copied


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
    model_only_snv = source / "tests/fixtures/executable-release/model-only-snv.jsonl"
    if sha256(model_only_snv) != MODEL_ONLY_SNV_SHA256:
        fail("model-only SNV oracle identity mismatch")


def require_ready(path: pathlib.Path, component_statuses: set[str]) -> None:
    value = read_json(path)
    if not isinstance(value, dict) or value.get("status") != "ready":
        fail(f"qualification state is not ready: {path.name}")
    for component in ("snv", "runtime"):
        state = value.get(component)
        if not isinstance(state, dict) or state.get("status") not in component_statuses:
            fail(f"unexpected {component} state: {path.name}")


def require_progress(path: pathlib.Path, final: pathlib.Path, require_transfer: bool) -> None:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        fail(f"cannot read sync progress: {error}")
    if not lines or lines[0] != "sync: checking snv assets":
        fail("online sync progress did not start with the SNV check")
    transfer_count = 0
    counters: list[tuple[int, int]] = []
    completes: list[tuple[int, int]] = []
    for line in lines:
        transfer = TRANSFER.fullmatch(line)
        if transfer:
            transfer_count += 1
            counters.append((int(transfer.group(1)), int(transfer.group(2))))
        match = COMPLETE.fullmatch(line)
        if match:
            value = (int(match.group(1)), int(match.group(2)))
            completes.append(value)
            counters.append(value)
    if (require_transfer and transfer_count == 0) or len(completes) != 1 \
       or COMPLETE.fullmatch(lines[-1]) is None:
        fail("online sync progress lacks transfer or completion evidence")
    if any(current[0] < previous[0] or current[1] < previous[1]
           for previous, current in zip(counters, counters[1:])):
        fail("online sync progress counters decreased")
    outcome = read_json(final)
    if not isinstance(outcome, dict) or completes[0] != (
        outcome.get("downloaded_bytes"), outcome.get("resumed_bytes")
    ):
        fail("online sync progress totals do not match final JSON")


def http_body(path: pathlib.Path) -> object:
    try:
        response = path.read_bytes()
    except OSError as error:
        fail(f"cannot read HTTP response: {path.name}: {error}")
    head, separator, body = response.partition(b"\r\n\r\n")
    if separator != b"\r\n\r\n" or not head.startswith(b"HTTP/1.1 200 OK\r\n"):
        fail(f"HTTP response is not 200: {path.name}")
    if b"content-type: application/json" not in head.lower():
        fail(f"HTTP response is not JSON: {path.name}")
    try:
        return json.loads(body)
    except json.JSONDecodeError as error:
        fail(f"invalid HTTP JSON: {path.name}: {error}")


def main() -> None:
    if len(sys.argv) not in (3, 4) or (len(sys.argv) == 4 and sys.argv[3] != "--reuse-installed"):
        fail("usage: check-production-qualification.py <OUTPUT_DIR> <SOURCE_TREE> [--reuse-installed]")
    output = pathlib.Path(sys.argv[1])
    source = pathlib.Path(sys.argv[2])
    if not output.is_dir() or output.is_symlink() or not source.is_dir() or source.is_symlink():
        fail("qualification directories are unsafe")

    require_fixture_identities(source)

    reuse_installed = len(sys.argv) == 4
    require_ready(output / "sync-online.json", {"reused"} if reuse_installed else {"installed"})
    require_ready(output / "sync-offline.json", {"reused"})
    require_ready(output / "sync-quiet.json", {"reused"})
    require_ready(output / "status.json", {"ready"})
    if (output / "sync-offline.json").read_bytes() != (output / "sync-quiet.json").read_bytes():
        fail("quiet sync changed final JSON")
    require_progress(output / "sync-online.progress", output / "sync-online.json", not reuse_installed)

    expected_help = {
        "sync": "Usage: pangopup sync [--offline] [--progress | --quiet] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]",
        "status": "Usage: pangopup status [--data-dir <ABSOLUTE_PATH>]",
        "lookup": "Usage: pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] [--model-only] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]",
        "serve": "Usage: pangopup serve [--listen <ADDRESS>] [--data-dir <ABSOLUTE_PATH>] [--model-workers <1..8>] [--model-threads <1..8>] [--model-queue-capacity <1..1024>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]",
    }
    for command, expected in expected_help.items():
        try:
            first = (output / f"help-{command}.txt").read_text(encoding="utf-8").splitlines()[0]
        except (OSError, IndexError) as error:
            fail(f"cannot read focused help: {command}: {error}")
        if first != expected:
            fail(f"focused help mismatch: {command}")

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
    model_only = output / "model-only-SNV.jsonl"
    model_only_expected = source / "tests/fixtures/executable-release/model-only-snv.jsonl"
    if model_only.read_bytes() != model_only_expected.read_bytes():
        fail("model-only SNV oracle mismatch")
    automatic_snv = canonical_snv(output / "snv-ENSG00000010610.jsonl").splitlines()[0]
    if b'"kind":"precomputed"' not in automatic_snv \
       or b'"kind":"model"' not in model_only.read_bytes():
        fail("automatic versus forced SNV provenance mismatch")

    live = http_body(output / "http-livez.txt")
    ready = http_body(output / "http-readyz.txt")
    status = http_body(output / "http-status.txt")
    snv = http_body(output / "http-snv.txt")
    modeled = http_body(output / "http-model.txt")
    forced = http_body(output / "http-model-only.txt")
    model_value = json.loads(model_expected.read_bytes())
    model_only_value = json.loads(model_only_expected.read_bytes())
    if live != {"status": "live"} or not isinstance(ready, dict) or ready.get("status") != "ready":
        fail("HTTP health response mismatch")
    if not isinstance(status, dict) or status.get("version") != "0.2.0" or status.get("readiness") != "ready":
        fail("HTTP status response mismatch")
    automatic_expected = json.loads(
        (source / "tests/fixtures/snv-regression/expected/ENSG00000010610.jsonl")
        .read_text(encoding="utf-8")
        .splitlines()[0]
    )
    if not isinstance(snv, dict) or len(snv.get("results", [])) != 1 \
       or without_bundle_identity(snv["results"][0]) != without_bundle_identity(automatic_expected):
        fail("HTTP SNV response mismatch")
    if modeled != {"results": [model_value]}:
        fail("HTTP model response mismatch")
    if forced != {"results": [model_only_value]}:
        fail("HTTP model-only SNV response mismatch")

    print(f"requests_sha256={REQUESTS_SHA256}")
    print(f"snv_canonical_actual_sha256={hashlib.sha256(combined_actual).hexdigest()}")
    print(f"snv_canonical_expected_sha256={hashlib.sha256(combined_expected).hexdigest()}")
    print(f"model_oracle_sha256={sha256(model_expected)}")
    print(f"model_actual_sha256={sha256(model_actual)}")
    print(f"model_only_snv_actual_sha256={sha256(model_only)}")
    print("http_surface=passed")
    print("production qualification passed")


if __name__ == "__main__":
    main()
