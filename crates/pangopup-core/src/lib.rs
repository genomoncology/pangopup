//! Exact, validated vocabulary shared by Pangopup adapters.

use std::{fmt, num::NonZeroU32, str::FromStr};

/// A failure to construct one of Pangopup's public genomic value types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueError {
    InvalidContig(String),
    ZeroPosition,
    InvalidBase(String),
    SameAlleles,
    InvalidGene(String),
    ScoreOutOfRange(u16),
    RelativePositionOutOfRange(i16),
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContig(value) => write!(f, "unsupported GRCh38 primary contig {value}"),
            Self::ZeroPosition => f.write_str("genomic position must be one-based"),
            Self::InvalidBase(value) => write!(f, "DNA base must be A, C, G, or T, got {value}"),
            Self::SameAlleles => f.write_str("reference and alternate bases must differ"),
            Self::InvalidGene(value) => {
                write!(
                    f,
                    "Ensembl gene ID must be ENSG followed by 11 digits, got {value}"
                )
            }
            Self::ScoreOutOfRange(value) => {
                write!(
                    f,
                    "score magnitude must be in 0..=100 hundredths, got {value}"
                )
            }
            Self::RelativePositionOutOfRange(value) => {
                write!(f, "relative position must be in -50..=50, got {value}")
            }
        }
    }
}

impl std::error::Error for ValueError {}

/// A primary chromosome in the GRCh38 score source.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Grch38Contig(u8);

impl Grch38Contig {
    pub const X: Self = Self(23);
    pub const Y: Self = Self(24);
    pub const M: Self = Self(25);

    pub fn autosome(number: u8) -> Result<Self, ValueError> {
        if (1..=22).contains(&number) {
            Ok(Self(number))
        } else {
            Err(ValueError::InvalidContig(number.to_string()))
        }
    }
}

impl FromStr for Grch38Contig {
    type Err = ValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let number = value.strip_prefix("chr").unwrap_or(value);
        match number {
            "X" => Ok(Self::X),
            "Y" => Ok(Self::Y),
            "M" => Ok(Self::M),
            _ => number
                .parse::<u8>()
                .ok()
                .filter(|number| (1..=22).contains(number))
                .map(Self)
                .ok_or_else(|| ValueError::InvalidContig(value.to_owned())),
        }
    }
}

impl fmt::Display for Grch38Contig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            23 => f.write_str("chrX"),
            24 => f.write_str("chrY"),
            25 => f.write_str("chrM"),
            number => write!(f, "chr{number}"),
        }
    }
}

/// A one-based genomic coordinate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenomicPosition(NonZeroU32);

impl GenomicPosition {
    pub fn new(value: u32) -> Result<Self, ValueError> {
        NonZeroU32::new(value)
            .map(Self)
            .ok_or(ValueError::ZeroPosition)
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

impl fmt::Display for GenomicPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

/// One concrete DNA base.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DnaBase {
    A,
    C,
    G,
    T,
}

impl DnaBase {
    pub const ALL: [Self; 4] = [Self::A, Self::C, Self::G, Self::T];

    pub fn parse(value: &str) -> Result<Self, ValueError> {
        match value.as_bytes() {
            [b'A'] => Ok(Self::A),
            [b'C'] => Ok(Self::C),
            [b'G'] => Ok(Self::G),
            [b'T'] => Ok(Self::T),
            _ => Err(ValueError::InvalidBase(value.to_owned())),
        }
    }
}

impl fmt::Display for DnaBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::A => "A",
            Self::C => "C",
            Self::G => "G",
            Self::T => "T",
        })
    }
}

/// An Ensembl gene identifier stored compactly as its numeric suffix.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnsemblGeneId(u64);

impl FromStr for EnsemblGeneId {
    type Err = ValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let digits = value.strip_prefix("ENSG");
        if value.len() != 15
            || digits.is_none_or(|digits| !digits.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(ValueError::InvalidGene(value.to_owned()));
        }
        let numeric = digits
            .and_then(|digits| digits.parse::<u64>().ok())
            .ok_or_else(|| ValueError::InvalidGene(value.to_owned()))?;
        Ok(Self(numeric))
    }
}

impl fmt::Display for EnsemblGeneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ENSG{:011}", self.0)
    }
}

/// An exact absolute score represented in hundredths.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScoreMagnitude(u8);

impl ScoreMagnitude {
    pub fn new(hundredths: u16) -> Result<Self, ValueError> {
        u8::try_from(hundredths)
            .ok()
            .filter(|value| *value <= 100)
            .map(Self)
            .ok_or(ValueError::ScoreOutOfRange(hundredths))
    }

    pub const fn hundredths(self) -> u8 {
        self.0
    }

    fn write_unsigned(self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02}", self.0 / 100, self.0 % 100)
    }
}

impl fmt::Display for ScoreMagnitude {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_unsigned(f)
    }
}

/// A genomic-coordinate delta in Pangolin's configured ±50 window.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelativePosition(i8);

impl RelativePosition {
    pub fn new(value: i16) -> Result<Self, ValueError> {
        i8::try_from(value)
            .ok()
            .filter(|value| (-50..=50).contains(value))
            .map(Self)
            .ok_or(ValueError::RelativePositionOutOfRange(value))
    }

