use pangopup_core::{EnsemblGeneId, GenomicPosition, Grch38Contig};
use pangopup_index::mask::{
    MaskDomainsOpen, MaskError, MaskProvider, MaskQueryBuffer, MaskQueryGene,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

static SCRATCH_SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize)]
struct Fixture {
    queries: Vec<QueryFixture>,
}

#[derive(Deserialize)]
struct QueryFixture {
    contig: String,
    position: u32,
    plus: Vec<ExpectedGene>,
    minus: Vec<ExpectedGene>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct ExpectedGene {
    id: String,
    boundaries: Vec<u32>,
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let serial = SCRATCH_SERIAL.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pangopup-production-mask-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create scratch");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove scratch");
    }
}

fn fixture() -> Fixture {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/gencode-mask-mini/fixture.json");
    serde_json::from_slice(&fs::read(path).expect("read fixture")).expect("parse fixture")
}

fn fixture_member() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/gencode-mask-mini/domains.pgm")
}

fn scratch_member(scratch: &Scratch) -> PathBuf {
    let path = scratch.path().join("domains.pgm");
    fs::copy(fixture_member(), &path).expect("copy fixture member");
    path
}

fn render(buffer: &MaskQueryBuffer, genes: &[MaskQueryGene]) -> Vec<ExpectedGene> {
    genes
        .iter()
        .map(|gene| ExpectedGene {
            id: gene.identity().to_string(),
            boundaries: buffer
                .boundaries(gene)
                .iter()
                .map(|boundary| boundary.get())
                .collect(),
        })
        .collect()
}

fn query(
    provider: &dyn MaskProvider,
    contig: Grch38Contig,
    position: u32,
    gene: Option<EnsemblGeneId>,
    output: &mut MaskQueryBuffer,
) -> Result<(), MaskError> {
    provider.query(
        contig,
        GenomicPosition::new(position).expect("position"),
        gene,
        output,
    )
}

#[test]
fn production_domains_api_matches_the_independent_miniature_oracle() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<MaskDomainsOpen>();

    let fixture = fixture();
    let path = fixture_member();
    let bytes = fs::read(&path).expect("read domains fixture");
    assert_eq!(bytes.len(), 880);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "76d4513ba12fea21f509a3b61d01c90b2f503c24b139c2a50a4c08569994cc43"
    );
    let provider = MaskDomainsOpen::open(&path).expect("open domains");
    assert_eq!(
        provider.file_len(),
        fs::metadata(&path).expect("metadata").len()
    );

    let mut output = MaskQueryBuffer::with_capacity(8, 32);
    for expected in &fixture.queries {
        query(
            &provider,
            Grch38Contig::from_str(&expected.contig).expect("query contig"),
            expected.position,
            None,
            &mut output,
        )
        .expect("query");
        assert_eq!(render(&output, output.plus()), expected.plus);
        assert_eq!(render(&output, output.minus()), expected.minus);
    }

    query(
        &provider,
        Grch38Contig::autosome(3).expect("chr3"),
        1,
        None,
        &mut output,
    )
    .expect("miss");
    assert!(output.plus().is_empty() && output.minus().is_empty());
}

#[test]
fn qualification_authenticates_the_held_member_and_rejects_bad_inputs() {
    const BYTES: u64 = 880;
    const SHA256: &str = "76d4513ba12fea21f509a3b61d01c90b2f503c24b139c2a50a4c08569994cc43";

    let scratch = Scratch::new();
    let path = scratch_member(&scratch);
    let (provider, identity) =
        MaskDomainsOpen::open_qualification(&path, BYTES, SHA256).expect("qualified open");
    assert_eq!(provider.file_len(), BYTES);
    assert_eq!(identity.bytes(), BYTES);
    assert_eq!(identity.sha256(), SHA256);

    assert!(matches!(
        MaskDomainsOpen::open_qualification(
            &path,
            BYTES,
            "06d4513ba12fea21f509a3b61d01c90b2f503c24b139c2a50a4c08569994cc43"
        ),
        Err(MaskError::Authentication("member SHA-256"))
    ));

    let mutation = mutate(&path, "qualification-mutation", |bytes| bytes[200] ^= 1);
    assert!(matches!(
        MaskDomainsOpen::open_qualification(&mutation, BYTES, SHA256),
        Err(MaskError::Authentication("member SHA-256"))
    ));

    let link = scratch.path().join("linked.pgm");
    symlink(&path, &link).expect("create symlink");
    assert!(MaskDomainsOpen::open_qualification(&link, BYTES, SHA256).is_err());
}

#[test]
fn production_api_rejects_both_unselected_codecs() {
    let scratch = Scratch::new();
    let path = scratch_member(&scratch);
    for codec in [1_u8, 3_u8] {
        let mutation = mutate(&path, &format!("codec-{codec}"), |bytes| {
            bytes[10] = codec;
        });
        assert!(matches!(
            MaskDomainsOpen::open(&mutation),
            Err(MaskError::UnsupportedCodec)
        ));
    }
}

