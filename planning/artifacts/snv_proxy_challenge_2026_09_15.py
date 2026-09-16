#!/usr/bin/env python3
"""Select two quiet loci and make a bounded literal allele challenge panel."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib


BASES = "ACGT"
KINDS = ("insertion", "deletion")


def read_jsonl(path: pathlib.Path) -> list[dict]:
    rows = []
    with path.open(encoding="utf-8") as source:
        for number, line in enumerate(source, 1):
            if line.strip():
                row = json.loads(line)
                if not isinstance(row, dict):
                    raise ValueError(f"{path}:{number}: expected object")
                rows.append(row)
    return rows


def literal(row: dict) -> tuple[str, int, str, str]:
    values = row.get("literal_tuple")
    if values is None:
        values = (row.get("contig"), row.get("position"), row.get("reference"), row.get("alternate"))
    if not isinstance(values, (tuple, list)) or len(values) != 4:
        raise ValueError("invalid literal tuple")
    contig, position, reference, alternate = values
    if not isinstance(contig, str) or not isinstance(position, int) or position < 1:
        raise ValueError("invalid contig or position")
    if not isinstance(reference, str) or not isinstance(alternate, str):
        raise ValueError("invalid alleles")
    if not reference or not alternate or any(base not in BASES for base in reference + alternate):
        raise ValueError("alleles must be uppercase A/C/G/T")
    if reference[0] != alternate[0]:
        raise ValueError("literal anchor differs")
    return contig, position, reference, alternate


def kind_of(key: tuple[str, int, str, str]) -> str:
    reference, alternate = key[2:]
    if len(reference) == 1 and len(alternate) > 1:
        return "insertion"
    if len(alternate) == 1 and len(reference) > 1:
        return "deletion"
    raise ValueError("only anchored literal insertions and deletions are supported")


def rank(key: tuple[str, int, str, str]) -> str:
    encoded = json.dumps(key, separators=(",", ":"), ensure_ascii=True).encode("ascii")
    return hashlib.sha256(encoded).hexdigest()


def write_jsonl(path: pathlib.Path, rows: list[dict]) -> None:
    with path.open("w", encoding="utf-8") as output:
        for row in rows:
            output.write(json.dumps(row, separators=(",", ":")) + "\n")


def select(census: pathlib.Path, labels: pathlib.Path, output: pathlib.Path) -> list[dict]:
    if len({path.resolve() for path in (census, labels, output)}) != 3:
        raise ValueError("input and output paths must differ")
    excluded = {(key[0], key[1]) for key in (literal(row) for row in read_jsonl(labels))}
    grouped: dict[tuple[str, int, str, str], list[dict]] = {}
    for row in read_jsonl(census):
        key = literal(row.get("input", row) if row.get("status") is not None else row)
        grouped.setdefault(key, []).append(row)
    candidates: dict[str, list[tuple[str, tuple[str, int, str, str]]]] = {kind: [] for kind in KINDS}
    for key, rows in grouped.items():
        if (key[0], key[1]) in excluded or any(row.get("status") is not None for row in rows):
            continue
        genes = [row.get("gene") for row in rows]
        if any(not isinstance(gene, str) or not gene for gene in genes) or len(genes) != len(set(genes)):
            raise ValueError(f"duplicate or absent mask gene at {key}")
        if not all(
            row.get("plus_minus_10", {}).get("complete") is True
            and row["plus_minus_10"].get("gain", {}).get("max_hundredths") == 0
            and row["plus_minus_10"].get("loss", {}).get("max_hundredths") == 0
            for row in rows
        ):
            continue
        kind = kind_of(key)
        if kind == "deletion" and len(key[2]) - 1 > 4:
            continue
        if any(row.get("kind") != kind for row in rows):
            raise ValueError(f"kind disagrees with literal at {key}")
        candidates[kind].append((rank(key), key))
    chosen = []
    used_loci = set(excluded)
    for kind in KINDS:
        available = sorted(candidates[kind])
        selected = next((key for _, key in available if (key[0], key[1]) not in used_loci), None)
        if selected is None:
            raise ValueError(f"no complete all-zero held-out {kind} locus")
        used_loci.add((selected[0], selected[1]))
        chosen.append(dict(zip(("contig", "position", "reference", "alternate"), selected)))
    write_jsonl(output, chosen)
    return chosen


def challenge(selected: pathlib.Path, probe: pathlib.Path, output: pathlib.Path, manifest: pathlib.Path) -> list[dict]:
    if len({path.resolve() for path in (selected, probe, output, manifest)}) != 4:
        raise ValueError("input and output paths must differ")
    origins = [literal(row) for row in read_jsonl(selected)]
    if len(origins) != 2 or {kind_of(key) for key in origins} != set(KINDS):
        raise ValueError("selected input needs one insertion and one deletion")
    if len({(key[0], key[1]) for key in origins}) != 2:
        raise ValueError("selected loci overlap")
    grouped: dict[tuple[str, int, str, str], list[dict]] = {}
    for row in read_jsonl(probe):
        key = literal(row.get("input", row) if row.get("status") is not None else row)
        if key not in origins:
            raise ValueError(f"updated probe contains unselected literal: {key}")
        grouped.setdefault(key, []).append(row)
    alleles = []
    records = []
    seen = set()
    for origin in origins:
        rows = grouped.get(origin, [])
        if not rows or any(row.get("status") is not None for row in rows):
            raise ValueError(f"updated probe lacks valid rows for {origin}")
        genes = [row.get("gene") for row in rows]
        if any(not isinstance(gene, str) or not gene for gene in genes) or len(genes) != len(set(genes)):
            raise ValueError(f"duplicate or absent mask gene in updated probe for {origin}")
        pinned = {row.get("reference_after_anchor") for row in rows}
        if len(pinned) != 1:
            raise ValueError(f"inconsistent pinned reference for {origin}")
        following = pinned.pop()
        if not isinstance(following, str) or len(following) < 4 or any(base not in BASES for base in following[:4]):
            raise ValueError(f"four pinned reference bases required for {origin}")
        anchor = origin[2][0]
        if origin[2][1:] != following[: len(origin[2]) - 1]:
            raise ValueError(f"original REF does not match pinned reference for {origin}")
        contig, position = origin[:2]
        variants = [(anchor, anchor + base, "insertion", base) for base in BASES]
        variants += [(anchor + following[:length], anchor, "deletion", following[:length]) for length in range(1, 5)]
        for reference, alternate, kind, sequence in variants:
            key = (contig, position, reference, alternate)
            if key in seen:
                raise ValueError(f"duplicate challenge literal: {key}")
            seen.add(key)
            alleles.append(dict(zip(("contig", "position", "reference", "alternate"), key)))
            records.append({"literal_tuple": key, "origin_literal_tuple": origin, "kind": kind, "sequence": sequence})
    write_jsonl(output, alleles)
    write_jsonl(manifest, records)
    return alleles


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    selection = commands.add_parser("select")
    selection.add_argument("--census", type=pathlib.Path, required=True)
    selection.add_argument("--labels-input", type=pathlib.Path, required=True)
    selection.add_argument("--output", type=pathlib.Path, required=True)
    panel = commands.add_parser("challenge")
    panel.add_argument("--selected", type=pathlib.Path, required=True)
    panel.add_argument("--probe", type=pathlib.Path, required=True)
    panel.add_argument("--output", type=pathlib.Path, required=True)
    panel.add_argument("--manifest", type=pathlib.Path, required=True)
    args = parser.parse_args()
    if args.command == "select":
        result = select(args.census, args.labels_input, args.output)
    else:
        result = challenge(args.selected, args.probe, args.output, args.manifest)
    print(json.dumps({"rows": len(result)}))


if __name__ == "__main__":
    main()
