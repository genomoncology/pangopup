import gzip
import importlib.util
import json
import pathlib
import tempfile
import unittest


MODULE_PATH = pathlib.Path(__file__).with_name("snv_proxy_select_2026_09_15.py")
SPEC = importlib.util.spec_from_file_location("snv_proxy_select", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)

HEADER = "contig\tposition\treference\talternate\tkind\tlength\ttrain_carriers\ttest_carriers\tanchor_genic\tspan_genic\n"


def row(position, reference, alternate, kind, carriers, anchor=1):
    length = abs(len(reference) - len(alternate))
    return f"chr22\t{position}\t{reference}\t{alternate}\t{kind}\t{length}\t0\t{carriers}\t{anchor}\t1\n"


class SelectorTests(unittest.TestCase):
    def run_fixture(self, rows, sample_per_kind=20):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            source = root / "variants.tsv.gz"
            with gzip.open(source, "wt", encoding="utf-8") as output:
                output.write(HEADER)
                output.writelines(rows)
            probe = root / "probe.jsonl"
            manifest = root / "manifest.jsonl"
            counts = MODULE.select(source, probe, manifest, sample_per_kind)
            return (
                counts,
                [json.loads(line) for line in probe.read_text().splitlines()],
                [json.loads(line) for line in manifest.read_text().splitlines()],
            )

    def test_filters_and_separates_probe_from_carrier_manifest(self):
        rows = [
            row(100, "A", "AT", "supported_insertion", 3),
            row(101, "AT", "A", "supported_deletion", 5),
            row(102, "A", "AG", "supported_insertion", 0),
            row(103, "A", "AC", "supported_insertion", 2, anchor=0),
            row(104, "A", "T", "snv", 4),
        ]
        counts, probe, manifest = self.run_fixture(rows)
        self.assertEqual(counts, {"supported_insertion": 1, "supported_deletion": 1})
        self.assertEqual({entry["position"] for entry in probe}, {100, 101})
        self.assertTrue(all(set(entry) == {"contig", "position", "reference", "alternate"} for entry in probe))
        self.assertEqual({tuple(entry["literal_tuple"]): entry["test_carriers"] for entry in manifest}, {
            ("chr22", 100, "A", "AT"): 3,
            ("chr22", 101, "AT", "A"): 5,
        })

    def test_hash_ranked_selection_is_order_independent_and_bounded(self):
        rows = [row(100 + index, "A", "AT", "supported_insertion", index + 1) for index in range(25)]
        rows += [row(200 + index, "AT", "A", "supported_deletion", 1) for index in range(23)]
        first = self.run_fixture(rows)
        second = self.run_fixture(list(reversed(rows)))
        self.assertEqual(first, second)
        self.assertEqual(first[0], {"supported_insertion": 20, "supported_deletion": 20})
        self.assertEqual(len(set(tuple(entry["literal_tuple"]) for entry in first[2])), 40)

    def test_rejects_duplicate_literal_tuple(self):
        duplicate = row(100, "A", "AT", "supported_insertion", 1)
        with self.assertRaisesRegex(ValueError, "duplicate"):
            self.run_fixture([duplicate, duplicate])

    def test_small_sample_is_nested_in_larger_sample(self):
        rows = [row(100 + index, "A", "AT", "supported_insertion", 1) for index in range(30)]
        rows += [row(200 + index, "AT", "A", "supported_deletion", 1) for index in range(30)]
        small = self.run_fixture(rows, 20)
        large = self.run_fixture(rows, 1000)
        self.assertEqual(small[1], [entry for entry in large[1] if entry in small[1]])
        self.assertEqual(small[2], [entry for entry in large[2] if entry in small[2]])


if __name__ == "__main__":
    unittest.main()
