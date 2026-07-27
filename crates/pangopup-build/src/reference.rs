//! Authenticated production reference build and private certification.
//!
//! Public paths remain stable while byte production and post-build checking
//! are deliberately separate provenance domains.

pub use crate::reference_builder::{
    ReferenceBuildMember, ReferenceBuildOutcome, build_reference_bundle,
};
pub use crate::reference_certification::{
    QUALIFICATION_MAX_BUILDER_HEAP_BYTES, QUALIFICATION_MAX_MEMBER_BYTES,
    QUALIFICATION_MAX_OPEN_HEAP_BYTES, QUALIFICATION_MAX_P50_NS, QUALIFICATION_MAX_P95_NS,
    ReferenceCertification, ReferenceInspectOutcome, ReferenceQualificationFailureClass,
    ReferenceQualificationMeasurements, ReferenceQualificationRejection, ReferenceWindowOutcome,
    ReferenceWindowProvenance, certify_reference_bundle, evaluate_reference_qualification,
    inspect_reference_bundle, production_context_dense_pages, reference_window,
};
