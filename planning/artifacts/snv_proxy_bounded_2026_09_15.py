#!/usr/bin/env python3
"""Compare the exact loss-zero bound with complete SNV neighborhoods."""

from __future__ import annotations

import argparse
import collections
import importlib.util
import json
import pathlib
from typing import Any


RECURRENCE = pathlib.Path(__file__).with_name("indel_recurrence_2026_09_12.py")
SPEC = importlib.util.spec_from_file_location("indel_recurrence", RECURRENCE)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)
BoundaryIndex = MODULE.BoundaryIndex
classify_allele = MODULE.classify_allele
read_exon_boundaries = MODULE.read_exon_boundaries


def rows(path: pathlib.Path):
    with path.open(encoding="utf-8") as source:
        for number, line in enumerate(source, 1):
            if line.strip():
                try:
                    value = json.loads(line)
                except json.JSONDecodeError as error:
                    raise ValueError(f"{path}:{number}: invalid JSON") from error
                if not isinstance(value, dict):
                    raise ValueError(f"{path}:{number}: expected JSON object")
                yield value


def literal(value: Any) -> tuple[str, int, str, str]:
    if not isinstance(value, (list, tuple)) or len(value) != 4:
        raise ValueError("literal_tuple must contain contig, position, reference, alternate")
    contig, position, reference, alternate = value
    if not isinstance(contig, str) or not isinstance(position, int):
        raise ValueError("literal has invalid contig or position")
    if not isinstance(reference, str) or not isinstance(alternate, str):
        raise ValueError("literal has invalid allele")
    return contig, position, reference, alternate


def row_literal(row: dict[str, Any]) -> tuple[str, int, str, str]:
    source = row.get("input", row)
    value = source.get("literal_tuple")
    if value is None:
        value = [source.get(name) for name in ("contig", "position", "reference", "alternate")]
    return literal(value)


def exact_loss_zero(boundaries: BoundaryIndex, key: tuple[str, int, str, str]) -> bool:
    """Return the conservative no-boundary proof for the prior transfer window."""
    contig, position, reference, _ = key
    return not boundaries.has_between(contig, position - 50, position + 49 + len(reference))


def metric() -> dict[str, int]:
    return {"distinct": 0, "test_carriers": 0}


def analyze(manifest_path: pathlib.Path, probe_path: pathlib.Path, gtf_path: pathlib.Path) -> dict[str, Any]:
    manifest: dict[tuple[str, int, str, str], int] = {}
    for row in rows(manifest_path):
        key = literal(row.get("literal_tuple"))
        weight = int(row.get("test_carriers", 0))
        if weight < 0 or key in manifest:
            raise ValueError(f"duplicate literal or negative weight: {key}")
        manifest[key] = weight
    if not manifest:
        raise ValueError("manifest is empty")

    grouped: dict[tuple[str, int, str, str], list[dict[str, Any]]] = collections.defaultdict(list)
    status_keys: set[tuple[str, int, str, str]] = set()
    for row in rows(probe_path):
        key = row_literal(row)
        if key not in manifest:
            raise ValueError(f"probe literal is absent from manifest: {key}")
        # Status, missing, and ambiguous rows never establish a zero result.
        if row.get("status") is None:
            grouped[key].append(row)
        else:
            status_keys.add(key)

    boundaries = read_exon_boundaries(gtf_path, next(iter(manifest))[0])
    by_kind: dict[str, dict[str, Any]] = {}
    for key, weight in sorted(manifest.items()):
        kind_value = classify_allele(key[2], key[3], max(len(key[2]), len(key[3])))
        if kind_value not in {"supported_insertion", "supported_deletion"}:
            raise ValueError(f"manifest literal is not a supported indel: {key}")
        kind = "insertion" if kind_value == "supported_insertion" else "deletion"
        bucket = by_kind.setdefault(kind, {"denominator": metric(), "rules": {}})
        bucket["denominator"]["distinct"] += 1
        bucket["denominator"]["test_carriers"] += weight
        exact = exact_loss_zero(boundaries, key)
        probe_rows = grouped.get(key, [])
        complete = key not in status_keys and bool(probe_rows) and all(bool(row.get("plus_minus_10", {}).get("complete")) for row in probe_rows)
        loss_zero = complete and all(row["plus_minus_10"].get("loss", {}).get("max_hundredths") == 0 for row in probe_rows)
        combined_zero = complete and all(
            row["plus_minus_10"].get(direction, {}).get("max_hundredths") == 0
            for row in probe_rows for direction in ("gain", "loss")
        )
        flags = {
            "exact_loss_baseline": exact,
            "snv_losszero": loss_zero,
            "overlap": exact and loss_zero,
            "snv_only_incremental_beyond_exact_loss": loss_zero and not exact,
            "combined_snv_zero_beyond_exact_loss": combined_zero and not exact,
        }
        for name, selected in flags.items():
            cell = bucket["rules"].setdefault(name, metric())
            if selected:
                cell["distinct"] += 1
                cell["test_carriers"] += weight

    return {"schema": "pangopup-snv-proxy-bounded-v1", "by_kind": by_kind}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    parser.add_argument("--probe", type=pathlib.Path, required=True)
    parser.add_argument("--gencode", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    args.output.write_text(json.dumps(analyze(args.manifest, args.probe, args.gencode), indent=2, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
