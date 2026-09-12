#!/usr/bin/env python3
"""Measure leakage-free indel catalogue recurrence in a population VCF."""

from __future__ import annotations

import argparse
import bisect
import collections
import gzip
import hashlib
import json
import pathlib
import sys


CURVE_SIZES = (100, 1_000, 10_000, 100_000, 1_000_000, 5_000_000)
DNA = frozenset("ACGT")


def carried_alternates(genotype: str, alternate_count: int) -> set[int]:
    genotype = genotype.replace("|", "/")
    result: set[int] = set()
    for value in genotype.split("/"):
        if value in {"", "."}:
            continue
        allele = int(value)
        if allele < 0 or allele > alternate_count:
            raise ValueError(f"genotype allele {allele} exceeds {alternate_count} alternates")
        if allele:
            result.add(allele)
    return result


def classify_allele(reference: str, alternate: str, maximum: int) -> str:
    if alternate == "*" or alternate.startswith("<") or "[" in alternate or "]" in alternate:
        return "symbolic"
    if not reference or not alternate or not set(reference) <= DNA or not set(alternate) <= DNA:
        return "unsupported_base"
    if len(reference) > maximum or len(alternate) > maximum:
        return "too_long"
    if len(reference) == len(alternate):
        return "snv" if len(reference) == 1 else "equal_length"
    if len(reference) == 1 and len(alternate) > 1 and reference[0] == alternate[0]:
        return "supported_insertion"
    if len(reference) > 1 and len(alternate) == 1 and reference[0] == alternate[0]:
        return "supported_deletion"
    return "unsupported_indel_shape"


def split_samples(populations: dict[str, str], holdout_fraction: float) -> tuple[set[str], set[str]]:
    if not 0.0 < holdout_fraction < 1.0:
        raise ValueError("holdout fraction must be between zero and one")
    by_population: dict[str, list[str]] = collections.defaultdict(list)
    for sample, population in populations.items():
        by_population[population].append(sample)
    train: set[str] = set()
    test: set[str] = set()
    for population in sorted(by_population):
        samples = sorted(
            by_population[population],
            key=lambda sample: (hashlib.sha256(sample.encode()).digest(), sample),
        )
        test_count = max(1, min(len(samples) - 1, round(len(samples) * holdout_fraction)))
        test.update(samples[:test_count])
        train.update(samples[test_count:])
    return train, test


class IntervalIndex:
    def __init__(self, intervals: dict[str, list[tuple[int, int]]]):
        self._intervals: dict[str, tuple[list[int], list[int]]] = {}
        for contig, values in intervals.items():
            merged: list[list[int]] = []
            for start, end in sorted(values):
                if start > end:
                    continue
                if merged and start <= merged[-1][1] + 1:
                    merged[-1][1] = max(merged[-1][1], end)
                else:
                    merged.append([start, end])
            self._intervals[contig] = (
                [value[0] for value in merged],
                [value[1] for value in merged],
            )

    def contains(self, contig: str, position: int) -> bool:
        values = self._intervals.get(contig)
        if values is None:
            return False
        starts, ends = values
        index = bisect.bisect_right(starts, position) - 1
        return index >= 0 and position <= ends[index]

    def overlaps_closed(self, contig: str, start: int, end: int) -> bool:
        values = self._intervals.get(contig)
        if values is None or start > end:
            return False
        starts, ends = values
        index = bisect.bisect_right(starts, end) - 1
        return index >= 0 and ends[index] >= start


class BoundaryIndex:
    def __init__(self, boundaries: dict[str, list[int]]):
        self._boundaries = {
            contig: sorted(set(values)) for contig, values in boundaries.items()
        }

    def has_between(self, contig: str, start: int, end: int) -> bool:
        values = self._boundaries.get(contig, [])
        index = bisect.bisect_left(values, start)
        return index < len(values) and values[index] <= end


def open_text(path: pathlib.Path):
    if path.suffix in {".gz", ".bgz"}:
        return gzip.open(path, "rt", encoding="utf-8", newline="")
    return path.open(encoding="utf-8", newline="")


def read_panel(path: pathlib.Path) -> dict[str, str]:
    populations: dict[str, str] = {}
    with path.open(encoding="utf-8") as handle:
        header = handle.readline().rstrip("\n").split("\t")
        sample_index = header.index("sample")
        population_index = header.index("pop")
        for line in handle:
            fields = line.rstrip("\n").split("\t")
            populations[fields[sample_index]] = fields[population_index]
    if not populations:
        raise ValueError("sample panel is empty")
    return populations


