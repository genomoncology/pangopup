//! Variant-level Pangolin scoring over the shipped reference, mask, and raw
//! model providers.
//!
//! This crate owns the fixed GRCh38 distance-50 composition and the small
//! lookup-first routing boundary above it. It does not own asset-opening or
//! transport policy and does not expose the raw model seam used by its
//! compatibility tests.

use pangopup_core::{
    DnaBase, EnsemblGeneId, GencodeGeneId, GenomicPosition, Grch38Contig, Grch38Snv, Grch38Variant,
    LookupError, LookupResult, ModelGeneScoreRecord, ModelRejection, ModelScoreResult,
    ModelScoringError, ModelWarning, PangolinScore, ReferenceError, ReferenceProvenance,
    ReferenceProvider, RelativePosition, ScoreMagnitude, ScoreProvider,
};
use pangopup_index::mask::{IdentifiedMaskDomains, MaskProvider, MaskQueryBuffer, MaskQueryGene};
use pangopup_model::{
    CHANNELS, CONTEXT_FLANKS, InferenceAccounting, ModelContext, ModelKernel, ModelRepresentation,
    ReplicateScores, Strand, StrandPair,
};
use std::{collections::BTreeSet, fmt};

const DISTANCE: i16 = 50;
const FLANK: u32 = 5_050;
const CONTEXT_BASES: usize = 10_100;
/// Largest reference or alternate allele eligible for model scoring.
pub const MAX_MODEL_ALLELE_BASES: usize = 100;
/// Largest exact-edit payload. Conversion adds one left anchor base.
pub const MAX_EXACT_EDIT_SEQUENCE_BASES: usize = MAX_MODEL_ALLELE_BASES - 1;

/// One pre-canonical GRCh38 edit whose left anchor must be read from the reference.
///
/// Callers must use [`Grch38ExactEdit::insertion`] or
/// [`Grch38ExactEdit::deletion`] so invalid geometry cannot bypass validation.
///
/// ```compile_fail
/// use pangopup_core::{GenomicPosition, Grch38Contig};
/// use pangopup_engine::Grch38ExactEdit;
///
/// let contig = Grch38Contig::autosome(1).unwrap();
/// let first = GenomicPosition::new(1).unwrap();
/// let edit = Grch38ExactEdit::Deletion {
///     contig,
///     start: first,
///     end: first,
///     deleted: "A".to_owned(),
/// };
/// ```
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Grch38ExactEdit(ExactEditKind);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum ExactEditKind {
    Insertion {
        contig: Grch38Contig,
        left: GenomicPosition,
        inserted: String,
    },
    Deletion {
        contig: Grch38Contig,
        start: GenomicPosition,
        deleted: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactEditError {
    NonAdjacentInsertion,
    ReversedDeletion,
    DeletionAtFirstBase,
    SequenceLengthMismatch,
    InvalidSequence,
}

impl fmt::Display for ExactEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NonAdjacentInsertion => "insertion coordinates must be adjacent",
            Self::ReversedDeletion => "deletion end must not precede its start",
            Self::DeletionAtFirstBase => "a deletion requires a left anchor",
            Self::SequenceLengthMismatch => "deletion sequence length must match its interval",
            Self::InvalidSequence => {
                "edit sequence must contain 1 through 99 uppercase A/C/G/T bases"
            }
        })
    }
}

impl std::error::Error for ExactEditError {}

impl Grch38ExactEdit {
    pub fn insertion(
        contig: Grch38Contig,
        left: GenomicPosition,
        right: GenomicPosition,
        inserted: impl Into<String>,
    ) -> Result<Self, ExactEditError> {
        let inserted = checked_edit_sequence(inserted.into())?;
        if left.get().checked_add(1) != Some(right.get()) {
            return Err(ExactEditError::NonAdjacentInsertion);
        }
        Ok(Self(ExactEditKind::Insertion {
            contig,
            left,
            inserted,
        }))
    }

