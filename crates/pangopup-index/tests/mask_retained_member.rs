use pangopup_core::{GenomicPosition, Grch38Contig};
use pangopup_index::mask::{MaskDomainsOpen, MaskProvider, MaskQueryBuffer, MaskQueryGene};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    str::FromStr,
};

const EXPECTED_BYTES: u64 = 6_703_320;
const EXPECTED_SHA256: &str = "714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702";

#[derive(Deserialize)]
struct WorkloadQuery {
    contig: String,
    position: u32,
    expected_sha256: String,
}

#[derive(Serialize)]
struct QueryResult {
    plus: Vec<QueryGene>,
    minus: Vec<QueryGene>,
}

#[derive(Serialize)]
struct QueryGene {
    id: String,
    boundaries: Vec<u32>,
}

fn sha256_hex(mut reader: impl Read) -> String {
    let mut hasher = Sha256::new();
    let mut bytes = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut bytes).expect("read bytes for SHA-256");
        if count == 0 {
            break;
        }
        hasher.update(&bytes[..count]);
    }
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn render(buffer: &MaskQueryBuffer, genes: &[MaskQueryGene]) -> Vec<QueryGene> {
    genes
        .iter()
        .map(|gene| QueryGene {
            id: gene.identity().to_string(),
            boundaries: buffer
                .boundaries(gene)
                .iter()
                .map(|boundary| boundary.get())
                .collect(),
        })
        .collect()
}

fn workload_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../planning/artifacts/012-performance-manifest.jsonl")
}

#[test]
#[ignore = "requires the exact retained Ticket 012 domains member"]
fn retained_domains_member_matches_oracle() {
    let member = env::var_os("PANGOPUP_MASK_MEMBER")
        .map(PathBuf::from)
        .expect("PANGOPUP_MASK_MEMBER must name the exact retained domains member");
    let metadata = member
        .metadata()
        .unwrap_or_else(|error| panic!("read {} metadata: {error}", member.display()));
    assert_eq!(metadata.len(), EXPECTED_BYTES, "retained member size");
    let digest = sha256_hex(
        File::open(&member)
            .unwrap_or_else(|error| panic!("open {} for hashing: {error}", member.display())),
    );
    assert_eq!(digest, EXPECTED_SHA256, "retained member SHA-256");

    let provider = MaskDomainsOpen::open(&member).expect("open retained domains member");
    let workload = BufReader::new(File::open(workload_path()).expect("open workload"));
    let mut output = MaskQueryBuffer::with_capacity(32, 1_024);
    let mut queries = 0_usize;
    for line in workload.lines() {
        let line = line.expect("read workload line");
        let Ok(query) = serde_json::from_str::<WorkloadQuery>(&line) else {
            continue;
        };
        provider
            .query(
                Grch38Contig::from_str(&query.contig).expect("workload contig"),
                GenomicPosition::new(query.position).expect("workload position"),
                None,
                &mut output,
            )
            .expect("retained-member query");
        let result = QueryResult {
            plus: render(&output, output.plus()),
            minus: render(&output, output.minus()),
        };
        let mut encoded = serde_jcs::to_vec(&result).expect("encode canonical result");
        encoded.push(b'\n');
        assert_eq!(
            sha256_hex(encoded.as_slice()),
            query.expected_sha256,
            "{}:{}",
            query.contig,
            query.position
        );
        queries += 1;
    }
    assert_eq!(queries, 1_000, "retained workload cardinality");
}
