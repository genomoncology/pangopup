#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# ///
"""Independently extract retained Ticket 004 answers from source TSV members."""

from __future__ import annotations

import gzip
import json
import multiprocessing
import sys
from collections import defaultdict
from decimal import Decimal
from pathlib import Path

ORDINARY = {}
COORDINATES = {}
QUERIES = []


def initialize(ordinary, coordinates, queries):
    global ORDINARY, COORDINATES, QUERIES
    ORDINARY = ordinary
    COORDINATES = coordinates
    QUERIES = queries


def score_text(value: bytes, loss: bool) -> str:
    magnitude = abs(Decimal(value.decode("ascii")))
    if magnitude == 0:
        return "0.00"
    rendered = f"{magnitude:.2f}"
    return f"-{rendered}" if loss else rendered


def inspect_member(path_text: str):
    path = Path(path_text)
    gene = path.name.removesuffix(".tsv.gz")
    records = []
    ambiguous_alts = defaultdict(set)
    with gzip.open(path, "rb") as source:
        header = source.readline()
        if header != b"chrom\tpos\tref\talt\tgain_score\tgain_pos\tloss_score\tloss_pos\n":
            raise ValueError(f"unexpected header in {path}")
        for raw in source:
            fields = raw.rstrip(b"\n").split(b"\t")
            if len(fields) != 8:
                raise ValueError(f"malformed source row in {path}")
            chrom = fields[0].decode("ascii")
            position = int(fields[1])
            reference = fields[2].decode("ascii")
            alternate = fields[3].decode("ascii")
            coordinate = (chrom, position)
            if reference == "N":
                if coordinate in COORDINATES:
                    ambiguous_alts[coordinate].add(alternate)
                continue
            key = (chrom, position, reference, alternate)
            for query_index in ORDINARY.get(key, ()):
                query_gene = QUERIES[query_index]["gene"]
                if query_gene is None or query_gene == gene:
                    records.append(
                        (
                            query_index,
                            {
                                "gene": gene,
                                "gain_score": score_text(fields[4], False),
                                "gain_position": int(fields[5]),
                                "loss_score": score_text(fields[6], True),
                                "loss_position": int(fields[7]),
                            },
                        )
                    )
    ambiguities = []
    for coordinate, alternates in ambiguous_alts.items():
        if len(alternates) != 3:
            raise ValueError(f"incomplete REF=N locus in {path}: {coordinate}")
        omitted = ({"A", "C", "G", "T"} - alternates).pop()
        for query_index in COORDINATES[coordinate]:
            query_gene = QUERIES[query_index]["gene"]
            if query_gene is None or query_gene == gene:
                ambiguities.append(
                    (
                        query_index,
                        {
                            "gene": gene,
                            "source_ref": "N",
                            "published_alts": sorted(alternates, key="ACGT".index),
                            "omitted_alt": omitted,
                        },
                    )
                )
    return records, ambiguities


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: 004-source-oracle.py QUERY_MANIFEST SOURCE_DIR")
    manifest = Path(sys.argv[1])
    source_dir = Path(sys.argv[2])
    queries = []
    ordinary = defaultdict(list)
    coordinates = defaultdict(list)
    for line in manifest.read_text().splitlines()[1:]:
        request_id, workload, order, variant, gene, request_count, record_count = line.split("\t")
        assembly, chrom, position, reference, alternate = variant.split(":")
        if assembly != "GRCh38":
            raise ValueError(f"unexpected assembly in {request_id}")
        query = {
            "request_id": request_id,
            "workload_class": workload,
            "stable_order": int(order),
            "variant": variant,
            "gene": None if gene == "." else gene,
            "expected_request_count": int(request_count),
            "expected_record_count": int(record_count),
        }
        index = len(queries)
        queries.append(query)
        ordinary[(chrom, int(position), reference, alternate)].append(index)
        coordinates[(chrom, int(position))].append(index)
    members = sorted(str(path) for path in source_dir.glob("ENSG*.tsv.gz"))
    if not members:
        raise ValueError("source directory has no ENSG gzip members")
    found_records = defaultdict(list)
    found_ambiguities = defaultdict(list)
    with multiprocessing.Pool(
        initializer=initialize,
        initargs=(dict(ordinary), dict(coordinates), queries),
    ) as pool:
        for records, ambiguities in pool.imap_unordered(inspect_member, members, chunksize=8):
            for index, record in records:
                found_records[index].append(record)
            for index, ambiguity in ambiguities:
                found_ambiguities[index].append(ambiguity)
    for index, query in enumerate(queries):
        records = sorted(found_records[index], key=lambda value: value["gene"])
        ambiguities = sorted(found_ambiguities[index], key=lambda value: value["gene"])
        if len(records) != query["expected_record_count"]:
            raise ValueError(
                f'{query["request_id"]} expected {query["expected_record_count"]} records, found {len(records)}'
            )
        status = (
            "mixed" if records and ambiguities else
            "found" if records else
            "ambiguous_source_reference" if ambiguities else
            "not_found"
        )
        output = {
            "request_id": query["request_id"],
            "workload_class": query["workload_class"],
            "stable_order": query["stable_order"],
            "variant": query["variant"],
            "gene": query["gene"],
            "status": status,
            "records": records,
            "source_reference_ambiguities": ambiguities,
        }
        print(json.dumps(output, separators=(",", ":")))


if __name__ == "__main__":
    main()
