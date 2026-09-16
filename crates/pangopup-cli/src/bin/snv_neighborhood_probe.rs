//! One-off, read-only SNV landscape measurement for literal anchored indels.
//! Run with: snv_neighborhood_probe SNV_BUNDLE REFERENCE_BUNDLE MASK < indels.jsonl
//! Each input line is {"contig":"chr22","position":1,"reference":"A","alternate":"AT"}.
//! An optional stable `gene` restricts output to that containing mask gene.

use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, Grch38Snv, ReferenceProvider,
    ScoreProvider,
};
use pangopup_index::{
    BundleOpen,
    mask::{MaskDomainsOpen, MaskProvider, MaskQueryBuffer},
    reference::ReferenceBundleOpen,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    env,
    io::{self, BufRead},
    path::Path,
};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Input {
    contig: String,
    position: u32,
    reference: String,
    alternate: String,
    gene: Option<String>,
}

#[derive(Serialize)]
struct Output {
    contig: String,
    position: u32,
    reference: String,
    alternate: String,
    gene: String,
    kind: &'static str,
    changed_length: usize,
    changed_span: [u32; 2],
    inserted_sequence: String,
    reference_after_anchor: String,
    snv_bundle_id: String,
    reference_bundle_id: String,
    anchor: Window,
    plus_minus_10: Window,
}

#[derive(Serialize)]
struct Window {
    span: [u32; 2],
    expected_records: usize,
    present_records: usize,
    missing_records: usize,
    ambiguous_records: usize,
    other_gene_records: usize,
    other_genes: Vec<String>,
    complete: bool,
    gain: Direction,
    loss: Direction,
}

#[derive(Default, Serialize)]
struct Direction {
    max_hundredths: Option<u8>,
    nonzero_records: usize,
    zero_fraction: Option<f64>,
    nearest_nonzero_distance: Option<u32>,
}

#[derive(Clone, Copy)]
enum Observation {
    Score { gain: u8, loss: u8 },
    Missing,
    Ambiguous,
    OtherGene,
}

fn changed_span(
    input: &Input,
    contig_length: u64,
) -> Result<([u32; 2], &'static str, usize, String), String> {
    if input.position == 0 || input.reference.is_empty() || input.alternate.is_empty() {
        return Err("invalid position or empty allele".into());
    }
    if !input
        .reference
        .bytes()
        .chain(input.alternate.bytes())
        .all(|b| matches!(b, b'A' | b'C' | b'G' | b'T'))
    {
        return Err("alleles must be uppercase A/C/G/T".into());
    }
    if input.reference.as_bytes()[0] != input.alternate.as_bytes()[0] {
        return Err("indel anchor differs".into());
    }
    let (kind, length, inserted) = if input.reference.len() == 1 && input.alternate.len() > 1 {
        (
            "insertion",
            input.alternate.len() - 1,
            input.alternate[1..].to_owned(),
        )
    } else if input.alternate.len() == 1 && input.reference.len() > 1 {
        ("deletion", input.reference.len() - 1, String::new())
    } else {
        return Err("only anchored literal insertions and deletions are supported".into());
    };
    let end = input
        .position
        .checked_add(input.reference.len() as u32 - 1)
        .ok_or("position overflow")?;
    if u64::from(end) > contig_length {
        return Err("reference allele extends past contig".into());
    }
    let changed_end = if kind == "insertion" {
        input.position
    } else {
        end
    };
    Ok(([input.position, changed_end], kind, length, inserted))
}

fn window_span(changed: [u32; 2], flank: u32, contig_length: u64) -> [u32; 2] {
    [
        changed[0].saturating_sub(flank).max(1),
        u64::from(changed[1])
            .saturating_add(u64::from(flank))
            .min(contig_length) as u32,
    ]
}

fn distance(position: u32, changed: [u32; 2]) -> u32 {
    if position < changed[0] {
        changed[0] - position
    } else {
        position.saturating_sub(changed[1])
    }
}

fn reference_matches(input: &Input, observed: &[u8]) -> bool {
    observed == input.reference.as_bytes()
}