    pub fn deletion(
        contig: Grch38Contig,
        start: GenomicPosition,
        end: GenomicPosition,
        deleted: impl Into<String>,
    ) -> Result<Self, ExactEditError> {
        let deleted = checked_edit_sequence(deleted.into())?;
        if start.get() == 1 {
            return Err(ExactEditError::DeletionAtFirstBase);
        }
        let length = end
            .get()
            .checked_sub(start.get())
            .and_then(|span| span.checked_add(1))
            .ok_or(ExactEditError::ReversedDeletion)?;
        if usize::try_from(length).ok() != Some(deleted.len()) {
            return Err(ExactEditError::SequenceLengthMismatch);
        }
        Ok(Self(ExactEditKind::Deletion {
            contig,
            start,
            deleted,
        }))
    }
}

fn checked_edit_sequence(sequence: String) -> Result<String, ExactEditError> {
    if !(1..=MAX_EXACT_EDIT_SEQUENCE_BASES).contains(&sequence.len())
        || !sequence
            .as_bytes()
            .iter()
            .all(|base| matches!(base, b'A' | b'C' | b'G' | b'T'))
    {
        return Err(ExactEditError::InvalidSequence);
    }
    Ok(sequence)
}

/// The outcome of constructing a literal allele from an exact edit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactEditConversionError {
    InvalidRequest,
    Rejected(Grch38Variant),
    ReferenceProvider(ReferenceError),
}

/// Convert an exact edit to the one canonical literal tuple used by routing and caching.
pub fn convert_exact_edit(
    reference: &dyn ReferenceProvider,
    edit: &Grch38ExactEdit,
) -> Result<Grch38Variant, ExactEditConversionError> {
    match &edit.0 {
        ExactEditKind::Insertion {
            contig,
            left,
            inserted,
        } => {
            let mut adjacent = [0_u8; 2];
            copy_edit_window(reference, *contig, *left, &mut adjacent)?;
            let anchor = checked_anchor(adjacent[0])?;
            let mut alternate = String::with_capacity(inserted.len() + 1);
            alternate.push(char::from(anchor));
            alternate.push_str(inserted);
            Grch38Variant::new(*contig, *left, char::from(anchor).to_string(), alternate)
                .map_err(|_| ExactEditConversionError::InvalidRequest)
        }
        ExactEditKind::Deletion {
            contig,
            start,
            deleted,
        } => {
            let anchor_position = start
                .get()
                .checked_sub(1)
                .and_then(|position| GenomicPosition::new(position).ok())
                .ok_or(ExactEditConversionError::InvalidRequest)?;
            let mut observed = vec![0_u8; deleted.len() + 1];
            copy_edit_window(reference, *contig, anchor_position, &mut observed)?;
            let anchor = checked_anchor(observed[0])?;
            let reference_allele = format!("{}{}", char::from(anchor), deleted);
            let variant = Grch38Variant::new(
                *contig,
                anchor_position,
                reference_allele,
                char::from(anchor).to_string(),
            )
            .map_err(|_| ExactEditConversionError::InvalidRequest)?;
            if observed[1..] != deleted.as_bytes()[..] {
                return Err(ExactEditConversionError::Rejected(variant));
            }
            Ok(variant)
        }
    }
}

fn copy_edit_window(
    reference: &dyn ReferenceProvider,
    contig: Grch38Contig,
    start: GenomicPosition,
    destination: &mut [u8],
) -> Result<(), ExactEditConversionError> {
    reference
        .copy_window(contig, start, destination)
        .map_err(|error| match error {
            ReferenceError::OutOfBounds | ReferenceError::EmptyWindow => {
                ExactEditConversionError::InvalidRequest
            }
            _ => ExactEditConversionError::ReferenceProvider(error),
        })
}

