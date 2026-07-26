//! Variant-level Pangolin scoring over the shipped reference, mask, and raw
//! model providers.
//!
//! This crate owns the fixed GRCh38 distance-50 composition. It deliberately
//! does not route through the precomputed SNV index or expose the raw model
//! seam used by its compatibility tests.

use pangopup_core::{
    GencodeGeneId, GenomicPosition, Grch38Contig, Grch38Variant, ModelGeneScoreRecord,
    ModelRejection, ModelScoreResult, ModelScoringError, ModelWarning, PangolinScore,
    ReferenceError, ReferenceProvider, RelativePosition, ScoreMagnitude,
};
use pangopup_index::mask::{MaskProvider, MaskQueryBuffer, MaskQueryGene};
use pangopup_model::{
    CHANNELS, CONTEXT_FLANKS, ModelContext, ModelKernel, ReplicateScores, Strand,
};
use std::collections::BTreeSet;

const DISTANCE: i16 = 50;
const FLANK: u32 = 5_050;
const CONTEXT_BASES: usize = 10_100;
const MAX_ALLELE_BASES: usize = 100;

/// Mutable, single-owner composition of the three production providers.
pub struct VariantScorer {
    engine: ScoringEngine,
}

impl VariantScorer {
    pub fn new(
        reference: impl ReferenceProvider + 'static,
        mask: impl MaskProvider + 'static,
        model: ModelKernel,
    ) -> Self {
        Self {
            engine: ScoringEngine {
                reference: Box::new(reference),
                mask: Box::new(ProductionMask::new(mask)),
                kernel: Box::new(ProductionKernel(model)),
            },
        }
    }

