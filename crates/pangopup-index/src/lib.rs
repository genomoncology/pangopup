//! Private, explicitly decoded Pangopup runtime formats.

/// Benchmark-only GRCh38 reference candidates used to select Ticket 011's
/// production payload. Nothing in the runtime lookup path depends on these
/// experimental codecs.
#[doc(hidden)]
pub mod reference_candidates;

/// Benchmark-only GENCODE masking candidates used to select Ticket 012's
/// production payload. The candidate magic is intentionally not a production
/// compatibility promise.
#[doc(hidden)]
pub mod mask_candidates;

/// Production GRCh38 reference bundle and mmap provider.
pub mod reference;

mod snv;

pub use snv::*;