fn checked_anchor(anchor: u8) -> Result<u8, ExactEditConversionError> {
    matches!(anchor, b'A' | b'C' | b'G' | b'T')
        .then_some(anchor)
        .ok_or(ExactEditConversionError::InvalidRequest)
}

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
        assert_eq!(
            model.representation(),
            ModelRepresentation::Singleton,
            "ordinary VariantScorer requires the selected singleton model"
        );
        Self::from_parts(reference, mask, model)
    }

    /// Construct a scorer for the retained Ticket 022 comparison harness.
    ///
    /// Ordinary callers must use [`Self::new`].
    #[doc(hidden)]
    pub fn new_experimental(
        reference: impl ReferenceProvider + 'static,
        mask: impl MaskProvider + 'static,
        model: ModelKernel,
    ) -> Self {
        Self::from_parts(reference, mask, model)
    }

    fn from_parts(
        reference: impl ReferenceProvider + 'static,
        mask: impl MaskProvider + 'static,
        model: ModelKernel,
    ) -> Self {
        Self {
            engine: ScoringEngine {
                reference: Box::new(reference),
                mask: Box::new(ProductionMask::new(mask)),
                kernel: Box::new(ProductionKernel(model)),
                last_accounting: InferenceAccounting::default(),
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

    #[must_use]
    pub fn model_representation(&self) -> ModelRepresentation {
        self.engine.kernel.representation()
    }

    #[must_use]
    pub fn last_model_accounting(&self) -> InferenceAccounting {
        self.engine.last_accounting
    }
}

/// One owned request at the lookup-first routing boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteRequest {
    variant: Grch38Variant,
    gene: Option<EnsemblGeneId>,
}

impl RouteRequest {
    pub const fn new(variant: Grch38Variant, gene: Option<EnsemblGeneId>) -> Self {
        Self { variant, gene }
    }

    pub const fn variant(&self) -> &Grch38Variant {
        &self.variant
    }

    pub const fn gene(&self) -> Option<EnsemblGeneId> {
        self.gene
    }
}

/// Validate the parts of a model request that depend only on the submitted
/// literal variant.
///
/// This boundary performs no lookup and touches no reference, mask, or model
/// provider. Reference-dependent eligibility remains part of scoring.
pub fn validate_model_request(variant: &Grch38Variant) -> Result<(), ModelRejection> {
    validate_model_request_shape(variant).map(|_| ())
}

fn validate_model_request_shape(variant: &Grch38Variant) -> Result<VariantShape, ModelRejection> {
    let shape = classify(variant)?;
    let reference_length = variant.reference().len();
    let alternate_length = variant.alternate().len();
    if reference_length > MAX_MODEL_ALLELE_BASES || alternate_length > MAX_MODEL_ALLELE_BASES {
        return Err(ModelRejection::AlleleTooLong {
            reference_length,
            alternate_length,
        });
    }
    let start_value = variant
        .position()
        .get()
        .checked_sub(FLANK)
        .ok_or(ModelRejection::InsufficientReferenceContext)?;
    GenomicPosition::new(start_value)
        .map(|_| shape)
        .map_err(|_| ModelRejection::InsufficientReferenceContext)
}

/// A request that did not have an authoritative precomputed result.
///
/// Values can be created only by [`LookupFirstRouter::inspect`], so model
/// completion necessarily consumes the exact variant and filter that were
/// inspected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelRequired {
    request: RouteRequest,
}

impl ModelRequired {
    pub const fn variant(&self) -> &Grch38Variant {
        self.request.variant()
    }

    pub const fn gene(&self) -> Option<EnsemblGeneId> {
        self.request.gene()
    }
}

/// A caller's explicit request to bypass precomputed lookup and use the model.
///
/// Unlike [`ModelRequired`], constructing this value does not require or
/// consult a [`ScoreProvider`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplicitModelRequest {
    request: RouteRequest,
}

impl ExplicitModelRequest {
    pub const fn new(request: RouteRequest) -> Self {
        Self { request }
    }

    pub const fn variant(&self) -> &Grch38Variant {
        self.request.variant()
    }

    pub const fn gene(&self) -> Option<EnsemblGeneId> {
        self.request.gene()
    }
}

/// The result of the cheap lookup-first decision.
#[derive(Clone, Debug, Eq, PartialEq)]
// Keeping the authoritative value inline avoids one heap allocation on every
// successful SNV hit, the highest-priority route.
#[allow(clippy::large_enum_variant)]
pub enum RouteDecision {
    Authoritative(RoutedResult),
    ModelRequired(ModelRequired),
}

/// Exact provenance bound to one concrete model fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelProvenance {
    model_bundle_id: String,
    model_profile: String,
    reference: ReferenceProvenance,
    mask_bytes: u64,
    mask_sha256: String,
    effective_cpu_policy: String,
}

