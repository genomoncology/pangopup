#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""Measure current PangoPup process resources against retained production assets."""

from __future__ import annotations

import argparse
import contextlib
import csv
import hashlib
import json
import os
import platform
import selectors
import signal
import socket
import sqlite3
import statistics
import subprocess
import tempfile
import time
import tomllib
import urllib.request
from pathlib import Path
from typing import Any

SCHEMA = "pangopup.runtime-resources.v1"
MODEL_VARIANT = "GRCh38:chr12:6801303:G:GA"
MODEL_GENE = "ENSG00000010610"
SNV_GENE = "ENSG00000000003"
M09_RECORD = {
    "gene": "ENSG00000010610.10",
    "gain_score": "0.00",
    "gain_position": 0,
    "loss_score": "0.00",
    "loss_position": -50,
    "warnings": [],
}
EXPECTED = {
    "snv_bundle_id": "sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3",
    "runtime_profile_id": "sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c",
    "model_bundle_id": "sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43",
    "reference_bundle_id": "sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f",
    "mask_sha256": "sha256:714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702",
}
EXPECTED_CACHE = {
    "snv_profile": "63f3842ea6cb40ebc0a2b6ca23fba4f35d53f829d96c33f597a2c5bcac238ca6",
    "snv_transport_sha256": "f9b7501087226fb35cbfa66fa9b903cc21eb8bbbacb067363b9eeef487ee9e9a",
    "runtime_profile": "d1caf6346bb24378f720056416fa6286f1153ccaf0c6a0778494f557035ef59e",
    "runtime_transport_sha256": "415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3",
}


