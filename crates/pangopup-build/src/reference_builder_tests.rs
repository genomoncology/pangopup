use super::*;
use pangopup_core::Grch38Contig;
use pangopup_index::reference::{MINI_PROFILE, ReferenceContigPlan, ReferenceMemberWriter};
use std::{io::Cursor, sync::atomic::AtomicUsize};

static SERIAL: AtomicUsize = AtomicUsize::new(0);
const MINI_FASTA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/reference-production-mini/source.fa"
));
const MINI_REPORT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/reference-production-mini/assembly_report.txt"
));

struct Temp(PathBuf);
impl Temp {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "pangopup-reference-unit-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create unit temp");
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn parse_mini_fasta(bytes: &[u8]) -> Result<Vec<String>, CommandError> {
    let profile = profile(MINI_PROFILE).expect("mini profile");
    let temp = Temp::new("fasta");
    let plans = std::array::from_fn(|index| ReferenceContigPlan {
        contig: Grch38Contig::from_code((index + 1) as u8).expect("contig"),
        bases: profile.lengths[index],
    });
    let mut writer =
        ReferenceMemberWriter::create(&temp.0.join("reference.pgr"), &plans).expect("writer");
    let mut reader = BufReader::new(Cursor::new(bytes));
    let extras = parse_fasta(&mut reader, &profile, &mut writer)?;
    writer.finish().map_err(index_build_error)?;
    Ok(extras)
}

#[test]
fn report_parser_rejects_malformed_duplicate_and_missing_required_rows() {
    let profile = profile(MINI_PROFILE).expect("mini profile");
    parse_assembly_report(&profile, MINI_REPORT.as_bytes()).expect("valid report");

    let mut duplicate = MINI_REPORT.to_owned();
    duplicate.push_str(MINI_REPORT.lines().nth(1).expect("required row"));
    duplicate.push('\n');
    assert!(parse_assembly_report(&profile, duplicate.as_bytes()).is_err());

    assert!(parse_assembly_report(&profile, b"malformed\trow\n").is_err());
    let missing = MINI_REPORT
        .lines()
        .filter(|line| !line.contains("NC_000001.11"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(parse_assembly_report(&profile, missing.as_bytes()).is_err());
}

#[test]
fn fasta_parser_accepts_reordering_and_rejects_missing_duplicate_and_resource_excess() {
    assert_eq!(
        parse_mini_fasta(MINI_FASTA.as_bytes()).expect("reordered valid FASTA"),
        vec!["NC_000025.0"]
    );

    let duplicate = format!("{MINI_FASTA}>NC_000001.11 duplicate\n{}\n", "A".repeat(30));
    assert!(parse_mini_fasta(duplicate.as_bytes()).is_err());

    let missing = MINI_FASTA
        .split_inclusive('\n')
        .skip_while(|line| !line.starts_with(">NC_000001.11"))
        .skip(2)
        .collect::<String>();
    assert!(parse_mini_fasta(missing.as_bytes()).is_err());

    let mut excess = MINI_FASTA.to_owned();
    excess.push_str(">E\nA\n");
    assert_eq!(
        parse_mini_fasta(excess.as_bytes())
            .expect_err("record ceiling")
            .message,
        "reference FASTA record count exceeds limit"
    );

    let long_accession = format!(">{}\nA\n", "E".repeat(65));
    assert_eq!(
        parse_mini_fasta(long_accession.as_bytes())
            .expect_err("accession ceiling")
            .message,
        "reference FASTA accession exceeds limit"
    );

    let mut decoded = DecodedLimit::new(Cursor::new(vec![b'A'; 33]), 32);
    let mut output = Vec::new();
    assert!(decoded.read_to_end(&mut output).is_err());
    assert_eq!(output.len(), 32);
}

#[test]
fn publication_parent_sync_failure_is_removed_and_durably_resynced() {
    let temp = Temp::new("rollback");
    let stage = temp.0.join("stage");
    let output = temp.0.join("output");
    fs::create_dir(&stage).expect("stage");
    fs::write(stage.join("member"), b"member").expect("member");
    let mut guard = StageGuard {
        path: stage.clone(),
        armed: true,
    };
    let calls = std::cell::Cell::new(0_u8);
    let result = publish_stage_with(&stage, &temp.0, &output, &mut guard, |_| {
        let call = calls.get();
        calls.set(call + 1);
        if call == 0 {
            Err(CommandError::new("IO", "injected first parent sync"))
        } else {
            Ok(())
        }
    });
    assert_eq!(result.expect_err("injected failure").code, "IO");
    assert_eq!(calls.get(), 2);
    assert!(!stage.exists());
    assert!(!output.exists());
}
