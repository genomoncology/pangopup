use pangopup_build::reference::build_reference_bundle;
use pangopup_core::{
    DnaBase, GenomicPosition, Grch38Contig, Grch38Snv, ReferenceProvider, ScoreProvider,
};
use pangopup_index::{BundleManifest, BundleOpen, reference::ReferenceBundleOpen};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

const INCUMBENT_SOURCE: &str =
    "sha256:fa5d9fc3c3482aeca671e90e75752738019b911c3cba1549bd847856bf3986af";

struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "pangopup-builder-provenance-{}-{label}",
            std::process::id(),
        ));
        if path.exists() {
            fs::remove_dir_all(&path).expect("remove stale provenance scratch");
        }
        fs::create_dir(&path).expect("create provenance scratch");
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove provenance scratch");
    }
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures() -> PathBuf {
    repository().join("tests/fixtures")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn migration() -> Value {
    serde_json::from_slice(
        &fs::read(fixtures().join("builder-provenance-v1/migration.json"))
            .expect("migration evidence"),
    )
    .expect("migration JSON")
}

fn copy_bundle_members(source: &Path, destination: &Path, members: &[&str]) {
    fs::create_dir(destination).expect("create legacy bundle");
    for member in members {
        fs::copy(source.join(member), destination.join(member)).expect("copy unchanged member");
    }
}

#[test]
fn source_fingerprint_migration_is_limited_to_snv_bundle_provenance() {
    let migration = migration();
    let root = fixtures().join("snv-regression");
    let legacy_manifest =
        fs::read(fixtures().join("builder-provenance-v1/snv-legacy-manifest.json"))
            .expect("legacy SNV manifest");
    let migrated_manifest = fs::read(root.join("bundle/manifest.json")).expect("SNV manifest");
    assert_eq!(
        sha256(&legacy_manifest),
        migration["snv"]["legacy_manifest"]["sha256"]
            .as_str()
            .expect("legacy manifest hash")
    );
    assert_eq!(
        sha256(&migrated_manifest),
        migration["snv"]["migrated_manifest"]["sha256"]
            .as_str()
            .expect("migrated manifest hash")
    );

    let mut rebound: BundleManifest =
        serde_json::from_slice(&migrated_manifest).expect("migrated SNV manifest JSON");
    assert_ne!(rebound.builder.source_sha256, INCUMBENT_SOURCE);
    rebound.builder.source_sha256 = INCUMBENT_SOURCE.to_owned();
    assert_eq!(
        serde_jcs::to_vec(&rebound).expect("canonical rebound SNV manifest"),
        legacy_manifest,
        "the manifest migration must change only builder.source_sha256"
    );

    let invariant = migration["snv"]["invariant_files"]
        .as_array()
        .expect("invariant files");
    assert_eq!(invariant.len(), 10);
    for file in invariant {
        let relative = file["path"].as_str().expect("invariant path");
        assert_eq!(
            sha256(&fs::read(root.join(relative)).expect("invariant bytes")),
            file["sha256"].as_str().expect("invariant hash"),
            "{relative}"
        );
    }

    let old_bundle_id = format!("sha256:{}", sha256(&legacy_manifest));
    let new_bundle_id = format!("sha256:{}", sha256(&migrated_manifest));
    for file in migration["snv"]["expected_jsonl"]
        .as_array()
        .expect("expected JSONL files")
    {
        let relative = file["path"].as_str().expect("expected path");
        let bytes = fs::read(root.join(relative)).expect("expected JSONL");
        assert_eq!(
            sha256(&bytes),
            file["migrated_sha256"].as_str().expect("migrated hash"),
            "{relative}"
        );
        let text = std::str::from_utf8(&bytes).expect("expected JSONL UTF-8");
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(
            lines.len() as u64,
            file["lines"].as_u64().expect("expected line count"),
            "{relative}"
        );
        assert!(
            lines.iter().all(|line| {
                serde_json::from_str::<Value>(line)
                    .expect("expected JSON object")["provenance"]["bundle_id"]
                    .as_str()
                    == Some(new_bundle_id.as_str())
            }),
            "{relative} migrated bundle IDs"
        );
        let incumbent = text.replace(&new_bundle_id, &old_bundle_id);
        assert_eq!(
            incumbent.matches(&old_bundle_id).count(),
            lines.len(),
            "{relative} must contain exactly one bundle ID per row"
        );
        assert_eq!(
            sha256(incumbent.as_bytes()),
            file["incumbent_sha256"].as_str().expect("incumbent hash"),
            "{relative} changes must be limited to provenance.bundle_id"
        );
    }
}

#[test]
fn source_fingerprint_legacy_snv_manifest_opens_unchanged_members() {
    let temp = Temp::new("snv");
    let current = fixtures().join("snv-regression/bundle");
    let legacy = temp.0.join("snv-legacy");
    copy_bundle_members(&current, &legacy, &["NOTICE", "scores.pgi"]);
    fs::copy(
        fixtures().join("builder-provenance-v1/snv-legacy-manifest.json"),
        legacy.join("manifest.json"),
    )
    .expect("copy legacy SNV manifest");

    let opened = BundleOpen::open(&legacy).expect("open pre-migration SNV manifest");
    let snv = Grch38Snv::new(
        Grch38Contig::autosome(12).expect("chr12"),
        GenomicPosition::new(6_801_301).expect("position"),
        DnaBase::G,
        DnaBase::A,
    )
    .expect("SNV");
    let result = opened.lookup(snv, None).expect("legacy lookup");
    assert_eq!(result.records().len(), 1);
    assert_eq!(result.records()[0].gene().to_string(), "ENSG00000010610");
    assert_eq!(
        result
            .provenance()
            .precomputed()
            .expect("precomputed provenance")
            .bundle_id(),
        "sha256:f7d93978715603eeebb72c7bb1af744e0d3bb5f976c94c3daaeae2c0e6d58fbc"
    );
}

#[test]
fn source_fingerprint_reference_members_and_legacy_reader_are_invariant() {
    let temp = Temp::new("reference");
    let generated = temp.0.join("reference-current");
    let source = fixtures().join("reference-production-mini/source.fa.gz");
    let report = fixtures().join("reference-production-mini/assembly_report.txt");
    build_reference_bundle("pangopup-reference-mini-v1", &source, &report, &generated)
        .expect("build migrated reference");

    let migration = migration();
    let legacy_manifest =
        fs::read(fixtures().join("builder-provenance-v1/reference-legacy-manifest.json"))
            .expect("legacy reference manifest");
    let migrated_manifest =
        fs::read(generated.join("manifest.json")).expect("migrated reference manifest");
    assert_eq!(
        sha256(&legacy_manifest),
        migration["reference"]["legacy_manifest"]["sha256"]
            .as_str()
            .expect("legacy reference manifest hash")
    );
    assert_eq!(
        sha256(&migrated_manifest),
        migration["reference"]["migrated_manifest"]["sha256"]
            .as_str()
            .expect("migrated reference manifest hash")
    );
    assert_eq!(
        sha256(&fs::read(generated.join("NOTICE")).expect("reference notice")),
        migration["reference"]["notice"]["sha256"]
            .as_str()
            .expect("reference notice hash")
    );
    assert_eq!(
        sha256(&fs::read(generated.join("reference.pgr")).expect("reference member")),
        migration["reference"]["member"]["sha256"]
            .as_str()
            .expect("reference member hash")
    );

    let mut rebound: Value =
        serde_json::from_slice(&migrated_manifest).expect("reference manifest JSON");
    let migrated_source = rebound["builder"]["source_sha256"]
        .as_str()
        .expect("reference source fingerprint")
        .to_owned();
    assert_ne!(migrated_source, INCUMBENT_SOURCE);
    rebound["builder"]["source_sha256"] = Value::String(INCUMBENT_SOURCE.to_owned());
    assert_eq!(
        serde_jcs::to_vec(&rebound).expect("canonical rebound reference manifest"),
        legacy_manifest,
        "the manifest migration must change only builder.source_sha256"
    );

    let snv_source = serde_json::from_slice::<Value>(
        &fs::read(fixtures().join("snv-regression/bundle/manifest.json"))
            .expect("current SNV manifest"),
    )
    .expect("current SNV manifest JSON")["builder"]["source_sha256"]
        .as_str()
        .expect("SNV source fingerprint")
        .to_owned();
    assert_ne!(snv_source, migrated_source);

    let legacy = temp.0.join("reference-legacy");
    copy_bundle_members(&generated, &legacy, &["NOTICE", "reference.pgr"]);
    fs::copy(
        fixtures().join("builder-provenance-v1/reference-legacy-manifest.json"),
        legacy.join("manifest.json"),
    )
    .expect("copy legacy reference manifest");
    let opened = ReferenceBundleOpen::open(&legacy).expect("open pre-migration reference manifest");
    let mut bases = [0_u8; 15];
    opened
        .copy_window(
            Grch38Contig::autosome(1).expect("chr1"),
            GenomicPosition::new(1).expect("one"),
            &mut bases,
        )
        .expect("legacy reference window");
    assert_eq!(&bases, b"ACGTRYSWKMBDHVN");
}

#[test]
fn source_fingerprint_production_identity_constants_are_unchanged() {
    let release = include_str!("../../pangopup-assets/src/release.rs");
    for identity in [
        "10fd5d7715a611f9b7f20040887391502535ac7860bc6a1eda2bfdda79682b64",
        "c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3",
        "6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27",
    ] {
        assert!(release.contains(identity), "{identity}");
    }
}
