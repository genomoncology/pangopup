#!/usr/bin/env python3
"""Exercise the model-only container release comparison without Docker."""

import copy
import json
import pathlib
import subprocess
import sys
import tempfile


ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE = ROOT / "tests/fixtures/container-qualification/production-model-oracle.json"
CHECKER = ROOT / "scripts/check-container-model-oracle.py"
KNOWN_NAMES = {
    "ENSG00000010610": ("CD4", "HGNC:1678"),
    "ENSG00000141499": ("WRAP53", "HGNC:25522"),
    "ENSG00000141510": ("TP53", "HGNC:11998"),
}


def rendered_results():
    expected = json.loads(ORACLE.read_text(encoding="utf-8"))
    results = copy.deepcopy(expected["results"])
    for result in results:
        result["provenance"] = dict(expected["provenance"], software_version="0.5.0")
        for record in result["records"]:
            stable = record["gene"].split(".")[0]
            record["stable_gene"] = stable
            symbol, hgnc_id = KNOWN_NAMES.get(stable, ("GENE", "HGNC:1"))
            record["gene_names"] = {
                "symbol": symbol,
                "source": "hgnc",
                "hgnc_id": hgnc_id,
                "ncbi_gene_id": 1,
                "alias_symbols": ["OtherName"],
            }
    return results


def check(results):
    with tempfile.TemporaryDirectory(prefix="container-model-oracle-") as temporary:
        output = pathlib.Path(temporary) / "model-results.jsonl"
        output.write_text(
            "".join(json.dumps(item, separators=(",", ":")) + "\n" for item in results),
            encoding="utf-8",
        )
        return subprocess.run(
            [sys.executable, str(CHECKER), str(ORACLE), str(output), "0.5.0"],
            capture_output=True,
            text=True,
            check=False,
        )


def expect_rejected(label, mutate):
    results = rendered_results()
    mutate(results)
    completed = check(results)
    assert completed.returncode != 0, f"accepted {label}"


def main():
    results = rendered_results()
    completed = check(results)
    assert completed.returncode == 0, completed.stderr

    expect_rejected("changed score", lambda rows: rows[0]["records"][0].update(gain_score="0.99"))
    expect_rejected("extra result field", lambda rows: rows[0].update(drift_probe=True))
    expect_rejected("missing software version", lambda rows: rows[0]["provenance"].pop("software_version"))
    expect_rejected("wrong software version", lambda rows: rows[0]["provenance"].update(software_version="0.4.1"))
    expect_rejected("bad stable gene", lambda rows: rows[0]["records"][0].update(stable_gene="ENSG00000000000"))
    expect_rejected("missing known gene name", lambda rows: rows[0]["records"][0].pop("gene_names"))
    expect_rejected("wrong known gene symbol", lambda rows: rows[0]["records"][0]["gene_names"].update(symbol="CD8"))
    expect_rejected("wrong known HGNC identifier", lambda rows: rows[0]["records"][0]["gene_names"].update(hgnc_id="HGNC:2"))
    expect_rejected("null gene name", lambda rows: rows[2]["records"][0].update(gene_names=None))
    expect_rejected("malformed gene name", lambda rows: rows[0]["records"][0].update(gene_names={"symbol": "CD4"}))
    expect_rejected("extra gene-name field", lambda rows: rows[0]["records"][0]["gene_names"].update(drift_probe=True))
    expect_rejected("missing result", lambda rows: rows.pop())
    print("container model oracle: current fields admitted; scoring drift rejected")


if __name__ == "__main__":
    main()