    /// Score one literal supported GRCh38 variant with masked distance-50
    /// semantics.
    pub fn score(
        &mut self,
        variant: &Grch38Variant,
    ) -> Result<ModelScoreResult, ModelScoringError> {
        self.engine.score(variant)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VariantShape {
    EqualLength,
    Insertion,
    Deletion,
}

#[derive(Clone, Debug)]
struct MaskGene {
    identity: GencodeGeneId,
    boundaries: Vec<GenomicPosition>,
}

#[derive(Clone, Debug, Default)]
struct MaskGenes {
    plus: Vec<MaskGene>,
    minus: Vec<MaskGene>,
}

trait GeneMaskSource {
    fn query(
        &mut self,
        contig: Grch38Contig,
        position: GenomicPosition,
    ) -> Result<MaskGenes, ModelScoringError>;
}

struct ProductionMask<M> {
    provider: M,
    buffer: MaskQueryBuffer,
}

impl<M> ProductionMask<M> {
    fn new(provider: M) -> Self {
        Self {
            provider,
            buffer: MaskQueryBuffer::with_capacity(16, 512),
        }
    }

    fn copy_genes(&self, genes: &[MaskQueryGene]) -> Vec<MaskGene> {
        genes
            .iter()
            .map(|gene| MaskGene {
                identity: gene.identity(),
                boundaries: self.buffer.boundaries(gene).to_vec(),
            })
            .collect()
    }
}

impl<M: MaskProvider> GeneMaskSource for ProductionMask<M> {
    fn query(
        &mut self,
        contig: Grch38Contig,
        position: GenomicPosition,
    ) -> Result<MaskGenes, ModelScoringError> {
        self.provider
            .query(contig, position, None, &mut self.buffer)
            .map_err(|_| ModelScoringError::MaskProvider)?;
        Ok(MaskGenes {
            plus: self.copy_genes(self.buffer.plus()),
            minus: self.copy_genes(self.buffer.minus()),
        })
    }
}

#[derive(Clone, Debug)]
struct RawScores {
    score_length: usize,
    values: Vec<f32>,
}

impl RawScores {
    fn from_replicates(scores: ReplicateScores) -> Self {
        Self {
            score_length: scores.score_length(),
            values: scores.values().to_vec(),
        }
    }

    fn channel(&self, ordinal: usize) -> Option<&[f32]> {
        let start = ordinal.checked_sub(1)?.checked_mul(self.score_length)?;
        self.values.get(start..start + self.score_length)
    }

    fn validate(&self) -> Result<(), ModelScoringError> {
        if self.score_length == 0
            || self.values.len() != CHANNELS * self.score_length
            || self
                .values
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        {
            return Err(ModelScoringError::InvalidModelOutput);
        }
        Ok(())
    }
}

trait RawKernel {
    fn infer(&mut self, context: &[u8], strand: Strand) -> Result<RawScores, ModelScoringError>;
}

struct ProductionKernel(ModelKernel);

impl RawKernel for ProductionKernel {
    fn infer(&mut self, context: &[u8], strand: Strand) -> Result<RawScores, ModelScoringError> {
        let context = ModelContext::new(context).map_err(|_| ModelScoringError::ModelProvider)?;
        self.0
            .infer(&context, strand)
            .map(RawScores::from_replicates)
            .map_err(|_| ModelScoringError::ModelProvider)
    }
}

struct ScoringEngine {
    reference: Box<dyn ReferenceProvider>,
    mask: Box<dyn GeneMaskSource>,
    kernel: Box<dyn RawKernel>,
}

impl ScoringEngine {
    fn score(&mut self, variant: &Grch38Variant) -> Result<ModelScoreResult, ModelScoringError> {
        let shape = match classify(variant) {
            Ok(shape) => shape,
            Err(rejection) => return Ok(ModelScoreResult::rejected(rejection)),
        };
        let reference_length = variant.reference().len();
        let alternate_length = variant.alternate().len();
        if reference_length > MAX_ALLELE_BASES || alternate_length > MAX_ALLELE_BASES {
            return Ok(ModelScoreResult::rejected(ModelRejection::AlleleTooLong {
                reference_length,
                alternate_length,
            }));
        }

        let Some(start_value) = variant.position().get().checked_sub(FLANK) else {
            return Ok(ModelScoreResult::rejected(
                ModelRejection::InsufficientReferenceContext,
            ));
        };
        let Ok(start) = GenomicPosition::new(start_value) else {
            return Ok(ModelScoreResult::rejected(
                ModelRejection::InsufficientReferenceContext,
            ));
        };
        let mut reference = vec![0_u8; CONTEXT_BASES + reference_length];
        match self
            .reference
            .copy_window(variant.contig(), start, &mut reference)
        {
            Ok(()) => {}
            Err(ReferenceError::OutOfBounds) => {
                return Ok(ModelScoreResult::rejected(
                    ModelRejection::InsufficientReferenceContext,
                ));
            }
            Err(error) => return Err(ModelScoringError::ReferenceProvider(error)),
        }

        let anchor = FLANK as usize;
        if reference.get(anchor..anchor + reference_length) != Some(variant.reference().as_bytes())
        {
            return Ok(ModelScoreResult::rejected(
                ModelRejection::ReferenceMismatch,
            ));
        }
        if let Some((offset, symbol)) = reference
            .iter()
            .copied()
            .enumerate()
            .find(|(_, base)| !matches!(base, b'A' | b'C' | b'G' | b'T' | b'N'))
        {
            return Ok(ModelScoreResult::rejected(
                ModelRejection::UnsupportedReferenceSymbol { offset, symbol },
            ));
        }

        let mut alternate = Vec::with_capacity(CONTEXT_BASES + alternate_length);
        alternate.extend_from_slice(&reference[..anchor]);
        alternate.extend_from_slice(variant.alternate().as_bytes());
        alternate.extend_from_slice(&reference[anchor + reference_length..]);

        let genes = self.mask.query(variant.contig(), variant.position())?;
        if genes.plus.is_empty() && genes.minus.is_empty() {
            return Ok(ModelScoreResult::rejected(ModelRejection::NotInGene));
        }

        let mut records = Vec::with_capacity(genes.plus.len() + genes.minus.len());
        if !genes.plus.is_empty() {
            let reference_scores = self.infer_checked(&reference, Strand::Plus)?;
            let alternate_scores = self.infer_checked(&alternate, Strand::Plus)?;
            let arrays = post_ensemble(shape, &reference_scores, &alternate_scores)?;
            records.extend(score_genes(arrays, &genes.plus, variant.position())?);
        }
        if !genes.minus.is_empty() {
            let reference_scores = self.infer_checked(&reference, Strand::Minus)?;
            let alternate_scores = self.infer_checked(&alternate, Strand::Minus)?;
            let arrays = post_ensemble(shape, &reference_scores, &alternate_scores)?;
            records.extend(score_genes(arrays, &genes.minus, variant.position())?);
        }
        Ok(ModelScoreResult::scored(records))
    }

    fn infer_checked(
        &mut self,
        context: &[u8],
        strand: Strand,
    ) -> Result<RawScores, ModelScoringError> {
        let expected_length = context
            .len()
            .checked_sub(CONTEXT_FLANKS)
            .ok_or(ModelScoringError::InvalidModelOutput)?;
        let scores = self.kernel.infer(context, strand)?;
        scores.validate()?;
        if scores.score_length != expected_length {
            return Err(ModelScoringError::InvalidModelOutput);
        }
        Ok(scores)
    }
}

fn classify(variant: &Grch38Variant) -> Result<VariantShape, ModelRejection> {
    let reference = variant.reference().as_bytes();
    let alternate = variant.alternate().as_bytes();
    if reference.len() == alternate.len() {
        return Ok(VariantShape::EqualLength);
    }
    if reference.len() == 1 && alternate.len() > 1 && reference[0] == alternate[0] {
        return Ok(VariantShape::Insertion);
    }
    if reference.len() > 1 && alternate.len() == 1 && reference[0] == alternate[0] {
        return Ok(VariantShape::Deletion);
    }
    Err(ModelRejection::UnsupportedVariantShape)
}

#[derive(Clone, Debug)]
enum ScoreArrays {
    F32 { loss: Vec<f32>, gain: Vec<f32> },
    F64 { loss: Vec<f64>, gain: Vec<f64> },
}

fn post_ensemble(
    shape: VariantShape,
    reference: &RawScores,
    alternate: &RawScores,
) -> Result<ScoreArrays, ModelScoringError> {
    reference.validate()?;
    alternate.validate()?;
    match shape {
        VariantShape::EqualLength => post_equal(reference, alternate),
        VariantShape::Insertion => post_insertion(reference, alternate),
        VariantShape::Deletion => post_deletion(reference, alternate),
    }
}

fn post_equal(
    reference: &RawScores,
    alternate: &RawScores,
) -> Result<ScoreArrays, ModelScoringError> {
    if reference.score_length != alternate.score_length {
        return Err(ModelScoringError::InvalidModelOutput);
    }
    let tissues = tissue_differences_f32(reference, alternate, |alt, _| Ok(alt.to_vec()))?;
    let (loss, gain) = extrema(&tissues);
    Ok(ScoreArrays::F32 { loss, gain })
}

fn post_insertion(
    reference: &RawScores,
    alternate: &RawScores,
) -> Result<ScoreArrays, ModelScoringError> {
    let difference = alternate
        .score_length
        .checked_sub(reference.score_length)
        .filter(|difference| *difference > 0)
        .ok_or(ModelScoringError::InvalidModelOutput)?;
    let tissues = tissue_differences_f32(reference, alternate, |alt, target| {
        collapse_insertion(alt, target, difference)
    })?;
    let (loss, gain) = extrema(&tissues);
    Ok(ScoreArrays::F32 { loss, gain })
}

fn post_deletion(
    reference: &RawScores,
    alternate: &RawScores,
) -> Result<ScoreArrays, ModelScoringError> {
    let difference = reference
        .score_length
        .checked_sub(alternate.score_length)
        .filter(|difference| *difference > 0)
        .ok_or(ModelScoringError::InvalidModelOutput)?;
    let mut tissues = Vec::with_capacity(4);
    for tissue in 0..4 {
        let mut replicates = Vec::with_capacity(3);
        for replicate in 0..3 {
            let ordinal = tissue * 3 + replicate + 1;
            let reference_channel = reference
                .channel(ordinal)
                .ok_or(ModelScoringError::InvalidModelOutput)?;
            let alternate_channel = alternate
                .channel(ordinal)
                .ok_or(ModelScoringError::InvalidModelOutput)?;
            let expanded = expand_deletion(alternate_channel, reference.score_length, difference)?;
            replicates.push(
                expanded
                    .into_iter()
                    .zip(reference_channel)
                    .map(|(alternate, reference)| alternate - f64::from(*reference))
                    .collect::<Vec<_>>(),
            );
        }
        tissues.push(mean_three(&replicates)?);
    }
    let (loss, gain) = extrema(&tissues);
    Ok(ScoreArrays::F64 { loss, gain })
}

fn tissue_differences_f32(
    reference: &RawScores,
    alternate: &RawScores,
    reconcile: impl Fn(&[f32], usize) -> Result<Vec<f32>, ModelScoringError>,
) -> Result<Vec<Vec<f32>>, ModelScoringError> {
    let mut tissues = Vec::with_capacity(4);
    for tissue in 0..4 {
        let mut replicates = Vec::with_capacity(3);
        for replicate in 0..3 {
            let ordinal = tissue * 3 + replicate + 1;
            let reference_channel = reference
                .channel(ordinal)
                .ok_or(ModelScoringError::InvalidModelOutput)?;
            let alternate_channel = alternate
                .channel(ordinal)
                .ok_or(ModelScoringError::InvalidModelOutput)?;
            let reconciled = reconcile(alternate_channel, reference.score_length)?;
            if reconciled.len() != reference.score_length {
                return Err(ModelScoringError::InvalidModelOutput);
            }
            replicates.push(
                reconciled
                    .into_iter()
                    .zip(reference_channel)
                    .map(|(alternate, reference)| alternate - reference)
                    .collect::<Vec<_>>(),
            );
        }
        tissues.push(mean_three(&replicates)?);
    }
    Ok(tissues)
}

fn collapse_insertion(
    alternate: &[f32],
    target: usize,
    difference: usize,
) -> Result<Vec<f32>, ModelScoringError> {
    let collapse_start = DISTANCE as usize;
    let collapse_end = collapse_start
        .checked_add(difference + 1)
        .ok_or(ModelScoringError::InvalidModelOutput)?;
    if alternate.len() != target + difference || collapse_end > alternate.len() {
        return Err(ModelScoringError::InvalidModelOutput);
    }
    let collapsed = alternate[collapse_start..collapse_end]
        .iter()
        .copied()
        .reduce(|left, right| if right > left { right } else { left })
        .ok_or(ModelScoringError::InvalidModelOutput)?;
    let mut result = Vec::with_capacity(target);
    result.extend_from_slice(&alternate[..collapse_start]);
    result.push(collapsed);
    result.extend_from_slice(&alternate[collapse_end..]);
    Ok(result)
}

fn expand_deletion(
    alternate: &[f32],
    target: usize,
    difference: usize,
) -> Result<Vec<f64>, ModelScoringError> {
    let split = DISTANCE as usize + 1;
    if alternate.len() + difference != target || split > alternate.len() {
        return Err(ModelScoringError::InvalidModelOutput);
    }
    let mut result = Vec::with_capacity(target);
    result.extend(alternate[..split].iter().map(|value| f64::from(*value)));
    result.resize(result.len() + difference, 0.0_f64);
    result.extend(alternate[split..].iter().map(|value| f64::from(*value)));
    Ok(result)
}

trait Arithmetic: Copy + PartialOrd {
    fn zero() -> Self;
    fn add(self, other: Self) -> Self;
    fn divide_three(self) -> Self;
}

impl Arithmetic for f32 {
    fn zero() -> Self {
        0.0
    }

    fn add(self, other: Self) -> Self {
        self + other
    }

    fn divide_three(self) -> Self {
        self / 3.0_f32
    }
}

impl Arithmetic for f64 {
    fn zero() -> Self {
        0.0
    }

    fn add(self, other: Self) -> Self {
        self + other
    }

    fn divide_three(self) -> Self {
        self / 3.0_f64
    }
}

fn mean_three<T: Arithmetic>(replicates: &[Vec<T>]) -> Result<Vec<T>, ModelScoringError> {
    let [first, second, third] = replicates else {
        return Err(ModelScoringError::InvalidModelOutput);
    };
    if first.is_empty() || second.len() != first.len() || third.len() != first.len() {
        return Err(ModelScoringError::InvalidModelOutput);
    }
    Ok(first
        .iter()
        .zip(second)
        .zip(third)
        .map(|((first, second), third)| first.add(*second).add(*third).divide_three())
        .collect())
}

fn extrema<T: Arithmetic>(tissues: &[Vec<T>]) -> (Vec<T>, Vec<T>) {
    let length = tissues.first().map_or(0, Vec::len);
    let mut loss = Vec::with_capacity(length);
    let mut gain = Vec::with_capacity(length);
    for index in 0..length {
        let mut minimum = tissues[0][index];
        let mut maximum = minimum;
        for tissue in &tissues[1..] {
            if tissue[index] < minimum {
                minimum = tissue[index];
            }
            if tissue[index] > maximum {
                maximum = tissue[index];
            }
        }
        loss.push(minimum);
        gain.push(maximum);
    }
    (loss, gain)
}

fn score_genes(
    arrays: ScoreArrays,
    genes: &[MaskGene],
    position: GenomicPosition,
) -> Result<Vec<ModelGeneScoreRecord>, ModelScoringError> {
    match arrays {
        ScoreArrays::F32 { loss, gain } => score_typed(loss, gain, genes, position),
        ScoreArrays::F64 { loss, gain } => score_typed(loss, gain, genes, position),
    }
}

trait PublicScore: Arithmetic {
    fn gain_hundredths(self) -> Result<u16, ModelScoringError>;
    fn loss_hundredths(self) -> Result<u16, ModelScoringError>;
}

impl PublicScore for f32 {
    fn gain_hundredths(self) -> Result<u16, ModelScoringError> {
        checked_gain((self * 100.0_f32).round_ties_even() as f64)
    }

    fn loss_hundredths(self) -> Result<u16, ModelScoringError> {
        checked_loss((self * 100.0_f32).round_ties_even() as f64)
    }
}

impl PublicScore for f64 {
    fn gain_hundredths(self) -> Result<u16, ModelScoringError> {
        checked_gain((self * 100.0_f64).round_ties_even())
    }

    fn loss_hundredths(self) -> Result<u16, ModelScoringError> {
        checked_loss((self * 100.0_f64).round_ties_even())
    }
}

fn checked_gain(rounded: f64) -> Result<u16, ModelScoringError> {
    if !rounded.is_finite() || !(0.0..=100.0).contains(&rounded) {
        return Err(ModelScoringError::InvalidModelOutput);
    }
    Ok(rounded as u16)
}

fn checked_loss(rounded: f64) -> Result<u16, ModelScoringError> {
    if !rounded.is_finite() || !(-100.0..=0.0).contains(&rounded) {
        return Err(ModelScoringError::InvalidModelOutput);
    }
    Ok((-rounded) as u16)
}

fn score_typed<T: PublicScore>(
    mut loss: Vec<T>,
    mut gain: Vec<T>,
    genes: &[MaskGene],
    position: GenomicPosition,
) -> Result<Vec<ModelGeneScoreRecord>, ModelScoringError> {
    if loss.is_empty() || loss.len() != gain.len() {
        return Err(ModelScoringError::InvalidModelOutput);
    }
    let window_start = i64::from(position.get()) - i64::from(DISTANCE);
    let mut result = Vec::with_capacity(genes.len());
    for gene in genes {
        let warning = apply_mask(&mut loss, &mut gain, &gene.boundaries, window_start);
        let warnings = warning.into_iter().collect();

        let gain_index = first_maximum(&gain);
        let loss_index = first_minimum(&loss);
        let gain_position = relative_position(gain_index)?;
        let loss_position = relative_position(loss_index)?;
        let score = PangolinScore::new(
            ScoreMagnitude::new(gain[gain_index].gain_hundredths()?)
                .map_err(|_| ModelScoringError::InvalidModelOutput)?,
            gain_position,
            ScoreMagnitude::new(loss[loss_index].loss_hundredths()?)
                .map_err(|_| ModelScoringError::InvalidModelOutput)?,
            loss_position,
        );
        result.push(ModelGeneScoreRecord::new(gene.identity, score, warnings));
    }
    Ok(result)
}

fn apply_mask<T: Arithmetic>(
    loss: &mut [T],
    gain: &mut [T],
    boundaries: &[GenomicPosition],
    window_start: i64,
) -> Option<ModelWarning> {
    let indices: Vec<usize> = boundaries
        .iter()
        .filter_map(|boundary| {
            usize::try_from(i64::from(boundary.get()) - window_start)
                .ok()
                .filter(|index| *index < loss.len())
        })
        .collect();
    if boundaries.is_empty() {
        for value in loss {
            *value = numpy_maximum(*value, T::zero());
        }
        Some(ModelWarning::NoAnnotatedSites)
    } else {
        for index in &indices {
            gain[*index] = numpy_minimum(gain[*index], T::zero());
        }
        let indices: BTreeSet<_> = indices.into_iter().collect();
        for (index, value) in loss.iter_mut().enumerate() {
            if !indices.contains(&index) {
                *value = numpy_maximum(*value, T::zero());
            }
        }
        None
    }
}

fn numpy_minimum<T: Copy + PartialOrd>(left: T, right: T) -> T {
    if left < right { left } else { right }
}

fn numpy_maximum<T: Copy + PartialOrd>(left: T, right: T) -> T {
    if left > right { left } else { right }
}

fn first_maximum<T: PartialOrd>(values: &[T]) -> usize {
    let mut best = 0;
    for index in 1..values.len() {
        if values[index] > values[best] {
            best = index;
        }
    }
    best
}

fn first_minimum<T: PartialOrd>(values: &[T]) -> usize {
    let mut best = 0;
    for index in 1..values.len() {
        if values[index] < values[best] {
            best = index;
        }
    }
    best
}

fn relative_position(index: usize) -> Result<RelativePosition, ModelScoringError> {
    let value = i16::try_from(index)
        .ok()
        .and_then(|index| index.checked_sub(DISTANCE))
        .ok_or(ModelScoringError::InvalidModelOutput)?;
    RelativePosition::new(value).map_err(|_| ModelScoringError::InvalidModelOutput)
}

#[cfg(test)]
mod tests;
