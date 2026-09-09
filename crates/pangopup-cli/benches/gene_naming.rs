//! What gene naming costs one command-line lookup.
//!
//! Two budgets, both hard failures. The first is the cost of resolving every
//! name a single-variant lookup returns, taken once from a cold process. The
//! second is the difference between rendering those records with names and
//! rendering the same records without them.
//!
//! Run it with `cargo bench --package pangopup-cli --bench gene_naming`. It
//! needs no installed asset and no network.

use pangopup_index::gene_names::shipped;
use std::{error::Error, hint::black_box, time::{Duration, Instant}};

/// The budget for one lookup's naming work. The ticket states one millisecond
/// and it is a ceiling to clear, not a target to beat.
const BUDGET: Duration = Duration::from_millis(1);

/// The accessions one single-variant lookup returns. `GRCh38:chr12:6801301:G:A`
/// returns one record. A busy locus returns several, so the budget is measured
/// against a locus that returns more records than the published dataset holds
/// for almost any position.
const LOOKUP_ACCESSIONS: [&str; 8] = [
    "ENSG00000010610",
    "ENSG00000141499",
    "ENSG00000141510",
    "ENSG00000157764",
    "ENSG00000169129",
    "ENSG00000175727",
    "ENSG00000185974",
    "ENSG00000233887",
];

const ITERATIONS: u32 = 1_000;

fn main() -> Result<(), Box<dyn Error>> {
    // Cold. Nothing has touched the index in this process, so this measurement
    // pays every first-touch page fault the shipped index costs.
    let started = Instant::now();
    let mut resolved = 0_usize;
    for accession in LOOKUP_ACCESSIONS {
        if let Some(names) = shipped().names(accession) {
            black_box(&names);
            resolved += 1;
        }
    }
    let cold = started.elapsed();
    println!("cold_resolution_micros={}", cold.as_micros());
    println!("cold_resolved_names={resolved}");
    assert!(
        resolved >= 7,
        "the shipped index must name the accessions this budget is measured over"
    );
    assert!(
        cold <= BUDGET,
        "resolving every gene name a single-variant lookup returns took {cold:?}, over the {BUDGET:?} budget"
    );

    // Warm. The difference between rendering a lookup's records with names and
    // rendering the same records without them is the cost naming adds to every
    // invocation.
    let with_names = median(ITERATIONS, || {
        for accession in LOOKUP_ACCESSIONS {
            let names = shipped().names(accession);
            black_box(serde_json::to_string(&names).expect("naming JSON"));
        }
    });
    let without_names = median(ITERATIONS, || {
        for accession in LOOKUP_ACCESSIONS {
            black_box(accession);
        }
    });
    let added = with_names.saturating_sub(without_names);
    println!("with_names_micros={}", with_names.as_micros());
    println!("without_names_micros={}", without_names.as_micros());
    println!("added_micros={}", added.as_micros());
    assert!(
        added <= BUDGET,
        "naming added {added:?} to one lookup, over the {BUDGET:?} budget"
    );
    Ok(())
}

fn median(iterations: u32, mut operation: impl FnMut()) -> Duration {
    let mut samples = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        let started = Instant::now();
        operation();
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    samples[samples.len() / 2]
}
