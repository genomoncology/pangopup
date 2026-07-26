"""Generate the tiny checked ONNX Runtime bundle and independent oracle."""

from __future__ import annotations

import hashlib
import argparse
import json
from pathlib import Path
import struct
import sys

import onnx
from onnx import TensorProto, helper


def canonical(value: object) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode()


def identity(filename: str, data: bytes) -> dict[str, object]:
    return {
        "bytes": len(data),
        "filename": filename,
        "sha256": f"sha256:{hashlib.sha256(data).hexdigest()}",
    }


def score_nodes(input_name: str, prefix: str, output_name: str) -> list[onnx.NodeProto]:
    cropped = f"{prefix}_cropped"
    return [
        helper.make_node("MaxPool", [input_name], [cropped], kernel_shape=[10001]),
        helper.make_node(
            "Concat", [cropped, cropped, cropped], [output_name], axis=1
        ),
    ]


def graph_bytes(representation: str) -> bytes:
    if representation == "singleton":
        inputs = [
            helper.make_tensor_value_info(
                "sequence", TensorProto.FLOAT, [1, 4, "N"]
            )
        ]
        outputs = [
            helper.make_tensor_value_info(
                "replicate_scores", TensorProto.FLOAT, [1, 12, "N_minus_10000"]
            )
        ]
        nodes = score_nodes("sequence", "sequence", "replicate_scores")
    elif representation == "zero-padded-batch":
        inputs = [
            helper.make_tensor_value_info(
                "sequence", TensorProto.FLOAT, ["B", 4, "N"]
            )
        ]
        outputs = [
            helper.make_tensor_value_info(
                "replicate_scores",
                TensorProto.FLOAT,
                ["B", 12, "N_minus_10000"],
            )
        ]
        nodes = score_nodes("sequence", "sequence", "replicate_scores")
    elif representation == "paired-strand-batch":
        inputs = [
            helper.make_tensor_value_info(
                "reference", TensorProto.FLOAT, ["B", 4, "N_ref"]
            ),
            helper.make_tensor_value_info(
                "alternate", TensorProto.FLOAT, ["B", 4, "N_alt"]
            ),
        ]
        outputs = [
            helper.make_tensor_value_info(
                "reference_scores",
                TensorProto.FLOAT,
                ["B", 12, "N_ref_minus_10000"],
            ),
            helper.make_tensor_value_info(
                "alternate_scores",
                TensorProto.FLOAT,
                ["B", 12, "N_alt_minus_10000"],
            ),
        ]
        nodes = score_nodes(
            "reference", "reference", "reference_scores"
        ) + score_nodes("alternate", "alternate", "alternate_scores")
    else:
        raise RuntimeError("unknown fixture representation")
    graph = helper.make_graph(
        nodes,
        "pangopup_model_kernel_mini",
        inputs,
        outputs,
    )
    model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 17)])
    model.ir_version = 12
    model.producer_name = "pangopup"
    model.producer_version = "onnx-1.19.1-fixture-v1"
    model.model_version = 1
    onnx.checker.check_model(model, full_check=True)
    return model.SerializeToString(deterministic=True)


def sequences() -> list[dict[str, str]]:
    reference = ["N"] * 10017
    reference[0] = "A"
    reference[8] = "T"
    reference[10000] = "C"
    reference[10008] = "T"
    reference[10016] = "G"
    alternate = reference.copy()
    alternate[0] = "G"
    alternate[8] = "A"
    alternate[10008] = "C"
    alternate[10016] = "T"
    ref = "".join(reference)
    alt = "".join(alternate)
    return [
        {"allele": "reference", "bases": ref, "case_id": "mini-plus", "strand": "+"},
        {"allele": "alternate", "bases": alt, "case_id": "mini-plus", "strand": "+"},
        {"allele": "reference", "bases": ref, "case_id": "mini-minus", "strand": "-"},
        {"allele": "alternate", "bases": alt, "case_id": "mini-minus", "strand": "-"},
    ]


def complement(base: str) -> str:
    return {"A": "T", "C": "G", "G": "C", "T": "A", "N": "N"}[base]


