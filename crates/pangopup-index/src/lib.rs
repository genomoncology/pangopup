//! Private, explicitly decoded Pangopup runtime formats.

/// Production, domains-only GENCODE mask mmap provider.
pub mod mask;

/// Production GRCh38 reference bundle and mmap provider.
pub mod reference;
pub mod reference_admission;
mod reference_reader;
mod reference_wire;
mod reference_writer;

mod snv;

pub use snv::*;
