"""Generate an independent Pangolin checkpoint inventory and raw-score oracle.

This helper is executed from authenticated embedded bytes by pangopup-build.
It never imports Python from the upstream checkout.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import struct
import sys
from typing import Any

import numpy as np
import onnx
import torch


MODEL_SOURCE = (
    "pangolin/model.py",
    3011,
    "4a1c5c2570aafe1452bb43332255321677e6c6c817adf84b9dd438e3ca4be6f8",
)
CORPUS_FILES = {
    "manifest.json": (
        5337,
        "fd12a0d6b503d1e572c0561eb43e66f19c55c4d073b25bced25be6303fd0553b",
    ),
    "cases.jsonl": (
        220071,
        "2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8",
    ),
    "NOTICE": (
        1652,
        "edb9addea955d89820b82cc77c86b2e879f843081dcd57b0940dcefe1698d5da",
    ),
}
CHECKPOINTS = (
    (1, "final.1.0.3.v2", 1, "f0478fab173b75f7f7e9fe96688bad6c50fa4a46d70557f423b110caaf565501"),
    (2, "final.2.0.3.v2", 1, "c4c6bb4880fa6fb28b14182ae3ea0600edb07056158f55325b5e6e6e48fc9f26"),
    (3, "final.3.0.3.v2", 1, "ec685a6e7105a4486c1f89a005458a13deb3fe7171f13d434f4877e386d10676"),
    (4, "final.1.2.3.v2", 4, "559c05de3e1ce65c2515ca3e92ef85edb0ec2e47686ca58060e25891ce06eb3a"),
    (5, "final.2.2.3.v2", 4, "48758ba8b95eee9aa9feea52672ef06ca1b34111299c27f8a710f734d8b9aae5"),
    (6, "final.3.2.3.v2", 4, "7cb576c2b24db4fdd6970c4ca4fb7c20ae1b1d8ae80645ebbe689848b5743129"),
    (7, "final.1.4.3.v2", 7, "c50b12e0c0af776d5674ca5e346493f8265783494d4df383364de9c1136657f6"),
    (8, "final.2.4.3.v2", 7, "e03303bed4fd6f135ec0f6c1b192cce954ea42d0646f44d17b4a6fbb2b1f610e"),
    (9, "final.3.4.3.v2", 7, "9476d2e25520d7ff15bece0cd5d3b657e3b1dd3cc5fcab1d9c3b62bea7a0c5b6"),
    (10, "final.1.6.3.v2", 10, "2aae563fa18a8a9b6699c6c96e0d32b8ec7543f8f805fb3bc9de77302cc9f66e"),
    (11, "final.2.6.3.v2", 10, "7d3c0b1b2a60067b940dec315567874fbc8bcd322f1b7c76bf969f51f0f53f7f"),
    (12, "final.3.6.3.v2", 10, "756e7721a382cace24e9bfea5b543af5623f2487d9a3efe7385e9c76367005fd"),
)
CHECKPOINT_BYTES = 2_877_321
IN_MAP = np.asarray(
    [
        [0, 0, 0, 0],
        [1, 0, 0, 0],
        [0, 1, 0, 0],
        [0, 0, 1, 0],
        [0, 0, 0, 1],
    ],
    dtype=np.float32,
)


def fail(message: str) -> None:
    raise RuntimeError(message)


def read_authenticated(path: Path, expected_bytes: int, expected_sha: str) -> bytes:
    flags = os.O_RDONLY | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_nlink != 1
            or metadata.st_size != expected_bytes
        ):
            fail(f"wrong file shape for {path.name}")
        with os.fdopen(descriptor, "rb", closefd=False) as source:
            data = source.read(expected_bytes + 1)
    finally:
        os.close(descriptor)
    if len(data) != expected_bytes:
        fail(f"wrong byte length for {path.name}")
    if hashlib.sha256(data).hexdigest() != expected_sha:
        fail(f"wrong SHA-256 for {path.name}")
    return data


def load_model_class(upstream: Path) -> type[torch.nn.Module]:
    filename, length, digest = MODEL_SOURCE
    source = read_authenticated(upstream / filename, length, digest)
    namespace: dict[str, Any] = {
        "__name__": "pangopup_authenticated_pangolin_model",
        "__file__": "authenticated/pangolin/model.py",
    }
    code = compile(source, "authenticated/pangolin/model.py", "exec")
    exec(code, namespace)
    return namespace["Pangolin"]


def authenticated_inputs(upstream: Path, corpus: Path) -> tuple[type[torch.nn.Module], list[bytes], list[dict[str, Any]]]:
    model_class = load_model_class(upstream)
    corpus_bytes = {
        name: read_authenticated(corpus / name, length, digest)
        for name, (length, digest) in CORPUS_FILES.items()
    }
    cases = [
        json.loads(line)
        for line in corpus_bytes["cases.jsonl"].decode("utf-8").splitlines()
        if line
    ]
    cases = [case for case in cases if case["kind"] == "model"]
    if len(cases) != 14:
        fail("compatibility corpus must contain fourteen model cases")
    checkpoints = [
        read_authenticated(
            upstream / "pangolin" / "models" / filename,
            CHECKPOINT_BYTES,
            digest,
        )
        for _, filename, _, digest in CHECKPOINTS
    ]
    return model_class, checkpoints, cases


def one_hot(sequence: str, strand: str) -> torch.Tensor:
    translated = (
        sequence.upper()
        .replace("A", "1")
        .replace("C", "2")
        .replace("G", "3")
        .replace("T", "4")
        .replace("N", "0")
    )
    if any(base not in "01234" for base in translated):
        fail("context contains a non-ACGTN base")
    indices = np.asarray([int(base) for base in translated], dtype=np.int8)
    if strand == "-":
        indices = (5 - indices[::-1]) % 5
    return torch.from_numpy(IN_MAP[indices].T.copy()).float()


def retained_sequences(cases: list[dict[str, Any]]) -> list[dict[str, Any]]:
    retained: list[dict[str, Any]] = []
    for case in cases:
        bases = case["context"]["bases"]
        anchor = case["context"]["anchor_offset"]
        ref = case["input"]["ref"]
        alt = case["input"]["alt"]
        if bases[anchor : anchor + len(ref)] != ref:
            fail(f"{case['id']} reference context does not match REF")
        alternate = bases[:anchor] + alt + bases[anchor + len(ref) :]
        for strand_record in case["strands"]:
            strand = strand_record["strand"]
            for allele, sequence in (("reference", bases), ("alternate", alternate)):
                if not 10_001 <= len(sequence) <= 10_200:
                    fail(f"{case['id']} {allele} length outside kernel bounds")
                retained.append(
                    {
                        "case_id": case["id"],
                        "context_sha256": f"sha256:{case['context']['sha256']}",
                        "strand": strand,
                        "allele": allele,
                        "sequence": sequence,
                        "tensor": one_hot(sequence, strand),
                    }
                )
    if len(retained) != 36:
        fail("compatibility corpus must yield thirty-six sequence evaluations")
    return retained


def canonical_tensor_bytes(tensor: torch.Tensor) -> tuple[str, bytes]:
    array = tensor.detach().cpu().contiguous().numpy()
    if tensor.dtype == torch.float32:
        return "f32", array.astype("<f4", copy=False).tobytes(order="C")
    if tensor.dtype == torch.int64:
        return "i64", array.astype("<i8", copy=False).tobytes(order="C")
    fail(f"unsupported tensor dtype {tensor.dtype}")


def canonical_json(record: dict[str, Any]) -> str:
    return json.dumps(record, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def f32_bits(values: np.ndarray) -> list[str]:
    values = values.astype("<f4", copy=False)
    return [struct.pack("<f", float(value)).hex() for value in values]


def write_evidence(upstream: Path, corpus: Path, inventory: Path, golden: Path) -> None:
    if (
        sys.version_info[:3] != (3, 13, 5)
        or torch.__version__ != "2.7.1+cpu"
        or np.__version__ != "2.5.1"
        or onnx.__version__ != "1.19.1"
        or torch.cuda.is_available()
    ):
        fail("maintainer environment identity is wrong")
    torch.set_num_threads(1)
    torch.set_num_interop_threads(1)
    model_class, checkpoint_bytes, cases = authenticated_inputs(upstream, corpus)
    sequences = retained_sequences(cases)
    inventory_lines: list[str] = []
    golden_records: list[dict[str, Any]] = []

    for (ordinal, filename, selected_channel, _), raw in zip(
        CHECKPOINTS, checkpoint_bytes, strict=True
    ):
        state = torch.load(io.BytesIO(raw), map_location="cpu", weights_only=True)
        if not isinstance(state, dict):
            fail(f"{filename} is not a state dictionary")
        model = model_class(32, np.asarray([11] * 8 + [21] * 4 + [41] * 4), np.asarray([1] * 4 + [4] * 4 + [10] * 4 + [25] * 4))
        result = model.load_state_dict(state, strict=True)
        if result.missing_keys or result.unexpected_keys:
            fail(f"{filename} did not strict-load")

        entries = 0
        elements = 0
        counters = 0
        for name, tensor in state.items():
            dtype, tensor_bytes = canonical_tensor_bytes(tensor)
            entries += 1
            elements += tensor.numel()
            counters += int(dtype == "i64")
            inventory_lines.append(
                canonical_json(
                    {
                        "checkpoint_ordinal": ordinal,
                        "dtype": dtype,
                        "elements": tensor.numel(),
                        "name": name,
                        "shape": list(tensor.shape),
                        "tensor_bytes": len(tensor_bytes),
                        "value_sha256": f"sha256:{hashlib.sha256(tensor_bytes).hexdigest()}",
                    }
                )
            )
        if (entries, elements, counters) != (252, 699_116, 32):
            fail(f"{filename} tensor inventory shape is wrong")

        model.eval()
        grouped: dict[int, list[tuple[int, dict[str, Any]]]] = {}
        for sequence_index, sequence in enumerate(sequences):
            grouped.setdefault(len(sequence["sequence"]), []).append(
                (sequence_index, sequence)
            )
        outputs: dict[int, np.ndarray] = {}
        with torch.no_grad():
            for group in grouped.values():
                batch = torch.stack([item["tensor"] for _, item in group], dim=0)
                scores = model(batch)[:, selected_channel, :].cpu().numpy()
                for row, (sequence_index, _) in enumerate(group):
                    values = scores[row]
                    if not np.all(np.isfinite(values)) or np.any(values < 0) or np.any(values > 1):
                        fail(f"{filename} produced invalid score")
                    outputs[sequence_index] = values
        for sequence_index, sequence in enumerate(sequences):
            values = outputs[sequence_index]
            if sequence["strand"] == "-":
                values = values[::-1]
            golden_records.append(
                {
                    "allele": sequence["allele"],
                    "case_id": sequence["case_id"],
                    "checkpoint_ordinal": ordinal,
                    "context_sha256": sequence["context_sha256"],
                    "score_bits": f32_bits(values),
                    "strand": sequence["strand"],
                }
            )

    scalar_count = sum(len(record["score_bits"]) for record in golden_records)
    if len(inventory_lines) != 12 * 252:
        fail("inventory record count is wrong")
    if len(golden_records) != 432 or scalar_count != 45_756:
        fail("golden coverage count is wrong")
    inventory.write_text("\n".join(inventory_lines) + "\n", encoding="utf-8")
    golden.write_text(
        "\n".join(canonical_json(record) for record in golden_records) + "\n",
        encoding="utf-8",
    )


def main() -> None:
    parser = argparse.ArgumentParser(allow_abbrev=False)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--inventory", type=Path, required=True)
    parser.add_argument("--golden", type=Path, required=True)
    args = parser.parse_args()
    write_evidence(args.upstream, args.corpus, args.inventory, args.golden)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"pangopup model evidence failed: {error}", file=sys.stderr)
        raise SystemExit(1) from None
