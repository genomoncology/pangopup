import importlib.util
import json
import pathlib
import tempfile
import unittest


PATH = pathlib.Path(__file__).with_name("snv_proxy_challenge_2026_09_15.py")
SPEC = importlib.util.spec_from_file_location("snv_proxy_challenge", PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def scored(position, ref, alt, gene="G1", *, complete=True, gain=0, loss=0, following="CGTA"):
    return {
        "contig": "chr22", "position": position, "reference": ref, "alternate": alt,
        "gene": gene, "kind": "insertion" if len(alt) > len(ref) else "deletion",
        "reference_after_anchor": following,
        "plus_minus_10": {"complete": complete, "gain": {"max_hundredths": gain}, "loss": {"max_hundredths": loss}},
    }


def write(path, rows):
    path.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")


class ChallengeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        self.census = self.root / "census.jsonl"
        self.labels = self.root / "labels.jsonl"
        self.selected = self.root / "selected.jsonl"
        self.probe = self.root / "updated.jsonl"
        self.output = self.root / "challenge.jsonl"
        self.manifest = self.root / "manifest.jsonl"

    def test_selection_rejects_partial_overlapping_genes_and_label_loci(self):
        rows = [
            scored(100, "A", "AT", "G1"), scored(100, "A", "AT", "G2", complete=False),
            scored(110, "A", "AG", "G1"), scored(110, "A", "AG", "G2"),
            scored(120, "A", "AC", "G1"), scored(130, "AC", "A", "G1"),
            scored(140, "AC", "A", "G1", gain=1), scored(150, "AC", "A", "G1"),
        ]
        write(self.census, rows)
        write(self.labels, [scored(120, "A", "AT"), scored(130, "A", "AG")])
        chosen = MODULE.select(self.census, self.labels, self.selected)
        self.assertEqual({row["position"] for row in chosen}, {110, 150})
        self.assertEqual(chosen, [json.loads(line) for line in self.selected.read_text().splitlines()])

    def test_selection_has_stable_sha_rank_and_requires_both_classes(self):
        rows = [scored(200 + n, "A", "AT") for n in range(8)]
        rows += [scored(300 + n, "AC", "A") for n in range(8)]
        write(self.labels, [])
        write(self.census, rows)
        first = MODULE.select(self.census, self.labels, self.selected)
        write(self.census, list(reversed(rows)))
        self.assertEqual(first, MODULE.select(self.census, self.labels, self.selected))
        self.assertEqual(first[0]["position"], min(range(200, 208), key=lambda n: MODULE.rank(("chr22", n, "A", "AT"))))
        write(self.census, rows[:8])
        with self.assertRaisesRegex(ValueError, "deletion"):
            MODULE.select(self.census, self.labels, self.selected)

    def test_selection_excludes_deletions_longer_than_four_bases(self):
        write(self.labels, [])
        write(self.census, [scored(50, "A", "AT"), scored(100, "ACGTAC", "A")])
        with self.assertRaisesRegex(ValueError, "deletion"):
            MODULE.select(self.census, self.labels, self.selected)

    def test_challenge_pins_reference_and_has_sixteen_unique_literals(self):
        origins = [scored(100, "A", "AT"), scored(200, "ACG", "A")]
        write(self.selected, origins)
        write(self.probe, [origins[0], scored(100, "A", "AT", "G2"), origins[1]])
        alleles = MODULE.challenge(self.selected, self.probe, self.output, self.manifest)
        records = [json.loads(line) for line in self.manifest.read_text().splitlines()]
        self.assertEqual(len(alleles), 16)
        self.assertEqual(len({tuple(row.values()) for row in alleles}), 16)
        self.assertEqual([row["reference"] for row in alleles[4:8]], ["AC", "ACG", "ACGT", "ACGTA"])
        self.assertTrue(all(set(row) == {"contig", "position", "reference", "alternate"} for row in alleles))
        self.assertEqual({tuple(row["origin_literal_tuple"]) for row in records}, {("chr22", 100, "A", "AT"), ("chr22", 200, "ACG", "A")})
        self.assertEqual([row["sequence"] for row in records[0:4]], list("ACGT"))

    def test_challenge_rejects_bad_pinned_ref_and_inconsistent_gene_rows(self):
        write(self.selected, [scored(100, "A", "AT"), scored(200, "ACG", "A")])
        write(self.probe, [scored(100, "A", "AT"), scored(200, "ACG", "A", following="TTTT")])
        with self.assertRaisesRegex(ValueError, "original REF"):
            MODULE.challenge(self.selected, self.probe, self.output, self.manifest)
        write(self.probe, [scored(100, "A", "AT"), scored(100, "A", "AT", "G2", following="TGTA"), scored(200, "ACG", "A")])
        with self.assertRaisesRegex(ValueError, "inconsistent"):
            MODULE.challenge(self.selected, self.probe, self.output, self.manifest)
        write(self.probe, [scored(100, "A", "AT"), scored(100, "A", "AT"), scored(200, "ACG", "A")])
        with self.assertRaisesRegex(ValueError, "duplicate"):
            MODULE.challenge(self.selected, self.probe, self.output, self.manifest)

    def test_missing_census_status_is_not_treated_as_zero(self):
        write(self.labels, [])
        write(self.census, [
            scored(100, "A", "AT"),
            {"status": "no_mask_gene", "input": {"contig": "chr22", "position": 100, "reference": "A", "alternate": "AT"}},
            scored(200, "AC", "A"),
        ])
        with self.assertRaisesRegex(ValueError, "insertion"):
            MODULE.select(self.census, self.labels, self.selected)


if __name__ == "__main__":
    unittest.main()
