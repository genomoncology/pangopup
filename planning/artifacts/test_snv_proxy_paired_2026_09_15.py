import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SPEC = importlib.util.spec_from_file_location("paired", Path(__file__).with_name("snv_proxy_paired_2026_09_15.py"))
PAIRED = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PAIRED)


def census(position, gene, anchor, flank, complete=True, loss_anchor=None, loss_flank=None):
    def window(maximum, loss_maximum):
        return {"complete": complete, "gain": {"max_hundredths": maximum},
                "loss": {"max_hundredths": maximum if loss_maximum is None else loss_maximum}}
    return {"contig": "chr22", "position": position, "reference": "A", "alternate": "AT",
            "gene": gene, "snv_bundle_id": "snv-bundle", "anchor": window(anchor, loss_anchor),
            "plus_minus_10": window(flank, loss_flank)}


def model(position, records, status="found"):
    return {"contig": "chr22", "position": position, "ref": "A", "alt": "AT",
            "status": status, "records": records, "provenance": {"kind": "model", "model_bundle_id": "model-bundle"}}


def record(gene, gain, loss):
    return {"stable_gene": gene, "gain_score": gain, "loss_score": loss}


class PairedTests(unittest.TestCase):
    def analyze(self, manifest_rows, model_rows, census_rows):
        with tempfile.TemporaryDirectory() as directory:
            paths = [Path(directory) / name for name in ("manifest", "model", "census")]
            for path, items in zip(paths, (manifest_rows, model_rows, census_rows)):
                path.write_text("\n".join(json.dumps(row) for row in items) + "\n", encoding="utf-8")
            return PAIRED.analyze(*paths)

    def test_overlapping_genes_require_every_matching_complete_row(self):
        manifest = [{"literal_tuple": ["chr22", 100, "A", "AT"], "test_carriers": 3}]
        labels = [model(100, [record("ENSG1", "0.00", "-0.20"),
                              record("ENSG2", "0.00", "0.00")])]
        probe = [census(100, "ENSG1.1", 0, 0), census(100, "ENSG2", 20, 20, False)]
        result = self.analyze(manifest, labels, probe)
        anchor = result["by_kind"]["insertion"]["windows"]["anchor"]
        self.assertEqual(anchor["eligible_complete"], {"distinct": 0, "test_carriers": 0})
        self.assertEqual(anchor["incomplete_or_missing_snv"], {"distinct": 1, "test_carriers": 3})

    def test_signed_loss_and_threshold_equality_are_false_dismissals(self):
        manifest = [{"literal_tuple": ["chr22", 100, "A", "AT"], "test_carriers": 7}]
        labels = [model(100, [record("ENSG1", "0.00", "-0.20")])]
        probe = [census(100, "ENSG1.2", 10, 5)]
        result = self.analyze(manifest, labels, probe)
        anchor = result["by_kind"]["insertion"]["windows"]["anchor"]
        cell = anchor["rules"]["combined"]["10"]["20"]
        self.assertEqual(cell["fast_below_threshold"], {"distinct": 1, "test_carriers": 7})
        self.assertEqual(cell["false_dismissals"], {"distinct": 1, "test_carriers": 7})
        above = anchor["rules"]["combined"]["5"]["20"]
        self.assertEqual(above["true_positives"], {"distinct": 1, "test_carriers": 7})
        self.assertEqual(result["variants"][0]["windows"]["anchor"]["model_max_hundredths"]["loss"], 20)

    def test_gain_loss_and_combined_rules_answer_separate_questions(self):
        manifest = [{"literal_tuple": ["chr22", 100, "A", "AT"], "test_carriers": 5}]
        labels = [model(100, [record("ENSG1", "0.00", "-0.20")])]
        probe = [census(100, "ENSG1", 20, 20, loss_anchor=0, loss_flank=0)]
        result = self.analyze(manifest, labels, probe)
        rules = result["by_kind"]["insertion"]["windows"]["anchor"]["rules"]
        self.assertEqual(rules["gain"]["10"]["20"]["fast_above_cutoff"]["distinct"], 1)
        self.assertEqual(rules["gain"]["10"]["20"]["true_positives"]["distinct"], 0)
        self.assertEqual(rules["loss"]["10"]["20"]["false_dismissals"],
                         {"distinct": 1, "test_carriers": 5})
        self.assertEqual(rules["combined"]["10"]["20"]["true_positives"],
                         {"distinct": 1, "test_carriers": 5})

    def test_missing_and_failed_model_do_not_enter_fast_denominator(self):
        manifest = [{"literal_tuple": ["chr22", p, "A", "AT"], "test_carriers": w}
                    for p, w in ((100, 2), (200, 4))]
        labels = [model(200, [], "error")]
        result = self.analyze(manifest, labels, [census(100, "ENSG1", 0, 0)])
        group = result["by_kind"]["insertion"]
        self.assertEqual(group["model_missing"], {"distinct": 1, "test_carriers": 2})
        self.assertEqual(group["model_failed"], {"distinct": 1, "test_carriers": 4})
        self.assertEqual(group["windows"]["anchor"]["eligible_complete"]["distinct"], 0)

    def test_model_gene_absent_from_census_is_incomplete_even_with_other_gene(self):
        manifest = [{"literal_tuple": ["chr22", 100, "A", "AT"], "test_carriers": 1}]
        result = self.analyze(manifest, [model(100, [record("ENSG2", "0.10", "0.00")])],
                              [census(100, "ENSG1", 0, 0)])
        self.assertFalse(result["variants"][0]["windows"]["anchor"]["eligible_complete"])


if __name__ == "__main__":
    unittest.main()
