//! Private, explicitly decoded Pangopup runtime formats.

/// The gene-name index the build carries.
pub mod gene_names;

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
