#!/usr/bin/env python3
"""Measure literal indel catalogue transfer without retaining person identifiers."""

from __future__ import annotations

import argparse
import collections
import gzip
import importlib.util
import itertools
import json
import pathlib
import statistics
import subprocess


RECURRENCE_PATH = pathlib.Path(__file__).with_name("indel_recurrence_2026_09_12.py")
RECURRENCE_SPEC = importlib.util.spec_from_file_location("indel_recurrence", RECURRENCE_PATH)
RECURRENCE = importlib.util.module_from_spec(RECURRENCE_SPEC)
assert RECURRENCE_SPEC.loader is not None
RECURRENCE_SPEC.loader.exec_module(RECURRENCE)
IntervalIndex = RECURRENCE.IntervalIndex
BoundaryIndex = RECURRENCE.BoundaryIndex
carried_alternates = RECURRENCE.carried_alternates
classify_allele = RECURRENCE.classify_allele
open_text = RECURRENCE.open_text
read_gene_intervals = RECURRENCE.read_gene_intervals
read_exon_boundaries = RECURRENCE.read_exon_boundaries


def info_value(info: str, name: str) -> str | None:
    marker = f"{name}="
    if info.startswith(marker):
        start = 0
    else:
        marker_start = info.find(f";{marker}")
        if marker_start < 0:
            return None
        start = marker_start + 1
    end = info.find(";", start)
    return info[start + len(marker) : end if end >= 0 else None]


def positive_info_value(info: str, name: str) -> bool:
    value = info_value(info, name)
    if value is None:
        return False
    for item in value.strip("[]'\"").split(","):
        try:
            if float(item) > 0:
                return True
        except ValueError:
            continue
    return False


