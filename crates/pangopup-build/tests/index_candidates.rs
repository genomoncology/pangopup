#[path = "../../pangopup-index/benches/support/candidates.rs"]
mod candidates;

use pangopup_build::collect_index_loci;
use std::{fs, path::Path};

fn allocation_count() -> u64 {
    0
}

#[test]
fn every_candidate_round_trips_the_checked_fixture_exactly() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pangolin-precompute");
    let (summary, input) = collect_index_loci(&fixture).expect("checked fixture");
    assert_eq!(summary.rows, 6_342);
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/candidate-roundtrips");
    candidates::assert_roundtrip(&input, &output).expect("all candidate round trips");
    for codec in candidates::codecs() {
        let path = output.join(format!("{}.candidate", codec_name(codec)));
        if path.exists() {
            fs::remove_file(path).expect("remove candidate artifact");
        }
    }
}

#[test]
fn candidate_gene_filter_is_logarithmic_on_complete_directory_scale() {
    let output =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/candidate-gene-filter-proof");
    candidates::assert_bounded_gene_filter(&output).expect("bounded candidate gene lookup");
}

fn codec_name(codec: candidates::Codec) -> String {
    match codec {
        candidates::Codec::Direct => "direct-sparse".to_owned(),
        candidates::Codec::Fixed => "fixed-11".to_owned(),
        candidates::Codec::Zstd(size) => format!("zstd-{size}"),
        candidates::Codec::Lz4(size) => format!("lz4-{size}"),
    }
}