impl ModelProvenance {
    pub fn new(
        model_bundle_id: String,
        model_profile: String,
        reference: ReferenceProvenance,
        mask_bytes: u64,
        mask_sha256: String,
    ) -> Self {
        Self {
            model_bundle_id,
            model_profile,
            reference,
            mask_bytes,
            mask_sha256,
            effective_cpu_policy: pangopup_model::CpuPolicy::production_default().to_string(),
        }
    }

    /// Bind the operational session policy used to produce this result.
    #[must_use]
    pub fn with_effective_cpu_policy(mut self, policy: impl Into<String>) -> Self {
        self.effective_cpu_policy = policy.into();
        self
    }

    pub const fn scoring_semantics(&self) -> &'static str {
        "pangopup-variant-score-v1"
    }

    pub fn model_bundle_id(&self) -> &str {
        &self.model_bundle_id
    }

    pub fn model_profile(&self) -> &str {
        &self.model_profile
    }

    pub const fn reference(&self) -> &ReferenceProvenance {
        &self.reference
    }

    pub const fn mask_bytes(&self) -> u64 {
        self.mask_bytes
    }

    pub fn mask_sha256(&self) -> &str {
        &self.mask_sha256
    }

    pub fn effective_cpu_policy(&self) -> &str {
        &self.effective_cpu_policy
    }

    pub const fn masked(&self) -> bool {
        true
    }

    pub const fn window(&self) -> u32 {
        DISTANCE as u32
    }
}

/// One complete routed answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoutedResult {
    Precomputed {
        variant: Grch38Variant,
        result: LookupResult,
    },
    Modeled {
        variant: Grch38Variant,
        records: Vec<ModelGeneScoreRecord>,
        provenance: ModelProvenance,
    },
}

impl RoutedResult {
    pub const fn variant(&self) -> &Grch38Variant {
        match self {
            Self::Precomputed { variant, .. } | Self::Modeled { variant, .. } => variant,
        }
    }

    pub const fn precomputed(&self) -> Option<&LookupResult> {
        match self {
            Self::Precomputed { result, .. } => Some(result),
            Self::Modeled { .. } => None,
        }
    }

    pub fn modeled_records(&self) -> Option<&[ModelGeneScoreRecord]> {
        match self {
            Self::Precomputed { .. } => None,
            Self::Modeled { records, .. } => Some(records),
        }
    }

    pub const fn model_provenance(&self) -> Option<&ModelProvenance> {
        match self {
            Self::Precomputed { .. } => None,
            Self::Modeled { provenance, .. } => Some(provenance),
        }
    }
}

/// The small lookup-first router over one precomputed score provider.
pub struct LookupFirstRouter<P> {
    provider: P,
}

impl<P: ScoreProvider> LookupFirstRouter<P> {
    pub const fn new(provider: P) -> Self {
        Self { provider }
    }

    /// Inspect a request without opening or invoking model components.
    pub fn inspect(&self, request: RouteRequest) -> Result<RouteDecision, LookupError> {
        let variant = request.variant();
        if variant.reference().len() == 1 && variant.alternate().len() == 1 {
            let reference = DnaBase::parse(variant.reference())
                .expect("Grch38Variant guarantees one uppercase A/C/G/T base");
            let alternate = DnaBase::parse(variant.alternate())
                .expect("Grch38Variant guarantees one uppercase A/C/G/T base");
            let snv = Grch38Snv::new(variant.contig(), variant.position(), reference, alternate)
                .expect("Grch38Variant guarantees distinct alleles");
            let result = self.provider.lookup(snv, request.gene())?;
            if !result.records().is_empty() || !result.source_reference_ambiguities().is_empty() {
                return Ok(RouteDecision::Authoritative(RoutedResult::Precomputed {
                    variant: request.variant,
                    result,
                }));
            }
        }
        Ok(RouteDecision::ModelRequired(ModelRequired { request }))
    }
}

/// Expected or operational failure while completing a model-required route.
#[derive(Debug)]
pub enum ModelFallbackError {
    Rejected(ModelRejection),
    Scoring(ModelScoringError),
}