class ContractError(RuntimeError):
    """The measurement input or observation did not satisfy its contract."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def parse_quantity_lines(text: str, required: tuple[str, ...]) -> dict[str, int]:
    """Parse `Name: integer kB` records used by Linux proc files."""
    parsed: dict[str, int] = {}
    for line in text.splitlines():
        if ":" not in line:
            continue
        name, raw = line.split(":", 1)
        if name not in required:
            continue
        fields = raw.split()
        if len(fields) not in (1, 2) or not fields[0].isdigit():
            raise ContractError(f"malformed {name} field")
        if len(fields) == 2 and fields[1] != "kB":
            raise ContractError(f"unexpected {name} unit")
        parsed[name] = int(fields[0])
    missing = set(required) - parsed.keys()
    if missing:
        raise ContractError(f"missing fields: {', '.join(sorted(missing))}")
    return parsed


def parse_proc_stat(text: str) -> dict[str, int]:
    """Parse process-relative minor/major faults from /proc/PID/stat."""
    close = text.rfind(")")
    if close < 2:
        raise ContractError("malformed /proc stat command field")
    fields = text[close + 2 :].split()
    if len(fields) < 10:
        raise ContractError("truncated /proc stat")
    try:
        return {"minor_faults": int(fields[7]), "major_faults": int(fields[9])}
    except ValueError as error:
        raise ContractError("malformed /proc stat fault counter") from error


def parse_gnu_time(text: str) -> dict[str, int | float]:
    values: dict[str, str] = {}
    for line in text.splitlines():
        if line.startswith("PANGOPUP_") and "=" in line:
            key, value = line.split("=", 1)
            values[key] = value
    required = {
        "PANGOPUP_ELAPSED",
        "PANGOPUP_MAX_RSS_KB",
        "PANGOPUP_MINOR_FAULTS",
        "PANGOPUP_MAJOR_FAULTS",
    }
    if values.keys() != required:
        raise ContractError("GNU time fields are missing, duplicated, or unexpected")
    try:
        return {
            "elapsed_ms": float(values["PANGOPUP_ELAPSED"]) * 1000,
            "max_rss_kb": int(values["PANGOPUP_MAX_RSS_KB"]),
            "minor_faults": int(values["PANGOPUP_MINOR_FAULTS"]),
            "major_faults": int(values["PANGOPUP_MAJOR_FAULTS"]),
        }
    except ValueError as error:
        raise ContractError("malformed GNU time value") from error


def check_fault_progression(samples: list[dict[str, Any]]) -> None:
    previous: dict[int, tuple[int, int]] = {}
    for sample in samples:
        pid = sample["pid"]
        current = (sample["minor_faults"], sample["major_faults"])
        if pid in previous and (
            current[0] < previous[pid][0] or current[1] < previous[pid][1]
        ):
            raise ContractError("fault counter decreased within one process")
        previous[pid] = current


def aggregate(samples: list[dict[str, Any]], field: str) -> dict[str, float | int]:
    values = [sample[field] for sample in samples]
    if not values:
        raise ContractError("cannot aggregate an empty sample")
    return {"median": statistics.median(values), "maximum": max(values)}


def git(repo: Path, *args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=repo, check=True, text=True, capture_output=True
    ).stdout.strip()


def validate_source(repo: Path, expected_commit: str | None) -> str:
    if git(repo, "status", "--porcelain", "--untracked-files=all"):
        raise ContractError("source worktree is dirty")
    commit = git(repo, "rev-parse", "HEAD")
    if len(commit) != 40 or (expected_commit is not None and commit != expected_commit):
        raise ContractError("source commit does not match the run contract")
    return commit


def validate_assets(data: Path) -> dict[str, Any]:
    active = json.loads((data / "active.json").read_text())
    runtime_active = json.loads((data / "runtime/active.json").read_text())
    if active.get("bundle_id") != EXPECTED["snv_bundle_id"]:
        raise ContractError("unexpected active SNV bundle")
    if runtime_active.get("profile_id") != EXPECTED["runtime_profile_id"]:
        raise ContractError("unexpected active runtime profile")
    profile_dir = (
        data / "runtime/profiles" / EXPECTED["runtime_profile_id"].split(":", 1)[1]
    )
    profile = json.loads((profile_dir / "profile.json").read_text())
    actual = {
        "snv_bundle_id": profile["snv"]["bundle_id"],
        "runtime_profile_id": runtime_active["profile_id"],
        "model_bundle_id": profile["model"]["bundle_id"],
        "reference_bundle_id": profile["reference"]["bundle_id"],
        "mask_sha256": profile["mask"]["member_sha256"],
    }
    if actual != EXPECTED:
        raise ContractError("runtime profile identities do not match the run contract")
    if profile["scoring"]["cpu_policy"] != "sequential:1/1":
        raise ContractError("runtime profile is not the portable 1x1 policy")
    receipt = json.loads((profile_dir / "receipt.json").read_text())
    installed: dict[str, int] = {}
    for kind in ("model", "reference", "mask"):
        member = receipt[kind]
        path = data / "runtime" / member["path"]
        if path.is_dir():
            candidates = [
                item
                for item in path.iterdir()
                if item.name not in {"NOTICE", "manifest.json"}
            ]
            if len(candidates) != 1:
                raise ContractError(f"cannot identify installed {kind} member")
            path = candidates[0]
        if not path.is_file() or path.stat().st_size != member["size"]:
            raise ContractError(f"installed {kind} member is absent or has wrong size")
        installed[kind] = member["size"]
    snv_path = (
        data
        / "bundles"
        / EXPECTED["snv_bundle_id"].split(":", 1)[1]
        / "bundle/scores.pgi"
    )
    if not snv_path.is_file() or snv_path.stat().st_size != 15_033_158_255:
        raise ContractError("installed SNV member is absent or has wrong size")
    installed["snv"] = snv_path.stat().st_size
    installed["total"] = sum(installed.values())
    tree_bytes = sum(path.stat().st_size for path in data.rglob("*") if path.is_file())
    return {
        "identities": actual,
        "installed_bytes": installed,
        "installed_tree_bytes": tree_bytes,
    }


def tree_metadata(data: Path) -> list[tuple[str, int, int, int]]:
    return [
        (str(path.relative_to(data)), stat.st_size, stat.st_mtime_ns, stat.st_mode)
        for path in sorted(data.rglob("*"))
        if path.is_file() and (stat := path.stat())
    ]


def authenticated_member(directory: Path, name: str, size: int, digest: str) -> int:
    path = directory / name
    if not path.is_file() or path.stat().st_size != size:
        raise ContractError(f"retained transport member {name} has the wrong size")
    if sha256(path) != digest.removeprefix("sha256:"):
        raise ContractError(f"retained transport member {name} has the wrong digest")
    return size


def validate_cache(cache: Path) -> dict[str, Any]:
    """Authenticate both retained transport profiles and derive their byte totals."""
    snv = cache / "profiles" / EXPECTED_CACHE["snv_profile"] / "transport"
    runtime = cache / "profiles" / EXPECTED_CACHE["runtime_profile"] / "transport"
    snv_manifest_path = snv / "transport.json"
    runtime_manifest_path = runtime / "runtime-transport.json"
    if sha256(snv_manifest_path) != EXPECTED_CACHE["snv_transport_sha256"]:
        raise ContractError("retained SNV transport manifest identity mismatch")
    if sha256(runtime_manifest_path) != EXPECTED_CACHE["runtime_transport_sha256"]:
        raise ContractError("retained runtime transport manifest identity mismatch")
    snv_manifest = json.loads(snv_manifest_path.read_text())
    runtime_manifest = json.loads(runtime_manifest_path.read_text())
    if (
        snv_manifest.get("schema") != "pangopup.snv-transport.v1"
        or snv_manifest.get("bundle", {}).get("bundle_id") != EXPECTED["snv_bundle_id"]
        or runtime_manifest.get("schema") != "pangopup.runtime-transport.v1"
        or runtime_manifest.get("runtime_profile_id") != EXPECTED["runtime_profile_id"]
    ):
        raise ContractError("retained transport manifest contract mismatch")
    snv_members = [
        (
            snv_manifest["bundle"][name]["path"],
            int(snv_manifest["bundle"][name]["size"]),
            snv_manifest["bundle"][name]["sha256"],
        )
        for name in ("manifest", "notice")
    ]
    snv_members.extend(
        (part["path"], int(part["size"]), part["sha256"])
        for part in snv_manifest["payload"]["parts"]
    )
    runtime_members = [
        (member["name"], int(member["stored_bytes"]), member["stored_sha256"])
        for member in runtime_manifest["members"]
    ]
    expected_snv_names = {"transport.json", *(name for name, _, _ in snv_members)}
    expected_runtime_names = {
        "runtime-transport.json",
        *(name for name, _, _ in runtime_members),
    }
    if {path.name for path in snv.iterdir() if path.is_file()} != expected_snv_names:
        raise ContractError("retained SNV transport inventory mismatch")
    if {
        path.name for path in runtime.iterdir() if path.is_file()
    } != expected_runtime_names:
        raise ContractError("retained runtime transport inventory mismatch")
    snv_bytes = snv_manifest_path.stat().st_size + sum(
        authenticated_member(snv, *member) for member in snv_members
    )
    runtime_bytes = runtime_manifest_path.stat().st_size + sum(
        authenticated_member(runtime, *member) for member in runtime_members
    )
    return {
        "profiles": dict(EXPECTED_CACHE),
        "download_bytes": {
            "snv": snv_bytes,
            "runtime": runtime_bytes,
            "total": snv_bytes + runtime_bytes,
        },
    }


def load_workloads(path: Path) -> dict[int, list[str]]:
    groups: dict[int, list[tuple[int, str]]] = {1: [], 10: [], 100: []}
    with path.open(newline="") as source:
        for row in csv.DictReader(source, delimiter="\t"):
            count = int(row["expected_request_count"])
            if count in groups and row["workload_class"] == f"primary-filtered-{count}":
                groups[count].append((int(row["stable_order"]), row["variant"]))
    result: dict[int, list[str]] = {}
    for count, rows in groups.items():
        rows.sort()
        result[count] = [variant for _, variant in rows]
        if len(result[count]) != count or len(set(result[count])) != count:
            raise ContractError(f"manifest does not contain exact {count}-SNV workload")
    return result


def proc_sample(
    pid: int, checkpoint: str, elapsed_ms: float | None = None
) -> dict[str, Any]:
    proc = Path("/proc") / str(pid)
    memory = parse_quantity_lines((proc / "smaps_rollup").read_text(), ("Rss", "Pss"))
    status = parse_quantity_lines((proc / "status").read_text(), ("VmSize", "VmHWM"))
    faults = parse_proc_stat((proc / "stat").read_text())
    return {
        "kind": "service-checkpoint",
        "checkpoint": checkpoint,
        "pid": pid,
        "elapsed_ms": elapsed_ms,
        "rss_kb": memory["Rss"],
        "pss_kb": memory["Pss"],
        "virtual_kb": status["VmSize"],
        "high_water_rss_kb": status["VmHWM"],
        **faults,
    }


def http_json(
    address: str, path: str, body: dict[str, Any] | None = None
) -> tuple[dict[str, Any], float]:
    payload = None if body is None else json.dumps(body, separators=(",", ":")).encode()
    request = urllib.request.Request(
        f"http://{address}{path}",
        data=payload,
        headers={} if payload is None else {"content-type": "application/json"},
        method="GET" if payload is None else "POST",
    )
    started = time.perf_counter()
    with urllib.request.urlopen(request, timeout=180) as response:
        raw = response.read()
        if response.status != 200:
            raise ContractError(f"HTTP {path} returned {response.status}")
    return json.loads(raw), (time.perf_counter() - started) * 1000


def score(
    address: str, variants: list[str], expected_kind: str, gene: str | None = None
) -> tuple[bytes, float]:
    body: dict[str, Any] = {"variants": variants}
    if gene is not None:
        body["gene"] = gene
    value, elapsed = http_json(address, "/v1/score", body)
    results = value.get("results")
    if not isinstance(results, list) or len(results) != len(variants):
        raise ContractError("score response count does not match request")
    if any(result.get("status") != "found" for result in results):
        failures = [
            (variants[index], result.get("status"))
            for index, result in enumerate(results)
            if result.get("status") != "found"
        ]
        raise ContractError(
            f"score response contains non-found results: {failures[:3]}"
        )
    if any(
        result.get("provenance", {}).get("kind") != expected_kind for result in results
    ):
        raise ContractError(f"score response did not retain {expected_kind} provenance")
    if expected_kind == "model":
        if variants != [MODEL_VARIANT] or gene != MODEL_GENE:
            raise ContractError("model measurement is not the pinned M09 request")
        result = results[0]
        if (
            result.get("records") != [M09_RECORD]
            or result.get("source_reference_ambiguities") != []
        ):
            raise ContractError("model result does not match pinned M09 shape")
        provenance = result["provenance"]
        if (
            provenance.get("effective_cpu_policy") != "sequential:1/1"
            or provenance.get("model_bundle_id") != EXPECTED["model_bundle_id"]
            or provenance.get("reference_bundle_id") != EXPECTED["reference_bundle_id"]
            or provenance.get("mask_sha256") != EXPECTED["mask_sha256"]
        ):
            raise ContractError(
                "model result provenance does not match pinned M09 identity"
            )
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode(), elapsed


def read_startup_line(child: Any, timeout: float = 30.0) -> bytes:
    if child.stdout is None:
        raise ContractError("service stdout was not captured")
    selector = selectors.DefaultSelector()
    selector.register(child.stdout, selectors.EVENT_READ)
    deadline = time.monotonic() + timeout
    buffered = bytearray()
    try:
        while b"\n" not in buffered:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not selector.select(remaining):
                raise ContractError(
                    "service did not emit a listening event before deadline"
                )
            chunk = os.read(child.stdout.fileno(), 4096)
            if not chunk:
                raise ContractError("service exited before emitting a listening event")
            buffered.extend(chunk)
            if len(buffered) > 8192:
                raise ContractError("service listening event exceeds 8192 bytes")
    finally:
        selector.close()
    return bytes(buffered.split(b"\n", 1)[0])


def parse_listening_line(line: bytes) -> str:
    try:
        event = json.loads(line)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ContractError("service listening event is malformed") from error
    address = event.get("address") if isinstance(event, dict) else None
    if (
        not isinstance(event, dict)
        or event.get("event") != "listening"
        or not isinstance(address, str)
    ):
        raise ContractError("service listening event has the wrong shape")
    return address


def child_stderr(child: Any) -> str:
    stream = getattr(child, "_pangopup_stderr", None) or getattr(child, "stderr", None)
    if stream is None:
        return ""
    stream.flush()
    stream.seek(0)
    return stream.read().decode(errors="replace").strip()


def stop_service(child: Any, require_success: bool = True) -> None:
    exit_code = child.poll()
    escalated = False
    stderr = ""
    try:
        if exit_code is None:
            child.send_signal(signal.SIGTERM)
            try:
                exit_code = child.wait(timeout=30)
            except subprocess.TimeoutExpired:
                escalated = True
                child.kill()
                try:
                    exit_code = child.wait(timeout=10)
                except subprocess.TimeoutExpired as error:
                    raise ContractError(
                        "service survived SIGKILL escalation"
                    ) from error
        stderr = child_stderr(child)
    finally:
        for stream in (
            getattr(child, "stdout", None),
            getattr(child, "_pangopup_stderr", None),
        ):
            if stream is not None:
                stream.close()
    if require_success and (escalated or exit_code != 0):
        detail = f": {stderr}" if stderr else ""
        raise ContractError(f"service did not stop cleanly (exit {exit_code}){detail}")


def start_service(binary: Path, data: Path, cache: Path) -> tuple[Any, str]:
    stderr = tempfile.TemporaryFile(mode="w+b")
    try:
        child = subprocess.Popen(
            [
                str(binary),
                "serve",
                "--listen",
                "127.0.0.1:0",
                "--data-dir",
                str(data),
                "--model-workers",
                "1",
                "--model-threads",
                "1",
                "--model-cache",
                str(cache),
            ],
            stdout=subprocess.PIPE,
            stderr=stderr,
        )
    except BaseException:
        stderr.close()
        raise
    child._pangopup_stderr = stderr
    try:
        address = parse_listening_line(read_startup_line(child))
        ready, _ = http_json(address, "/readyz")
        if ready != {"status": "ready"}:
            raise ContractError("service did not become ready")
        return child, address
    except BaseException:
        stop_service(child, require_success=False)
        raise


@contextlib.contextmanager
def service_session(binary: Path, data: Path, cache: Path):
    child, address = start_service(binary, data, cache)
    try:
        yield child, address
    except BaseException:
        stop_service(child, require_success=False)
        raise
    else:
        stop_service(child)


def sqlite_entries(path: Path) -> int:
    if not path.is_file():
        raise ContractError("model cache database was not created")
    connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    try:
        row = connection.execute("SELECT count(*) FROM entries").fetchone()
    finally:
        connection.close()
    if row is None:
        raise ContractError("model cache count query returned no row")
    return int(row[0])


def cli_sample(binary: Path, data: Path, variant: str, gene: str) -> dict[str, Any]:
    command = [
        "/usr/bin/time",
        "-f",
        "PANGOPUP_ELAPSED=%e\\nPANGOPUP_MAX_RSS_KB=%M\\nPANGOPUP_MINOR_FAULTS=%R\\nPANGOPUP_MAJOR_FAULTS=%F",
        str(binary),
        "lookup",
        "--data-dir",
        str(data),
        "--variant",
        variant,
        "--gene",
        gene,
        "--format",
        "jsonl",
    ]
    started = time.perf_counter()
    completed = subprocess.run(command, text=True, capture_output=True)
    observer_elapsed_ms = (time.perf_counter() - started) * 1000
    if completed.returncode != 0:
        raise ContractError(f"CLI lookup failed: {completed.stderr.strip()}")
    lines = [json.loads(line) for line in completed.stdout.splitlines() if line]
    if len(lines) != 1 or lines[0].get("provenance", {}).get("kind") != "precomputed":
        raise ContractError(
            f"CLI sample did not return one precomputed result for {gene!r}: {completed.stdout[:300]!r}"
        )
    timing = parse_gnu_time(completed.stderr)
    gnu_elapsed_ms = timing.pop("elapsed_ms")
    return {
        "kind": "cli",
        "checkpoint": "snv-1",
        "elapsed_ms": observer_elapsed_ms,
        "gnu_elapsed_ms": gnu_elapsed_ms,
        **timing,
    }


def cpu_model() -> str:
    for line in Path("/proc/cpuinfo").read_text().splitlines():
        if line.startswith("model name"):
            return line.split(":", 1)[1].strip()
    raise ContractError("Linux /proc/cpuinfo has no model name")


def summarize(samples: list[dict[str, Any]]) -> dict[str, Any]:
    checkpoints = sorted({sample["checkpoint"] for sample in samples})
    result: dict[str, Any] = {}
    for checkpoint in checkpoints:
        selected = [sample for sample in samples if sample["checkpoint"] == checkpoint]
        fields = [
            field
            for field in (
                "elapsed_ms",
                "rss_kb",
                "pss_kb",
                "virtual_kb",
                "high_water_rss_kb",
                "max_rss_kb",
            )
            if all(sample.get(field) is not None for sample in selected)
        ]
        result[checkpoint] = {field: aggregate(selected, field) for field in fields}
    return result


def write_report(path: Path, metadata: dict[str, Any], summary: dict[str, Any]) -> None:
    installed = metadata["assets"]["installed_bytes"]
    downloads = metadata["transport"]["download_bytes"]
    lines = [
        "# Ticket 053 — Current runtime resource measurements",
        "",
        "## Result",
        "",
        "This is a five-round, warm-page-cache observation of the current complete Linux product. It is an observed baseline, not a universal minimum or a cold-cache benchmark.",
        "",
        "## Identity",
        "",
        f"- Commit: `{metadata['commit']}`",
        f"- Binary version: `{metadata['version']}`",
        f"- Binary SHA-256: `{metadata['binary_sha256']}`",
        f"- Host: `{metadata['host']}`",
        f"- Kernel: `{metadata['kernel']}`",
        f"- CPU: `{metadata['cpu']}`",
        f"- Rust: `{metadata['rustc']}`",
        "- Model policy: `sequential:1/1` (one worker, one model thread)",
        "",
        f"- SNV transport manifest SHA-256: `{metadata['transport']['profiles']['snv_transport_sha256']}`",
        f"- Runtime transport manifest SHA-256: `{metadata['transport']['profiles']['runtime_transport_sha256']}`",
        "- Model workload: `M09-insertion-short-plus`, `GRCh38:chr12:6801303:G:GA`, filtered to `ENSG00000010610` with one exact expected record",
        "",
        "## Exact asset sizes",
        "",
        "| Component | Download bytes | Installed runtime-member bytes |",
        "|---|---:|---:|",
        f"| SNV lookup | {downloads['snv']:,} | {installed['snv']:,} |",
        f"| Model/reference/mask | {downloads['runtime']:,} | {installed['model'] + installed['reference'] + installed['mask']:,} |",
        f"| Combined | {downloads['total']:,} | {installed['total']:,} |",
        "",
        f"Including receipts, manifests, notices, and lock files, the complete installed data tree is {metadata['assets']['installed_tree_bytes']:,} bytes.",
        "",
        "The 15 GB SNV index is a fixed-width, direct random-access file. PangoPup maps it into virtual address space; Linux loads only pages that a query touches. The mapped virtual size is therefore not the same thing as RAM use. File-backed resident pages reported in RSS/PSS are reclaimable by the operating system.",
        "",
        "## Measurements",
        "",
        "Medians and maxima are across five fresh-process rounds. Memory values are KiB.",
        "",
        "| Checkpoint | elapsed ms median / max | RSS median / max | PSS median / max | virtual median / max | peak RSS median / max |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for name, values in summary.items():

        def pair(field: str) -> str:
            value = values.get(field)
            return (
                "—"
                if value is None
                else f"{value['median']:,.1f} / {value['maximum']:,.1f}"
            )

        lines.append(
            f"| {name} | {pair('elapsed_ms')} | {pair('rss_kb')} | {pair('pss_kb')} | {pair('virtual_kb')} | {pair('high_water_rss_kb') if 'high_water_rss_kb' in values else pair('max_rss_kb')} |"
        )
    lines += [
        "",
        "## Method and limitations",
        "",
        "- Every round started a fresh service with the production model loaded before the ready checkpoint.",
        "- The same service then served ordered 1-, 10-, and 100-SNV precomputed requests and pinned compatibility case `M09-insertion-short-plus`, filtered to its expected `ENSG00000010610.10` record. A second fresh service reused that round's single SQLite entry.",
        "- Download bytes were derived from and checked against every digest-authenticated member in the retained SNV and runtime transport manifests; they were not copied from documentation constants.",
        "- `/proc/<pid>/smaps_rollup`, `/proc/<pid>/status`, and `/proc/<pid>/stat` supplied service memory and fault counters. GNU `time` observed the short CLI process.",
        "- GNU `time` retained its elapsed counter for the CLI, but it rounded these sub-10-ms processes to 0.00 seconds; the table uses the observer's higher-resolution monotonic wall clock for CLI elapsed time.",
        "- The host page cache was warm and could not be defensibly cleared. Timing is descriptive, not a cold-start guarantee.",
        "- RSS includes reclaimable file-backed mmap pages; PSS apportions shared resident pages. Virtual size includes mappings and is not physical-memory demand.",
        "- The disposable measurement caches were isolated and removed after successful collection. Retained installed data was read only.",
        "",
    ]
    path.write_text("\n".join(lines))


def run(args: argparse.Namespace) -> None:
    if platform.system() != "Linux" or not Path("/proc/self/smaps_rollup").is_file():
        raise ContractError("measurement requires Linux /proc with smaps_rollup")
    repo = args.repo.resolve()
    data = args.data_dir.resolve()
    retained_cache = args.cache_root.resolve()
    commit = validate_source(repo, args.expected_commit)
    assets = validate_assets(data)
    transport = validate_cache(retained_cache)
    installed_before = tree_metadata(data)
    cache_before = tree_metadata(retained_cache)
    manifest = repo / "planning/artifacts/004-query-manifest.tsv"
    workloads = load_workloads(manifest)
    subprocess.run(
        [
            "cargo",
            "build",
            "--release",
            "--locked",
            "-p",
            "pangopup-cli",
            "--bin",
            "pangopup",
        ],
        cwd=repo,
        check=True,
    )
    binary = repo / "target/release/pangopup"
    version_output = subprocess.run(
        [binary, "--version"], check=True, text=True, capture_output=True
    )
    workspace_version = tomllib.loads((repo / "Cargo.toml").read_text())["workspace"][
        "package"
    ]["version"]
    if (
        version_output.stdout.strip() != f"pangopup {workspace_version}"
        or version_output.stderr
    ):
        raise ContractError("binary version output does not match clean source")
    metadata = {
        "kind": "metadata",
        "commit": commit,
        "version": workspace_version,
        "binary_sha256": sha256(binary),
        "host": socket.gethostname(),
        "kernel": platform.release(),
        "cpu": cpu_model(),
        "rustc": subprocess.run(
            ["rustc", "--version"], check=True, text=True, capture_output=True
        ).stdout.strip(),
        "rounds": args.rounds,
        "assets": assets,
        "transport": transport,
        "workloads": {
            "snv_manifest_sha256": "sha256:" + sha256(manifest),
            "snv_request_counts": [1, 10, 100],
            "model_case": {
                "id": "M09-insertion-short-plus",
                "variant": MODEL_VARIANT,
                "gene_filter": MODEL_GENE,
                "expected_record": M09_RECORD,
            },
        },
        "page_cache": "warm",
        "model_policy": "sequential:1/1",
    }
    samples: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="pangopup-ticket-053-") as root_text:
        root = Path(root_text)
        for round_number in range(1, args.rounds + 1):
            cache = root / f"round-{round_number}/model-results.sqlite3"
            cache.parent.mkdir(mode=0o700)
            with service_session(binary, data, cache) as (child, address):
                round_samples = [proc_sample(child.pid, "service-ready")]
                for count in (1, 10, 100):
                    _, elapsed = score(address, workloads[count], "precomputed")
                    round_samples.append(
                        proc_sample(child.pid, f"service-snv-{count}", elapsed)
                    )
                model_bytes, uncached_ms = score(
                    address, [MODEL_VARIANT], "model", MODEL_GENE
                )
                model_sample = proc_sample(
                    child.pid, "service-model-uncached", uncached_ms
                )
                model_sample["result_sha256"] = hashlib.sha256(model_bytes).hexdigest()
                round_samples.append(model_sample)
                check_fault_progression(round_samples)
            if sqlite_entries(cache) != 1:
                raise ContractError(
                    "uncached service did not create exactly one SQLite entry"
                )
            with service_session(binary, data, cache) as (child, address):
                cached_samples = [proc_sample(child.pid, "service-cache-ready")]
                database_before = cache.read_bytes()
                cached_bytes, cached_ms = score(
                    address, [MODEL_VARIANT], "model", MODEL_GENE
                )
                cached_sample = proc_sample(
                    child.pid, "service-model-cached", cached_ms
                )
                cached_sample["result_sha256"] = hashlib.sha256(
                    cached_bytes
                ).hexdigest()
                cached_samples.append(cached_sample)
                if model_bytes != cached_bytes:
                    raise ContractError(
                        "uncached and cached model results are not byte-identical"
                    )
                if cache.read_bytes() != database_before:
                    raise ContractError(
                        "SQLite database bytes changed during cache hit"
                    )
                if cached_ms >= uncached_ms / 10:
                    raise ContractError(
                        "cached model request was not at least ten times faster"
                    )
                check_fault_progression(cached_samples)
            cli = cli_sample(binary, data, workloads[1][0], SNV_GENE)
            for sample in [*round_samples, *cached_samples, cli]:
                sample["round"] = round_number
                samples.append(sample)
    if len(samples) != args.rounds * 8:
        raise ContractError("measurement sample count does not match the contract")
    if tree_metadata(data) != installed_before:
        raise ContractError(
            "retained installed data metadata changed during measurement"
        )
    if tree_metadata(retained_cache) != cache_before:
        raise ContractError(
            "retained transport cache metadata changed during measurement"
        )
    summary = summarize(samples)
    args.output_jsonl.parent.mkdir(parents=True, exist_ok=True)
    with args.output_jsonl.open("w") as output:
        output.write(json.dumps({"schema": SCHEMA, **metadata}, sort_keys=True) + "\n")
        for sample in samples:
            output.write(
                json.dumps({"schema": SCHEMA, **sample}, sort_keys=True) + "\n"
            )
        output.write(
            json.dumps(
                {"schema": SCHEMA, "kind": "summary", "checkpoints": summary},
                sort_keys=True,
            )
            + "\n"
        )
    write_report(args.output_report, metadata, summary)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--data-dir", type=Path, required=True)
    parser.add_argument("--cache-root", type=Path, required=True)
    parser.add_argument("--expected-commit")
    parser.add_argument("--rounds", type=int, default=5, choices=range(1, 11))
    parser.add_argument("--output-jsonl", type=Path, required=True)
    parser.add_argument("--output-report", type=Path, required=True)
    args = parser.parse_args()
    try:
        run(args)
    except (
        ContractError,
        OSError,
        subprocess.SubprocessError,
        json.JSONDecodeError,
    ) as error:
        print(f"measurement rejected: {error}", file=os.sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
