//! Streaming validation and characterization of published Pangolin score files.

use flate2::bufread::GzDecoder;
use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, PangolinScore, RelativePosition,
    ScoreMagnitude,
};
use pangopup_index::{
    AmbiguousInputLocus, IndexError, IndexReader, InputAlternative, InputLocus, OrdinaryInputLocus,
    WriteSummary, write_index,
};
use sha2::{Digest, Sha256};
use std::{
    convert::Infallible,
    ffi::OsString,
    fmt,
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

pub mod compatibility;
mod production;
pub use production::{BuildOutcome, CommandError, VerifyOutcome, build_bundle, verify_bundle};

pub const SOURCE_HEADER: &str = "chrom\tpos\tref\talt\tgain_score\tgain_pos\tloss_score\tloss_pos";
/// Maximum decompressed header bytes, including an optional line ending.
pub const MAX_SOURCE_HEADER_BYTES: usize = 128;
/// Maximum decompressed data-row bytes, including an optional line ending.
pub const MAX_SOURCE_ROW_BYTES: usize = 256;

/// A source validation or I/O failure with member and optional line context.
#[derive(Debug)]
pub struct SourceError {
    member: String,
    line: Option<u64>,
    reason: String,
}

impl SourceError {
    fn member(member: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            member: member.into(),
            line: None,
            reason: reason.into(),
        }
    }

    fn line(member: &str, line: u64, reason: impl Into<String>) -> Self {
        Self {
            member: member.to_owned(),
            line: Some(line),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}:{line}: {}", self.member, self.reason),
            None => write!(f, "{}: {}", self.member, self.reason),
        }
    }
}

impl std::error::Error for SourceError {}

/// Distinguishes source failures from a fallible streaming consumer's failure.
#[derive(Debug)]
pub enum InspectMemberError<E> {
    Source(SourceError),
    Visitor(E),
}

impl<E> From<SourceError> for InspectMemberError<E> {
    fn from(error: SourceError) -> Self {
        Self::Source(error)
    }
}

impl<E: fmt::Display> fmt::Display for InspectMemberError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(f),
            Self::Visitor(error) => write!(f, "source visitor failed: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for InspectMemberError<E> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceDirection {
    Ascending,
    Descending,
}

impl fmt::Display for SourceDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        })
    }
}

/// One alternate and its score within an ordinary source locus.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlleleScore {
    pub alternate: DnaBase,
    pub score: PangolinScore,
}

/// One complete three-alternate source locus that can represent public SNVs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrdinarySourceLocus {
    pub gene: EnsemblGeneId,
    pub contig: Grch38Contig,
    pub position: GenomicPosition,
    pub reference: DnaBase,
    pub alternatives: [AlleleScore; 3],
}

/// The published, build-only `REF=N` exception shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmbiguousReferenceLocus {
    pub gene: EnsemblGeneId,
    pub contig: Grch38Contig,
    pub position: GenomicPosition,
    pub alternatives: [AlleleScore; 3],
    pub omitted: DnaBase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLocus {
    Ordinary(OrdinarySourceLocus),
    AmbiguousReference(AmbiguousReferenceLocus),
}

/// Receives validated loci immediately in source order.
pub trait SourceVisitor {
    type Error;

    fn visit(&mut self, locus: &SourceLocus) -> Result<(), Self::Error>;
}

impl<F, E> SourceVisitor for F
where
    F: FnMut(&SourceLocus) -> Result<(), E>,
{
    type Error = E;

    fn visit(&mut self, locus: &SourceLocus) -> Result<(), Self::Error> {
        self(locus)
    }
}

struct Ignore;

impl SourceVisitor for Ignore {
    type Error = Infallible;

    fn visit(&mut self, _locus: &SourceLocus) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Canonical characterization of one source member.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileSummary {
    pub gene: EnsemblGeneId,
    pub contig: Grch38Contig,
    pub direction: SourceDirection,
    pub first: u32,
    pub last: u32,
    pub rows: u64,
    pub loci: u64,
    pub segments: u64,
    pub gaps: u64,
    pub omitted_bases: u64,
    pub ambiguous_ref_loci: u64,
    pub n_omit_a: u64,
    pub n_omit_t: u64,
}

impl fmt::Display for FileSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "file gene={} contig={} direction={} first={} last={} rows={} loci={} segments={} gaps={} omitted_bases={} ambiguous_ref_loci={} n_omit_a={} n_omit_t={}",
            self.gene,
            self.contig,
            self.direction,
            self.first,
            self.last,
            self.rows,
            self.loci,
            self.segments,
            self.gaps,
            self.omitted_bases,
            self.ambiguous_ref_loci,
            self.n_omit_a,
            self.n_omit_t
        )
    }
}

/// Corpus-wide aggregate; all counters deliberately use `u64`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TotalSummary {
    pub genes: u64,
    pub rows: u64,
    pub loci: u64,
    pub ascending: u64,
    pub descending: u64,
    pub segments: u64,
    pub gaps: u64,
    pub omitted_bases: u64,
    pub ambiguous_ref_loci: u64,
    pub n_omit_a: u64,
    pub n_omit_t: u64,
}

