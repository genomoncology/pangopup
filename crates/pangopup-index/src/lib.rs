//! Private, explicitly decoded Pangopup runtime formats.

/// Production, domains-only GENCODE mask mmap provider.
pub mod mask;

/// Production GRCh38 reference bundle and mmap provider.
pub mod reference;

mod snv;

pub use snv::*;