def measure_lines(lines, catalogue, genes, boundaries, contig: str, maximum: int) -> dict[str, object]:
    sample_count = None
    requests: list[set[tuple[str, int, str, str]]] = []
    hits: list[set[tuple[str, int, str, str]]] = []
    pass_requests: list[set[tuple[str, int, str, str]]] = []
    pass_hits: list[set[tuple[str, int, str, str]]] = []
    gnomad_genomes: list[set[tuple[str, int, str, str]]] = []
    gnomad_any: list[set[tuple[str, int, str, str]]] = []
    catalogue_or_gnomad_genomes: list[set[tuple[str, int, str, str]]] = []
    catalogue_or_gnomad_any: list[set[tuple[str, int, str, str]]] = []
    pass_gnomad_genomes: list[set[tuple[str, int, str, str]]] = []
    pass_gnomad_any: list[set[tuple[str, int, str, str]]] = []
    pass_catalogue_or_gnomad_genomes: list[set[tuple[str, int, str, str]]] = []
    pass_catalogue_or_gnomad_any: list[set[tuple[str, int, str, str]]] = []
    exact_loss_zero: list[set[tuple[str, int, str, str]]] = []
    catalogue_or_exact_loss: list[set[tuple[str, int, str, str]]] = []
    gnomad_genomes_or_exact_loss: list[set[tuple[str, int, str, str]]] = []
    gnomad_any_or_exact_loss: list[set[tuple[str, int, str, str]]] = []
    pass_exact_loss_zero: list[set[tuple[str, int, str, str]]] = []
    pass_catalogue_or_exact_loss: list[set[tuple[str, int, str, str]]] = []
    pass_gnomad_genomes_or_exact_loss: list[set[tuple[str, int, str, str]]] = []
    pass_gnomad_any_or_exact_loss: list[set[tuple[str, int, str, str]]] = []
    rejections: collections.Counter[str] = collections.Counter()
    for line in lines:
        if line.startswith("##"):
            continue
        if line.startswith("#CHROM"):
            sample_count = len(line.rstrip("\n").split("\t")) - 9
            requests = [set() for _ in range(sample_count)]
            hits = [set() for _ in range(sample_count)]
            pass_requests = [set() for _ in range(sample_count)]
            pass_hits = [set() for _ in range(sample_count)]
            gnomad_genomes = [set() for _ in range(sample_count)]
            gnomad_any = [set() for _ in range(sample_count)]
            catalogue_or_gnomad_genomes = [set() for _ in range(sample_count)]
            catalogue_or_gnomad_any = [set() for _ in range(sample_count)]
            pass_gnomad_genomes = [set() for _ in range(sample_count)]
            pass_gnomad_any = [set() for _ in range(sample_count)]
            pass_catalogue_or_gnomad_genomes = [set() for _ in range(sample_count)]
            pass_catalogue_or_gnomad_any = [set() for _ in range(sample_count)]
            exact_loss_zero = [set() for _ in range(sample_count)]
            catalogue_or_exact_loss = [set() for _ in range(sample_count)]
            gnomad_genomes_or_exact_loss = [set() for _ in range(sample_count)]
            gnomad_any_or_exact_loss = [set() for _ in range(sample_count)]
            pass_exact_loss_zero = [set() for _ in range(sample_count)]
            pass_catalogue_or_exact_loss = [set() for _ in range(sample_count)]
            pass_gnomad_genomes_or_exact_loss = [set() for _ in range(sample_count)]
            pass_gnomad_any_or_exact_loss = [set() for _ in range(sample_count)]
            continue
        if line.startswith("#"):
            continue
        if sample_count is None:
            raise ValueError("VCF column header is missing")
        fields = line.rstrip("\n").split("\t")
        record_contig, position_text, _, reference, alternate_text, _, filter_value, info = fields[:8]
        if record_contig != contig:
            continue
        position = int(position_text)
        alternates = alternate_text.split(",")
        classifications = [classify_allele(reference, alternate, maximum) for alternate in alternates]
        in_gnomad_genomes = positive_info_value(info, "GnomAD_Genomes_AC")
        in_gnomad_any = in_gnomad_genomes or positive_info_value(info, "GnomAD_AC")
        format_fields = fields[8].split(":")
        gt_index = format_fields.index("GT")
        for sample_index, sample in enumerate(fields[9:]):
            sample_fields = sample.split(":")
            genotype = sample_fields[gt_index] if gt_index < len(sample_fields) else "."
            for alternate_index in carried_alternates(genotype, len(alternates)):
                classification = classifications[alternate_index - 1]
                if classification not in {"supported_insertion", "supported_deletion"}:
                    rejections[classification] += 1
                    continue
                if not genes.contains(contig, position):
                    continue
                key = (contig, position, reference, alternates[alternate_index - 1])
                loss_is_exact_zero = not boundaries.has_between(
                    contig,
                    position - 50,
                    position + 49 + len(reference),
                )
                requests[sample_index].add(key)
                if key in catalogue:
                    hits[sample_index].add(key)
                if in_gnomad_genomes:
                    gnomad_genomes[sample_index].add(key)
                if in_gnomad_any:
                    gnomad_any[sample_index].add(key)
                if key in catalogue or in_gnomad_genomes:
                    catalogue_or_gnomad_genomes[sample_index].add(key)
                if key in catalogue or in_gnomad_any:
                    catalogue_or_gnomad_any[sample_index].add(key)
                if loss_is_exact_zero:
                    exact_loss_zero[sample_index].add(key)
                if key in catalogue or loss_is_exact_zero:
                    catalogue_or_exact_loss[sample_index].add(key)
                if in_gnomad_genomes or loss_is_exact_zero:
                    gnomad_genomes_or_exact_loss[sample_index].add(key)
                if in_gnomad_any or loss_is_exact_zero:
                    gnomad_any_or_exact_loss[sample_index].add(key)
                if filter_value in {"PASS", "."}:
                    pass_requests[sample_index].add(key)
                    if key in catalogue:
                        pass_hits[sample_index].add(key)
                    if in_gnomad_genomes:
                        pass_gnomad_genomes[sample_index].add(key)
                    if in_gnomad_any:
                        pass_gnomad_any[sample_index].add(key)
                    if key in catalogue or in_gnomad_genomes:
                        pass_catalogue_or_gnomad_genomes[sample_index].add(key)
                    if key in catalogue or in_gnomad_any:
                        pass_catalogue_or_gnomad_any[sample_index].add(key)
                    if loss_is_exact_zero:
                        pass_exact_loss_zero[sample_index].add(key)
                    if key in catalogue or loss_is_exact_zero:
                        pass_catalogue_or_exact_loss[sample_index].add(key)
                    if in_gnomad_genomes or loss_is_exact_zero:
                        pass_gnomad_genomes_or_exact_loss[sample_index].add(key)
                    if in_gnomad_any or loss_is_exact_zero:
                        pass_gnomad_any_or_exact_loss[sample_index].add(key)
    if sample_count is None:
        raise ValueError("VCF column header is missing")
    per_person_requests = [len(values) for values in requests]
    per_person_hits = [len(values) for values in hits]
    per_person_pass_requests = [len(values) for values in pass_requests]
    per_person_pass_hits = [len(values) for values in pass_hits]
    return {
        "people": sample_count,
        "genic_supported_requests": sum(per_person_requests),
        "catalogue_hits": sum(per_person_hits),
        "pass_genic_supported_requests": sum(per_person_pass_requests),
        "pass_catalogue_hits": sum(per_person_pass_hits),
        "gnomad_genomes_requests": sum(map(len, gnomad_genomes)),
        "gnomad_any_requests": sum(map(len, gnomad_any)),
        "catalogue_or_gnomad_genomes_requests": sum(map(len, catalogue_or_gnomad_genomes)),
        "catalogue_or_gnomad_any_requests": sum(map(len, catalogue_or_gnomad_any)),
        "pass_gnomad_genomes_requests": sum(map(len, pass_gnomad_genomes)),
        "pass_gnomad_any_requests": sum(map(len, pass_gnomad_any)),
        "pass_catalogue_or_gnomad_genomes_requests": sum(map(len, pass_catalogue_or_gnomad_genomes)),
        "pass_catalogue_or_gnomad_any_requests": sum(map(len, pass_catalogue_or_gnomad_any)),
        "exact_loss_zero_requests": sum(map(len, exact_loss_zero)),
        "catalogue_or_exact_loss_requests": sum(map(len, catalogue_or_exact_loss)),
        "gnomad_genomes_or_exact_loss_requests": sum(map(len, gnomad_genomes_or_exact_loss)),
        "gnomad_any_or_exact_loss_requests": sum(map(len, gnomad_any_or_exact_loss)),
        "pass_exact_loss_zero_requests": sum(map(len, pass_exact_loss_zero)),
        "pass_catalogue_or_exact_loss_requests": sum(map(len, pass_catalogue_or_exact_loss)),
        "pass_gnomad_genomes_or_exact_loss_requests": sum(map(len, pass_gnomad_genomes_or_exact_loss)),
        "pass_gnomad_any_or_exact_loss_requests": sum(map(len, pass_gnomad_any_or_exact_loss)),
        "per_person_requests": per_person_requests,
        "per_person_hits": per_person_hits,
        "per_person_pass_requests": per_person_pass_requests,
        "per_person_pass_hits": per_person_pass_hits,
        "rejections": dict(sorted(rejections.items())),
    }


