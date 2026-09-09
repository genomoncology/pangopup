//! The shipped gene-name index answers a single accession from bytes that
//! travel with the build.
//!
//! Every assertion here reads the committed index. None of them reaches the
//! network and none of them needs an installed asset.

use pangopup_index::gene_names::{
    GeneNameSource, GeneNaming, UnnamedReason, shipped, shipped_naming_measured,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// The 4096-byte page budget one accession lookup may address, counted from
/// opening the index through returning the name. The index is a sorted key
/// array, a fixed-stride record array and one contiguous string run per gene,
/// so the whole path addresses the binary-search probes, one record and one
/// string run. The budget is the bound that keeps the cost independent of how
/// many genes the index holds.
const PAGE_BUDGET: u64 = 16;

/// The floor the same measurement may not fall below. A reader that answered
/// from a structure built beside the index rather than from the index bytes
/// would address fewer pages than a binary search costs.
const PAGE_FLOOR: u64 = 2;

fn repository(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

#[test]
fn the_shipped_index_says_which_source_named_each_gene() {
    let index = shipped();

    let braf = index.names("ENSG00000157764").expect("BRAF is named");
    assert_eq!(braf.symbol, "BRAF");
    assert_eq!(braf.source, GeneNameSource::Hgnc);
    assert_eq!(braf.hgnc_id.as_deref(), Some("HGNC:1097"));
    assert_eq!(braf.ncbi_gene_id, Some(673));
    assert_eq!(braf.alias_symbols, ["BRAF1", "BRAF-1"]);

    // HGNC does not name this accession at all. NCBI does, so the index
    // reaches a gene the previous single-source design left unnamed.
    let ervfc1 = index.names("ENSG00000233887").expect("ERVFC1 is named");
    assert_eq!(ervfc1.symbol, "ERVFC1");
    assert_eq!(ervfc1.source, GeneNameSource::Ncbi);
    assert_eq!(
        ervfc1.hgnc_id, None,
        "a gene NCBI named reports no HGNC identifier rather than an empty one"
    );
    assert_eq!(ervfc1.ncbi_gene_id, Some(105_373_297));
    assert_eq!(
        ervfc1.alias_symbols,
        ["Fc1env", "HERV-Fc1env"],
        "alias symbols survive from the NCBI source too"
    );
}

#[test]
fn hgnc_supplies_the_whole_record_where_it_names_a_gene() {
    // HGNC approves CYP2D7 for this accession and reports NCBI Gene 1564.
    // NCBI reports symbol LOC105377203 and gene 105377203 for the same
    // accession. HGNC names it, so NCBI is not consulted for any of its
    // fields. Reporting 105377203 here would mean the two sources were merged
    // field by field.
    let names = shipped().names("ENSG00000205702").expect("CYP2D7 is named");
    assert_eq!(names.symbol, "CYP2D7");
    assert_eq!(names.source, GeneNameSource::Hgnc);
    assert_eq!(names.hgnc_id.as_deref(), Some("HGNC:2624"));
    assert_eq!(names.ncbi_gene_id, Some(1564));
}

#[test]
fn an_accession_two_records_name_reports_no_name() {
    let index = shipped();

    // HGNC carries two approved records for this accession.
    assert_eq!(
        index.naming("ENSG00000175658"),
        Some(GeneNaming::Unnamed(UnnamedReason::ConflictingRecords))
    );
    assert_eq!(index.names("ENSG00000175658"), None);

    // NCBI carries two records for this accession and HGNC does not reach it.
    assert_eq!(
        index.naming("ENSG00000235059"),
        Some(GeneNaming::Unnamed(UnnamedReason::ConflictingRecords))
    );
    assert_eq!(index.names("ENSG00000235059"), None);

    // Neither source reaches this accession. It is a scoreable gene and the
    // index reports nothing rather than inventing a label.
    assert_eq!(index.naming("ENSG00000002079"), None);
    assert_eq!(index.names("ENSG00000002079"), None);
}

#[test]
fn the_index_carries_both_sources() {
    let counts = shipped().counts();
    assert!(
        counts.named_by_hgnc >= 41_000,
        "HGNC names the bulk of the index: {counts:?}"
    );
    assert!(
        counts.named_by_ncbi >= 2_000,
        "NCBI reaches genes HGNC does not: {counts:?}"
    );
    assert_eq!(
        counts.accessions,
        counts.named_by_hgnc + counts.named_by_ncbi + counts.unnamed,
        "every admitted accession is named by one source or reported unnamed"
    );
}

#[test]
fn resolving_one_gene_name_addresses_a_bounded_number_of_pages() {
    for accession in [
        "ENSG00000157764",
        "ENSG00000233887",
        "ENSG00000175658",
        "ENSG00000002079",
    ] {
        // The measurement opens the shipped index and resolves the accession
        // under one page set. A reader that decoded the whole index at open,
        // and then answered from a map, would address every page of it here.
        let (_, metrics) = shipped_naming_measured(accession);
        assert!(
            metrics.unique_pages_addressed <= PAGE_BUDGET,
            "{accession} addressed {} pages from open, over the budget of {PAGE_BUDGET}",
            metrics.unique_pages_addressed
        );
        assert!(
            metrics.unique_pages_addressed >= PAGE_FLOOR,
            "{accession} addressed {} pages, under the {PAGE_FLOOR} a binary search over the \
             shipped index costs, so the answer did not come from the index bytes",
            metrics.unique_pages_addressed
        );
    }
}

#[test]
fn the_recorded_provenance_matches_the_committed_index() {
    let member = repository("assets/gene-names/gene-names.pgn");
    let bytes = fs::read(&member).expect("the repository ships the gene-name index");
    let record: Value = serde_json::from_slice(
        &fs::read(repository("assets/gene-names/provenance.json"))
            .expect("the repository records what the index was built from"),
    )
    .expect("provenance JSON");

    assert_eq!(
        record["index"]["bytes"].as_u64(),
        Some(bytes.len() as u64),
        "the record states the committed index's own byte count"
    );
    assert_eq!(
        record["index"]["sha256"].as_str(),
        Some(format!("sha256:{:x}", Sha256::digest(&bytes)).as_str()),
        "the record states the committed index's own digest"
    );

    let sources = record["sources"].as_array().expect("two source releases");
    assert_eq!(sources.len(), 2, "the index is built from two sources");
    for source in sources {
        assert!(source["publisher"].is_string(), "{source}");
        assert!(source["file"].is_string(), "{source}");
        assert!(source["url"].is_string(), "{source}");
        assert!(source["bytes"].as_u64().is_some_and(|v| v > 0), "{source}");
        assert!(
            source["sha256"]
                .as_str()
                .is_some_and(|digest| digest.starts_with("sha256:")),
            "{source}"
        );
        assert!(source["rows"].as_u64().is_some_and(|v| v > 0), "{source}");
    }

    let hgnc = &sources[0];
    assert!(
        hgnc["file"]
            .as_str()
            .is_some_and(|file| file.starts_with("hgnc_complete_set_")),
        "the first source is the dated HGNC release: {hgnc}"
    );
    assert_eq!(
        hgnc["refetchable"].as_bool(),
        Some(true),
        "HGNC publishes immutable dated releases"
    );

    let ncbi = &sources[1];
    assert_eq!(
        ncbi["file"].as_str(),
        Some("Homo_sapiens.gene_info.gz"),
        "the second source is the NCBI gene information file"
    );
    assert_eq!(
        ncbi["refetchable"].as_bool(),
        Some(false),
        "NCBI replaces this file at a fixed URL, so the recorded bytes cannot be fetched again"
    );
}
