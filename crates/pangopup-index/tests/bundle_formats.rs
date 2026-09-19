use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Snv, PangolinScore, RelativePosition,
    ScoreMagnitude, ScoreProvider,
};
use pangopup_index::{
    AmbiguousInputLocus, BundleManifest, BundleOpen, IndexError, InputAlternative, InputLocus,
    OrdinaryInputLocus, bundle_id, canonical_manifest_bytes,
    sparse_writer::{SPARSE_INDEX_FORMAT, SparseIndexWriter},
    write_index,
};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

const FIXED_FORMAT: &str = "pangopup.fixed11.v1";
const FIXED_MEDIA_TYPE: &str = "application/vnd.pangopup.fixed11";
const SPARSE_MEDIA_TYPE: &str = "application/vnd.pangopup.sparse-direct";

struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "pangopup-bundle-formats-{label}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("create temporary directory");
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove temporary directory");
    }
}

#[test]
fn fixed_and_sparse_bundles_have_identical_provider_answers() {
    let temp = Temp::new("provider-parity");
    let genes = fixture();
    let fixed_path = temp.0.join("fixed.pgi");
    let sparse_path = temp.0.join("sparse.pgi");
    write_fixed(&fixed_path, &genes);
    write_sparse(&temp.0.join("sparse.scratch"), &sparse_path, &genes);

    let (fixed, fixed_manifest) = open_bundle(FIXED_FORMAT, FIXED_MEDIA_TYPE, &fixed_path);
    let (sparse, sparse_manifest) =
        open_bundle(SPARSE_INDEX_FORMAT, SPARSE_MEDIA_TYPE, &sparse_path);
    assert_eq!(fixed.bundle_id(), bundle_id(&fixed_manifest));
    assert_eq!(sparse.bundle_id(), bundle_id(&sparse_manifest));
    assert_ne!(fixed.bundle_id(), sparse.bundle_id());

    let overlap = snv(64, DnaBase::T, DnaBase::A);
    let cases = [
        (overlap, None),
        (overlap, Some(gene(2))),
        (overlap, Some(gene(3))),
        (snv(64, DnaBase::A, DnaBase::C), None),
        (snv(99, DnaBase::A, DnaBase::C), None),
    ];
    for (query, filter) in cases {
        let fixed_result = fixed.lookup(query, filter).expect("fixed lookup");
        let sparse_result = sparse.lookup(query, filter).expect("sparse lookup");
        assert_eq!(fixed_result.records(), sparse_result.records());
        assert_eq!(
            fixed_result.source_reference_ambiguities(),
            sparse_result.source_reference_ambiguities()
        );
        let fixed_provenance = fixed_result
            .provenance()
            .precomputed()
            .expect("fixed provenance");
        let sparse_provenance = sparse_result
            .provenance()
            .precomputed()
            .expect("sparse provenance");
        assert_eq!(fixed_provenance.bundle_id(), fixed.bundle_id());
        assert_eq!(sparse_provenance.bundle_id(), sparse.bundle_id());
        assert_eq!(
            (
                fixed_provenance.source_doi(),
                fixed_provenance.source_archive_md5(),
                fixed_provenance.masked(),
                fixed_provenance.window()
            ),
            (
                sparse_provenance.source_doi(),
                sparse_provenance.source_archive_md5(),
                sparse_provenance.masked(),
                sparse_provenance.window()
            )
        );
    }

    let mixed = sparse.lookup(overlap, None).expect("mixed sparse result");
    assert_eq!(
        mixed
            .records()
            .iter()
            .map(|record| record.gene())
            .collect::<Vec<_>>(),
        vec![gene(1), gene(2)]
    );
    assert_eq!(mixed.source_reference_ambiguities().len(), 1);
    assert_eq!(mixed.source_reference_ambiguities()[0].gene(), gene(3));

    let filtered = sparse
        .lookup(overlap, Some(gene(2)))
        .expect("filtered sparse overlap");
    assert_eq!(filtered.records().len(), 1);
    assert_eq!(filtered.records()[0].gene(), gene(2));
    assert!(filtered.source_reference_ambiguities().is_empty());

    let ambiguity = sparse
        .lookup(overlap, Some(gene(3)))
        .expect("filtered sparse ambiguity");
    assert!(ambiguity.records().is_empty());
    assert_eq!(ambiguity.source_reference_ambiguities().len(), 1);

    let mismatch = sparse
        .lookup(snv(64, DnaBase::A, DnaBase::C), None)
        .expect("reference mismatch");
    assert!(mismatch.records().is_empty());
    assert_eq!(mismatch.source_reference_ambiguities().len(), 1);

    let miss = sparse
        .lookup(snv(99, DnaBase::A, DnaBase::C), None)
        .expect("sparse miss");
    assert!(miss.records().is_empty());
    assert!(miss.source_reference_ambiguities().is_empty());
}

