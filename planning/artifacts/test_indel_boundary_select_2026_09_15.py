import gzip
import json
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import indel_boundary_select_2026_09_15 as selector


class BoundarySelectionTests(unittest.TestCase):
    def test_changed_span_excludes_anchor(self):
        self.assertEqual(selector.changed_span(100, "A", "AT"), (101, 101))
        self.assertEqual(selector.changed_span(100, "ACGT", "A"), (101, 103))

    def test_distance_uses_changed_span_and_closed_threshold(self):
        index = selector.BoundaryIndex({"chr22": [100, 110]})
        self.assertEqual(selector.boundary_distance(index, "chr22", 101, 101, 5), 1)
        self.assertEqual(selector.boundary_distance(index, "chr22", 104, 104, 5), 4)
        self.assertEqual(selector.boundary_distance(index, "chr22", 105, 105, 5), 5)
        self.assertIsNone(selector.boundary_distance(index, "chr22", 116, 116, 5))
        self.assertEqual(selector.boundary_distance(index, "chr22", 106, 112, 5), 0)

    def test_sha_order_is_stable_and_per_kind_limit_is_ten(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            gtf = root / "exons.gtf.gz"
            with gzip.open(gtf, "wt", encoding="utf-8") as output:
                output.write('chr22\ttest\texon\t100\t110\t.\t+\t.\tgene_id "G";\n')
            literals = [("chr22", pos, "A", "AT") for pos in range(95, 108)]
            expected = sorted(literals, key=selector.hash_rank)[:10]
            for name, ordered in (("forward", literals), ("reverse", list(reversed(literals)))):
                catalogue = root / f"{name}.tsv.gz"
                with gzip.open(catalogue, "wt", encoding="utf-8") as output:
                    output.write("contig\tposition\treference\talternate\tkind\tlength\ttrain_carriers\ttest_carriers\tanchor_genic\tspan_genic\n")
                    for contig, pos, ref, alt in ordered:
                        output.write(f"{contig}\t{pos}\t{ref}\t{alt}\tsupported_insertion\t1\t0\t1\t1\t1\n")
                probe = root / f"{name}.jsonl"
                manifest = root / f"{name}-manifest.jsonl"
                counts = selector.select(catalogue, gtf, probe, manifest)
                self.assertEqual(counts["supported_insertion"], 10)
                actual = [tuple(json.loads(line)[field] for field in selector.PROBE_FIELDS)
                          for line in probe.read_text(encoding="utf-8").splitlines()]
                self.assertEqual(actual, expected)
            with self.assertRaises(ValueError):
                selector.select(catalogue, gtf, probe, manifest, sample_per_kind=11)

    def test_selection_prefers_short_lengths_then_hash_rank_and_excludes_inputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            catalogue = root / "variants.tsv.gz"
            gtf = root / "gencode.gtf.gz"
            probe = root / "probe.jsonl"
            manifest = root / "manifest.jsonl"
            labels = root / "labels.jsonl"
            high = root / "high.jsonl"
            challenge = root / "challenge.jsonl"
            with gzip.open(gtf, "wt", encoding="utf-8") as output:
                output.write('chr22\ttest\texon\t110\t130\t.\t+\t.\tgene_id "G";\n')
            rows = [
                ("chr22", 109, "A", "AT", "supported_insertion", 1, 3, 1),
                ("chr22", 108, "A", "AG", "supported_insertion", 1, 4, 1),
                ("chr22", 107, "A", "AC", "supported_insertion", 1, 5, 1),
                ("chr22", 106, "A", "ATTTTT", "supported_insertion", 5, 6, 1),
                ("chr22", 108, "AC", "A", "supported_deletion", 1, 7, 1),
                ("chr22", 105, "ACCCCC", "A", "supported_deletion", 5, 8, 1),
                ("chr22", 140, "A", "AT", "supported_insertion", 1, 9, 1),
                ("chr22", 109, "A", "AA", "supported_insertion", 1, 0, 1),
                ("chr22", 109, "A", "AG", "supported_insertion", 1, 9, 0),
            ]
            with gzip.open(catalogue, "wt", encoding="utf-8") as output:
                output.write("contig\tposition\treference\talternate\tkind\tlength\ttrain_carriers\ttest_carriers\tanchor_genic\tspan_genic\n")
                for contig, pos, ref, alt, kind, length, carriers, genic in rows:
                    output.write(f"{contig}\t{pos}\t{ref}\t{alt}\t{kind}\t{length}\t0\t{carriers}\t{genic}\t1\n")
            labels.write_text(json.dumps({"literal_tuple": ["chr22", 109, "A", "AT"]}) + "\n", encoding="utf-8")
            high.write_text(json.dumps({"input": {"contig": "chr22", "position": 108, "reference": "A", "alternate": "AG"}}) + "\n", encoding="utf-8")
            challenge.write_text(json.dumps({"contig": "chr22", "position": 108, "reference": "AC", "alternate": "A"}) + "\n", encoding="utf-8")
            counts = selector.select(catalogue, gtf, probe, manifest, sample_per_kind=2,
                                     exclusion_paths=[labels, high, challenge])
            self.assertEqual(counts, {"supported_insertion": 2, "supported_deletion": 1})
            selected = [json.loads(line) for line in probe.read_text(encoding="utf-8").splitlines()]
            records = [json.loads(line) for line in manifest.read_text(encoding="utf-8").splitlines()]
            self.assertEqual(len(selected), 3)
            self.assertEqual(set(selected[0]), {"contig", "position", "reference", "alternate"})
            self.assertEqual([row["position"] for row in selected[:2]], [107, 106])
            self.assertEqual(records[0]["test_carriers"], 5)
            self.assertEqual(records[0]["boundary_distance"], 2)
            self.assertEqual(records[1]["changed_length"], 5)
            self.assertTrue(all(tuple(row["literal_tuple"]) not in {
                ("chr22", 109, "A", "AT"), ("chr22", 108, "A", "AG"),
                ("chr22", 108, "AC", "A")
            } for row in records))


if __name__ == "__main__":
    unittest.main()
