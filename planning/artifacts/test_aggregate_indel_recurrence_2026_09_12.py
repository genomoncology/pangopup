import gzip
import importlib.util
import json
import pathlib
import tempfile
import unittest


MODULE_PATH = pathlib.Path(__file__).with_name("aggregate_indel_recurrence_2026_09_12.py")
SPEC = importlib.util.spec_from_file_location("aggregate_indel_recurrence", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class AggregateTests(unittest.TestCase):
    def test_combines_chromosomes_into_one_globally_ranked_curve(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            header = "contig\tposition\treference\talternate\tkind\tlength\ttrain_carriers\ttest_carriers\tanchor_genic\tspan_genic\n"
            rows = {
                "chr1": "chr1\t100\tA\tAT\tsupported_insertion\t1\t5\t2\t1\t1\n",
                "chr22": "chr22\t200\tAAA\tA\tsupported_deletion\t2\t0\t3\t1\t1\n",
            }
            variants = []
            summaries = []
            for contig in ("chr1", "chr22"):
                variant_path = root / f"{contig}-variants.tsv.gz"
                with gzip.open(variant_path, "wt", encoding="utf-8") as handle:
                    handle.write(header + rows[contig])
                variants.append(variant_path)
                summary_path = root / f"{contig}-recurrence.json"
                summary_path.write_text(json.dumps({
                    "schema": "pangopup-indel-recurrence-v1",
                    "contig": contig,
                    "samples": {
                        "panel": 10,
                        "train": 8,
                        "test": 2,
                        "train_sha256": "train",
                        "test_sha256": "test",
                        "train_by_population": {"EUR": 8},
                        "test_by_population": {"EUR": 2},
                    },
                    "counts": {"input_records": 10},
                    "test_by_population": {
                        "EUR": {
                            "requests": 2 if contig == "chr1" else 3,
                            "hits_full_training_catalogue": 2 if contig == "chr1" else 0,
                        }
                    },
                }), encoding="utf-8")
                summaries.append(summary_path)

            summary, curves = MODULE.aggregate(variants, summaries)

            self.assertEqual(summary["contigs"], ["chr1", "chr22"])
            self.assertEqual(summary["counts"]["input_records"], 20)
            self.assertEqual(summary["test_by_population"]["EUR"], {
                "hits_full_training_catalogue": 2,
                "requests": 5,
            })
            anchor = next(row for row in curves if row["scope"] == "anchor_genic")
            self.assertEqual(anchor["catalogue_entries"], 1)
            self.assertEqual(anchor["test_carried_requests"], 5)
            self.assertEqual(anchor["hits"], 2)


if __name__ == "__main__":
    unittest.main()