impl TotalSummary {
    fn add(&mut self, file: FileSummary) {
        self.genes += 1;
        self.rows += file.rows;
        self.loci += file.loci;
        match file.direction {
            SourceDirection::Ascending => self.ascending += 1,
            SourceDirection::Descending => self.descending += 1,
        }
        self.segments += file.segments;
        self.gaps += file.gaps;
        self.omitted_bases += file.omitted_bases;
        self.ambiguous_ref_loci += file.ambiguous_ref_loci;
        self.n_omit_a += file.n_omit_a;
        self.n_omit_t += file.n_omit_t;
    }
}

impl fmt::Display for TotalSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "total genes={} rows={} loci={} ascending={} descending={} segments={} gaps={} omitted_bases={} ambiguous_ref_loci={} n_omit_a={} n_omit_t={}",
            self.genes,
            self.rows,
            self.loci,
            self.ascending,
            self.descending,
            self.segments,
            self.gaps,
            self.omitted_bases,
            self.ambiguous_ref_loci,
            self.n_omit_a,
            self.n_omit_t
        )
    }
}

/// Discover, validate, and characterize all direct source members.
pub fn inspect_directory(
    source_dir: &Path,
    output: &mut dyn Write,
) -> Result<TotalSummary, SourceError> {
    let mut members = discover_members(source_dir)?;
    if members.is_empty() {
        return Err(SourceError::member(
            source_dir.display().to_string(),
            "source directory contains no .tsv.gz members",
        ));
    }
    members.sort_by(|left, right| left.0.cmp(&right.0));

    let mut total = TotalSummary::default();
    for (filename, path) in members {
        let filename = filename
            .into_string()
            .map_err(|_| SourceError::member("<non-UTF-8 filename>", "invalid source filename"))?;
        let gene = parse_member_gene(&filename)?;
        let summary = match inspect_member(&path, &filename, gene, &mut Ignore) {
            Ok(summary) => summary,
            Err(InspectMemberError::Source(error)) => return Err(error),
            Err(InspectMemberError::Visitor(never)) => match never {},
        };
        writeln!(output, "{summary}")
            .map_err(|error| SourceError::member("<stdout>", error.to_string()))?;
        total.add(summary);
    }
    writeln!(output, "{total}")
        .map_err(|error| SourceError::member("<stdout>", error.to_string()))?;
    Ok(total)
}

/// Discover and validate all members while streaming loci to one visitor.
pub fn visit_directory<V: SourceVisitor + ?Sized>(
    source_dir: &Path,
    visitor: &mut V,
) -> Result<TotalSummary, InspectMemberError<V::Error>> {
    let mut members = discover_members(source_dir).map_err(InspectMemberError::Source)?;
    if members.is_empty() {
        return Err(InspectMemberError::Source(SourceError::member(
            source_dir.display().to_string(),
            "source directory contains no .tsv.gz members",
        )));
    }
    members.sort_by(|left, right| left.0.cmp(&right.0));
    let mut total = TotalSummary::default();
    for (filename, path) in members {
        let filename = filename.into_string().map_err(|_| {
            InspectMemberError::Source(SourceError::member(
                "<non-UTF-8 filename>",
                "invalid source filename",
            ))
        })?;
        let gene = parse_member_gene(&filename).map_err(InspectMemberError::Source)?;
        let summary = inspect_member(&path, &filename, gene, visitor)?;
        total.add(summary);
    }
    Ok(total)
}

/// Result of the bounded developer/admin prototype round trip.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrototypeSummary {
    pub source: TotalSummary,
    pub artifact: WriteSummary,
}

/// Source, index, or output failure from the prototype command.
#[derive(Debug)]
pub enum PrototypeError {
    Source(SourceError),
    Index(IndexError),
}

