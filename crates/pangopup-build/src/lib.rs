//! Deterministic Pangopup artifact builders and source inspectors.

mod command_error;
pub mod compatibility;
pub mod executable_release;
pub mod model;
pub mod naming;
mod production;
pub mod reference;
mod reference_builder;
mod reference_certification;
pub mod runtime_profile;
pub mod runtime_release;
#[cfg(feature = "runtime-v2-qualification")]
pub mod runtime_v2_qualification;
mod snv;
mod source_fingerprint;
mod sparse_candidate;
pub mod sparse_latency;
mod sparse_release;

pub use command_error::CommandError;
pub use production::{BuildOutcome, VerifyOutcome, build_bundle, verify_bundle};
pub use snv::*;
pub use sparse_candidate::{
    ADR_0027_SIZE_GATE_BYTES, MAX_GENE_CAPACITY_BYTES, MAX_GENE_LOCI, SparseCandidateArguments,
    SparseCandidateOutcome, SparseCandidateReport, build_sparse_candidate,
};
pub use sparse_release::{
    PRODUCTION_V1_AUTHORITY_BUNDLE_ID, SparseBundleArguments, SparseBundleOutcome,
    SparseReleaseOutcome, assemble_sparse_bundle, prepare_sparse_release,
};