impl fmt::Display for ModelFallbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(error) => write!(formatter, "model rejected variant: {error}"),
            Self::Scoring(error) => write!(formatter, "model scoring failed: {error}"),
        }
    }
}

impl std::error::Error for ModelFallbackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Rejected(_) => None,
            Self::Scoring(error) => Some(error),
        }
    }
}

/// One scorer and the identities captured from its exact concrete components.
pub struct ModelFallback {
    scorer: VariantScorer,
    provenance: ModelProvenance,
}

impl ModelFallback {
    pub fn new(
        reference: impl ReferenceProvider + 'static,
        mask: IdentifiedMaskDomains,
        model: ModelKernel,
    ) -> Self {
        let reference_provenance = reference.provenance().clone();
        let model_bundle_id = model.bundle_identity().to_string();
        let model_profile = model.profile().to_owned();
        let mask_identity = mask.identity();
        let provenance = ModelProvenance {
            model_bundle_id,
            model_profile,
            reference: reference_provenance,
            mask_bytes: mask_identity.bytes(),
            mask_sha256: format!("sha256:{}", mask_identity.sha256()),
            effective_cpu_policy: model.cpu_policy().to_string(),
        };
        Self {
            scorer: VariantScorer::new(reference, mask, model),
            provenance,
        }
    }

    pub const fn provenance(&self) -> &ModelProvenance {
        &self.provenance
    }

    /// Complete exactly one token emitted by the lookup-first router.
    pub fn complete(
        &mut self,
        required: ModelRequired,
    ) -> Result<RoutedResult, ModelFallbackError> {
        let filter = required.gene();
        let mut routed = self.complete_unfiltered(required)?;
        if let RoutedResult::Modeled { records, .. } = &mut routed
            && let Some(filter) = filter
        {
            records.retain(|record| record.gene().stable() == filter);
        }
        Ok(routed)
    }

    /// Complete one caller-requested model route without consulting lookup.
    pub fn complete_explicit(
        &mut self,
        request: ExplicitModelRequest,
    ) -> Result<RoutedResult, ModelFallbackError> {
        let filter = request.gene();
        let mut routed = self.complete_unfiltered_explicit(request)?;
        if let RoutedResult::Modeled { records, .. } = &mut routed
            && let Some(filter) = filter
        {
            records.retain(|record| record.gene().stable() == filter);
        }
        Ok(routed)
    }

    /// Complete one route without applying its optional stable-gene filter.
    ///
    /// Runtime composition uses this narrow seam to persist one reusable,
    /// complete modeled value and applies the request filter after retrieval.
    pub fn complete_unfiltered(
        &mut self,
        required: ModelRequired,
    ) -> Result<RoutedResult, ModelFallbackError> {
        self.complete_request_unfiltered(required.request)
    }

    /// Complete an explicit model route without applying its optional filter.
    pub fn complete_unfiltered_explicit(
        &mut self,
        request: ExplicitModelRequest,
    ) -> Result<RoutedResult, ModelFallbackError> {
        self.complete_request_unfiltered(request.request)
    }