#[test]
fn bundle_open_rejects_unknown_and_mismatched_formats_before_lookup() {
    let temp = Temp::new("format-admission");
    let genes = fixture();
    let fixed_path = temp.0.join("fixed.pgi");
    let sparse_path = temp.0.join("sparse.pgi");
    write_fixed(&fixed_path, &genes);
    write_sparse(&temp.0.join("sparse.scratch"), &sparse_path, &genes);

    let unknown = manifest_bytes("pangopup.unknown.v1", FIXED_MEDIA_TYPE, &fixed_path);
    assert!(matches!(
        open_members(&unknown, &fixed_path).expect_err("unknown format"),
        IndexError::Incompatible("index format version")
    ));

    for (format, media_type, payload) in [
        (FIXED_FORMAT, SPARSE_MEDIA_TYPE, &fixed_path),
        (SPARSE_INDEX_FORMAT, FIXED_MEDIA_TYPE, &sparse_path),
    ] {
        let manifest = manifest_bytes(format, media_type, payload);
        assert!(matches!(
            open_members(&manifest, payload).expect_err("mismatched media type"),
            IndexError::Corrupt("manifest members")
        ));
    }

    for (format, media_type, payload) in [
        (FIXED_FORMAT, FIXED_MEDIA_TYPE, &sparse_path),
        (SPARSE_INDEX_FORMAT, SPARSE_MEDIA_TYPE, &fixed_path),
    ] {
        let manifest = manifest_bytes(format, media_type, payload);
        assert!(
            open_members(&manifest, payload).is_err(),
            "{format} must reject the other payload format"
        );
    }

    let sparse_manifest = manifest_bytes(SPARSE_INDEX_FORMAT, SPARSE_MEDIA_TYPE, &sparse_path);
    let mut sparse_with_extension: serde_json::Value =
        serde_json::from_slice(&sparse_manifest).expect("sparse manifest value");
    sparse_with_extension
        .as_object_mut()
        .expect("sparse manifest object")
        .insert("future_extension".to_owned(), true.into());
    let sparse_with_extension =
        serde_jcs::to_vec(&sparse_with_extension).expect("canonical extended manifest");
    assert!(matches!(
        open_members(&sparse_with_extension, &sparse_path)
            .expect_err("supported sparse schema remains closed"),
        IndexError::Corrupt("manifest JSON")
    ));
}

#[test]
fn fixed_only_accessors_reject_sparse_bundles() {
    let temp = Temp::new("fixed-only");
    let genes = fixture();
    let fixed_path = temp.0.join("fixed.pgi");
    let sparse_path = temp.0.join("sparse.pgi");
    write_fixed(&fixed_path, &genes);
    write_sparse(&temp.0.join("sparse.scratch"), &sparse_path, &genes);
    let (fixed, _) = open_bundle(FIXED_FORMAT, FIXED_MEDIA_TYPE, &fixed_path);
    let (sparse, _) = open_bundle(SPARSE_INDEX_FORMAT, SPARSE_MEDIA_TYPE, &sparse_path);

    fixed.index().expect("fixed reader accessor");
    fixed
        .lookup_batch_measured(&[(snv(64, DnaBase::T, DnaBase::A), Some(gene(1)))])
        .expect("fixed measured lookup");

    assert!(matches!(
        sparse.index().expect_err("fixed reader accessor"),
        IndexError::Incompatible("fixed-v1 operation requires a fixed-v1 bundle")
    ));
    assert!(matches!(
        sparse
            .lookup_batch_measured(&[(snv(64, DnaBase::T, DnaBase::A), Some(gene(1)))])
            .expect_err("fixed measured lookup"),
        IndexError::Incompatible("fixed-v1 operation requires a fixed-v1 bundle")
    ));
}