def read_gene_intervals(path: pathlib.Path, contig: str) -> IntervalIndex:
    intervals: dict[str, list[tuple[int, int]]] = collections.defaultdict(list)
    with open_text(path) as handle:
        for line in handle:
            if not line or line.startswith("#"):
                continue
            fields = line.rstrip("\n").split("\t", 8)
            if len(fields) != 9 or fields[0] != contig or fields[2] != "gene":
                continue
            # PangoPup preserves gffutils point-query behavior as (start, end].
            intervals[contig].append((int(fields[3]) + 1, int(fields[4])))
    if not intervals.get(contig):
        raise ValueError(f"no gene intervals found for {contig}")
    return IntervalIndex(intervals)


def read_exon_boundaries(path: pathlib.Path, contig: str) -> BoundaryIndex:
    boundaries: dict[str, list[int]] = collections.defaultdict(list)
    with open_text(path) as handle:
        for line in handle:
            if not line or line.startswith("#"):
                continue
            fields = line.rstrip("\n").split("\t", 8)
            if len(fields) != 9 or fields[0] != contig or fields[2] != "exon":
                continue
            boundaries[contig].extend((int(fields[3]), int(fields[4])))
    if contig not in boundaries:
        raise ValueError(f"no exon boundaries found for {contig}")
    return BoundaryIndex(boundaries)


def digest_names(names: set[str]) -> str:
    payload = "".join(f"{name}\n" for name in sorted(names)).encode()
    return hashlib.sha256(payload).hexdigest()


class VariantStats:
    __slots__ = (
        "key",
        "kind",
        "length",
        "train_carriers",
        "test_carriers",
        "test_by_population",
        "anchor_genic",
        "span_genic",
    )

    def __init__(self, key: tuple[str, int, str, str], kind: str, length: int):
        self.key = key
        self.kind = kind
        self.length = length
        self.train_carriers = 0
        self.test_carriers = 0
        self.test_by_population: collections.Counter[str] = collections.Counter()
        self.anchor_genic = False
        self.span_genic = False


def summarize_variants(variants: list[VariantStats], group_key) -> dict[str, dict[str, int]]:
    grouped: dict[str, collections.Counter[str]] = collections.defaultdict(collections.Counter)
    for variant in variants:
        group = group_key(variant)
        values = grouped[group]
        values["unique_variants"] += 1
        values["training_catalogue_entries"] += variant.train_carriers > 0
        values["test_unique_requests"] += variant.test_carriers > 0
        values["test_carried_requests"] += variant.test_carriers
        if variant.train_carriers:
            values["hits_full_training_catalogue"] += variant.test_carriers
    columns = (
        "unique_variants",
        "training_catalogue_entries",
        "test_unique_requests",
        "test_carried_requests",
        "hits_full_training_catalogue",
    )
    return {
        group: {column: values[column] for column in columns}
        for group, values in sorted(grouped.items())
    }


def curve_for_scope(variants: list[VariantStats], scope: str) -> list[dict[str, object]]:
    if scope == "all_supported":
        selected = variants
    elif scope == "anchor_genic":
        selected = [variant for variant in variants if variant.anchor_genic]
    elif scope == "span_genic":
        selected = [variant for variant in variants if variant.span_genic]
    else:
        raise ValueError(f"unknown scope {scope}")
    denominator = sum(variant.test_carriers for variant in selected)
    test_unique = sum(variant.test_carriers > 0 for variant in selected)
    ranked = sorted(
        (variant for variant in selected if variant.train_carriers > 0),
        key=lambda variant: (-variant.train_carriers, variant.key),
    )
    points = list(CURVE_SIZES)
    if len(ranked) not in points:
        points.append(len(ranked))
    points.sort()
    rows: list[dict[str, object]] = []
    cumulative_hits = 0
    next_point = 0
    for index, variant in enumerate(ranked, start=1):
        cumulative_hits += variant.test_carriers
        while next_point < len(points) and index == points[next_point]:
            rows.append(
                {
                    "scope": scope,
                    "catalogue_entries": index,
                    "test_carried_requests": denominator,
                    "test_unique_requests": test_unique,
                    "hits": cumulative_hits,
                    "hit_rate": cumulative_hits / denominator if denominator else None,
                }
            )
            next_point += 1
    if not ranked:
        rows.append(
            {
                "scope": scope,
                "catalogue_entries": 0,
                "test_carried_requests": denominator,
                "test_unique_requests": test_unique,
                "hits": 0,
                "hit_rate": 0.0 if denominator else None,
            }
        )
    return rows