impl fmt::Display for PrototypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(f),
            Self::Index(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for PrototypeError {}

/// Build, reopen, and exhaustively verify one checked source directory.
pub fn prototype_roundtrip(
    source_dir: &Path,
    output: &Path,
) -> Result<PrototypeSummary, PrototypeError> {
    let mut loci = Vec::new();
    let source = visit_directory(source_dir, &mut |locus: &SourceLocus| {
        loci.push(index_locus(*locus));
        Ok::<_, Infallible>(())
    })
    .map_err(|error| match error {
        InspectMemberError::Source(error) => PrototypeError::Source(error),
        InspectMemberError::Visitor(never) => match never {},
    })?;
    let artifact = write_index(output, &loci).map_err(PrototypeError::Index)?;
    let reader = IndexReader::open(output).map_err(PrototypeError::Index)?;
    reader.verify_exact(&loci).map_err(PrototypeError::Index)?;
    Ok(PrototypeSummary { source, artifact })
}

/// Collect a bounded validated corpus for prototype tests and benchmarks.
/// Full-corpus production builds use a streaming writer in the following slice.
pub fn collect_index_loci(
    source_dir: &Path,
) -> Result<(TotalSummary, Vec<InputLocus>), SourceError> {
    let mut loci = Vec::new();
    let summary = visit_directory(source_dir, &mut |locus: &SourceLocus| {
        loci.push(index_locus(*locus));
        Ok::<_, Infallible>(())
    })
    .map_err(|error| match error {
        InspectMemberError::Source(error) => error,
        InspectMemberError::Visitor(never) => match never {},
    })?;
    Ok((summary, loci))
}

const BENCHMARK_REQUIRED_GENES: [&str; 6] = [
    "ENSG00000010610",
    "ENSG00000141499",
    "ENSG00000141510",
    "ENSG00000169129",
    "ENSG00000175727",
    "ENSG00000185974",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchmarkCorpusSummary {
    pub selected_genes: u64,
    pub loci: u64,
    pub rows: u64,
    pub observed_member_sha256: String,
}

#[derive(Debug)]
pub struct BenchmarkCorpusError(String);

impl fmt::Display for BenchmarkCorpusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BenchmarkCorpusError {}

struct MemberChoice {
    name: String,
    path: PathBuf,
    bytes: u64,
    direction: SourceDirection,
    quartile: usize,
    selection: &'static str,
}

/// Build the deterministic stratified logical corpus consumed by the benchmark.
pub fn prepare_benchmark_corpus(
    source_dir: &Path,
    output: &Path,
    manifest: &Path,
) -> Result<BenchmarkCorpusSummary, BenchmarkCorpusError> {
    let discovered = discover_members(source_dir)
        .map_err(|error| BenchmarkCorpusError(error.to_string()))?
        .into_iter()
        .map(|(name, path)| {
            let name = name
                .into_string()
                .map_err(|_| BenchmarkCorpusError("non-UTF-8 member name".to_owned()))?;
            let bytes = fs::metadata(&path)
                .map_err(|error| BenchmarkCorpusError(error.to_string()))?
                .len();
            let direction = quick_direction(&path, &name)?;
            Ok((name, path, bytes, direction))
        })
        .collect::<Result<Vec<_>, BenchmarkCorpusError>>()?;
    if discovered.len() != 19_913 {
        return Err(BenchmarkCorpusError(format!(
            "expected 19913 source members, found {}",
            discovered.len()
        )));
    }
    let mut by_size: Vec<_> = (0..discovered.len()).collect();
    by_size.sort_by_key(|index| (discovered[*index].2, discovered[*index].0.clone()));
    let discovered_len = discovered.len();
    let mut quartiles = vec![0_usize; discovered_len];
    for (rank, original) in by_size.into_iter().enumerate() {
        quartiles[original] = (rank * 4 / discovered_len).min(3);
    }
    let mut strata: [Vec<usize>; 8] = std::array::from_fn(|_| Vec::new());
    for (index, member) in discovered.iter().enumerate() {
        if BENCHMARK_REQUIRED_GENES
            .iter()
            .any(|gene| member.0 == format!("{gene}.tsv.gz"))
        {
            continue;
        }
        let direction = usize::from(member.3 == SourceDirection::Descending);
        strata[direction * 4 + quartiles[index]].push(index);
    }
    let mut selected = Vec::with_capacity(134);
    for gene in BENCHMARK_REQUIRED_GENES {
        let name = format!("{gene}.tsv.gz");
        let index = discovered
            .iter()
            .position(|member| member.0 == name)
            .ok_or_else(|| BenchmarkCorpusError(format!("missing required member {name}")))?;
        let (name, path, bytes, direction) = discovered[index].clone();
        selected.push(MemberChoice {
            name,
            path,
            bytes,
            direction,
            quartile: quartiles[index],
            selection: "required-edge",
        });
    }
    for stratum in &mut strata {
        stratum.sort_by(|left, right| discovered[*left].0.cmp(&discovered[*right].0));
        if stratum.len() < 16 {
            return Err(BenchmarkCorpusError(
                "stratum contains fewer than 16 genes".to_owned(),
            ));
        }
        for sample in 0..16 {
            let position = sample * (stratum.len() - 1) / 15;
            let index = stratum[position];
            let (name, path, bytes, direction) = discovered[index].clone();
            selected.push(MemberChoice {
                name,
                path,
                bytes,
                direction,
                quartile: quartiles[index],
                selection: "stratified",
            });
        }
    }
    selected.sort_by(|left, right| left.name.cmp(&right.name));
    if selected.len() != 134 {
        return Err(BenchmarkCorpusError(
            "selected gene count is not 134".to_owned(),
        ));
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| BenchmarkCorpusError(error.to_string()))?;
    }
    let mut corpus =
        File::create(output).map_err(|error| BenchmarkCorpusError(error.to_string()))?;
    corpus
        .write_all(b"PGLOG001\0\0\0\0\0\0\0\0")
        .map_err(|error| BenchmarkCorpusError(error.to_string()))?;
    let mut digest = Sha256::new();
    let mut loci = 0_u64;
    let mut rows = 0_u64;
    for choice in &selected {
        let name = choice.name.as_bytes();
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name);
        digest.update(choice.bytes.to_le_bytes());
        let mut member =
            File::open(&choice.path).map_err(|error| BenchmarkCorpusError(error.to_string()))?;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = member
                .read(&mut buffer)
                .map_err(|error| BenchmarkCorpusError(error.to_string()))?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
        let gene_text = choice
            .name
            .strip_suffix(".tsv.gz")
            .ok_or_else(|| BenchmarkCorpusError("invalid selected member name".to_owned()))?;
        let gene = EnsemblGeneId::from_str(gene_text)
            .map_err(|error| BenchmarkCorpusError(error.to_string()))?;
        let summary = inspect_member(
            &choice.path,
            &choice.name,
            gene,
            &mut |source: &SourceLocus| write_logical_locus(&mut corpus, index_locus(*source)),
        )
        .map_err(|error| BenchmarkCorpusError(error.to_string()))?;
        loci = loci
            .checked_add(summary.loci)
            .ok_or_else(|| BenchmarkCorpusError("locus count overflow".to_owned()))?;
        rows = rows
            .checked_add(summary.rows)
            .ok_or_else(|| BenchmarkCorpusError("row count overflow".to_owned()))?;
    }
    corpus
        .seek(SeekFrom::Start(8))
        .and_then(|_| corpus.write_all(&loci.to_le_bytes()))
        .and_then(|_| corpus.sync_all())
        .map_err(|error| BenchmarkCorpusError(error.to_string()))?;

    let observed_member_sha256 = format!("{:x}", digest.finalize());
    if let Some(parent) = manifest.parent() {
        fs::create_dir_all(parent).map_err(|error| BenchmarkCorpusError(error.to_string()))?;
    }
    let mut report =
        File::create(manifest).map_err(|error| BenchmarkCorpusError(error.to_string()))?;
    writeln!(report, "# Selected gene manifest")
        .and_then(|_| writeln!(report, "# source_doi=10.5281/zenodo.15649338"))
        .and_then(|_| writeln!(report, "# archive=Pangolin_hg38_snvs_masked.zip"))
        .and_then(|_| writeln!(report, "# published_bytes=12988141317"))
        .and_then(|_| writeln!(report, "# published_md5=679ef0b50e511b6102b4b88fbf811108"))
        .and_then(|_| writeln!(report, "# observed_member_sha256={observed_member_sha256}"))
        .and_then(|_| {
            writeln!(
                report,
                "gene\tdirection\tsize_quartile\tcompressed_bytes\tselection"
            )
        })
        .map_err(|error| BenchmarkCorpusError(error.to_string()))?;
    for choice in &selected {
        writeln!(
            report,
            "{}\t{}\t{}\t{}\t{}",
            choice.name.trim_end_matches(".tsv.gz"),
            choice.direction,
            choice.quartile + 1,
            choice.bytes,
            choice.selection
        )
        .map_err(|error| BenchmarkCorpusError(error.to_string()))?;
    }
    Ok(BenchmarkCorpusSummary {
        selected_genes: selected.len() as u64,
        loci,
        rows,
        observed_member_sha256,
    })
}