def load_catalogue(path: pathlib.Path, contig: str) -> set[tuple[str, int, str, str]]:
    result: set[tuple[str, int, str, str]] = set()
    with gzip.open(path, "rt", encoding="utf-8") as handle:
        header = handle.readline().rstrip("\n").split("\t")
        columns = {name: index for index, name in enumerate(header)}
        for line in handle:
            fields = line.rstrip("\n").split("\t")
            if (
                fields[columns["contig"]] == contig
                and fields[columns["anchor_genic"]] == "1"
                and int(fields[columns["train_carriers"]]) > 0
            ):
                result.add(
                    (
                        contig,
                        int(fields[columns["position"]]),
                        fields[columns["reference"]],
                        fields[columns["alternate"]],
                    )
                )
    return result


def load_sites_catalogue_lines(
    lines,
    genes,
    contig: str,
    maximum: int,
) -> set[tuple[str, int, str, str]]:
    result: set[tuple[str, int, str, str]] = set()
    for line in lines:
        if line.startswith("#"):
            continue
        fields = line.rstrip("\n").split("\t", 8)
        record_contig, position_text, _, reference, alternate_text, _, _, info = fields[:8]
        if record_contig != contig:
            continue
        position = int(position_text)
        if not genes.contains(contig, position):
            continue
        alternates = alternate_text.split(",")
        allele_count_text = info_value(info, "AC") or ""
        allele_counts = allele_count_text.split(",")
        if len(alternates) != len(allele_counts):
            continue
        for alternate, allele_count in zip(alternates, allele_counts):
            try:
                observed = int(allele_count) > 0
            except ValueError:
                observed = False
            if observed and classify_allele(reference, alternate, maximum) in {
                "supported_insertion",
                "supported_deletion",
            }:
                result.add((contig, position, reference, alternate))
    return result


def load_sites_catalogue(path: pathlib.Path, genes, args) -> set[tuple[str, int, str, str]]:
    with open_text(path) as handle:
        return load_sites_catalogue_lines(
            handle,
            genes,
            args.contig,
            args.max_allele_bases,
        )


