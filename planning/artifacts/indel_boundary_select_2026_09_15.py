#!/usr/bin/env python3
"""Select bounded held-out literal indels close to GENCODE exon boundaries."""

from __future__ import annotations

import argparse
import gzip
import json
import pathlib

from indel_recurrence_2026_09_12 import BoundaryIndex, classify_allele, read_exon_boundaries
from snv_proxy_select_2026_09_15 import REQUIRED_COLUMNS, hash_rank


KINDS = ("supported_insertion", "supported_deletion")
PROBE_FIELDS = ("contig", "position", "reference", "alternate")


def changed_span(position: int, reference: str, alternate: str) -> tuple[int, int]:
    """Return changed reference coordinates, without the shared VCF anchor."""
    kind = classify_allele(reference, alternate, max(len(reference), len(alternate)))
    if kind == "supported_insertion":
        return position + 1, position + 1
    if kind == "supported_deletion":
        return position + 1, position + len(reference) - 1
    raise ValueError("literal allele is not a supported anchored indel")


def boundary_distance(
    boundaries: BoundaryIndex, contig: str, start: int, end: int, maximum: int = 5
) -> int | None:
    """Find the closest boundary within maximum bases of a closed changed span."""
    if maximum < 0 or start > end:
        raise ValueError("invalid boundary window")
    for distance in range(maximum + 1):
        if boundaries.has_between(contig, start - distance, end + distance):
            return distance
    return None


def literal_from_json(row: dict) -> tuple[str, int, str, str]:
    source = row.get("input", row)
    values = source.get("literal_tuple")
    if values is None:
        values = [source.get(field) for field in PROBE_FIELDS]
    if not isinstance(values, (list, tuple)) or len(values) != 4:
        raise ValueError("excluded row lacks a four-field literal")
    contig, position, reference, alternate = values
    if not isinstance(contig, str) or not isinstance(position, int) or not isinstance(reference, str) or not isinstance(alternate, str):
        raise ValueError("excluded literal has invalid field types")
    return contig, position, reference, alternate


def read_exclusions(paths: list[pathlib.Path]) -> set[tuple[str, int, str, str]]:
    excluded = set()
    for path in paths:
        with path.open(encoding="utf-8") as source:
            for line_number, line in enumerate(source, 1):
                if not line.strip():
                    continue
                row = json.loads(line)
                if not isinstance(row, dict):
                    raise ValueError(f"{path}:{line_number}: expected JSON object")
                excluded.add(literal_from_json(row))
    return excluded


def select(
    catalogue: pathlib.Path,
    gtf: pathlib.Path,
    probe_output: pathlib.Path,
    manifest_output: pathlib.Path,
    sample_per_kind: int = 10,
    exclusion_paths: list[pathlib.Path] | None = None,
    contig: str = "chr22",
) -> dict[str, int]:
    if not 1 <= sample_per_kind <= 10:
        raise ValueError("sample_per_kind must be between 1 and 10")
    exclusions = exclusion_paths or []
    paths = [catalogue, gtf, probe_output, manifest_output, *exclusions]
    if len({path.resolve() for path in paths}) != len(paths):
        raise ValueError("all input and output paths must differ")
    boundaries = read_exon_boundaries(gtf, contig)
    excluded = read_exclusions(exclusions)
    candidates: dict[str, list[tuple[str, tuple[str, int, str, str], int, int, int]]] = {
        kind: [] for kind in KINDS
    }
    seen = set()
    with gzip.open(catalogue, "rt", encoding="utf-8", newline="") as source:
        header = source.readline().rstrip("\r\n").split("\t")
        if len(header) != len(set(header)) or not set(REQUIRED_COLUMNS).issubset(header):
            raise ValueError("catalogue header lacks unique required columns")
        columns = {name: header.index(name) for name in REQUIRED_COLUMNS}
        for line_number, line in enumerate(source, 2):
            fields = line.rstrip("\r\n").split("\t")
            if len(fields) != len(header):
                raise ValueError(f"line {line_number}: catalogue column count differs")
            if fields[columns["contig"]] != contig:
                continue
            kind = fields[columns["kind"]]
            if kind not in KINDS or fields[columns["anchor_genic"]] != "1":
                continue
            carriers = int(fields[columns["test_carriers"]])
            if carriers < 0:
                raise ValueError(f"line {line_number}: negative test_carriers")
            if carriers == 0:
                continue
            key = (
                contig,
                int(fields[columns["position"]]),
                fields[columns["reference"]],
                fields[columns["alternate"]],
            )
            if key in seen:
                raise ValueError(f"line {line_number}: duplicate literal tuple")
            seen.add(key)
            if key in excluded:
                continue
            actual_kind = classify_allele(key[2], key[3], max(len(key[2]), len(key[3])))
            length = abs(len(key[2]) - len(key[3]))
            if actual_kind != kind or length != int(fields[columns["length"]]):
                raise ValueError(f"line {line_number}: catalogue kind or length differs from literal")
            start, end = changed_span(key[1], key[2], key[3])
            distance = boundary_distance(boundaries, contig, start, end)
            if distance is not None:
                candidates[kind].append((hash_rank(key), key, carriers, length, distance))

    selected = []
    counts = {}
    for kind in KINDS:
        ranked = sorted(candidates[kind], key=lambda item: (item[3] > 4, item[0], item[1]))
        chosen = ranked[:sample_per_kind]
        selected.extend(chosen)
        counts[kind] = len(chosen)

    with probe_output.open("w", encoding="utf-8") as probe, manifest_output.open("w", encoding="utf-8") as manifest:
        for _, key, carriers, length, distance in selected:
            probe.write(json.dumps(dict(zip(PROBE_FIELDS, key)), separators=(",", ":")) + "\n")
            manifest.write(json.dumps({
                "literal_tuple": key,
                "test_carriers": carriers,
                "changed_length": length,
                "boundary_distance": distance,
            }, separators=(",", ":")) + "\n")
    return counts


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalogue", type=pathlib.Path, required=True)
    parser.add_argument("--gtf", type=pathlib.Path, required=True)
    parser.add_argument("--probe-output", type=pathlib.Path, required=True)
    parser.add_argument("--manifest-output", type=pathlib.Path, required=True)
    parser.add_argument("--sample-per-kind", type=int, default=10)
    parser.add_argument("--contig", default="chr22")
    parser.add_argument("--labels-input", type=pathlib.Path)
    parser.add_argument("--high-input", type=pathlib.Path)
    parser.add_argument("--challenge-input", type=pathlib.Path)
    args = parser.parse_args()
    exclusion_paths = [path for path in (args.labels_input, args.high_input, args.challenge_input) if path]
    print(json.dumps(select(args.catalogue, args.gtf, args.probe_output, args.manifest_output,
                            args.sample_per_kind, exclusion_paths, args.contig)))


if __name__ == "__main__":
    main()