fn quick_direction(path: &Path, member: &str) -> Result<SourceDirection, BenchmarkCorpusError> {
    let file = File::open(path).map_err(|error| BenchmarkCorpusError(error.to_string()))?;
    let mut reader = BufReader::new(GzDecoder::new(BufReader::new(file)));
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|error| BenchmarkCorpusError(error.to_string()))?;
    let mut positions = Vec::with_capacity(2);
    while positions.len() < 2 {
        line.clear();
        if reader
            .read_line(&mut line)
            .map_err(|error| BenchmarkCorpusError(error.to_string()))?
            == 0
        {
            return Err(BenchmarkCorpusError(format!(
                "{member}: fewer than two loci"
            )));
        }
        let position = line
            .split('\t')
            .nth(1)
            .and_then(|value| value.parse::<u32>().ok())
            .ok_or_else(|| BenchmarkCorpusError(format!("{member}: invalid position")))?;
        if positions.last().copied() != Some(position) {
            positions.push(position);
        }
    }
    Ok(if positions[1] > positions[0] {
        SourceDirection::Ascending
    } else {
        SourceDirection::Descending
    })
}

fn write_logical_locus(output: &mut File, locus: InputLocus) -> Result<(), io::Error> {
    let (kind, gene, contig, position, allele, alternatives) = match locus {
        InputLocus::Ordinary(locus) => (
            0_u8,
            locus.gene,
            locus.contig,
            locus.position,
            locus.reference,
            locus.alternatives,
        ),
        InputLocus::Ambiguous(locus) => (
            1_u8,
            locus.gene,
            locus.contig,
            locus.position,
            locus.omitted,
            locus.alternatives,
        ),
    };
    let mut record = [0_u8; 32];
    record[0] = kind;
    record[1] = contig.code();
    record[2] = match allele {
        DnaBase::A => 0,
        DnaBase::C => 1,
        DnaBase::G => 2,
        DnaBase::T => 3,
    };
    record[4..12].copy_from_slice(&gene.numeric().to_le_bytes());
    record[12..16].copy_from_slice(&position.get().to_le_bytes());
    for (index, alternative) in alternatives.iter().enumerate() {
        let offset = 16 + index * 5;
        record[offset] = match alternative.alternate {
            DnaBase::A => 0,
            DnaBase::C => 1,
            DnaBase::G => 2,
            DnaBase::T => 3,
        };
        record[offset + 1] = alternative.score.gain().hundredths();
        record[offset + 2] = (alternative.score.gain_position().get() as i16 + 50) as u8;
        record[offset + 3] = alternative.score.loss().hundredths();
        record[offset + 4] = (alternative.score.loss_position().get() as i16 + 50) as u8;
    }
    output.write_all(&record)
}

/// Perform the cheap structural open used by the corrupt-artifact spec.
pub fn prototype_open(path: &Path) -> Result<u64, IndexError> {
    IndexReader::open(path).map(|reader| reader.file_len())
}

