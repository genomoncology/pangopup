use pangopup_core::{GenomicPosition, Grch38Contig};
use pangopup_index::mask::{MaskDomainsOpen, MaskProvider, MaskQueryBuffer};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

const EXPECTED_BYTES: usize = 260;
const EXPECTED_SHA256: &str = "004f9f95be50b92fd5c67ca44a785e950c20e5455a903ad9350b68c91566f827";

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Oracle {
    schema: String,
    genes: Vec<Gene>,
    query: Query,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Gene {
    id: String,
    contig: String,
    strand: String,
    start: u32,
    end: u32,
    rank: u32,
    canonical_boundaries: Vec<u32>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Query {
    contig: String,
    position: u32,
    plus: Vec<QueryGene>,
    minus: Vec<QueryGene>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct QueryGene {
    id: String,
    boundaries: Vec<u32>,
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/route-mask")
}

fn literal_oracle() -> Oracle {
    Oracle {
        schema: "pangopup-route-mask-v1".to_owned(),
        genes: vec![Gene {
            id: "ENSG00000000001.1".to_owned(),
            contig: "chr1".to_owned(),
            strand: "+".to_owned(),
            start: 1,
            end: 10_101,
            rank: 0,
            canonical_boundaries: Vec::new(),
        }],
        query: Query {
            contig: "chr1".to_owned(),
            position: 5_051,
            plus: vec![QueryGene {
                id: "ENSG00000000001.1".to_owned(),
                boundaries: Vec::new(),
            }],
            minus: Vec::new(),
        },
    }
}

/// Fixture-only encoding of exactly `literal_oracle`.
///
/// This intentionally accepts no inputs and exposes no file-writing API.
fn encode_literal_oracle() -> Vec<u8> {
    let mut bytes = vec![0_u8; EXPECTED_BYTES];
    bytes[0..8].copy_from_slice(b"PGMBEN01");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10] = 2;
    bytes[11] = 1;
    bytes[16..24].copy_from_slice(&(EXPECTED_BYTES as u64).to_le_bytes());
    bytes[24..32].copy_from_slice(&160_u64.to_le_bytes());

    for (index, (offset, count, stride)) in [
        (160_u64, 1_u64, 40_u32),
        (200, 1, 40),
        (240, 0, 4),
        (240, 1, 16),
        (256, 1, 4),
    ]
    .into_iter()
    .enumerate()
    {
        let start = 32 + index * 24;
        bytes[start..start + 8].copy_from_slice(&offset.to_le_bytes());
        bytes[start + 8..start + 16].copy_from_slice(&count.to_le_bytes());
        bytes[start + 16..start + 20].copy_from_slice(&stride.to_le_bytes());
    }

    bytes[160] = 1;
    bytes[176..184].copy_from_slice(&1_u64.to_le_bytes());
    bytes[192..200].copy_from_slice(&1_u64.to_le_bytes());

    bytes[200] = 1;
    bytes[204..208].copy_from_slice(&1_u32.to_le_bytes());
    bytes[208..212].copy_from_slice(&10_101_u32.to_le_bytes());
    bytes[216..224].copy_from_slice(&1_u64.to_le_bytes());
    bytes[224..228].copy_from_slice(&1_u32.to_le_bytes());

    bytes[240..244].copy_from_slice(&2_u32.to_le_bytes());
    bytes[244..248].copy_from_slice(&10_101_u32.to_le_bytes());
    bytes[252..256].copy_from_slice(&1_u32.to_le_bytes());
    bytes
}

#[test]
fn checked_route_mask_reproduces_the_literal_oracle_and_queries_exactly() {
    let oracle: Oracle = serde_json::from_slice(
        &fs::read(fixture_root().join("fixture.json")).expect("read route mask oracle"),
    )
    .expect("parse route mask oracle");
    assert_eq!(oracle, literal_oracle());

    let expected = encode_literal_oracle();
    assert_eq!(expected.len(), EXPECTED_BYTES);
    assert_eq!(format!("{:x}", Sha256::digest(&expected)), EXPECTED_SHA256);
    let checked = fs::read(fixture_root().join("domains.pgm")).expect("read checked route mask");
    assert_eq!(checked, expected);

    let identified = MaskDomainsOpen::open_identified(&fixture_root().join("domains.pgm"))
        .expect("open identified route mask");
    assert_eq!(identified.identity().bytes(), EXPECTED_BYTES as u64);
    assert_eq!(identified.identity().sha256(), EXPECTED_SHA256);
    let mut output = MaskQueryBuffer::default();
    for (position, matches) in [(1, false), (5_051, true), (10_101, true), (10_102, false)] {
        identified
            .query(
                Grch38Contig::autosome(1).expect("chr1"),
                GenomicPosition::new(position).expect("position"),
                None,
                &mut output,
            )
            .expect("route mask query");
        assert_eq!(!output.plus().is_empty(), matches, "position {position}");
        assert!(output.minus().is_empty());
        if matches {
            assert_eq!(output.plus()[0].identity().to_string(), "ENSG00000000001.1");
            assert!(output.boundaries(&output.plus()[0]).is_empty());
        }
    }
}