fn after_anchor_with<F>(anchor: u32, contig_length: u64, mut copy: F) -> Result<String, String>
where
    F: FnMut(u32, &mut [u8]) -> Result<(), String>,
{
    let available = contig_length.saturating_sub(u64::from(anchor)).min(4) as usize;
    if available == 0 {
        return Ok(String::new());
    }
    let start = anchor.checked_add(1).ok_or("anchor position overflow")?;
    let mut bases = vec![0; available];
    copy(start, &mut bases)?;
    String::from_utf8(bases).map_err(|_| "reference returned non-ASCII sequence".into())
}

fn validate_input(
    input: &Input,
    reference: &ReferenceBundleOpen,
    contig: Grch38Contig,
) -> Result<(), (&'static str, String)> {
    changed_span(input, reference.contig_length(contig))
        .map_err(|message| ("invalid_literal", message))?;
    let position = GenomicPosition::new(input.position)
        .map_err(|error| ("invalid_literal", error.to_string()))?;
    let mut observed = vec![0; input.reference.len()];
    reference
        .copy_window(contig, position, &mut observed)
        .map_err(|error| ("reference_unavailable", error.to_string()))?;
    if !reference_matches(input, &observed) {
        return Err((
            "reference_mismatch",
            "reference allele does not match pinned reference".into(),
        ));
    }
    Ok(())
}

fn summarize<F>(span: [u32; 2], changed: [u32; 2], mut query: F) -> Result<Window, String>
where
    F: FnMut(u32, usize) -> Result<(Observation, Option<String>), String>,
{
    let mut result = Window {
        span,
        expected_records: 0,
        present_records: 0,
        missing_records: 0,
        ambiguous_records: 0,
        other_gene_records: 0,
        other_genes: Vec::new(),
        complete: false,
        gain: Direction::default(),
        loss: Direction::default(),
    };
    let mut genes = BTreeSet::new();
    for position in span[0]..=span[1] {
        for alternate in 0..3 {
            result.expected_records += 1;
            let (observation, other_gene) = query(position, alternate)?;
            match observation {
                Observation::Score { gain, loss } => {
                    result.present_records += 1;
                    for (value, direction) in [(gain, &mut result.gain), (loss, &mut result.loss)] {
                        direction.max_hundredths =
                            Some(direction.max_hundredths.map_or(value, |old| old.max(value)));
                        if value > 0 {
                            direction.nonzero_records += 1;
                            let d = distance(position, changed);
                            direction.nearest_nonzero_distance = Some(
                                direction
                                    .nearest_nonzero_distance
                                    .map_or(d, |old| old.min(d)),
                            );
                        }
                    }
                }
                Observation::Missing => result.missing_records += 1,
                Observation::Ambiguous => result.ambiguous_records += 1,
                Observation::OtherGene => result.other_gene_records += 1,
            }
            if let Some(gene) = other_gene {
                genes.insert(gene);
            }
        }
    }
    result.other_genes = genes.into_iter().collect();
    result.complete = result.present_records == result.expected_records;
    for direction in [&mut result.gain, &mut result.loss] {
        if result.present_records > 0 {
            direction.zero_fraction = Some(
                (result.present_records - direction.nonzero_records) as f64
                    / result.present_records as f64,
            );
        }
    }
    Ok(result)
}