fn index_locus(locus: SourceLocus) -> InputLocus {
    match locus {
        SourceLocus::Ordinary(locus) => {
            let mut alternatives = locus.alternatives.map(|value| InputAlternative {
                alternate: value.alternate,
                score: value.score,
            });
            alternatives.sort_by_key(|value| value.alternate);
            InputLocus::Ordinary(OrdinaryInputLocus {
                gene: locus.gene,
                contig: locus.contig,
                position: locus.position,
                reference: locus.reference,
                alternatives,
            })
        }
        SourceLocus::AmbiguousReference(locus) => {
            let mut alternatives = locus.alternatives.map(|value| InputAlternative {
                alternate: value.alternate,
                score: value.score,
            });
            alternatives.sort_by_key(|value| value.alternate);
            InputLocus::Ambiguous(AmbiguousInputLocus {
                gene: locus.gene,
                contig: locus.contig,
                position: locus.position,
                alternatives,
                omitted: locus.omitted,
            })
        }
    }
}

/// Validate one already-identified member and stream its loci to a visitor.
pub fn inspect_member<V: SourceVisitor + ?Sized>(
    path: &Path,
    member: &str,
    gene: EnsemblGeneId,
    visitor: &mut V,
) -> Result<FileSummary, InspectMemberError<V::Error>> {
    inspect_member_inner(path, member, gene, visitor, None)
}

pub(crate) fn inspect_member_hashed<V: SourceVisitor + ?Sized>(
    path: &Path,
    member: &str,
    gene: EnsemblGeneId,
    visitor: &mut V,
    hash: &mut Sha256,
) -> Result<FileSummary, InspectMemberError<V::Error>> {
    inspect_member_inner(path, member, gene, visitor, Some(hash))
}

struct MemberReader<'a> {
    inner: File,
    hash: Option<&'a mut Sha256>,
    bytes: u64,
}

impl Read for MemberReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        if let Some(hash) = self.hash.as_deref_mut() {
            hash.update(&buffer[..read]);
        }
        self.bytes = self
            .bytes
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::other("source member byte count overflow"))?;
        Ok(read)
    }
}

fn inspect_member_inner<V: SourceVisitor + ?Sized>(
    path: &Path,
    member: &str,
    gene: EnsemblGeneId,
    visitor: &mut V,
    mut hash: Option<&mut Sha256>,
) -> Result<FileSummary, InspectMemberError<V::Error>> {
    let file = File::open(path).map_err(|error| SourceError::member(member, error.to_string()))?;
    let expected_size = file
        .metadata()
        .map_err(|error| SourceError::member(member, error.to_string()))?
        .len();
    if let Some(hash) = hash.as_deref_mut() {
        hash.update((member.len() as u64).to_le_bytes());
        hash.update(member.as_bytes());
        hash.update(expected_size.to_le_bytes());
    }
    let compressed = BufReader::new(MemberReader {
        inner: file,
        hash,
        bytes: 0,
    });
    let decoder = GzDecoder::new(compressed);
    let mut reader = BufReader::new(decoder);
    let mut line = Vec::with_capacity(MAX_SOURCE_ROW_BYTES);

    let header_bytes = read_bounded_line(
        &mut reader,
        &mut line,
        MAX_SOURCE_HEADER_BYTES,
        member,
        1,
        "header",
    )?;
    if header_bytes == 0 {
        return Err(SourceError::line(member, 1, "missing source header").into());
    }
    let header = source_text(&line, member, 1)?;
    validate_line_ending(&line, member, 1)?;
    if strip_line_ending(header) != SOURCE_HEADER {
        return Err(SourceError::line(
            member,
            1,
            "header does not match the eight-column source contract",
        )
        .into());
    }

    let mut state = FileState::new(gene);
    let mut line_number = 1_u64;
    loop {
        let bytes = read_bounded_line(
            &mut reader,
            &mut line,
            MAX_SOURCE_ROW_BYTES,
            member,
            line_number + 1,
            "row",
        )?;
        if bytes == 0 {
            break;
        }
        line_number += 1;
        validate_line_ending(&line, member, line_number)?;
        let text = source_text(&line, member, line_number)?;
        let row = parse_row(strip_line_ending(text), member, line_number)?;
        state.push(row, member, line_number, visitor)?;
    }
    let decoder = reader.into_inner();
    let mut compressed = decoder.into_inner();
    let trailing = compressed
        .fill_buf()
        .map_err(|error| SourceError::member(member, error.to_string()))?;
    if !trailing.is_empty() {
        let reason = if trailing.starts_with(&[0x1f, 0x8b]) {
            "concatenated gzip members are not permitted"
        } else {
            "trailing bytes after the gzip member are not permitted"
        };
        return Err(SourceError::member(member, reason).into());
    }
    let compressed = compressed.into_inner();
    if compressed.bytes != expected_size {
        return Err(SourceError::member(
            member,
            "source member changed length while it was being read",
        )
        .into());
    }
    state.finish(member, visitor)
}

fn validate_line_ending(bytes: &[u8], member: &str, line: u64) -> Result<(), SourceError> {
    let allowed_cr = bytes.ends_with(b"\r\n").then_some(bytes.len() - 2);
    if bytes
        .iter()
        .enumerate()
        .any(|(index, byte)| *byte == b'\r' && Some(index) != allowed_cr)
    {
        return Err(SourceError::line(
            member,
            line,
            "bare carriage return is not a permitted line ending",
        ));
    }
    Ok(())
}

