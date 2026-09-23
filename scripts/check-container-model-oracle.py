#!/usr/bin/env python3
"""Compare production container model output with the retained score oracle."""

import json
import pathlib
import re
import sys


KNOWN_NAMED_GENES = {
    "ENSG00000010610": ("CD4", "HGNC:1678"),
    "ENSG00000141499": ("WRAP53", "HGNC:25522"),
    "ENSG00000141510": ("TP53", "HGNC:11998"),
}
NAME_FIELDS = {
    "symbol", "source", "hgnc_id", "ncbi_gene_id", "prev_symbols", "alias_symbols"
}
ENSG = re.compile(r"(ENSG[0-9]{11})(?:\.[0-9]+)?\Z")
HGNC = re.compile(r"HGNC:[1-9][0-9]*\Z")


def fail(message):
    raise SystemExit(f"container model oracle: {message}")


def closed_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate field {key}")
        value[key] = item
    return value


def parse_json(source, label):
    try:
        return json.loads(source, object_pairs_hook=closed_object)
    except (ValueError, json.JSONDecodeError) as error:
        fail(f"invalid {label}: {error}")


def check_names(names, label):
    if not isinstance(names, dict):
        fail(f"{label}: gene_names must be an object")
    if not {"symbol", "source"} <= names.keys() or names.keys() - NAME_FIELDS:
        fail(f"{label}: unexpected gene_names fields")
    if not isinstance(names["symbol"], str) or not names["symbol"].strip():
        fail(f"{label}: invalid gene_names.symbol")
    if names["source"] not in ("hgnc", "ncbi"):
        fail(f"{label}: invalid gene_names.source")
    if "hgnc_id" in names:
        if names["source"] != "hgnc" or not isinstance(names["hgnc_id"], str) \
                or not HGNC.fullmatch(names["hgnc_id"]):
            fail(f"{label}: invalid gene_names.hgnc_id")
    if "ncbi_gene_id" in names:
        identifier = names["ncbi_gene_id"]
        if type(identifier) is not int or identifier <= 0:
            fail(f"{label}: invalid gene_names.ncbi_gene_id")
    for field in ("prev_symbols", "alias_symbols"):
        if field in names and (
            not isinstance(names[field], list)
            or not names[field]
            or any(not isinstance(item, str) or not item.strip() for item in names[field])
        ):
            fail(f"{label}: invalid gene_names.{field}")


def normalize(item, version, row):
    if not isinstance(item, dict):
        fail(f"result {row}: expected an object")
    provenance = item.get("provenance")
    if not isinstance(provenance, dict):
        fail(f"result {row}: missing provenance")
    if provenance.pop("software_version", None) != version:
        fail(f"result {row}: missing or wrong software version")
    records = item.get("records")
    if not isinstance(records, list) or not records:
        fail(f"result {row}: missing records")
    for index, record in enumerate(records, 1):
        label = f"result {row} record {index}"
        if not isinstance(record, dict):
            fail(f"{label}: expected an object")
        gene = record.get("gene")
        match = ENSG.fullmatch(gene) if isinstance(gene, str) else None
        if match is None or record.pop("stable_gene", None) != match.group(1):
            fail(f"{label}: missing or wrong stable_gene")
        if "gene_names" not in record:
            if match.group(1) in KNOWN_NAMED_GENES:
                fail(f"{label}: missing gene_names for a known named gene")
            continue
        names = record.pop("gene_names")
        check_names(names, label)
        if match.group(1) in KNOWN_NAMED_GENES:
            symbol, hgnc_id = KNOWN_NAMED_GENES[match.group(1)]
            if names["source"] != "hgnc" or names["symbol"] != symbol \
                    or names.get("hgnc_id") != hgnc_id:
                fail(f"{label}: changed known gene name or identifier")
    return item


def main():
    if len(sys.argv) != 4:
        fail("usage: check-container-model-oracle.py <ORACLE> <OUTPUT_JSONL> <VERSION>")
    oracle_path, output_path, version = map(str, sys.argv[1:])
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        fail("invalid expected software version")
    try:
        oracle = parse_json(pathlib.Path(oracle_path).read_text(encoding="utf-8"), "oracle")
        lines = pathlib.Path(output_path).read_text(encoding="utf-8").splitlines()
    except OSError as error:
        fail(f"cannot read fixture or output: {error}")
    if not isinstance(oracle, dict) or not isinstance(oracle.get("results"), list) \
            or not isinstance(oracle.get("provenance"), dict):
        fail("invalid oracle shape")
    if len(oracle["results"]) != 14 or len(lines) != 14:
        fail("expected exactly 14 oracle and output results")
    for index, line in enumerate(lines, 1):
        actual = normalize(parse_json(line, f"output line {index}"), version, index)
        if actual.get("provenance") != oracle["provenance"]:
            fail(f"result {index}: model provenance changed")
        actual.pop("provenance")
        if actual != oracle["results"][index - 1]:
            fail(f"result {index}: score or response changed")
    print("production container model oracle: 14 exact scored results")


if __name__ == "__main__":
    main()
