//! Private, explicitly decoded Pangopup runtime formats.

/// Benchmark-only GRCh38 reference candidates used to select Ticket 011's
/// production payload. Nothing in the runtime lookup path depends on these
/// experimental codecs.
#[doc(hidden)]
pub mod reference_candidates;

/// Production, domains-only GENCODE mask mmap provider.
pub mod mask;

/// Production GRCh38 reference bundle and mmap provider.
pub mod reference;

mod snv;

pub use snv::*;