    fn complete_request_unfiltered(
        &mut self,
        request: RouteRequest,
    ) -> Result<RoutedResult, ModelFallbackError> {
        let result = self
            .scorer
            .score(request.variant())
            .map_err(ModelFallbackError::Scoring)?;
        let records = match result {
            ModelScoreResult::Scored(records) => records,
            ModelScoreResult::Rejected(rejection) => {
                return Err(ModelFallbackError::Rejected(rejection));
            }
        };
        Ok(RoutedResult::Modeled {
            variant: request.variant,
            records,
            provenance: self.provenance.clone(),
        })
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

trait GeneMaskSource: Send {
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

trait RawKernel: Send {
    fn infer(&mut self, context: &[u8], strand: Strand) -> Result<RawScores, ModelScoringError>;

    fn representation(&self) -> ModelRepresentation {
        ModelRepresentation::Singleton
    }

    fn infer_variant(
        &mut self,
        _reference: &[u8],
        _alternate: &[u8],
        _strands: &[Strand],
    ) -> Result<(Vec<(RawScores, RawScores)>, InferenceAccounting), ModelScoringError> {
        Err(ModelScoringError::ModelProvider)
    }
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

    fn representation(&self) -> ModelRepresentation {
        self.0.representation()
    }

    fn infer_variant(
        &mut self,
        reference: &[u8],
        alternate: &[u8],
        strands: &[Strand],
    ) -> Result<(Vec<(RawScores, RawScores)>, InferenceAccounting), ModelScoringError> {
        let reference =
            ModelContext::new(reference).map_err(|_| ModelScoringError::ModelProvider)?;
        let alternate =
            ModelContext::new(alternate).map_err(|_| ModelScoringError::ModelProvider)?;
        let pairs = strands
            .iter()
            .map(|strand| StrandPair {
                reference: &reference,
                alternate: &alternate,
                strand: *strand,
            })
            .collect::<Vec<_>>();
        let result = self
            .0
            .infer_variant(&pairs)
            .map_err(|_| ModelScoringError::ModelProvider)?;
        let accounting = result.accounting();
        Ok((
            result
                .into_pairs()
                .into_iter()
                .map(|pair| {
                    let (reference, alternate) = pair.into_parts();
                    (
                        RawScores::from_replicates(reference),
                        RawScores::from_replicates(alternate),
                    )
                })
                .collect(),
            accounting,
        ))
    }
}

struct ScoringEngine {
    reference: Box<dyn ReferenceProvider>,
    mask: Box<dyn GeneMaskSource>,
    kernel: Box<dyn RawKernel>,
    last_accounting: InferenceAccounting,
}

impl ScoringEngine {
    fn score(&mut self, variant: &Grch38Variant) -> Result<ModelScoreResult, ModelScoringError> {
        self.last_accounting = InferenceAccounting::default();
        let shape = match validate_model_request_shape(variant) {
            Ok(shape) => shape,
            Err(rejection) => return Ok(ModelScoreResult::rejected(rejection)),
        };
        let reference_length = variant.reference().len();
        let alternate_length = variant.alternate().len();
        let start = GenomicPosition::new(
            variant
                .position()
                .get()
                .checked_sub(FLANK)
                .expect("validated model request has a left-context start"),
        )
        .expect("validated model request has a nonzero left-context start");
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
        if self.kernel.representation() == ModelRepresentation::Singleton {
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
            let active_strands =
                usize::from(!genes.plus.is_empty()) + usize::from(!genes.minus.is_empty());
            self.last_accounting = InferenceAccounting {
                session_invocations: active_strands * 2,
                logical_context_evaluations: active_strands * 2,
                batch_size: 1,
                padded_input_elements: 0,
            };
        } else {
            let mut strands = Vec::with_capacity(2);
            if !genes.plus.is_empty() {
                strands.push(Strand::Plus);
            }
            if !genes.minus.is_empty() {
                strands.push(Strand::Minus);
            }
            let (pairs, accounting) = self
                .kernel
                .infer_variant(&reference, &alternate, &strands)?;
            self.last_accounting = accounting;
            if pairs.len() != strands.len() {
                return Err(ModelScoringError::InvalidModelOutput);
            }
            for ((reference_scores, alternate_scores), strand) in pairs.into_iter().zip(strands) {
                self.validate_pair(&reference, &alternate, &reference_scores, &alternate_scores)?;
                let arrays = post_ensemble(shape, &reference_scores, &alternate_scores)?;
                let strand_genes = match strand {
                    Strand::Plus => &genes.plus,
                    Strand::Minus => &genes.minus,
                };
                records.extend(score_genes(arrays, strand_genes, variant.position())?);
            }
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

    fn validate_pair(
        &self,
        reference: &[u8],
        alternate: &[u8],
        reference_scores: &RawScores,
        alternate_scores: &RawScores,
    ) -> Result<(), ModelScoringError> {
        reference_scores.validate()?;
        alternate_scores.validate()?;
        if reference_scores.score_length != reference.len() - CONTEXT_FLANKS
            || alternate_scores.score_length != alternate.len() - CONTEXT_FLANKS
        {
            return Err(ModelScoringError::InvalidModelOutput);
        }
        Ok(())
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
