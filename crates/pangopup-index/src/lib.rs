//! Private, explicitly decoded Pangopup runtime formats.

/// Production, domains-only GENCODE mask mmap provider.
pub mod mask;

/// Production GRCh38 reference bundle and mmap provider.
pub mod reference;
pub mod reference_admission;

mod snv;

pub use snv::*;
