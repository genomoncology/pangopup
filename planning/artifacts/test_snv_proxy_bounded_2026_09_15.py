import importlib.util
import json
import pathlib
import tempfile
import unittest


PATH = pathlib.Path(__file__).with_name("snv_proxy_bounded_2026_09_15.py")
SPEC = importlib.util.spec_from_file_location("bounded", PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def probe(position, reference="A", alternate="AT", complete=True, gain=0, loss=0):
    window = {"complete": complete, "gain": {"max_hundredths": gain}, "loss": {"max_hundredths": loss}}
    return {"contig": "chr22", "position": position, "reference": reference, "alternate": alternate, "plus_minus_10": window}


class BoundedTests(unittest.TestCase):
    def run_analysis(self, manifest, probe_rows, exons):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest_path = root / "manifest.jsonl"
            probe_path = root / "probe.jsonl"
            gtf_path = root / "gencode.gtf"
            manifest_path.write_text("\n".join(json.dumps(row) for row in manifest) + "\n")
            probe_path.write_text("\n".join(json.dumps(row) for row in probe_rows) + ("\n" if probe_rows else ""))
            gtf_path.write_text("\n".join(f"chr22\ttest\texon\t{start}\t{end}\t.\t+\t.\tgene_id \\\"G{i}\\\";" for i, (start, end) in enumerate(exons)) + "\n")
            return MODULE.analyze(manifest_path, probe_path, gtf_path)

    def test_deletion_expands_transfer_interval_by_reference_length(self):
        result = self.run_analysis(
            [{"literal_tuple": ["chr22", 100, "AAAA", "A"], "test_carriers": 3}],
            [probe(100, "AAAA", "A")], [(150, 150)],
        )
        rules = result["by_kind"]["deletion"]["rules"]
        self.assertEqual(rules["exact_loss_baseline"], {"distinct": 0, "test_carriers": 0})

    def test_boundary_inclusive_at_both_transfer_endpoints(self):
        for boundary in (50, 153):
            result = self.run_analysis(
                [{"literal_tuple": ["chr22", 100, "AAAA", "A"], "test_carriers": 1}],
                [probe(100, "AAAA", "A")], [(boundary, boundary)],
            )
            self.assertEqual(result["by_kind"]["deletion"]["rules"]["exact_loss_baseline"]["distinct"], 0)

    def test_overlap_and_incremental_rules_keep_weights(self):
        manifest = [
            {"literal_tuple": ["chr22", 100, "A", "AT"], "test_carriers": 2},
            {"literal_tuple": ["chr22", 200, "A", "AG"], "test_carriers": 5},
        ]
        result = self.run_analysis(manifest, [probe(100), probe(200, alternate="AG")], [(151, 151)])
        rules = result["by_kind"]["insertion"]["rules"]
        self.assertEqual(rules["snv_losszero"], {"distinct": 2, "test_carriers": 7})
        self.assertEqual(rules["overlap"], {"distinct": 1, "test_carriers": 2})
        self.assertEqual(rules["snv_only_incremental_beyond_exact_loss"], {"distinct": 1, "test_carriers": 5})

    def test_missing_or_ambiguous_probe_is_not_zero(self):
        result = self.run_analysis(
            [{"literal_tuple": ["chr22", 100, "A", "AT"], "test_carriers": 4}],
            [{"status": "ambiguous", "input": {"contig": "chr22", "position": 100, "reference": "A", "alternate": "AT"}}], [(1000, 1000)],
        )
        self.assertEqual(result["by_kind"]["insertion"]["rules"]["snv_losszero"], {"distinct": 0, "test_carriers": 0})


if __name__ == "__main__":
    unittest.main()
