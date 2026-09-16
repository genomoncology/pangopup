import importlib.util
import json
import pathlib
import tempfile
import unittest


MODULE_PATH = pathlib.Path(__file__).with_name("snv_proxy_analyze_2026_09_15.py")
SPEC = importlib.util.spec_from_file_location("snv_proxy_analyze", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def scored(contig, position, ref, alt, gene, complete, gain, loss, kind):
    window = lambda g, l: {"complete": complete, "gain": {"max_hundredths": g}, "loss": {"max_hundredths": l}}
    return {"contig": contig, "position": position, "reference": ref, "alternate": alt, "gene": gene, "kind": kind, "anchor": window(gain, loss), "plus_minus_10": window(gain, loss)}


class AnalyzerTests(unittest.TestCase):
    def test_groups_genes_and_weights_complete_zero_and_distributions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest = root / "manifest.jsonl"
            manifest.write_text("""{"literal_tuple":["chr22",100,"A","AT"],"test_carriers":4}
{"literal_tuple":["chr22",200,"CC","C"],"test_carriers":2}
""", encoding="utf-8")
            probe = root / "probe.jsonl"
            rows = [
                scored("chr22", 100, "A", "AT", "G1", True, 0, 0, "insertion"),
                scored("chr22", 100, "A", "AT", "G2", True, 20, 0, "insertion"),
                scored("chr22", 200, "CC", "C", "G1", False, 0, 0, "deletion"),
            ]
            probe.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="utf-8")
            result = MODULE.analyze(probe, manifest)
            self.assertEqual(result["distinct_variants"], 2)
            self.assertEqual(result["carrier_weight"], 6)
            insertion = result["by_kind"]["insertion"]["windows"]["anchor"]
            self.assertEqual(insertion["all_gene_complete"], 1)
            self.assertEqual(insertion["all_gene_complete_carrier_weight"], 4)
            self.assertEqual(insertion["all_zero_gain"], 0)
            self.assertEqual(insertion["all_zero_gain_carrier_weight"], 0)
            self.assertEqual(insertion["all_zero_loss"], 1)
            self.assertEqual(insertion["all_zero_loss_carrier_weight"], 4)
            self.assertEqual(insertion["all_zero_combined"], 0)
            self.assertEqual(insertion["all_zero_combined_carrier_weight"], 0)
            self.assertEqual(insertion["cutoff_potential"]["0"]["loss"], {"distinct": 1, "test_carriers": 4})
            self.assertEqual(insertion["cutoff_potential"]["5"]["combined"], {"distinct": 0, "test_carriers": 0})
            self.assertEqual(insertion["cutoff_potential"]["20"]["combined"], {"distinct": 1, "test_carriers": 4})
            self.assertEqual(insertion["gain_max_hundredths"], {"20": 1})
            deletion = result["by_kind"]["deletion"]["windows"]["anchor"]
            self.assertEqual(deletion["all_gene_complete"], 0)
            self.assertEqual(deletion["all_gene_complete_carrier_weight"], 0)
            self.assertEqual(deletion["all_zero_combined"], 0)

    def test_statuses_are_preserved_and_weighted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest = root / "manifest.jsonl"
            manifest.write_text(json.dumps({"literal_tuple": ["chr22", 100, "A", "AT"], "test_carriers": 7}) + "\n", encoding="utf-8")
            probe = root / "probe.jsonl"
            probe.write_text(json.dumps({"status": "no_mask_gene", "input": {"contig": "chr22", "position": 100, "reference": "A", "alternate": "AT"}}) + "\n", encoding="utf-8")
            result = MODULE.analyze(probe, manifest)
            self.assertEqual(result["non_score_statuses"], {"distinct_rows": {"no_mask_gene": 1}, "carrier_weight": {"no_mask_gene": 7}})

    def test_missing_manifest_and_missing_scores_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest = root / "manifest.jsonl"
            manifest.write_text(json.dumps({"literal_tuple": ["chr22", 100, "A", "AT"], "test_carriers": 1}) + "\n", encoding="utf-8")
            probe = root / "probe.jsonl"
            probe.write_text(json.dumps(scored("chr22", 101, "A", "AT", "G1", True, 0, 0, "insertion")) + "\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                MODULE.analyze(probe, manifest)


if __name__ == "__main__":
    unittest.main()