#[test]
fn stable_filter_is_contig_local_and_retains_exact_par_identity() {
    let path = fixture_member();
    let provider = MaskDomainsOpen::open(&path).expect("open domains");
    let stable = EnsemblGeneId::from_str("ENSG00000228572").expect("stable gene");
    let mut output = MaskQueryBuffer::default();

    query(&provider, Grch38Contig::X, 101, Some(stable), &mut output).expect("X query");
    assert_eq!(output.plus()[0].identity().to_string(), "ENSG00000228572.7");
    query(&provider, Grch38Contig::Y, 201, Some(stable), &mut output).expect("Y query");
    assert_eq!(
        output.plus()[0].identity().to_string(),
        "ENSG00000228572.7_PAR_Y"
    );

    let absent = EnsemblGeneId::from_str("ENSG00000099999").expect("absent stable gene");
    query(&provider, Grch38Contig::Y, 201, Some(absent), &mut output).expect("filtered miss");
    assert!(output.plus().is_empty() && output.minus().is_empty());
}

fn mutate(path: &Path, suffix: &str, change: impl FnOnce(&mut Vec<u8>)) -> PathBuf {
    let target = path.with_extension(suffix);
    let mut bytes = fs::read(path).expect("read fixture");
    change(&mut bytes);
    fs::write(&target, bytes).expect("write mutation");
    target
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64"))
}

type ByteMutation = (&'static str, fn(&mut Vec<u8>));

#[test]
fn open_is_bounded_but_header_directory_and_touched_payload_corruption_fail_closed() {
    let scratch = Scratch::new();
    let path = scratch_member(&scratch);

    for (suffix, change) in [
        (
            "bad-header",
            (|bytes: &mut Vec<u8>| bytes[0] ^= 1) as fn(&mut Vec<u8>),
        ),
        ("bad-directory", |bytes: &mut Vec<u8>| bytes[160] = 0),
        ("truncated", |bytes: &mut Vec<u8>| bytes.truncate(159)),
    ] {
        let mutation = mutate(&path, suffix, change);
        assert!(MaskDomainsOpen::open(&mutation).is_err(), "{suffix}");
    }

    let payload_mutations: [ByteMutation; 5] = [
        ("gene", |bytes| {
            let offset = u64_at(bytes, 32 + 24) as usize;
            bytes[offset + 4..offset + 8].copy_from_slice(&0_u32.to_le_bytes());
        }),
        ("boundary", |bytes| {
            let offset = u64_at(bytes, 32 + 2 * 24) as usize;
            bytes[offset..offset + 4].copy_from_slice(&0_u32.to_le_bytes());
        }),
        ("domain", |bytes| {
            let offset = u64_at(bytes, 32 + 3 * 24) as usize;
            bytes[offset + 8..offset + 12].copy_from_slice(&u32::MAX.to_le_bytes());
        }),
        ("posting", |bytes| {
            let offset = u64_at(bytes, 32 + 4 * 24) as usize;
            bytes[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        }),
        ("rank", |bytes| {
            let offset = u64_at(bytes, 32 + 24) as usize;
            let rank = bytes[offset + 40 + 12..offset + 40 + 16].to_vec();
            bytes[offset + 80 + 12..offset + 80 + 16].copy_from_slice(&rank);
        }),
    ];
    for (suffix, change) in payload_mutations {
        let mutation = mutate(&path, suffix, change);
        let provider = MaskDomainsOpen::open(&mutation).expect("cheap open");
        let mut output = MaskQueryBuffer::with_capacity(8, 32);
        let (contig, position) = if suffix == "rank" {
            (Grch38Contig::autosome(2).expect("chr2"), 13)
        } else {
            (Grch38Contig::autosome(1).expect("chr1"), 2)
        };
        assert!(
            query(&provider, contig, position, None, &mut output).is_err(),
            "{suffix}"
        );
        assert!(
            output.plus().is_empty() && output.minus().is_empty(),
            "{suffix} left partial output"
        );
    }
}

#[test]
fn domains_reject_foreign_postings_and_wrong_contig_genes_without_partial_output() {
    let scratch = Scratch::new();
    let path = scratch_member(&scratch);
    let original = MaskDomainsOpen::open(&path).expect("open original");
    let contig = Grch38Contig::autosome(1).expect("chr1");
    let mut output = MaskQueryBuffer::with_capacity(8, 32);

    let foreign = mutate(&path, "foreign-posting", |bytes| {
        let posting = u64_at(bytes, 32 + 4 * 24) as usize;
        bytes[posting..posting + 4].copy_from_slice(&1_u32.to_le_bytes());
    });
    query(&original, contig, 2, None, &mut output).expect("populate output");
    assert_eq!(output.plus().len(), 1);
    let foreign_provider = MaskDomainsOpen::open(&foreign).expect("cheap foreign open");
    assert!(matches!(
        query(&foreign_provider, contig, 2, None, &mut output),
        Err(MaskError::Invalid("posting gene range"))
    ));
    assert!(output.plus().is_empty() && output.minus().is_empty());

    let wrong_contig = mutate(&path, "wrong-contig-gene", |bytes| {
        let gene = u64_at(bytes, 32 + 24) as usize;
        bytes[gene] = 2;
    });
    query(&original, contig, 2, None, &mut output).expect("repopulate output");
    assert_eq!(output.plus().len(), 1);
    let wrong_contig_provider = MaskDomainsOpen::open(&wrong_contig).expect("cheap contig open");
    assert!(matches!(
        query(&wrong_contig_provider, contig, 2, None, &mut output),
        Err(MaskError::Invalid("posting gene contig or span"))
    ));
    assert!(output.plus().is_empty() && output.minus().is_empty());
}
