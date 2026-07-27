//! Production `PGRREF01` GRCh38 reference bundle.
//!
//! The public facade preserves the original API while the wire contract,
//! byte-producing writer, and single mmap reader have separate ownership.

pub use crate::reference_reader::{
    IdentifiedReferenceBundle, ReferenceBundleOpen, ReferenceMemberIdentity,
};
pub use crate::reference_wire::*;
pub use crate::reference_writer::ReferenceMemberWriter;

pub(crate) use crate::reference_reader::open_held_installed;