fn discover_members(source_dir: &Path) -> Result<Vec<(OsString, PathBuf)>, SourceError> {
    let entries = fs::read_dir(source_dir).map_err(|error| {
        SourceError::member(source_dir.display().to_string(), error.to_string())
    })?;
    let mut members = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            SourceError::member(source_dir.display().to_string(), error.to_string())
        })?;
        let filename = entry.file_name();
        let is_tsv_gz = filename.to_string_lossy().ends_with(".tsv.gz");
        if !is_tsv_gz {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|error| SourceError::member(filename.to_string_lossy(), error.to_string()))?;
        if file_type.is_symlink() {
            return Err(SourceError::member(
                filename.to_string_lossy(),
                "source member must be a direct regular file, not a symlink",
            ));
        }
        if file_type.is_file() {
            members.push((filename, entry.path()));
        }
    }
    Ok(members)
}

fn parse_member_gene(filename: &str) -> Result<EnsemblGeneId, SourceError> {
    let gene = filename.strip_suffix(".tsv.gz").unwrap_or(filename);
    if format!("{gene}.tsv.gz") != filename {
        return Err(SourceError::member(filename, "invalid source filename"));
    }
    EnsemblGeneId::from_str(gene).map_err(|_| {
        SourceError::member(
            filename,
            "filename must be exactly ENSG followed by 11 digits and .tsv.gz",
        )
    })
}

fn read_bounded_line(
    reader: &mut impl BufRead,
    line_buffer: &mut Vec<u8>,
    limit: usize,
    member: &str,
    line: u64,
    kind: &str,
) -> Result<usize, SourceError> {
    line_buffer.clear();
    loop {
        let available = reader
            .fill_buf()
            .map_err(|error| SourceError::line(member, line, error.to_string()))?;
        if available.is_empty() {
            return Ok(line_buffer.len());
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |index| index + 1);
        if line_buffer.len().saturating_add(take) > limit {
            return Err(SourceError::line(
                member,
                line,
                format!("source {kind} exceeds {limit} bytes"),
            ));
        }
        line_buffer.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline.is_some() {
            return Ok(line_buffer.len());
        }
    }
}

fn source_text<'a>(bytes: &'a [u8], member: &str, line: u64) -> Result<&'a str, SourceError> {
    std::str::from_utf8(bytes)
        .map_err(|_| SourceError::line(member, line, "source line is not valid UTF-8"))
}

fn strip_line_ending(text: &str) -> &str {
    let without_newline = text.strip_suffix('\n').unwrap_or(text);
    without_newline
        .strip_suffix('\r')
        .unwrap_or(without_newline)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReferenceBase {
    Concrete(DnaBase),
    Ambiguous,
}

impl fmt::Display for ReferenceBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Concrete(base) => base.fmt(f),
            Self::Ambiguous => f.write_str("N"),
        }
    }
}

#[derive(Clone, Copy)]
struct ParsedRow {
    contig: Grch38Contig,
    position: GenomicPosition,
    reference: ReferenceBase,
    value: AlleleScore,
}

fn parse_row(text: &str, member: &str, line: u64) -> Result<ParsedRow, SourceError> {
    let mut fields = text.split('\t');
    let values: [&str; 8] = std::array::from_fn(|_| fields.next().unwrap_or(""));
    if fields.next().is_some() || values.iter().any(|field| field.is_empty()) {
        return Err(SourceError::line(
            member,
            line,
            "expected exactly eight nonempty tab-separated fields",
        ));
    }

    let contig =
        parse_source_contig(values[0]).map_err(|reason| SourceError::line(member, line, reason))?;
    let position_value = values[1].parse::<u32>().map_err(|_| {
        SourceError::line(
            member,
            line,
            format!("invalid one-based position {}", values[1]),
        )
    })?;
    let position = GenomicPosition::new(position_value)
        .map_err(|error| SourceError::line(member, line, error.to_string()))?;
    let reference = if values[2] == "N" {
        ReferenceBase::Ambiguous
    } else {
        ReferenceBase::Concrete(
            DnaBase::parse(values[2])
                .map_err(|error| SourceError::line(member, line, error.to_string()))?,
        )
    };
    let alternate = DnaBase::parse(values[3])
        .map_err(|error| SourceError::line(member, line, error.to_string()))?;
    if reference == ReferenceBase::Concrete(alternate) {
        return Err(SourceError::line(
            member,
            line,
            "reference and alternate bases must differ",
        ));
    }

    let gain = parse_score(values[4], ScoreKind::Gain)
        .map_err(|reason| SourceError::line(member, line, reason))?;
    let gain_position =
        parse_relative(values[5]).map_err(|reason| SourceError::line(member, line, reason))?;
    let loss = parse_score(values[6], ScoreKind::Loss)
        .map_err(|reason| SourceError::line(member, line, reason))?;
    let loss_position =
        parse_relative(values[7]).map_err(|reason| SourceError::line(member, line, reason))?;

    Ok(ParsedRow {
        contig,
        position,
        reference,
        value: AlleleScore {
            alternate,
            score: PangolinScore::new(gain, gain_position, loss, loss_position),
        },
    })
}

