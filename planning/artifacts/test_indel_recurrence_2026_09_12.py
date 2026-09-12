import importlib.util
import argparse
import gzip
import json
import pathlib
import tempfile
import unittest


MODULE_PATH = pathlib.Path(__file__).with_name("indel_recurrence_2026_09_12.py")
SPEC = importlib.util.spec_from_file_location("indel_recurrence", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class GenotypeTests(unittest.TestCase):
    def test_reports_each_carried_alternate_once(self):
        self.assertEqual(MODULE.carried_alternates("0|1", 2), {1})
        self.assertEqual(MODULE.carried_alternates("1/1", 2), {1})
        self.assertEqual(MODULE.carried_alternates("1|2", 2), {1, 2})
        self.assertEqual(MODULE.carried_alternates(".", 2), set())

    def test_rejects_an_out_of_range_allele(self):
        with self.assertRaises(ValueError):
            MODULE.carried_alternates("0|3", 2)


class AlleleTests(unittest.TestCase):
    def test_classifies_supported_anchored_indels(self):
        self.assertEqual(MODULE.classify_allele("A", "AT", 100), "supported_insertion")
        self.assertEqual(MODULE.classify_allele("AT", "A", 100), "supported_deletion")

    def test_separates_other_shapes_and_rejections(self):
        self.assertEqual(MODULE.classify_allele("A", "T", 100), "snv")
        self.assertEqual(MODULE.classify_allele("AT", "GC", 100), "equal_length")
        self.assertEqual(MODULE.classify_allele("A", "<DEL>", 100), "symbolic")
        self.assertEqual(MODULE.classify_allele("N", "NA", 100), "unsupported_base")
        self.assertEqual(MODULE.classify_allele("A", "A" + "T" * 100, 100), "too_long")
        self.assertEqual(MODULE.classify_allele("A", "TA", 100), "unsupported_indel_shape")


class SplitTests(unittest.TestCase):
    def test_split_is_deterministic_and_population_stratified(self):
        populations = {
            "A1": "AFR",
            "A2": "AFR",
            "A3": "AFR",
            "A4": "AFR",
            "A5": "AFR",
            "E1": "EUR",
            "E2": "EUR",
            "E3": "EUR",
            "E4": "EUR",
            "E5": "EUR",
        }
        first = MODULE.split_samples(populations, 0.2)
        second = MODULE.split_samples(dict(reversed(list(populations.items()))), 0.2)
        self.assertEqual(first, second)
        train, test = first
        self.assertEqual(len(test), 2)
        self.assertEqual({populations[sample] for sample in test}, {"AFR", "EUR"})
        self.assertFalse(train & test)
        self.assertEqual(train | test, set(populations))


class IntervalTests(unittest.TestCase):
    def test_uses_pangopup_open_left_gene_membership(self):
        intervals = MODULE.IntervalIndex({"chr22": [(101, 200), (301, 400)]})
        self.assertFalse(intervals.contains("chr22", 100))
        self.assertTrue(intervals.contains("chr22", 101))
        self.assertTrue(intervals.contains("chr22", 200))
        self.assertFalse(intervals.contains("chr22", 201))

    def test_counts_a_deletion_by_anchor_and_span_separately(self):
        intervals = MODULE.IntervalIndex({"chr22": [(101, 200)]})
        self.assertFalse(intervals.contains("chr22", 99))
        self.assertTrue(intervals.overlaps_closed("chr22", 99, 101))

    def test_boundary_index_checks_the_closed_scoring_window(self):
        boundaries = MODULE.BoundaryIndex({"chr22": [100, 151, 250]})
        self.assertTrue(boundaries.has_between("chr22", 101, 151))
        self.assertFalse(boundaries.has_between("chr22", 152, 249))


class CurveTests(unittest.TestCase):
    def test_full_training_catalogue_counts_only_heldout_carriers_of_seen_variants(self):
        seen = MODULE.VariantStats(("chr22", 100, "A", "AT"), "supported_insertion", 1)
        seen.train_carriers = 3
        seen.test_carriers = 2
        missed = MODULE.VariantStats(("chr22", 200, "A", "AG"), "supported_insertion", 1)
        missed.test_carriers = 5

        curve = MODULE.curve_for_scope([seen, missed], "all_supported")

        self.assertEqual(curve, [{
            "scope": "all_supported",
            "catalogue_entries": 1,
            "test_carried_requests": 7,
            "test_unique_requests": 2,
            "hits": 2,
            "hit_rate": 2 / 7,
        }])

    def test_distribution_reports_unique_catalogue_and_request_counts(self):
        shared = MODULE.VariantStats(("chr22", 100, "A", "AT"), "supported_insertion", 1)
        shared.train_carriers = 3
        shared.test_carriers = 2
        heldout_only = MODULE.VariantStats(("chr22", 200, "AAA", "A"), "supported_deletion", 2)
        heldout_only.test_carriers = 5

        distribution = MODULE.summarize_variants([shared, heldout_only], lambda variant: str(variant.length))

        self.assertEqual(distribution, {
            "1": {
                "unique_variants": 1,
                "training_catalogue_entries": 1,
                "test_unique_requests": 1,
                "test_carried_requests": 2,
                "hits_full_training_catalogue": 2,
            },
            "2": {
                "unique_variants": 1,
                "training_catalogue_entries": 0,
                "test_unique_requests": 1,
                "test_carried_requests": 5,
                "hits_full_training_catalogue": 0,
            },
        })


class EndToEndTests(unittest.TestCase):
    def test_analyze_keeps_training_catalogue_and_gene_membership_separate(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            panel = root / "panel.tsv"
            panel.write_text(
                "sample\tpop\nA1\tAFR\nA2\tAFR\nE1\tEUR\nE2\tEUR\n",
                encoding="utf-8",
            )
            populations = MODULE.read_panel(panel)
            train, test = MODULE.split_samples(populations, 0.5)
            samples = ["A1", "A2", "E1", "E2"]

            def genotypes(carriers):
                return ["0|1" if sample in carriers else "0|0" for sample in samples]

            shared = {next(iter(train)), next(iter(test))}
            heldout_only = set(test)
            vcf = root / "cohort.vcf.gz"
            with gzip.open(vcf, "wt", encoding="utf-8") as handle:
                handle.write("##fileformat=VCFv4.2\n")
                handle.write("#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\t" + "\t".join(samples) + "\n")
                handle.write("chr22\t150\t.\tA\tAT\t.\t.\t.\tGT\t" + "\t".join(genotypes(shared)) + "\n")
                handle.write("chr22\t99\t.\tAAA\tA\t.\t.\t.\tGT\t" + "\t".join(genotypes(shared)) + "\n")
                handle.write("chr22\t160\t.\tA\tAG\t.\t.\t.\tGT\t" + "\t".join(genotypes(heldout_only)) + "\n")
            gtf = root / "genes.gtf.gz"
            with gzip.open(gtf, "wt", encoding="utf-8") as handle:
                handle.write("chr22\ttest\tgene\t100\t200\t.\t+\t.\tgene_id \"G1\";\n")
            output_json = root / "result.json"
            output_curve = root / "curve.tsv"
            output_variants = root / "variants.tsv.gz"
            arguments = argparse.Namespace(
                panel=panel,
                gtf=gtf,
                vcf=vcf,
                contig="chr22",
                holdout_fraction=0.5,
                max_allele_bases=100,
                output_json=output_json,
                output_curve=output_curve,
                output_variants=output_variants,
            )

            summary, curves, variants = MODULE.analyze(arguments)
            MODULE.write_outputs(arguments, summary, curves, variants)

            self.assertEqual(summary["samples"]["train"], 2)
            self.assertEqual(summary["samples"]["test"], 2)
            self.assertEqual(summary["counts"]["supported_unique"], 3)
            self.assertEqual(summary["counts"]["anchor_genic_unique"], 2)
            self.assertEqual(summary["counts"]["span_genic_unique"], 3)
            all_supported = next(row for row in curves if row["scope"] == "all_supported")
            self.assertEqual(all_supported["test_carried_requests"], 4)
            self.assertEqual(all_supported["hits"], 2)
            self.assertEqual(all_supported["hit_rate"], 0.5)
            self.assertEqual(json.loads(output_json.read_text())["schema"], "pangopup-indel-recurrence-v1")
            with gzip.open(output_variants, "rt", encoding="utf-8") as handle:
                rows = handle.read().splitlines()
            self.assertEqual(
                rows[0],
                "contig\tposition\treference\talternate\tkind\tlength\ttrain_carriers\ttest_carriers\tanchor_genic\tspan_genic",
            )
            self.assertEqual(len(rows), 4)


if __name__ == "__main__":
    unittest.main()