#[test]
fn exhaustive_bundle_traversal_dispatches_by_format() {
    let temp = Temp::new("exhaustive-dispatch");
    let genes = fixture();
    let expected = genes.iter().flatten().copied().collect::<Vec<_>>();
    let fixed_path = temp.0.join("fixed.pgi");
    let sparse_path = temp.0.join("sparse.pgi");
    write_fixed(&fixed_path, &genes);
    write_sparse(&temp.0.join("sparse.scratch"), &sparse_path, &genes);
    let (fixed, _) = open_bundle(FIXED_FORMAT, FIXED_MEDIA_TYPE, &fixed_path);
    let (sparse, _) = open_bundle(SPARSE_INDEX_FORMAT, SPARSE_MEDIA_TYPE, &sparse_path);

    let mut fixed_loci = Vec::new();
    let fixed_summary = fixed
        .visit_all_bounded(u64::MAX, u64::MAX, |locus| {
            fixed_loci.push(locus);
            Ok::<_, ()>(())
        })
        .expect("fixed traversal");
    let mut sparse_loci = Vec::new();
    let sparse_summary = sparse
        .visit_all_bounded(0, 0, |locus| {
            sparse_loci.push(locus);
            Ok::<_, ()>(())
        })
        .expect("streaming sparse traversal");
    assert_eq!(fixed_loci, expected);
    assert_eq!(sparse_loci, expected);
    assert_eq!(fixed_summary, sparse_summary);

    assert!(matches!(
        fixed.visit_all_bounded(0, 0, |_| Ok::<_, ()>(())),
        Err(pangopup_index::VisitAllError::Index(error))
            if error.to_string().contains("allocation limit")
    ));
}

#[test]
fn sparse_touched_record_corruption_reaches_the_provider_error_boundary() {
    let temp = Temp::new("touched-corruption");
    let genes = fixture();
    let sparse_path = temp.0.join("sparse.pgi");
    write_sparse(&temp.0.join("sparse.scratch"), &sparse_path, &genes);

    let mut bytes = fs::read(&sparse_path).expect("read sparse payload");
    let block = u64_at(&bytes, 56) as usize;
    let raw = u64_at(&bytes, 72) as usize + u64_at(&bytes, block + 16) as usize;
    let count = u32_at(&bytes, block + 12) as usize;
    let active = u32_at(&bytes, block + 28) as usize;
    let pairs = u32_at(&bytes, block + 32) as usize;
    let values = raw
        + 16
        + (count * 2).div_ceil(8)
        + count.div_ceil(8)
        + count.div_ceil(64) * 8
        + (active * 6).div_ceil(8);
    bytes[values + (pairs - 1) * 2 + 1] |= 0xc0;
    fs::write(&sparse_path, bytes).expect("write sparse corruption");

    let (sparse, _) = open_bundle(SPARSE_INDEX_FORMAT, SPARSE_MEDIA_TYPE, &sparse_path);
    assert!(
        sparse
            .lookup(snv(64, DnaBase::T, DnaBase::A), Some(gene(1)))
            .is_err()
    );
}

#[cfg(feature = "test-read-audit")]
#[test]
fn sparse_bundle_open_does_not_read_ordinary_payload() {
    use pangopup_index::sparse_reader::{
        test_reset_sparse_payload_read_bytes, test_sparse_payload_read_bytes,
    };

    let temp = Temp::new("bounded-open");
    let genes = fixture();
    let sparse_path = temp.0.join("sparse.pgi");
    write_sparse(&temp.0.join("sparse.scratch"), &sparse_path, &genes);
    test_reset_sparse_payload_read_bytes();
    let (sparse, _) = open_bundle(SPARSE_INDEX_FORMAT, SPARSE_MEDIA_TYPE, &sparse_path);
    assert_eq!(test_sparse_payload_read_bytes(), 0);
    sparse
        .lookup(snv(64, DnaBase::T, DnaBase::A), None)
        .expect("positive-control sparse lookup");
    assert!(test_sparse_payload_read_bytes() > 0);
}

