import importlib.util
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).with_name("indel_catalogue_transfer_2026_09_12.py")
SPEC = importlib.util.spec_from_file_location("indel_catalogue_transfer", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class TransferTests(unittest.TestCase):
    def test_finds_an_info_value_at_the_start_or_middle(self):
        self.assertEqual(MODULE.info_value("AC=4;AN=20", "AC"), "4")
        self.assertEqual(MODULE.info_value("AN=20;AC=4,2;AF=0.2,0.1", "AC"), "4,2")
        self.assertIsNone(MODULE.info_value("AN=20", "AC"))

    def test_loads_positive_supported_genic_alleles_from_sites_vcf(self):
        genes = MODULE.IntervalIndex({"chr22": [(101, 200)]})
        lines = [
            "##fileformat=VCFv4.2",
            "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO",
            "chr22\t150\t.\tA\tAT,AG\t.\tPASS\tAC=4,0",
            "chr22\t160\t.\tAAA\tA,<DEL>\t.\tPASS\tAC=2,3",
            "chr22\t170\t.\tA\tC\t.\tPASS\tAC=8",
            "chr22\t180\t.\tA\tAG\t.\tPASS\tAN=100",
            "chr22\t250\t.\tA\tAG\t.\tPASS\tAC=9",
        ]

        result = MODULE.load_sites_catalogue_lines(lines, genes, "chr22", 100)

        self.assertEqual(
            result,
            {
                ("chr22", 150, "A", "AT"),
                ("chr22", 160, "AAA", "A"),
            },
        )

    def test_counts_literal_supported_genic_requests_and_catalogue_hits_per_person(self):
        genes = MODULE.IntervalIndex({"chr22": [(101, 200)]})
        boundaries = MODULE.BoundaryIndex({"chr22": []})
        catalogue = {("chr22", 150, "A", "AT")}
        lines = [
            "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tP1\tP2",
            "chr22\t150\t.\tA\tAT\t.\tPASS\tGnomAD_Genomes_AC=10;GnomAD_AC=12\tGT\t0/1\t0/0",
            "chr22\t160\t.\tAAA\tA\t.\tPASS\tGnomAD_Genomes_AC=0;GnomAD_AC=2\tGT\t1/1\t0/1",
            "chr22\t170\t.\tA\t<DEL>\t.\tPASS\t.\tGT\t0/1\t0/1",
            "chr22\t250\t.\tA\tAG\t.\tPASS\t.\tGT\t0/1\t0/1",
        ]

        result = MODULE.measure_lines(lines, catalogue, genes, boundaries, "chr22", 100)

        self.assertEqual(result["people"], 2)
        self.assertEqual(result["genic_supported_requests"], 3)
        self.assertEqual(result["catalogue_hits"], 1)
        self.assertEqual(result["per_person_requests"], [2, 1])
        self.assertEqual(result["per_person_hits"], [1, 0])
        self.assertEqual(result["gnomad_genomes_requests"], 1)
        self.assertEqual(result["gnomad_any_requests"], 3)
        self.assertEqual(result["catalogue_or_gnomad_genomes_requests"], 1)
        self.assertEqual(result["catalogue_or_gnomad_any_requests"], 3)
        self.assertEqual(result["exact_loss_zero_requests"], 3)
        self.assertEqual(result["rejections"], {"symbolic": 2})

    def test_counts_pass_and_nonpass_traffic_separately(self):
        genes = MODULE.IntervalIndex({"chr22": [(101, 200)]})
        boundaries = MODULE.BoundaryIndex({"chr22": [150]})
        catalogue = {("chr22", 150, "A", "AT")}
        lines = [
            "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tP1",
            "chr22\t150\t.\tA\tAT\t.\tq10\t.\tGT\t0/1",
        ]

        result = MODULE.measure_lines(lines, catalogue, genes, boundaries, "chr22", 100)

        self.assertEqual(result["genic_supported_requests"], 1)
        self.assertEqual(result["pass_genic_supported_requests"], 0)
        self.assertEqual(result["catalogue_hits"], 1)
        self.assertEqual(result["pass_catalogue_hits"], 0)
        self.assertEqual(result["exact_loss_zero_requests"], 0)


if __name__ == "__main__":
    unittest.main()
