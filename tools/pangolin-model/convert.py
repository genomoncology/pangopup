"""Export the twelve authenticated Pangolin checkpoints as one ONNX graph.

This helper is executed from authenticated embedded bytes by pangopup-build.
It never imports Python from the upstream checkout.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import os
from pathlib import Path
import stat
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
CHECKPOINTS = (
    ("final.1.0.3.v2", 1, "f0478fab173b75f7f7e9fe96688bad6c50fa4a46d70557f423b110caaf565501"),
    ("final.2.0.3.v2", 1, "c4c6bb4880fa6fb28b14182ae3ea0600edb07056158f55325b5e6e6e48fc9f26"),
    ("final.3.0.3.v2", 1, "ec685a6e7105a4486c1f89a005458a13deb3fe7171f13d434f4877e386d10676"),
    ("final.1.2.3.v2", 4, "559c05de3e1ce65c2515ca3e92ef85edb0ec2e47686ca58060e25891ce06eb3a"),
    ("final.2.2.3.v2", 4, "48758ba8b95eee9aa9feea52672ef06ca1b34111299c27f8a710f734d8b9aae5"),
    ("final.3.2.3.v2", 4, "7cb576c2b24db4fdd6970c4ca4fb7c20ae1b1d8ae80645ebbe689848b5743129"),
    ("final.1.4.3.v2", 7, "c50b12e0c0af776d5674ca5e346493f8265783494d4df383364de9c1136657f6"),
    ("final.2.4.3.v2", 7, "e03303bed4fd6f135ec0f6c1b192cce954ea42d0646f44d17b4a6fbb2b1f610e"),
    ("final.3.4.3.v2", 7, "9476d2e25520d7ff15bece0cd5d3b657e3b1dd3cc5fcab1d9c3b62bea7a0c5b6"),
    ("final.1.6.3.v2", 10, "2aae563fa18a8a9b6699c6c96e0d32b8ec7543f8f805fb3bc9de77302cc9f66e"),
    ("final.2.6.3.v2", 10, "7d3c0b1b2a60067b940dec315567874fbc8bcd322f1b7c76bf969f51f0f53f7f"),
    ("final.3.6.3.v2", 10, "756e7721a382cace24e9bfea5b543af5623f2487d9a3efe7385e9c76367005fd"),
)
CHECKPOINT_BYTES = 2_877_321


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
    exec(
        compile(source, "authenticated/pangolin/model.py", "exec"),
        namespace,
    )
    return namespace["Pangolin"]


class CombinedPangolin(torch.nn.Module):
    def __init__(self, models: list[torch.nn.Module], channels: list[int]):
        super().__init__()
        self.models = torch.nn.ModuleList(models)
        self.channels = channels

    def forward(self, sequence: torch.Tensor) -> torch.Tensor:
        return torch.stack(
            [
                model(sequence)[:, channel, :]
                for model, channel in zip(self.models, self.channels, strict=True)
            ],
            dim=1,
        )

class PairedPangolin(torch.nn.Module):
    def __init__(self, combined: CombinedPangolin):
        super().__init__()
        self.combined = combined

    def forward(
        self, reference: torch.Tensor, alternate: torch.Tensor
    ) -> tuple[torch.Tensor, torch.Tensor]:
        return self.combined(reference), self.combined(alternate)


def set_dimension(dimension: onnx.TensorShapeProto.Dimension, value: int | str) -> None:
    dimension.ClearField("dim_value")
    dimension.ClearField("dim_param")
    if isinstance(value, int):
        dimension.dim_value = value
    else:
        dimension.dim_param = value


def convert(upstream: Path, output: Path, representation: str) -> None:
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
    model_class = load_model_class(upstream)
    models: list[torch.nn.Module] = []
    channels: list[int] = []
    for filename, selected_channel, digest in CHECKPOINTS:
        raw = read_authenticated(
            upstream / "pangolin" / "models" / filename,
            CHECKPOINT_BYTES,
            digest,
        )
        state = torch.load(io.BytesIO(raw), map_location="cpu", weights_only=True)
        model = model_class(
            32,
            np.asarray([11] * 8 + [21] * 4 + [41] * 4),
            np.asarray([1] * 4 + [4] * 4 + [10] * 4 + [25] * 4),
        )
        result = model.load_state_dict(state, strict=True)
        if result.missing_keys or result.unexpected_keys:
            fail(f"{filename} did not strict-load")
        model.eval()
        models.append(model)
        channels.append(selected_channel)

    combined: torch.nn.Module = CombinedPangolin(models, channels)
    combined.eval()
    if representation == "singleton":
        arguments = (torch.zeros((1, 4, 10_101), dtype=torch.float32),)
        input_names = ["sequence"]
        output_names = ["replicate_scores"]
        dynamic_axes = {
            "sequence": {2: "N"},
            "replicate_scores": {2: "N_minus_10000"},
        }
        input_shapes: tuple[tuple[int | str, ...], ...] = ((1, 4, "N"),)
        output_shapes: tuple[tuple[int | str, ...], ...] = (
            (1, 12, "N_minus_10000"),
        )
    elif representation == "zero-padded-batch":
        arguments = (torch.zeros((2, 4, 10_101), dtype=torch.float32),)
        input_names = ["sequence"]
        output_names = ["replicate_scores"]
        dynamic_axes = {
            "sequence": {0: "B", 2: "N"},
            "replicate_scores": {0: "B", 2: "N_minus_10000"},
        }
        input_shapes = (("B", 4, "N"),)
        output_shapes = (("B", 12, "N_minus_10000"),)
    elif representation == "paired-strand-batch":
        combined = PairedPangolin(combined)
        combined.eval()
        arguments = (
            torch.zeros((2, 4, 10_101), dtype=torch.float32),
            torch.zeros((2, 4, 10_102), dtype=torch.float32),
        )
        input_names = ["reference", "alternate"]
        output_names = ["reference_scores", "alternate_scores"]
        dynamic_axes = {
            "reference": {0: "B", 2: "N_ref"},
            "alternate": {0: "B", 2: "N_alt"},
            "reference_scores": {0: "B", 2: "N_ref_minus_10000"},
            "alternate_scores": {0: "B", 2: "N_alt_minus_10000"},
        }
        input_shapes = (("B", 4, "N_ref"), ("B", 4, "N_alt"))
        output_shapes = (
            ("B", 12, "N_ref_minus_10000"),
            ("B", 12, "N_alt_minus_10000"),
        )
    else:
        fail("unknown representation")

    buffer = io.BytesIO()
    with torch.no_grad():
        torch.onnx.export(
            combined,
            arguments,
            buffer,
            export_params=True,
            opset_version=17,
            do_constant_folding=True,
            input_names=input_names,
            output_names=output_names,
            # Preserve the exact accepted v1 singleton export bytes; that
            # historical path applies its one dynamic axis to graph metadata
            # below. V2 candidates must give the exporter the full dynamic
            # batch/length contract so internal traced shapes are dynamic too.
            dynamic_axes=(
                dynamic_axes if representation != "singleton" else None
            ),
            dynamo=False,
        )
    graph = onnx.load_model_from_string(buffer.getvalue())
    if len(graph.graph.input) != len(input_names) or len(graph.graph.output) != len(
        output_names
    ):
        fail("exported graph has wrong input/output count")
    for outlet, name, expected_shape in zip(
        graph.graph.input, input_names, input_shapes, strict=True
    ):
        outlet.name = name
        shape = outlet.type.tensor_type.shape.dim
        if len(shape) != 3:
            fail("exported graph has wrong input rank")
        for dimension, value in zip(shape, expected_shape, strict=True):
            set_dimension(dimension, value)
    for outlet, name, expected_shape in zip(
        graph.graph.output, output_names, output_shapes, strict=True
    ):
        outlet.name = name
        shape = outlet.type.tensor_type.shape.dim
        if len(shape) != 3:
            fail("exported graph has wrong output rank")
        for dimension, value in zip(shape, expected_shape, strict=True):
            set_dimension(dimension, value)
    onnx.checker.check_model(graph, full_check=True)
    output.write_bytes(graph.SerializeToString(deterministic=True))


def main() -> None:
    parser = argparse.ArgumentParser(allow_abbrev=False)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--representation",
        choices=("singleton", "zero-padded-batch", "paired-strand-batch"),
        required=True,
    )
    args = parser.parse_args()
    convert(args.upstream, args.output, args.representation)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"pangopup model conversion failed: {error}", file=sys.stderr)
        raise SystemExit(1) from None