fn fixture() -> Vec<Vec<InputLocus>> {
    vec![
        vec![
            ordinary(1, 63, DnaBase::G, score(0)),
            ordinary(1, 64, DnaBase::T, score(10)),
        ],
        vec![ordinary(2, 64, DnaBase::T, score(30))],
        vec![exception(3, 64, DnaBase::T)],
    ]
}

fn ordinary(
    gene_number: u64,
    coordinate: u32,
    reference: DnaBase,
    score: PangolinScore,
) -> InputLocus {
    let alternatives = DnaBase::ALL
        .into_iter()
        .filter(|base| *base != reference)
        .map(|alternate| InputAlternative { alternate, score })
        .collect::<Vec<_>>()
        .try_into()
        .expect("three alternatives");
    InputLocus::Ordinary(OrdinaryInputLocus {
        gene: gene(gene_number),
        contig: "chr1".parse().expect("contig"),
        position: GenomicPosition::new(coordinate).expect("position"),
        reference,
        alternatives,
    })
}

fn exception(gene_number: u64, coordinate: u32, omitted: DnaBase) -> InputLocus {
    let alternatives = DnaBase::ALL
        .into_iter()
        .filter(|base| *base != omitted)
        .map(|alternate| InputAlternative {
            alternate,
            score: score(20),
        })
        .collect::<Vec<_>>()
        .try_into()
        .expect("three alternatives");
    InputLocus::Ambiguous(AmbiguousInputLocus {
        gene: gene(gene_number),
        contig: "chr1".parse().expect("contig"),
        position: GenomicPosition::new(coordinate).expect("position"),
        alternatives,
        omitted,
    })
}

fn score(seed: u16) -> PangolinScore {
    PangolinScore::new(
        ScoreMagnitude::new(seed).expect("gain"),
        RelativePosition::new(-10).expect("gain position"),
        ScoreMagnitude::new(seed + 1).expect("loss"),
        RelativePosition::new(10).expect("loss position"),
    )
}

fn gene(number: u64) -> EnsemblGeneId {
    EnsemblGeneId::from_numeric(number).expect("gene")
}

fn snv(coordinate: u32, reference: DnaBase, alternate: DnaBase) -> Grch38Snv {
    Grch38Snv::new(
        "chr1".parse().expect("contig"),
        GenomicPosition::new(coordinate).expect("position"),
        reference,
        alternate,
    )
    .expect("SNV")
}

fn write_fixed(path: &Path, genes: &[Vec<InputLocus>]) {
    let input = genes.iter().flatten().copied().collect::<Vec<_>>();
    write_index(path, &input).expect("write fixed index");
}

fn write_sparse(scratch: &Path, output: &Path, genes: &[Vec<InputLocus>]) {
    let mut writer = SparseIndexWriter::create(scratch).expect("create sparse writer");
    for gene in genes {
        writer.push_gene(gene).expect("push sparse gene");
    }
    writer.finish(output).expect("finish sparse index");
}

fn open_bundle(format: &str, media_type: &str, payload: &Path) -> (BundleOpen, Vec<u8>) {
    let manifest = manifest_bytes(format, media_type, payload);
    let opened = open_members(&manifest, payload).expect("open bundle members");
    (opened, manifest)
}

fn open_members(manifest: &[u8], payload: &Path) -> Result<BundleOpen, IndexError> {
    let notice = File::open(fixture_bundle().join("NOTICE")).expect("fixture notice");
    let scores = File::open(payload).expect("score payload");
    BundleOpen::open_members(manifest, &notice, &scores)
}

fn manifest_bytes(format: &str, media_type: &str, payload: &Path) -> Vec<u8> {
    let fixture = fixture_bundle();
    let mut manifest: BundleManifest =
        serde_json::from_slice(&fs::read(fixture.join("manifest.json")).expect("fixture manifest"))
            .expect("decode fixture manifest");
    manifest.index_format = format.to_owned();
    manifest.members[1].media_type = media_type.to_owned();
    manifest.members[1].size = fs::metadata(payload).expect("payload metadata").len();
    canonical_manifest_bytes(&manifest).expect("canonical manifest")
}

fn fixture_bundle() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/snv-regression/bundle")
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32"))
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64"))
}