fn parse_source_contig(text: &str) -> Result<Grch38Contig, String> {
    let canonical = matches!(text, "chrX" | "chrY" | "chrM")
        || text.strip_prefix("chr").is_some_and(|digits| {
            !digits.is_empty()
                && !digits.starts_with('0')
                && digits.bytes().all(|byte| byte.is_ascii_digit())
        });
    if !canonical {
        return Err(format!(
            "source contig must use canonical chr1..chr22, chrX, chrY, or chrM spelling, got {text}"
        ));
    }
    text.parse::<Grch38Contig>().map_err(|_| {
        format!(
            "source contig must use canonical chr1..chr22, chrX, chrY, or chrM spelling, got {text}"
        )
    })
}

#[derive(Clone, Copy)]
enum ScoreKind {
    Gain,
    Loss,
}

fn parse_score(text: &str, kind: ScoreKind) -> Result<ScoreMagnitude, String> {
    let (negative, unsigned) = match text.strip_prefix('-') {
        Some(unsigned) => (true, unsigned),
        None => (false, text),
    };
    if unsigned.is_empty() || unsigned.starts_with('+') {
        return Err(format!("invalid exact hundredth score {text}"));
    }
    let mut parts = unsigned.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some()
        || whole.len() != 1
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.is_some_and(|part| {
            part.is_empty() || part.len() > 2 || !part.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return Err(format!("invalid exact hundredth score {text}"));
    }
    let whole = whole
        .parse::<u16>()
        .map_err(|_| format!("invalid exact hundredth score {text}"))?;
    let fraction = match fraction.unwrap_or("") {
        "" => 0,
        one if one.len() == 1 => {
            one.parse::<u16>()
                .map_err(|_| format!("invalid exact hundredth score {text}"))?
                * 10
        }
        two => two
            .parse::<u16>()
            .map_err(|_| format!("invalid exact hundredth score {text}"))?,
    };
    let magnitude = whole * 100 + fraction;
    if magnitude > 100 {
        return Err(format!("score {text} is outside the permitted range"));
    }
    match kind {
        ScoreKind::Gain if negative && magnitude != 0 => {
            return Err(format!("gain score {text} must not be negative"));
        }
        ScoreKind::Loss if !negative && magnitude != 0 => {
            return Err(format!("loss score {text} must not be positive"));
        }
        _ => {}
    }
    ScoreMagnitude::new(magnitude).map_err(|error| error.to_string())
}

fn parse_relative(text: &str) -> Result<RelativePosition, String> {
    let value = text
        .parse::<i16>()
        .map_err(|_| format!("invalid relative position {text}"))?;
    RelativePosition::new(value).map_err(|error| error.to_string())
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct LocusKey {
    contig: Grch38Contig,
    position: GenomicPosition,
    reference: ReferenceBase,
}

struct Group {
    key: LocusKey,
    values: [Option<AlleleScore>; 3],
    value_count: usize,
    first_line: u64,
}

struct FileState {
    gene: EnsemblGeneId,
    contig: Option<Grch38Contig>,
    group: Option<Group>,
    direction: Option<SourceDirection>,
    first: Option<u32>,
    last: Option<u32>,
    previous_position: Option<u32>,
    rows: u64,
    loci: u64,
    gaps: u64,
    omitted_bases: u64,
    ambiguous_ref_loci: u64,
    n_omit_a: u64,
    n_omit_t: u64,
}

impl FileState {
    fn new(gene: EnsemblGeneId) -> Self {
        Self {
            gene,
            contig: None,
            group: None,
            direction: None,
            first: None,
            last: None,
            previous_position: None,
            rows: 0,
            loci: 0,
            gaps: 0,
            omitted_bases: 0,
            ambiguous_ref_loci: 0,
            n_omit_a: 0,
            n_omit_t: 0,
        }
    }

    fn push<V: SourceVisitor + ?Sized>(
        &mut self,
        row: ParsedRow,
        member: &str,
        line: u64,
        visitor: &mut V,
    ) -> Result<(), InspectMemberError<V::Error>> {
        if let Some(contig) = self.contig {
            if contig != row.contig {
                return Err(
                    SourceError::line(member, line, "source member mixes chromosomes").into(),
                );
            }
        } else {
            self.contig = Some(row.contig);
        }
        self.rows += 1;
        let key = LocusKey {
            contig: row.contig,
            position: row.position,
            reference: row.reference,
        };
        if self.group.as_ref().is_some_and(|group| group.key != key) {
            self.finalize_group(member, visitor)?;
            self.begin_locus(row.position.get(), member, line)?;
        } else if self.group.is_none() {
            self.begin_locus(row.position.get(), member, line)?;
        }

        let group = self.group.get_or_insert(Group {
            key,
            values: [None; 3],
            value_count: 0,
            first_line: line,
        });
        if group
            .values
            .iter()
            .flatten()
            .any(|value| value.alternate == row.value.alternate)
        {
            return Err(SourceError::line(
                member,
                line,
                format!(
                    "duplicate alternate {} at {}:{} {}",
                    row.value.alternate, row.contig, row.position, row.reference
                ),
            )
            .into());
        }
        if group.value_count == group.values.len() {
            return Err(
                SourceError::line(member, line, "locus contains more than three rows").into(),
            );
        }
        group.values[group.value_count] = Some(row.value);
        group.value_count += 1;
        Ok(())
    }

    fn begin_locus(&mut self, position: u32, member: &str, line: u64) -> Result<(), SourceError> {
        if self.first.is_none() {
            self.first = Some(position);
        }
        if let Some(previous) = self.previous_position {
            if previous == position {
                return Err(SourceError::line(
                    member,
                    line,
                    format!("locus at position {position} reappears with a different reference"),
                ));
            }
            let observed = if position > previous {
                SourceDirection::Ascending
            } else {
                SourceDirection::Descending
            };
            if self
                .direction
                .is_some_and(|direction| direction != observed)
            {
                return Err(SourceError::line(
                    member,
                    line,
                    "source coordinate direction reverses",
                ));
            }
            self.direction.get_or_insert(observed);
            let distance = u64::from(position.abs_diff(previous));
            if distance > 1 {
                self.gaps += 1;
                self.omitted_bases += distance - 1;
            }
        }
        self.previous_position = Some(position);
        self.last = Some(position);
        Ok(())
    }

    fn finalize_group<V: SourceVisitor + ?Sized>(
        &mut self,
        member: &str,
        visitor: &mut V,
    ) -> Result<(), InspectMemberError<V::Error>> {
        let Some(group) = self.group.take() else {
            return Ok(());
        };
        if group.value_count != 3 {
            return Err(SourceError::line(
                member,
                group.first_line,
                format!(
                    "locus contains {} rows; expected exactly three",
                    group.value_count
                ),
            )
            .into());
        }
        let alternatives = match group.values {
            [Some(first), Some(second), Some(third)] => [first, second, third],
            _ => {
                return Err(
                    SourceError::line(member, group.first_line, "invalid locus width").into(),
                );
            }
        };
        let locus = match group.key.reference {
            ReferenceBase::Concrete(reference) => {
                let expected = DnaBase::ALL
                    .into_iter()
                    .filter(|base| *base != reference)
                    .all(|base| alternatives.iter().any(|value| value.alternate == base));
                if !expected {
                    return Err(SourceError::line(
                        member,
                        group.first_line,
                        "ordinary locus does not contain exactly the other three alternate bases",
                    )
                    .into());
                }
                SourceLocus::Ordinary(OrdinarySourceLocus {
                    gene: self.gene,
                    contig: group.key.contig,
                    position: group.key.position,
                    reference,
                    alternatives,
                })
            }
            ReferenceBase::Ambiguous => {
                let omitted = DnaBase::ALL
                    .into_iter()
                    .find(|base| !alternatives.iter().any(|value| value.alternate == *base))
                    .ok_or_else(|| {
                        SourceError::line(member, group.first_line, "invalid REF=N alternate shape")
                    })?;
                match omitted {
                    DnaBase::A => self.n_omit_a += 1,
                    DnaBase::T => self.n_omit_t += 1,
                    _ => {
                        return Err(SourceError::line(
                            member,
                            group.first_line,
                            "REF=N alternate set must omit A or T",
                        )
                        .into());
                    }
                }
                self.ambiguous_ref_loci += 1;
                SourceLocus::AmbiguousReference(AmbiguousReferenceLocus {
                    gene: self.gene,
                    contig: group.key.contig,
                    position: group.key.position,
                    alternatives,
                    omitted,
                })
            }
        };
        self.loci += 1;
        visitor.visit(&locus).map_err(InspectMemberError::Visitor)?;
        Ok(())
    }

    fn finish<V: SourceVisitor + ?Sized>(
        mut self,
        member: &str,
        visitor: &mut V,
    ) -> Result<FileSummary, InspectMemberError<V::Error>> {
        self.finalize_group(member, visitor)?;
        let contig = self
            .contig
            .ok_or_else(|| SourceError::member(member, "source member has no data rows"))?;
        let first = self
            .first
            .expect("a file with a contig has a first position");
        let last = self.last.expect("a file with a contig has a last position");
        Ok(FileSummary {
            gene: self.gene,
            contig,
            direction: self.direction.unwrap_or(SourceDirection::Ascending),
            first,
            last,
            rows: self.rows,
            loci: self.loci,
            segments: self.gaps + 1,
            gaps: self.gaps,
            omitted_bases: self.omitted_bases,
            ambiguous_ref_loci: self.ambiguous_ref_loci,
            n_omit_a: self.n_omit_a,
            n_omit_t: self.n_omit_t,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_scores_normalize_zero_and_reject_extra_precision() {
        assert_eq!(
            parse_score("-0.0", ScoreKind::Gain)
                .expect("zero")
                .hundredths(),
            0
        );
        assert_eq!(
            parse_score("0.00", ScoreKind::Loss)
                .expect("zero")
                .hundredths(),
            0
        );
        assert_eq!(
            parse_score("0.21", ScoreKind::Gain)
                .expect("gain")
                .hundredths(),
            21
        );
        assert_eq!(
            parse_score("-1.0", ScoreKind::Loss)
                .expect("loss")
                .hundredths(),
            100
        );
        assert!(parse_score("0.001", ScoreKind::Gain).is_err());
        assert!(parse_score("-0.1", ScoreKind::Gain).is_err());
        assert!(parse_score("0.1", ScoreKind::Loss).is_err());
        assert!(parse_score("1.01", ScoreKind::Gain).is_err());
    }

    #[test]
    fn relative_score_boundaries_are_exact() {
        assert_eq!(parse_relative("-50").expect("lower").get(), -50);
        assert_eq!(parse_relative("50").expect("upper").get(), 50);
        assert!(parse_relative("-51").is_err());
        assert!(parse_relative("51").is_err());
    }
}
