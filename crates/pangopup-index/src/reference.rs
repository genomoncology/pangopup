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

use pangopup_core::Grch38Contig;

/// The accepted caller-facing names for one primary GRCh38 contig.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallerContigSpellings {
    canonical: &'static str,
    bare: &'static str,
    accession: &'static str,
    mitochondrial: bool,
}

impl CallerContigSpellings {
    pub const fn canonical(self) -> &'static str {
        self.canonical
    }

    pub fn accepts(self, value: &str) -> bool {
        value == self.bare
            || value == self.canonical
            || value == self.accession
            || (self.mitochondrial && matches!(value, "MT" | "chrMT"))
    }

    pub fn accepted(self) -> Vec<&'static str> {
        if self.mitochondrial {
            vec![self.bare, "MT", self.canonical, "chrMT", self.accession]
        } else {
            vec![self.bare, self.canonical, self.accession]
        }
    }
}

/// Return the caller-facing spellings accepted for one primary GRCh38 contig.
pub fn caller_contig_spellings(contig: Grch38Contig) -> CallerContigSpellings {
    const BARE: [&str; 25] = [
        "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
        "17", "18", "19", "20", "21", "22", "X", "Y", "M",
    ];
    const CANONICAL: [&str; 25] = [
        "chr1", "chr2", "chr3", "chr4", "chr5", "chr6", "chr7", "chr8", "chr9", "chr10", "chr11",
        "chr12", "chr13", "chr14", "chr15", "chr16", "chr17", "chr18", "chr19", "chr20", "chr21",
        "chr22", "chrX", "chrY", "chrM",
    ];
    let index = usize::from(contig.code() - 1);
    CallerContigSpellings {
        canonical: CANONICAL[index],
        bare: BARE[index],
        accession: required_accession(contig),
        mitochondrial: contig == Grch38Contig::M,
    }
}

/// Parse a caller-facing spelling without widening stored source identities.
pub fn parse_caller_contig(value: &str) -> Option<Grch38Contig> {
    (1_u8..=25).find_map(|code| {
        let contig = Grch38Contig::from_code(code).ok()?;
        caller_contig_spellings(contig)
            .accepts(value)
            .then_some(contig)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caller_spellings_and_parser_share_the_mitochondrial_policy() {
        let mitochondrial = caller_contig_spellings(Grch38Contig::M);
        assert_eq!(
            mitochondrial.accepted(),
            ["M", "MT", "chrM", "chrMT", "NC_012920.1"]
        );
        for spelling in mitochondrial.accepted() {
            assert_eq!(parse_caller_contig(spelling), Some(Grch38Contig::M));
        }
        assert_eq!(mitochondrial.canonical(), "chrM");
    }
}