def measure_file(path: pathlib.Path, catalogue, genes, boundaries, args) -> dict[str, object]:
    header_result = subprocess.run(
        [args.tabix, "-H", str(path)],
        check=True,
        capture_output=True,
        text=True,
    )
    header = next(
        line for line in reversed(header_result.stdout.splitlines()) if line.startswith("#CHROM")
    )
    process = subprocess.Popen(
        [args.tabix, str(path), args.contig],
        stdout=subprocess.PIPE,
        text=True,
    )
    assert process.stdout is not None
    result = measure_lines(
        itertools.chain([header], process.stdout),
        catalogue,
        genes,
        boundaries,
        args.contig,
        args.max_allele_bases,
    )
    if process.wait() != 0:
        raise subprocess.CalledProcessError(process.returncode, process.args)
    return result


def distribution(values: list[float]) -> dict[str, float | int | None]:
    if not values:
        return {"minimum": None, "median": None, "maximum": None}
    return {
        "minimum": min(values),
        "median": statistics.median(values),
        "maximum": max(values),
    }


def aggregate(results: list[dict[str, object]], catalogue_entries: int, contig: str) -> dict[str, object]:
    requests = [value for result in results for value in result["per_person_requests"]]
    hits = [value for result in results for value in result["per_person_hits"]]
    pass_requests = [value for result in results for value in result["per_person_pass_requests"]]
    pass_hits = [value for result in results for value in result["per_person_pass_hits"]]
    family_rates = [
        result["catalogue_hits"] / result["genic_supported_requests"]
        for result in results
        if result["genic_supported_requests"]
    ]
    person_rates = [hit / request for hit, request in zip(hits, requests) if request]
    rejection_counts: collections.Counter[str] = collections.Counter()
    for result in results:
        rejection_counts.update(result["rejections"])
    total_requests = sum(requests)
    total_hits = sum(hits)
    total_pass_requests = sum(pass_requests)
    total_pass_hits = sum(pass_hits)
    membership_fields = (
        "gnomad_genomes_requests",
        "gnomad_any_requests",
        "catalogue_or_gnomad_genomes_requests",
        "catalogue_or_gnomad_any_requests",
        "exact_loss_zero_requests",
        "catalogue_or_exact_loss_requests",
        "gnomad_genomes_or_exact_loss_requests",
        "gnomad_any_or_exact_loss_requests",
    )
    membership = {field: sum(result[field] for result in results) for field in membership_fields}
    pass_membership = {
        field: sum(result[f"pass_{field}"] for result in results) for field in membership_fields
    }

    def routing_rates(values, denominator):
        return {
            name: {
                "requests": value,
                "rate": value / denominator if denominator else None,
            }
            for name, value in values.items()
        }
    return {
        "schema": "pangopup-indel-catalogue-transfer-v1",
        "contig": contig,
        "input_files": len(results),
        "people": len(requests),
        "training_catalogue_entries": catalogue_entries,
        "all_records": {
            "genic_supported_requests": total_requests,
            "catalogue_hits": total_hits,
            "hit_rate": total_hits / total_requests if total_requests else None,
            "routing_coverage": routing_rates(membership, total_requests),
        },
        "pass_records": {
            "genic_supported_requests": total_pass_requests,
            "catalogue_hits": total_pass_hits,
            "hit_rate": total_pass_hits / total_pass_requests if total_pass_requests else None,
            "routing_coverage": routing_rates(pass_membership, total_pass_requests),
        },
        "per_person_request_count": distribution(requests),
        "per_person_hit_rate": distribution(person_rates),
        "per_family_hit_rate": distribution(family_rates),
        "rejections": dict(sorted(rejection_counts.items())),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    catalogue = parser.add_mutually_exclusive_group(required=True)
    catalogue.add_argument("--catalogue", type=pathlib.Path)
    catalogue.add_argument("--catalogue-sites-vcf", type=pathlib.Path)
    parser.add_argument("--gtf", type=pathlib.Path, required=True)
    parser.add_argument("--contig", default="chr22")
    parser.add_argument("--max-allele-bases", type=int, default=100)
    parser.add_argument("--tabix", default="tabix")
    parser.add_argument("--output-json", type=pathlib.Path, required=True)
    parser.add_argument("vcfs", nargs="+", type=pathlib.Path)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    genes = read_gene_intervals(args.gtf, args.contig)
    if args.catalogue is not None:
        catalogue = load_catalogue(args.catalogue, args.contig)
    else:
        catalogue = load_sites_catalogue(args.catalogue_sites_vcf, genes, args)
    boundaries = read_exon_boundaries(args.gtf, args.contig)
    results = [measure_file(path, catalogue, genes, boundaries, args) for path in args.vcfs]
    summary = aggregate(results, len(catalogue), args.contig)
    args.output_json.parent.mkdir(parents=True, exist_ok=True)
    args.output_json.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