def analyze(
    args: argparse.Namespace,
) -> tuple[dict[str, object], list[dict[str, object]], list[VariantStats]]:
    populations = read_panel(args.panel)
    train, test = split_samples(populations, args.holdout_fraction)
    genes = read_gene_intervals(args.gtf, args.contig)
    counts: collections.Counter[str] = collections.Counter()
    variants: dict[tuple[str, int, str, str], VariantStats] = {}
    test_by_population: dict[str, collections.Counter[str]] = collections.defaultdict(collections.Counter)
    sample_columns: list[tuple[int, str, str, str]] | None = None

    with open_text(args.vcf) as handle:
        for line in handle:
            if line.startswith("##"):
                continue
            if line.startswith("#CHROM"):
                header = line.rstrip("\n").split("\t")
                vcf_samples = {sample: index for index, sample in enumerate(header[9:], start=9)}
                missing = set(populations) - set(vcf_samples)
                if missing:
                    raise ValueError(f"{len(missing)} panel samples are absent from the VCF")
                sample_columns = []
                for sample in sorted(populations):
                    group = "test" if sample in test else "train"
                    sample_columns.append((vcf_samples[sample], group, populations[sample], sample))
                continue
            if line.startswith("#"):
                continue
            if sample_columns is None:
                raise ValueError("VCF column header is missing")
            counts["input_records"] += 1
            fields = line.rstrip("\n").split("\t")
            contig, position_text, _, reference, alternate_text = fields[:5]
            if contig != args.contig:
                continue
            position = int(position_text)
            alternates = alternate_text.split(",")
            classifications = [
                classify_allele(reference, alternate, args.max_allele_bases)
                for alternate in alternates
            ]
            counts["alternate_alleles"] += len(alternates)
            counts.update(f"allele_{classification}" for classification in classifications)
            supported_indices = {
                index
                for index, classification in enumerate(classifications, start=1)
                if classification in {"supported_insertion", "supported_deletion"}
            }
            if not supported_indices:
                if counts["input_records"] % 100_000 == 0:
                    print(f"read {counts['input_records']} records", file=sys.stderr, flush=True)
                continue
            per_alt_train: collections.Counter[int] = collections.Counter()
            per_alt_test: collections.Counter[int] = collections.Counter()
            per_alt_population: dict[int, collections.Counter[str]] = collections.defaultdict(collections.Counter)
            format_fields = fields[8].split(":")
            gt_index = format_fields.index("GT")
            if len(alternates) == 1 and fields[8] == "GT":
                for column, group, population, _sample in sample_columns:
                    if "1" not in fields[column]:
                        continue
                    if group == "train":
                        per_alt_train[1] += 1
                    else:
                        per_alt_test[1] += 1
                        per_alt_population[1][population] += 1
            else:
                for column, group, population, _sample in sample_columns:
                    sample_fields = fields[column].split(":")
                    genotype = sample_fields[gt_index] if gt_index < len(sample_fields) else "."
                    for alternate_index in carried_alternates(genotype, len(alternates)) & supported_indices:
                        if group == "train":
                            per_alt_train[alternate_index] += 1
                        else:
                            per_alt_test[alternate_index] += 1
                            per_alt_population[alternate_index][population] += 1
            for alternate_index in supported_indices:
                alternate = alternates[alternate_index - 1]
                kind = classifications[alternate_index - 1]
                key = (contig, position, reference, alternate)
                if key in variants:
                    raise ValueError(f"duplicate prepared variant {key}")
                variant = VariantStats(key, kind, abs(len(reference) - len(alternate)))
                variant.train_carriers = per_alt_train[alternate_index]
                variant.test_carriers = per_alt_test[alternate_index]
                variant.test_by_population.update(per_alt_population[alternate_index])
                variant.anchor_genic = genes.contains(contig, position)
                affected_end = position + len(reference) - 1
                variant.span_genic = genes.overlaps_closed(contig, position, affected_end)
                variants[key] = variant
                counts["supported_unique"] += 1
                counts[f"supported_{kind.removeprefix('supported_')}_unique"] += 1
                counts[f"supported_length_{variant.length}"] += 1
                if variant.anchor_genic:
                    counts["anchor_genic_unique"] += 1
                if variant.span_genic:
                    counts["span_genic_unique"] += 1
                for population, carriers in variant.test_by_population.items():
                    test_by_population[population]["requests"] += carriers
                    if variant.train_carriers:
                        test_by_population[population]["hits_full_training_catalogue"] += carriers
            if counts["input_records"] % 100_000 == 0:
                print(f"read {counts['input_records']} records", file=sys.stderr, flush=True)

    variant_values = list(variants.values())
    anchor_genic_variants = [variant for variant in variant_values if variant.anchor_genic]
    curves = [
        row
        for scope in ("all_supported", "anchor_genic", "span_genic")
        for row in curve_for_scope(variant_values, scope)
    ]
    summary: dict[str, object] = {
        "schema": "pangopup-indel-recurrence-v1",
        "contig": args.contig,
        "max_allele_bases": args.max_allele_bases,
        "holdout_fraction": args.holdout_fraction,
        "samples": {
            "panel": len(populations),
            "train": len(train),
            "test": len(test),
            "train_sha256": digest_names(train),
            "test_sha256": digest_names(test),
            "train_by_population": dict(sorted(collections.Counter(populations[s] for s in train).items())),
            "test_by_population": dict(sorted(collections.Counter(populations[s] for s in test).items())),
        },
        "counts": dict(sorted(counts.items())),
        "distribution": {
            "all_supported_by_kind": summarize_variants(variant_values, lambda variant: variant.kind),
            "all_supported_by_length": summarize_variants(variant_values, lambda variant: str(variant.length)),
            "all_supported_by_kind_and_length": summarize_variants(
                variant_values,
                lambda variant: f"{variant.kind}:{variant.length}",
            ),
            "anchor_genic_by_kind": summarize_variants(anchor_genic_variants, lambda variant: variant.kind),
            "anchor_genic_by_length": summarize_variants(anchor_genic_variants, lambda variant: str(variant.length)),
            "anchor_genic_by_kind_and_length": summarize_variants(
                anchor_genic_variants,
                lambda variant: f"{variant.kind}:{variant.length}",
            ),
        },
        "test_by_population": {
            population: dict(sorted(values.items()))
            for population, values in sorted(test_by_population.items())
        },
        "curve": curves,
    }
    return summary, curves, variant_values


