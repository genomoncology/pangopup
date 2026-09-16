#!/usr/bin/env python3
"""Select hash-ranked, held-out anchor-genic indels for a bounded SNV-proxy probe."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import pathlib


KINDS = ("supported_insertion", "supported_deletion")
REQUIRED_COLUMNS = (
    "contig", "position", "reference", "alternate", "kind", "length",
    "train_carriers", "test_carriers", "anchor_genic", "span_genic",
)


def hash_rank(literal_tuple: tuple[str, int, str, str]) -> str:
    encoded = json.dumps(literal_tuple, separators=(",", ":"), ensure_ascii=True).encode("ascii")
    return hashlib.sha256(encoded).hexdigest()


def select(
    source: pathlib.Path,
    probe_output: pathlib.Path,
    manifest_output: pathlib.Path,
    sample_per_kind: int = 20,
) -> dict[str, int]:
    """Write independent probe and carrier manifest files from a gzip catalogue."""
    if sample_per_kind < 1:
        raise ValueError("sample_per_kind must be positive")
    resolved = [path.resolve() for path in (source, probe_output, manifest_output)]
    if len(set(resolved)) != 3:
        raise ValueError("source, probe output, and manifest output must be different paths")

    candidates: dict[str, list[tuple[str, tuple[str, int, str, str], int]]] = {
        kind: [] for kind in KINDS
    }
    seen: set[tuple[str, int, str, str]] = set()
    with gzip.open(source, "rt", encoding="utf-8", newline="") as input_file:
        header = input_file.readline().rstrip("\r\n").split("\t")
        if len(header) != len(set(header)) or not set(REQUIRED_COLUMNS).issubset(header):
            raise ValueError("catalogue header lacks unique required columns")
        columns = {name: header.index(name) for name in REQUIRED_COLUMNS}
        for line_number, line in enumerate(input_file, 2):
            fields = line.rstrip("\r\n").split("\t")
            if len(fields) != len(header):
                raise ValueError(f"line {line_number}: catalogue column count differs")
            kind = fields[columns["kind"]]
            if kind not in KINDS or fields[columns["anchor_genic"]] != "1":
                continue
            test_carriers = int(fields[columns["test_carriers"]])
            if test_carriers < 0:
                raise ValueError(f"line {line_number}: negative test_carriers")
            if test_carriers == 0:
                continue
            literal_tuple = (
                fields[columns["contig"]],
                int(fields[columns["position"]]),
                fields[columns["reference"]],
                fields[columns["alternate"]],
            )
            if literal_tuple in seen:
                raise ValueError(f"line {line_number}: duplicate literal tuple")
            seen.add(literal_tuple)
            candidates[kind].append((hash_rank(literal_tuple), literal_tuple, test_carriers))

    selected = []
    counts = {}
    for kind in KINDS:
        ranked = sorted(candidates[kind], key=lambda item: (item[0], item[1]))
        selected.extend(ranked[:sample_per_kind])
        counts[kind] = min(len(ranked), sample_per_kind)

    with probe_output.open("w", encoding="utf-8") as probe, manifest_output.open("w", encoding="utf-8") as manifest:
        for _, literal_tuple, test_carriers in selected:
            contig, position, reference, alternate = literal_tuple
            probe.write(json.dumps({
                "contig": contig, "position": position,
                "reference": reference, "alternate": alternate,
            }, separators=(",", ":")) + "\n")
            manifest.write(json.dumps({
                "literal_tuple": literal_tuple, "test_carriers": test_carriers,
            }, separators=(",", ":")) + "\n")
    return counts


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalogue", type=pathlib.Path, required=True)
    parser.add_argument("--probe-output", type=pathlib.Path, required=True)
    parser.add_argument("--manifest-output", type=pathlib.Path, required=True)
    parser.add_argument("--sample-per-kind", type=int, default=20)
    args = parser.parse_args()
    print(json.dumps(select(args.catalogue, args.probe_output, args.manifest_output, args.sample_per_kind)))


if __name__ == "__main__":
    main()