    pub const fn get(self) -> i8 {
        self.0
    }
}

impl fmt::Display for RelativePosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A concrete GRCh38 single-nucleotide variant.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Grch38Snv {
    contig: Grch38Contig,
    position: GenomicPosition,
    reference: DnaBase,
    alternate: DnaBase,
}

impl Grch38Snv {
    pub fn new(
        contig: Grch38Contig,
        position: GenomicPosition,
        reference: DnaBase,
        alternate: DnaBase,
    ) -> Result<Self, ValueError> {
        if reference == alternate {
            return Err(ValueError::SameAlleles);
        }
        Ok(Self {
            contig,
            position,
            reference,
            alternate,
        })
    }

    pub const fn contig(self) -> Grch38Contig {
        self.contig
    }

    pub const fn position(self) -> GenomicPosition {
        self.position
    }

    pub const fn reference(self) -> DnaBase {
        self.reference
    }

    pub const fn alternate(self) -> DnaBase {
        self.alternate
    }
}

/// Pangolin's exact gain/loss magnitudes and genomic relative positions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PangolinScore {
    gain: ScoreMagnitude,
    gain_position: RelativePosition,
    loss: ScoreMagnitude,
    loss_position: RelativePosition,
}

impl PangolinScore {
    pub const fn new(
        gain: ScoreMagnitude,
        gain_position: RelativePosition,
        loss: ScoreMagnitude,
        loss_position: RelativePosition,
    ) -> Self {
        Self {
            gain,
            gain_position,
            loss,
            loss_position,
        }
    }

    pub const fn gain(self) -> ScoreMagnitude {
        self.gain
    }

    pub const fn gain_position(self) -> RelativePosition {
        self.gain_position
    }

    pub const fn loss(self) -> ScoreMagnitude {
        self.loss
    }

    pub const fn loss_position(self) -> RelativePosition {
        self.loss_position
    }

    pub fn loss_text(self) -> LossText {
        LossText(self.loss)
    }
}

/// Exact rendering of a loss magnitude with its semantic sign restored.
pub struct LossText(ScoreMagnitude);

impl fmt::Display for LossText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.hundredths() == 0 {
            f.write_str("0.00")
        } else {
            f.write_str("-")?;
            self.0.write_unsigned(f)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_boundaries_are_enforced() {
        assert_eq!(GenomicPosition::new(0), Err(ValueError::ZeroPosition));
        assert!(GenomicPosition::new(1).is_ok());
        assert!(ScoreMagnitude::new(0).is_ok());
        assert!(ScoreMagnitude::new(100).is_ok());
        assert_eq!(
            ScoreMagnitude::new(101),
            Err(ValueError::ScoreOutOfRange(101))
        );
        assert!(RelativePosition::new(-50).is_ok());
        assert!(RelativePosition::new(50).is_ok());
        assert_eq!(
            RelativePosition::new(51),
            Err(ValueError::RelativePositionOutOfRange(51))
        );
    }

    #[test]
    fn identifiers_and_snv_are_typed() {
        assert_eq!(
            "chr22".parse::<Grch38Contig>().expect("valid").to_string(),
            "chr22"
        );
        assert_eq!(
            "1".parse::<Grch38Contig>()
                .expect("adapter alias")
                .to_string(),
            "chr1"
        );
        assert_eq!(
            "chr01"
                .parse::<Grch38Contig>()
                .expect("adapter alias")
                .to_string(),
            "chr1"
        );
        assert!(Grch38Contig::autosome(1).is_ok());
        assert!(Grch38Contig::autosome(23).is_err());
        assert_eq!(
            "chrM".parse::<Grch38Contig>().expect("valid").to_string(),
            "chrM"
        );
        assert!("chr23".parse::<Grch38Contig>().is_err());
        assert_eq!(
            "ENSG00000141510"
                .parse::<EnsemblGeneId>()
                .expect("valid")
                .to_string(),
            "ENSG00000141510"
        );
        assert!("ENSG141510".parse::<EnsemblGeneId>().is_err());
        let position = GenomicPosition::new(1).expect("valid");
        assert!(Grch38Snv::new(Grch38Contig::X, position, DnaBase::A, DnaBase::T).is_ok());
        assert_eq!(
            Grch38Snv::new(Grch38Contig::X, position, DnaBase::A, DnaBase::A),
            Err(ValueError::SameAlleles)
        );
    }

    #[test]
    fn signed_loss_is_restored_exactly() {
        let score = PangolinScore::new(
            ScoreMagnitude::new(1).expect("valid"),
            RelativePosition::new(-50).expect("valid"),
            ScoreMagnitude::new(21).expect("valid"),
            RelativePosition::new(50).expect("valid"),
        );
        assert_eq!(score.gain().to_string(), "0.01");
        assert_eq!(score.gain_position().get(), -50);
        assert_eq!(score.loss().hundredths(), 21);
        assert_eq!(score.loss_position().get(), 50);
        assert_eq!(score.loss_text().to_string(), "-0.21");
        assert_eq!(
            LossText(ScoreMagnitude::new(0).expect("valid")).to_string(),
            "0.00"
        );
    }
}