fn probe(
    input: Input,
    gene: EnsemblGeneId,
    scores: &BundleOpen,
    reference: &ReferenceBundleOpen,
) -> Result<Output, String> {
    let contig: Grch38Contig = input
        .contig
        .parse()
        .map_err(|e: pangopup_core::ValueError| e.to_string())?;
    let length = reference.contig_length(contig);
    let (changed, kind, changed_length, inserted_sequence) = changed_span(&input, length)?;
    let position = GenomicPosition::new(input.position).map_err(|e| e.to_string())?;
    let mut submitted_ref = vec![0; input.reference.len()];
    reference
        .copy_window(contig, position, &mut submitted_ref)
        .map_err(|e| e.to_string())?;
    if !reference_matches(&input, &submitted_ref) {
        return Err("reference allele does not match pinned reference".into());
    }
    let outer = window_span(changed, 10, length);
    let start = GenomicPosition::new(outer[0]).map_err(|e| e.to_string())?;
    let mut bases = vec![0; (outer[1] - outer[0] + 1) as usize];
    reference
        .copy_window(contig, start, &mut bases)
        .map_err(|e| e.to_string())?;
    let reference_after_anchor = after_anchor_with(input.position, length, |next, destination| {
        let next = GenomicPosition::new(next).map_err(|error| error.to_string())?;
        reference
            .copy_window(contig, next, destination)
            .map_err(|error| error.to_string())
    })?;
    let lookup = |position: u32, alternate_index: usize| -> Result<_, String> {
        let reference_byte = bases[(position - outer[0]) as usize];
        let Ok(reference_base) =
            DnaBase::parse(std::str::from_utf8(&[reference_byte]).unwrap_or(""))
        else {
            return Ok((Observation::Ambiguous, None));
        };
        let alternate = DnaBase::ALL
            .into_iter()
            .filter(|base| *base != reference_base)
            .nth(alternate_index)
            .expect("three alternate bases");
        let snv = Grch38Snv::new(
            contig,
            GenomicPosition::new(position).expect("window positions are positive"),
            reference_base,
            alternate,
        )
        .expect("different bases");
        let result = scores
            .lookup(snv, None)
            .map_err(|error| format!("SNV index lookup failed at {contig}:{position}: {error}"))?;
        if result
            .source_reference_ambiguities()
            .iter()
            .any(|record| record.gene() == gene)
        {
            return Ok((Observation::Ambiguous, None));
        }
        if let Some(record) = result.records().iter().find(|record| record.gene() == gene) {
            return Ok((
                Observation::Score {
                    gain: record.score().gain().hundredths(),
                    loss: record.score().loss().hundredths(),
                },
                None,
            ));
        }
        let other = result
            .records()
            .first()
            .map(|record| record.gene().to_string());
        if other.is_some() {
            Ok((Observation::OtherGene, other))
        } else {
            Ok((Observation::Missing, None))
        }
    };
    Ok(Output {
        contig: contig.to_string(),
        position: input.position,
        reference: input.reference,
        alternate: input.alternate,
        gene: gene.to_string(),
        kind,
        changed_length,
        changed_span: changed,
        inserted_sequence,
        reference_after_anchor,
        snv_bundle_id: scores.bundle_id().to_owned(),
        reference_bundle_id: reference.provenance().bundle_id().to_owned(),
        anchor: summarize([input.position, input.position], changed, lookup)?,
        plus_minus_10: summarize(outer, changed, lookup)?,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        return Err(
            "usage: snv_neighborhood_probe SNV_BUNDLE REFERENCE_BUNDLE MASK < indels.jsonl".into(),
        );
    }
    let scores = BundleOpen::open(Path::new(&args[1]))?;
    let reference = ReferenceBundleOpen::open(Path::new(&args[2]))?;
    let mask = MaskDomainsOpen::open(Path::new(&args[3]))?;
    let mut buffer = MaskQueryBuffer::default();
    for (line_number, line) in io::stdin().lock().lines().enumerate() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let input: Input = serde_json::from_str(&line)?;
        let contig: Grch38Contig = input.contig.parse()?;
        if let Err((status, reason)) = validate_input(&input, &reference, contig) {
            println!(
                "{}",
                serde_json::json!({"status":status, "reason":reason, "input":input})
            );
            continue;
        }
        let position = GenomicPosition::new(input.position)?;
        let filter = input
            .gene
            .as_deref()
            .map(str::parse::<EnsemblGeneId>)
            .transpose()?;
        mask.query(contig, position, filter, &mut buffer)?;
        let genes: BTreeSet<_> = buffer
            .plus()
            .iter()
            .chain(buffer.minus())
            .map(|gene| gene.stable_identity())
            .collect();
        if genes.is_empty() {
            println!(
                "{}",
                serde_json::json!({"status":"no_mask_gene", "input":input})
            );
        }
        for gene in genes {
            let output = probe(input.clone(), gene, &scores, &reference)
                .map_err(|message| format!("line {}: {message}", line_number + 1))?;
            println!("{}", serde_json::to_string(&output)?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(position: u32, reference: &str, alternate: &str) -> Input {
        Input {
            contig: "chr22".into(),
            position,
            reference: reference.into(),
            alternate: alternate.into(),
            gene: Some("ENSG00000000001".into()),
        }
    }

    #[test]
    fn anchored_spans_and_clipped_endpoints() {
        let (span, kind, length, inserted) =
            changed_span(&input(1, "A", "AT"), 25).expect("valid insertion");
        assert_eq!(
            (span, kind, length, inserted.as_str()),
            ([1, 1], "insertion", 1, "T")
        );
        assert_eq!(window_span(span, 10, 25), [1, 11]);
        let (span, kind, length, _) =
            changed_span(&input(20, "ACGT", "A"), 25).expect("valid deletion");
        assert_eq!((span, kind, length), ([20, 23], "deletion", 3));
        assert_eq!(window_span(span, 10, 25), [10, 25]);
        assert_eq!([20, 20], [span[0], span[0]]);
        assert_eq!(distance(10, span), 10);
        assert_eq!(distance(25, span), 2);
    }

    #[test]
    fn missing_and_ambiguous_are_not_zero_and_other_genes_do_not_match() {
        let window = summarize([5, 6], [5, 5], |position, alternate| {
            Ok(match (position, alternate) {
                (5, 0) => (Observation::Score { gain: 0, loss: 0 }, None),
                (5, 1) => (Observation::Score { gain: 20, loss: 5 }, None),
                (5, 2) => (Observation::OtherGene, Some("ENSG00000000002".into())),
                (6, 0) => (Observation::Ambiguous, None),
                _ => (Observation::Missing, None),
            })
        })
        .expect("summary succeeds");
        assert_eq!(window.expected_records, 6);
        assert_eq!(
            (
                window.present_records,
                window.missing_records,
                window.ambiguous_records,
                window.other_gene_records
            ),
            (2, 2, 1, 1)
        );
        assert!(!window.complete);
        assert_eq!(window.gain.max_hundredths, Some(20));
        assert_eq!(window.loss.max_hundredths, Some(5));
        assert_eq!(window.gain.zero_fraction, Some(0.5));
        assert_eq!(window.gain.nearest_nonzero_distance, Some(0));
        assert_eq!(window.other_genes, ["ENSG00000000002"]);
    }

    #[test]
    fn lookup_failure_propagates_instead_of_becoming_missing() {
        let result = summarize([5, 5], [5, 5], |_, _| Err("corrupt index".into()));
        assert!(matches!(result, Err(message) if message == "corrupt index"));
    }

    #[test]
    fn no_gene_status_requires_a_valid_literal_and_matching_reference() {
        assert!(changed_span(&input(1, "A", "AT"), 10).is_ok());
        assert!(reference_matches(&input(1, "A", "AT"), b"A"));
        assert!(!reference_matches(&input(1, "A", "AT"), b"C"));
        assert!(changed_span(&input(1, "A", "CT"), 10).is_err());
        assert!(changed_span(&input(9, "ACG", "A"), 10).is_err());
    }

    #[test]
    fn next_four_reference_bases_clip_at_contig_end() {
        let result = after_anchor_with(7, 10, |start, destination| {
            assert_eq!(start, 8);
            destination.copy_from_slice(b"ACG");
            Ok(())
        })
        .expect("reference read succeeds");
        assert_eq!(result, "ACG");
        let result = after_anchor_with(10, 10, |_, _| panic!("must not read past contig"))
            .expect("empty sequence succeeds");
        assert_eq!(result, "");
    }

    #[test]
    fn next_four_reference_bases_propagate_read_failure() {
        let result = after_anchor_with(5, 10, |_, _| Err("reference read failed".into()));
        assert!(matches!(result, Err(message) if message == "reference read failed"));
    }
}
