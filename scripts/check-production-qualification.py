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
M09_SHA256 = "672af56707925ce071c808ab3dbef78cad39610efd5c20b10a2304425409c3ee"
MODEL_ONLY_SNV_SHA256 = "30dd2df1d4753f3c7f781ec4c6dc4d801f4672e2561e63174bc904a2c5101df3"
EXPECTED_SHA256 = {
    "ENSG00000010610": "4d1cb8886326e7cda154ffd0f694e230bdc686bd6888b5fcf9f426ae8bd06202",
    "ENSG00000141499": "57c816ccd474cacd4002ce8cd9c4020512f16d9d9c0f910674b33977ecbd7180",
    "ENSG00000141510": "80ba6cd7e1f716ab782a6dd28d6f4a33dde2e4d63b03cf1810ac9fe95eb8d40e",
    "ENSG00000169129": "8bff19032e3f7d1d5815f591074195a83ceaacf1da3b1ab95187abf798ff1e16",
    "ENSG00000175727": "3feed9aea064990b05120e3559af4ab63fc085f49e1f0aeef88ef1f846a8a3d5",
    "ENSG00000185974": "8d40ca2809c0e04c847fc7d12c2b49557b0a9394128a39d89b730a52ab801d3f",
    "unfiltered": "3d70dab863439ab57c2702dcde7af36732dac763844085687506b9ca25294a58",
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
    if not isinstance(status, dict) or status.get("version") != "0.4.0" or status.get("readiness") != "ready":
        fail("HTTP status response mismatch")
    automatic_expected = json.loads(
        (source / "tests/fixtures/snv-regression/expected/ENSG00000010610.jsonl")
        .read_text(encoding="utf-8")
        .splitlines()[0]
    )
    if automatic_expected["records"][0]["stable_gene"] \
       != model_only_value["records"][0]["stable_gene"]:
        fail("automatic and forced routes disagree on stable gene")
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
