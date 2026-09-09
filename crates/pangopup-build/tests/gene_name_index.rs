//! The offline gene-name index builder.
//!
//! These assertions read two miniature source excerpts from the checkout.
//! Nothing here reaches the network, so the determinism proof holds in every
//! gate.

use pangopup_build::naming::build_gene_name_index;
use pangopup_index::gene_names::{GeneNameIndex, GeneNameSource, GeneNaming, UnnamedReason};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/gene-naming-mini")
        .join(name)
}

fn build(output: &Path) -> pangopup_build::naming::GeneNameIndexReport {
    build_gene_name_index(
        &fixture("hgnc_complete_set_2026-09-04.tsv"),
        &fixture("Homo_sapiens.gene_info"),
        output,
    )
    .expect("build the miniature gene-name index")
}

#[test]
fn rebuilding_from_identical_source_bytes_produces_identical_index_bytes() {
    let scratch = tempfile::tempdir().expect("scratch");
    let first_path = scratch.path().join("first.pgn");
    let second_path = scratch.path().join("second.pgn");
    let first = build(&first_path);
    let second = build(&second_path);

    let first_bytes = std::fs::read(&first_path).expect("first index");
    let second_bytes = std::fs::read(&second_path).expect("second index");
    assert!(!first_bytes.is_empty(), "the builder writes an index");
    assert_eq!(
        first_bytes, second_bytes,
        "identical source bytes produce identical index bytes"
    );
    assert_eq!(first.index_sha256, second.index_sha256);
    assert_eq!(first.index_bytes, first_bytes.len() as u64);

    GeneNameIndex::open(&first_path)
        .expect("open the built index")
        .verify_canonical_structure()
        .expect("the writer packs its sections and string runs with no padding");
}

#[test]
fn the_builder_reports_the_sources_and_the_reach_it_built_from() {
    let scratch = tempfile::tempdir().expect("scratch");
    let report = build(&scratch.path().join("index.pgn"));

    assert_eq!(report.hgnc.file, "hgnc_complete_set_2026-09-04.tsv");
    assert_eq!(report.hgnc.rows, 10);
    assert!(report.hgnc.sha256.starts_with("sha256:"));
    assert!(report.hgnc.bytes > 0);
    assert_eq!(report.ncbi.file, "Homo_sapiens.gene_info");
    assert_eq!(report.ncbi.rows, 9);
    assert!(report.ncbi.sha256.starts_with("sha256:"));
    assert!(report.ncbi.bytes > 0);

    assert_eq!(report.named_by_hgnc, 7);
    assert_eq!(report.named_by_ncbi, 2);
    assert_eq!(report.unnamed, 3);
    assert_eq!(report.accessions, 12);
}

#[test]
fn ncbi_names_only_the_accessions_hgnc_does_not_reach() {
    let scratch = tempfile::tempdir().expect("scratch");
    let path = scratch.path().join("index.pgn");
    build(&path);
    let index = GeneNameIndex::open(&path).expect("open the built index");

    // HGNC reaches this accession and withholds the name because its approved
    // symbol is a clone-derived placeholder. NCBI names it GRK1. Reporting
    // GRK1 here would mean NCBI was consulted for an accession HGNC reaches.
    assert_eq!(
        index.naming("ENSG00000185974"),
        Some(GeneNaming::Unnamed(UnnamedReason::PlaceholderSymbol))
    );

    // HGNC lists two alias symbols for CD4 and NCBI lists five synonyms.
    let cd4 = index.names("ENSG00000010610").expect("CD4 is named");
    assert_eq!(cd4.source, GeneNameSource::Hgnc);
    assert_eq!(cd4.alias_symbols, ["T4", "Leu-3"]);

    // The HGNC excerpt clears TP53's NCBI Gene identifier. NCBI carries 7157.
    let tp53 = index.names("ENSG00000141510").expect("TP53 is named");
    assert_eq!(tp53.source, GeneNameSource::Hgnc);
    assert_eq!(tp53.ncbi_gene_id, None);

    // Named only by NCBI. Previous symbols stay empty because the NCBI source
    // publishes no previous-symbol field, and the synonyms become aliases.
    let ervfc1 = index.names("ENSG00000233887").expect("ERVFC1 is named");
    assert_eq!(ervfc1.source, GeneNameSource::Ncbi);
    assert_eq!(ervfc1.hgnc_id, None);
    assert_eq!(ervfc1.ncbi_gene_id, Some(105_373_297));
    assert!(ervfc1.prev_symbols.is_empty());
    assert_eq!(ervfc1.alias_symbols, ["Fc1env", "HERV-Fc1env"]);

    // Named only by NCBI, with no synonyms at all.
    let readthrough = index
        .names("ENSG00000249624")
        .expect("IFNAR2-IL10RB is named");
    assert_eq!(readthrough.symbol, "IFNAR2-IL10RB");
    assert_eq!(readthrough.source, GeneNameSource::Ncbi);
    assert!(readthrough.alias_symbols.is_empty());

    // Two NCBI records name one accession HGNC does not reach.
    assert_eq!(
        index.naming("ENSG00000235059"),
        Some(GeneNaming::Unnamed(UnnamedReason::ConflictingRecords))
    );
}

#[test]
fn a_source_row_without_a_human_ensembl_accession_enters_no_record() {
    let scratch = tempfile::tempdir().expect("scratch");
    let path = scratch.path().join("index.pgn");
    build(&path);
    let index = GeneNameIndex::open(&path).expect("open the built index");

    // The NCBI excerpt carries one row whose `tax_id` is 10090 and whose
    // Ensembl cross-reference is the human NKX3-1 accession. HGNC names that
    // accession, and the merged record must come from HGNC alone.
    let nkx3 = index.names("ENSG00000167034").expect("NKX3-1 is named");
    assert_eq!(nkx3.symbol, "NKX3-1");
    assert_eq!(nkx3.source, GeneNameSource::Hgnc);

    // The excerpt also carries CYP2D7BP, whose `dbXrefs` cell is `-`. It
    // reaches no accession, so it contributes no record and moves no count.
    let report = build(&scratch.path().join("again.pgn"));
    assert_eq!(report.accessions, 12);
}
