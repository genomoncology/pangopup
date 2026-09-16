#!/usr/bin/env python3
"""Summarize SNV-proxy probe JSONL without treating missing data as zero."""

from __future__ import annotations

import argparse
import collections
import json
import pathlib
from typing import Any


WINDOWS = ("anchor", "plus_minus_10")
DIRECTIONS = ("gain", "loss")
CUTOFFS = (0, 5, 10, 20, 50)


def _key(values: Any) -> tuple[str, int, str, str]:
    if not isinstance(values, (list, tuple)) or len(values) != 4:
        raise ValueError("literal_tuple must contain contig, position, reference, alternate")
    return (str(values[0]), int(values[1]), str(values[2]), str(values[3]))


def read_manifest(path: pathlib.Path) -> dict[tuple[str, int, str, str], int]:
    result: dict[tuple[str, int, str, str], int] = {}
    with path.open(encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip():
                continue
            row = json.loads(line)
            key = _key(row.get("literal_tuple"))
            weight = int(row.get("test_carriers", 0))
            if weight < 0 or key in result:
                raise ValueError(f"manifest line {line_number}: invalid or duplicate literal")
            result[key] = weight
    if not result:
        raise ValueError("manifest is empty")
    return result


def _score_row(row: dict[str, Any], manifest: dict[tuple[str, int, str, str], int]) -> tuple[tuple[str, int, str, str], int]:
    key = _key((row.get("contig"), row.get("position"), row.get("reference"), row.get("alternate")))
    if key not in manifest:
        raise ValueError(f"probe literal is absent from manifest: {key}")
    return key, manifest[key]


def _group_stats(rows: list[dict[str, Any]], manifest: dict[tuple[str, int, str, str], int]) -> dict[str, Any]:
    groups: dict[tuple[str, int, str, str], list[dict[str, Any]]] = collections.defaultdict(list)
    status_rows: collections.Counter[str] = collections.Counter()
    status_weight: collections.Counter[str] = collections.Counter()
    for row in rows:
        if row.get("status") is not None:
            key = _key(row.get("input", {}).get("literal_tuple", [])) if "literal_tuple" in row.get("input", {}) else _key((row.get("input", {}).get("contig"), row.get("input", {}).get("position"), row.get("input", {}).get("reference"), row.get("input", {}).get("alternate")))
            if key not in manifest:
                raise ValueError(f"status literal is absent from manifest: {key}")
            status = str(row["status"])
            status_rows[status] += 1
            status_weight[status] += manifest[key]
            continue
        key, _ = _score_row(row, manifest)
        groups[key].append(row)

    by_kind: dict[str, dict[str, Any]] = {}
    for key in sorted(set(manifest) | set(groups)):
        scored = groups.get(key, [])
        kind = scored[0].get("kind") if scored else ("insertion" if len(key[3]) > len(key[2]) else "deletion")
        item = by_kind.setdefault(kind, {"distinct_variants": 0, "carrier_weight": 0, "windows": {w: {"all_gene_complete": 0, "all_gene_complete_carrier_weight": 0, "incomplete_reasons": {reason: {"distinct": 0, "test_carriers": 0} for reason in ("missing_records", "ambiguous_records", "other_gene_records", "no_gene_row")}, "all_zero_gain": 0, "all_zero_gain_carrier_weight": 0, "all_zero_loss": 0, "all_zero_loss_carrier_weight": 0, "all_zero_combined": 0, "all_zero_combined_carrier_weight": 0, "cutoff_potential": {str(c): {direction: {"distinct": 0, "test_carriers": 0} for direction in (*DIRECTIONS, "combined")} for c in CUTOFFS}, "gain_max_hundredths": collections.Counter(), "loss_max_hundredths": collections.Counter() } for w in WINDOWS}})
        item["distinct_variants"] += 1
        item["carrier_weight"] += manifest[key]
        for window in WINDOWS:
            complete = bool(scored) and all(bool(row.get(window, {}).get("complete")) for row in scored)
            metric = item["windows"][window]
            metric["all_gene_complete"] += int(complete)
            metric["all_gene_complete_carrier_weight"] += manifest[key] if complete else 0
            if not complete:
                for reason in metric["incomplete_reasons"]:
                    affected = (
                        not scored
                        if reason == "no_gene_row"
                        else any(row.get(window, {}).get(reason, 0) > 0 for row in scored)
                    )
                    if affected:
                        bucket = metric["incomplete_reasons"][reason]
                        bucket["distinct"] += 1
                        bucket["test_carriers"] += manifest[key]
                continue
            for direction in DIRECTIONS:
                direction_row = [row[window][direction] for row in scored]
                if all(value.get("max_hundredths") == 0 for value in direction_row):
                    metric[f"all_zero_{direction}"] += 1
                    metric[f"all_zero_{direction}_carrier_weight"] += manifest[key]
                for value in direction_row:
                    maximum = value.get("max_hundredths")
                    if maximum is not None and maximum > 0:
                        metric[f"{direction}_max_hundredths"][str(maximum)] += 1
            if all(
                row[window]["gain"].get("max_hundredths") == 0
                and row[window]["loss"].get("max_hundredths") == 0
                for row in scored
            ):
                metric["all_zero_combined"] += 1
                metric["all_zero_combined_carrier_weight"] += manifest[key]
            maxima = {
                direction: max(row[window][direction]["max_hundredths"] for row in scored)
                for direction in DIRECTIONS
            }
            for cutoff in CUTOFFS:
                for direction in (*DIRECTIONS, "combined"):
                    selected = (
                        all(value <= cutoff for value in maxima.values())
                        if direction == "combined"
                        else maxima[direction] <= cutoff
                    )
                    if selected:
                        bucket = metric["cutoff_potential"][str(cutoff)][direction]
                        bucket["distinct"] += 1
                        bucket["test_carriers"] += manifest[key]

    def clean(value: Any) -> Any:
        if isinstance(value, collections.Counter):
            return dict(sorted(value.items(), key=lambda item: int(item[0])))
        if isinstance(value, dict):
            return {key: clean(item) for key, item in value.items()}
        return value

    return clean({"distinct_variants": len(manifest), "carrier_weight": sum(manifest.values()), "by_kind": by_kind, "non_score_statuses": {"distinct_rows": dict(sorted(status_rows.items())), "carrier_weight": dict(sorted(status_weight.items()))}})


def analyze(probe_path: pathlib.Path, manifest_path: pathlib.Path) -> dict[str, Any]:
    manifest = read_manifest(manifest_path)
    rows = [json.loads(line) for line in probe_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    return {"schema": "pangopup-snv-proxy-analysis-v1", "probe_rows": len(rows), **_group_stats(rows, manifest)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", type=pathlib.Path, required=True)
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    args.output.write_text(json.dumps(analyze(args.probe, args.manifest), indent=2, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