def write_outputs(
    args: argparse.Namespace,
    summary: dict[str, object],
    curves: list[dict[str, object]],
    variants: list[VariantStats],
) -> None:
    args.output_json.parent.mkdir(parents=True, exist_ok=True)
    args.output_json.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    args.output_curve.parent.mkdir(parents=True, exist_ok=True)
    with args.output_curve.open("w", encoding="utf-8", newline="") as handle:
        columns = (
            "scope",
            "catalogue_entries",
            "test_carried_requests",
            "test_unique_requests",
            "hits",
            "hit_rate",
        )
        handle.write("\t".join(columns) + "\n")
        for row in curves:
            handle.write("\t".join(str(row[column]) for column in columns) + "\n")
    args.output_variants.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(args.output_variants, "wt", encoding="utf-8", newline="") as handle:
        handle.write(
            "contig\tposition\treference\talternate\tkind\tlength\ttrain_carriers\t"
            "test_carriers\tanchor_genic\tspan_genic\n"
        )
        for variant in sorted(variants, key=lambda variant: variant.key):
            contig, position, reference, alternate = variant.key
            handle.write(
                f"{contig}\t{position}\t{reference}\t{alternate}\t{variant.kind}\t"
                f"{variant.length}\t{variant.train_carriers}\t{variant.test_carriers}\t"
                f"{int(variant.anchor_genic)}\t{int(variant.span_genic)}\n"
            )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vcf", type=pathlib.Path, required=True)
    parser.add_argument("--panel", type=pathlib.Path, required=True)
    parser.add_argument("--gtf", type=pathlib.Path, required=True)
    parser.add_argument("--contig", default="chr22")
    parser.add_argument("--holdout-fraction", type=float, default=0.2)
    parser.add_argument("--max-allele-bases", type=int, default=100)
    parser.add_argument("--output-json", type=pathlib.Path, required=True)
    parser.add_argument("--output-curve", type=pathlib.Path, required=True)
    parser.add_argument("--output-variants", type=pathlib.Path, required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    summary, curves, variants = analyze(args)
    write_outputs(args, summary, curves, variants)


if __name__ == "__main__":
    main()
