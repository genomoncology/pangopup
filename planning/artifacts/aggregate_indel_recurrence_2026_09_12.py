#!/usr/bin/env python3
"""Combine per-chromosome recurrence outputs into one global catalogue curve."""

from __future__ import annotations

import argparse
import collections
import gzip
import importlib.util
import json
import pathlib


RECURRENCE_PATH = pathlib.Path(__file__).with_name("indel_recurrence_2026_09_12.py")
RECURRENCE_SPEC = importlib.util.spec_from_file_location("indel_recurrence", RECURRENCE_PATH)
RECURRENCE = importlib.util.module_from_spec(RECURRENCE_SPEC)
assert RECURRENCE_SPEC.loader is not None
RECURRENCE_SPEC.loader.exec_module(RECURRENCE)
VariantStats = RECURRENCE.VariantStats
curve_for_scope = RECURRENCE.curve_for_scope
summarize_variants = RECURRENCE.summarize_variants


def read_variants(path: pathlib.Path) -> list[VariantStats]:
    result: list[VariantStats] = []
    with gzip.open(path, "rt", encoding="utf-8") as handle:
        header = handle.readline().rstrip("\n").split("\t")
        columns = {name: index for index, name in enumerate(header)}
        for line in handle:
            fields = line.rstrip("\n").split("\t")
            key = (
                fields[columns["contig"]],
                int(fields[columns["position"]]),
                fields[columns["reference"]],
                fields[columns["alternate"]],
            )
            variant = VariantStats(
                key,
                fields[columns["kind"]],
                int(fields[columns["length"]]),
            )
            variant.train_carriers = int(fields[columns["train_carriers"]])
            variant.test_carriers = int(fields[columns["test_carriers"]])
            variant.anchor_genic = fields[columns["anchor_genic"]] == "1"
            variant.span_genic = fields[columns["span_genic"]] == "1"
            result.append(variant)
    return result


def aggregate(
    variant_paths: list[pathlib.Path],
    summary_paths: list[pathlib.Path],
) -> tuple[dict[str, object], list[dict[str, object]]]:
    if not variant_paths or len(variant_paths) != len(summary_paths):
        raise ValueError("variant and summary inputs must be nonempty and paired")
    summaries = [json.loads(path.read_text(encoding="utf-8")) for path in summary_paths]
    for summary in summaries:
        if summary.get("schema") != "pangopup-indel-recurrence-v1":
            raise ValueError("unexpected recurrence summary schema")
    expected_samples = summaries[0]["samples"]
    if any(summary["samples"] != expected_samples for summary in summaries[1:]):
        raise ValueError("chromosome sample splits differ")
    variants: list[VariantStats] = []
    contigs: list[str] = []
    seen: set[tuple[str, int, str, str]] = set()
    for variant_path, summary in zip(variant_paths, summaries):
        current = read_variants(variant_path)
        current_contigs = {variant.key[0] for variant in current}
        if current_contigs != {summary["contig"]}:
            raise ValueError("variant rows do not match their recurrence summary contig")
        for variant in current:
            if variant.key in seen:
                raise ValueError(f"duplicate variant across inputs: {variant.key}")
            seen.add(variant.key)
        variants.extend(current)
        contigs.append(summary["contig"])
    if len(set(contigs)) != len(contigs):
        raise ValueError("duplicate summary contig")
    curves = [
        row
        for scope in ("all_supported", "anchor_genic", "span_genic")
        for row in curve_for_scope(variants, scope)
    ]
    counts: collections.Counter[str] = collections.Counter()
    population_counts: dict[str, collections.Counter[str]] = collections.defaultdict(collections.Counter)
    for summary in summaries:
        counts.update(summary["counts"])
        for population, values in summary["test_by_population"].items():
            population_counts[population].update(values)
    anchor_genic = [variant for variant in variants if variant.anchor_genic]
    result: dict[str, object] = {
        "schema": "pangopup-indel-recurrence-global-v1",
        "contigs": sorted(contigs, key=lambda value: int(value.removeprefix("chr"))),
        "samples": expected_samples,
        "counts": dict(sorted(counts.items())),
        "distribution": {
            "all_supported_by_kind": summarize_variants(variants, lambda variant: variant.kind),
            "all_supported_by_length": summarize_variants(variants, lambda variant: str(variant.length)),
            "anchor_genic_by_kind": summarize_variants(anchor_genic, lambda variant: variant.kind),
            "anchor_genic_by_length": summarize_variants(anchor_genic, lambda variant: str(variant.length)),
            "anchor_genic_by_kind_and_length": summarize_variants(
                anchor_genic,
                lambda variant: f"{variant.kind}:{variant.length}",
            ),
        },
        "test_by_population": {
            population: dict(sorted(values.items()))
            for population, values in sorted(population_counts.items())
        },
        "curve": curves,
    }
    return result, curves


def write_outputs(
    output_json: pathlib.Path,
    output_curve: pathlib.Path,
    summary: dict[str, object],
    curves: list[dict[str, object]],
) -> None:
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    output_curve.parent.mkdir(parents=True, exist_ok=True)
    columns = (
        "scope",
        "catalogue_entries",
        "test_carried_requests",
        "test_unique_requests",
        "hits",
        "hit_rate",
    )
    with output_curve.open("w", encoding="utf-8", newline="") as handle:
        handle.write("\t".join(columns) + "\n")
        for row in curves:
            handle.write("\t".join(str(row[column]) for column in columns) + "\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--variants", nargs="+", type=pathlib.Path, required=True)
    parser.add_argument("--summaries", nargs="+", type=pathlib.Path, required=True)
    parser.add_argument("--output-json", type=pathlib.Path, required=True)
    parser.add_argument("--output-curve", type=pathlib.Path, required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    summary, curves = aggregate(args.variants, args.summaries)
    write_outputs(args.output_json, args.output_curve, summary, curves)


if __name__ == "__main__":
    main()
