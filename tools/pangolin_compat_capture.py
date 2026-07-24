#!/usr/bin/env python3
# Copyright (C) 2026 GenomOncology LLC
#
# This program is free software: you can redistribute it and/or modify it under
# the terms of the GNU General Public License as published by the Free Software
# Foundation, version 3. This helper is a narrow wrapper around Pangolin 1.0.2
# and is used only to capture its post-ensemble arrays for Pangopup's frozen
# compatibility corpus.

"""Emit bounded raw Pangolin observations for a Rust-supplied case plan."""

import argparse
import inspect
import json
import os
import struct
import sys

import gffutils
import pyfastx
import torch

import pangolin.pangolin as upstream
from pangolin.model import AR, L, W, Pangolin


def bits(value, dtype):
    if dtype == "f32":
        return struct.pack(">f", float(value)).hex()
    if dtype == "f64":
        return struct.pack(">d", float(value)).hex()
    raise ValueError(f"unsupported NumPy score dtype: {dtype}")


def load_models():
    models = []
    model_root = os.path.join(os.path.dirname(inspect.getfile(upstream)), "models")
    for tissue in (0, 2, 4, 6):
        for replicate in (1, 2, 3):
            model = Pangolin(L, W, AR)
            path = os.path.join(model_root, f"final.{replicate}.{tissue}.3.v2")
            state = torch.load(path, map_location=torch.device("cpu"), weights_only=True)
            model.load_state_dict(state)
            model.eval()
            models.append(model)
    return models


def genes_for(db, contig, position):
    plus = []
    minus = []
    for gene in db.region((contig, position - 1, position - 1), featuretype="gene"):
        if gene[3] > position or gene[4] < position:
            continue
        boundaries = []
        for exon in db.children(gene, featuretype="exon"):
            boundaries.extend((int(exon[3]), int(exon[4])))
        item = {"id": gene["gene_id"][0], "boundaries": boundaries}
        (plus if gene[6] == "+" else minus).append(item)
    return plus, minus


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--plan", required=True)
    parser.add_argument("--reference", required=True)
    parser.add_argument("--annotation-db", required=True)
    args = parser.parse_args()

    torch.set_num_threads(1)
    torch.set_num_interop_threads(1)
    if torch.cuda.is_available():
        raise RuntimeError("capture requires CPU-only PyTorch")

    with open(args.plan, "r", encoding="utf-8") as handle:
        plan = json.load(handle)
    if not isinstance(plan, list) or len(plan) != 14:
        raise ValueError("Rust case plan must contain exactly 14 model cases")

    fasta = pyfastx.Fasta(args.reference)
    database = gffutils.FeatureDB(args.annotation_db)
    models = load_models()
    observations = []
    for case in plan:
        contig = case["contig"]
        position = int(case["position"])
        ref = case["ref"]
        alt = case["alt"]
        distance = int(case["distance"])
        sequence = fasta[contig][
            position - 5001 - distance : position + len(ref) + 4999 + distance
        ].seq
        if sequence[5000 + distance : 5000 + distance + len(ref)] != ref:
            raise ValueError(f"{case['id']}: reference anchor mismatch")
        alternate = sequence[: 5000 + distance] + alt + sequence[5000 + distance + len(ref) :]
        plus, minus = genes_for(database, contig, position)
        strands = []
        for strand, genes in (("+", plus), ("-", minus)):
            if not genes:
                continue
            loss, gain = upstream.compute_score(sequence, alternate, strand, distance, models)
            if loss.dtype != gain.dtype:
                raise ValueError(f"{case['id']}: loss/gain dtype mismatch")
            dtype = {"float32": "f32", "float64": "f64"}.get(str(loss.dtype))
            if dtype is None:
                raise ValueError(f"{case['id']}: unexpected score dtype {loss.dtype}")
            strands.append(
                {
                    "strand": strand,
                    "dtype": dtype,
                    "loss_bits": [bits(value, dtype) for value in loss],
                    "gain_bits": [bits(value, dtype) for value in gain],
                    "genes": genes,
                }
            )
        observations.append(
            {
                "id": case["id"],
                "imported_module": os.path.realpath(inspect.getfile(upstream)),
                "strands": strands,
            }
        )
    json.dump(observations, sys.stdout, separators=(",", ":"), ensure_ascii=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
