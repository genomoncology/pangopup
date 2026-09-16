#!/usr/bin/env python3
"""Compare a fixed model-labelled indel panel with complete nearby SNV rows."""

from __future__ import annotations

import argparse
from collections import defaultdict
from decimal import Decimal, InvalidOperation
import json
from pathlib import Path


WINDOWS = ("anchor", "plus_minus_10")
TARGETS = (1, 5, 10, 20, 50)
CUTOFFS = (0, 5, 10, 20, 50)
RULES = ("gain", "loss", "combined")


def literal(values):
    if not isinstance(values, (list, tuple)) or len(values) != 4:
        raise ValueError("literal_tuple must have four fields")
    return (str(values[0]), int(values[1]), str(values[2]), str(values[3]))


def row_literal(row, ref="reference", alt="alternate"):
    return literal((row.get("contig"), row.get("position"), row.get(ref), row.get(alt)))


def stable_gene(value):
    if not isinstance(value, str) or not value:
        raise ValueError("missing gene identity")
    return value.split(".", 1)[0]


def rows(path):
    with path.open(encoding="utf-8") as source:
        for number, line in enumerate(source, 1):
            if line.strip():
                try:
                    yield json.loads(line)
                except json.JSONDecodeError as error:
                    raise ValueError(f"{path}:{number}: invalid JSON") from error


def score_magnitude(value, direction):
    try:
        amount = Decimal(str(value)) * 100
    except (InvalidOperation, TypeError) as error:
        raise ValueError(f"invalid {direction} score: {value!r}") from error
    if amount != amount.to_integral_value() or abs(amount) > 100:
        raise ValueError(f"invalid {direction} score hundredths: {value!r}")
    if direction == "gain" and amount < 0 or direction == "loss" and amount > 0:
        raise ValueError(f"unexpected signed {direction} score: {value!r}")
    return int(abs(amount))


def metric():
    return {"distinct": 0, "test_carriers": 0}


def add(bucket, weight):
    bucket["distinct"] += 1
    bucket["test_carriers"] += weight


def analyze(manifest_path, model_path, census_path):
    manifest = {}
    for row in rows(manifest_path):
        key = literal(row.get("literal_tuple"))
        weight = int(row["test_carriers"])
        if key in manifest or weight < 0:
            raise ValueError(f"duplicate literal or negative weight in manifest: {key}")
        manifest[key] = weight
    if not manifest:
        raise ValueError("empty manifest")

    models = {}
    for row in rows(model_path):
        key = row_literal(row, "ref", "alt")
        if key not in manifest or key in models:
            raise ValueError(f"unselected or duplicate model literal: {key}")
        models[key] = row

    census = defaultdict(dict)
    for row in rows(census_path):
        source = row.get("input", row) if "status" in row else row
        key = row_literal(source)
        if key not in manifest:
            continue  # The census is larger than the nested labelled panel.
        if "status" in row:
            continue  # A failure cannot establish a complete gene neighborhood.
        gene = stable_gene(row.get("gene"))
        if gene in census[key]:
            raise ValueError(f"duplicate census gene for {key}: {gene}")
        census[key][gene] = row

    by_kind = {}
    details = []
    for key, weight in manifest.items():
        kind = "insertion" if len(key[3]) > len(key[2]) else "deletion"
        group = by_kind.setdefault(kind, {"panel": metric(), "model_missing": metric(),
                                          "model_failed": metric(), "windows": {}})
        add(group["panel"], weight)
        model = models.get(key)
        valid = bool(model and model.get("status") == "found" and model.get("records"))
        genes = {}
        if valid:
            try:
                for record in model["records"]:
                    gene = stable_gene(record.get("stable_gene") or record.get("gene"))
                    if gene in genes:
                        raise ValueError(f"duplicate model gene for {key}: {gene}")
                    genes[gene] = {
                        "gain": score_magnitude(record.get("gain_score"), "gain"),
                        "loss": score_magnitude(record.get("loss_score"), "loss"),
                    }
            except (KeyError, TypeError, ValueError):
                valid = False
                genes = {}
        if not valid:
            add(group["model_failed" if model else "model_missing"], weight)
        detail = {"literal_tuple": list(key), "kind": kind, "test_carriers": weight,
                  "model_status": model.get("status", "missing") if model else "missing",
                  "model_route": model.get("provenance") if model else None,
                  "model_genes": genes, "windows": {}}
        for window in WINDOWS:
            summary = group["windows"].setdefault(window, {
                "eligible_complete": metric(), "incomplete_or_missing_snv": metric(),
                "rules": {rule: {str(c): {str(t): {name: metric() for name in
                    ("fast_below_threshold", "false_dismissals", "fast_above_cutoff", "true_positives")}
                    for t in TARGETS} for c in CUTOFFS} for rule in RULES},
            })
            matching = census.get(key, {})
            complete = valid and all(
                gene in matching and matching[gene].get(window, {}).get("complete") is True
                and all(isinstance(matching[gene][window].get(direction, {}).get("max_hundredths"), int)
                        for direction in ("gain", "loss"))
                for gene in genes
            )
            if not complete:
                add(summary["incomplete_or_missing_snv"], weight)
                detail["windows"][window] = {"eligible_complete": False}
                continue
            add(summary["eligible_complete"], weight)
            maxima = {direction: max(matching[gene][window][direction]["max_hundredths"]
                                     for gene in genes) for direction in ("gain", "loss")}
            model_maxima = {direction: max(genes[gene][direction] for gene in genes)
                            for direction in ("gain", "loss")}
            detail["windows"][window] = {"eligible_complete": True,
                "snv_max_hundredths": maxima, "model_max_hundredths": model_maxima,
                "snv_routes": {gene: {
                    "snv_bundle_id": matching[gene].get("snv_bundle_id"),
                    "reference_bundle_id": matching[gene].get("reference_bundle_id"),
                } for gene in genes}}
            for rule in RULES:
                snv_max = max(maxima.values()) if rule == "combined" else maxima[rule]
                model_max = max(model_maxima.values()) if rule == "combined" else model_maxima[rule]
                for cutoff in CUTOFFS:
                    for target in TARGETS:
                        cell = summary["rules"][rule][str(cutoff)][str(target)]
                        if snv_max <= cutoff:
                            add(cell["fast_below_threshold"], weight)
                            if model_max >= target:
                                add(cell["false_dismissals"], weight)
                        else:
                            add(cell["fast_above_cutoff"], weight)
                            if model_max >= target:
                                add(cell["true_positives"], weight)
        details.append(detail)
    return {"schema": "pangopup-snv-proxy-paired-v2",
            "cutoff_rule": "gain/loss: matching-gene SNV direction max <= cutoff; combined: both direction maxima <= cutoff",
            "target_rule": "gain/loss: matching full-model direction max >= target; combined: either direction max >= target",
            "by_kind": by_kind, "variants": details}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--census", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.write_text(json.dumps(analyze(args.manifest, args.model, args.census),
                                      indent=2, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