def channel_scores(bases: str, strand: str) -> list[list[float]]:
    oriented = bases if strand == "+" else "".join(complement(base) for base in bases[::-1])
    outputs: list[list[float]] = []
    for base in "ACGT":
        values = [
            float(base in oriented[start : start + 10001])
            for start in range(len(oriented) - 10000)
        ]
        if strand == "-":
            values.reverse()
        outputs.append(values)
    return outputs * 3


def f32_bits(value: float) -> str:
    return struct.pack("<f", value).hex()


def write_fixture(output: Path, representation: str) -> None:
    if output.exists():
        raise RuntimeError("fixture output already exists")
    bundle = output / "bundle"
    evidence = output / "evidence"
    bundle.mkdir(parents=True)
    evidence.mkdir()

    generator = Path(__file__).read_bytes()
    generator_identity = identity("tools/pangolin-model/fixture.py", generator)
    empty_identity = identity("synthetic-empty", b"")
    source = {
        "checkpoints": [],
        "identity": "pangopup-synthetic-maxpool-v1",
        "model_source": generator_identity,
        "upstream_commit": "synthetic",
        "upstream_url": "synthetic://pangopup-model-kernel-mini-v1",
    }
    environment = {
        "numpy": "2.5.1",
        "onnx": "1.19.1",
        "python": "3.13.5",
        "pytorch": "2.7.1+cpu",
    }

    zero_sha = f"sha256:{hashlib.sha256(bytes(4)).hexdigest()}"
    inventory_records = [
        {
            "checkpoint_ordinal": ordinal,
            "dtype": "f32",
            "elements": 1,
            "name": "synthetic.channel",
            "shape": [1],
            "tensor_bytes": 4,
            "value_sha256": zero_sha,
        }
        for ordinal in range(1, 13)
    ]
    inventory = b"".join(canonical(record) + b"\n" for record in inventory_records)

    golden_records: list[dict[str, object]] = []
    retained = sequences()
    for ordinal in range(1, 13):
        for sequence in retained:
            scores = channel_scores(sequence["bases"], sequence["strand"])[ordinal - 1]
            golden_records.append(
                {
                    "allele": sequence["allele"],
                    "case_id": sequence["case_id"],
                    "checkpoint_ordinal": ordinal,
                    "context_sha256": (
                        "sha256:"
                        + hashlib.sha256(sequence["bases"].encode()).hexdigest()
                    ),
                    "score_bits": [f32_bits(value) for value in scores],
                    "strand": sequence["strand"],
                }
            )
    golden = b"".join(canonical(record) + b"\n" for record in golden_records)
    evidence_manifest = {
        "converter_helper": generator_identity,
        "corpus": {
            "cases": empty_identity,
            "manifest": empty_identity,
            "notice": empty_identity,
        },
        "counts": {
            "cases": 2,
            "channel_arrays": 48,
            "checkpoints": 12,
            "elements_per_checkpoint": 1,
            "int64_counters_per_checkpoint": 0,
            "scalar_values": 816,
            "sequence_evaluations": 4,
            "strands": 2,
            "tensors": 12,
            "tensors_per_checkpoint": 1,
        },
        "environment": environment,
        "evidence_helper": generator_identity,
        "members": [
            identity("checkpoint-tensors.jsonl", inventory),
            identity("kernel-golden.jsonl", golden),
        ],
        "profile": "pangopup-model-kernel-mini-v1",
        "schema": "pangopup-model-evidence-v1",
        "source": source,
    }
    evidence_manifest_bytes = canonical(evidence_manifest)
    (evidence / "checkpoint-tensors.jsonl").write_bytes(inventory)
    (evidence / "kernel-golden.jsonl").write_bytes(golden)
    (evidence / "manifest.json").write_bytes(evidence_manifest_bytes)

    model = graph_bytes(representation)
    notice = (
        "Pangopup synthetic model-kernel test fixture\n\n"
        "This tiny ONNX graph contains no Pangolin checkpoint weights or genomic data. "
        "It exists only to exercise the authenticated model-bundle and ONNX Runtime path "
        "in offline tests.\n"
    ).encode()
    channels = [
        {
            "checkpoint_ordinal": ordinal,
            "selected_channel": 1 if ordinal <= 3 else 4 if ordinal <= 6 else 7 if ordinal <= 9 else 10,
        }
        for ordinal in range(1, 13)
    ]
    graph_contract_v1 = {
        "channels": channels,
        "exporter": {
            "constant_folding": True,
            "dynamo": False,
            "dynamic_axis": 2,
        },
        "input": {"element_type": "f32", "name": "sequence", "shape": ["1", "4", "N"]},
        "opset": 17,
        "output": {
            "element_type": "f32",
            "name": "replicate_scores",
            "shape": ["1", "12", "N-10000"],
        },
    }
    if representation == "zero-padded-batch":
        profile = "pangopup-model-kernel-mini-zero-padded-v2"
        graph_contract = {
            "channels": channels,
            "exporter": {
                "constant_folding": True,
                "dynamo": False,
                "dynamic_axes": [0, 2],
            },
            "inputs": [
                {"element_type": "f32", "name": "sequence", "shape": ["B", "4", "N"]}
            ],
            "opset": 17,
            "outputs": [
                {
                    "element_type": "f32",
                    "name": "replicate_scores",
                    "shape": ["B", "12", "N-10000"],
                }
            ],
            "representation": representation,
        }
    elif representation == "paired-strand-batch":
        profile = "pangopup-model-kernel-mini-paired-strand-v2"
        graph_contract = {
            "channels": channels,
            "exporter": {
                "constant_folding": True,
                "dynamo": False,
                "dynamic_axes": [0, 2],
            },
            "inputs": [
                {"element_type": "f32", "name": "reference", "shape": ["B", "4", "N_ref"]},
                {"element_type": "f32", "name": "alternate", "shape": ["B", "4", "N_alt"]},
            ],
            "opset": 17,
            "outputs": [
                {
                    "element_type": "f32",
                    "name": "reference_scores",
                    "shape": ["B", "12", "N_ref-10000"],
                },
                {
                    "element_type": "f32",
                    "name": "alternate_scores",
                    "shape": ["B", "12", "N_alt-10000"],
                },
            ],
            "representation": representation,
        }
    else:
        profile = "pangopup-model-kernel-mini-v1"
        graph_contract = graph_contract_v1
    graph_contract = graph_contract if representation != "singleton" else {
        "channels": [
            {
                "checkpoint_ordinal": ordinal,
                "selected_channel": 1 if ordinal <= 3 else 4 if ordinal <= 6 else 7 if ordinal <= 9 else 10,
            }
            for ordinal in range(1, 13)
        ],
        "exporter": {
            "constant_folding": True,
            "dynamo": False,
            "dynamic_axis": 2,
        },
        "input": {"element_type": "f32", "name": "sequence", "shape": ["1", "4", "N"]},
        "opset": 17,
        "output": {
            "element_type": "f32",
            "name": "replicate_scores",
            "shape": ["1", "12", "N-10000"],
        },
    }
    bundle_manifest = {
        "conversion": {
            "checkpoint_inventory": identity("checkpoint-tensors.jsonl", inventory),
            "converter": generator_identity,
            "environment": environment,
            "graph": graph_contract,
            "qualification_evidence": identity(
                "manifest.json", evidence_manifest_bytes
            ),
        },
        "kind": "synthetic-test",
        "members": [identity("NOTICE", notice), identity("model.onnx", model)],
        "profile": profile,
        "schema": (
            "pangopup-model-bundle-v1"
            if representation == "singleton"
            else "pangopup-model-bundle-v2"
        ),
        "source": source,
    }
    (bundle / "NOTICE").write_bytes(notice)
    (bundle / "model.onnx").write_bytes(model)
    (bundle / "manifest.json").write_bytes(canonical(bundle_manifest))


if __name__ == "__main__":
    if onnx.__version__ != "1.19.1" or sys.version_info[:3] != (3, 13, 5):
        raise RuntimeError("fixture generator environment is wrong")
    parser = argparse.ArgumentParser(allow_abbrev=False)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--representation",
        choices=("singleton", "zero-padded-batch", "paired-strand-batch"),
        required=True,
    )
    args = parser.parse_args()
    write_fixture(args.output, args.representation)
