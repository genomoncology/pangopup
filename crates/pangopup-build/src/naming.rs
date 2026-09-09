//! The offline gene-name index builder and inspector.
//!
//! The builder reads one dated HGNC release and one NCBI gene information
//! file, applies the arbitration rule, and writes the `PGNAME01` index the
//! executable carries. It reaches no network: a maintainer downloads both
//! sources through the `gene-name-index` make target and passes the local
//! paths.
//!
//! HGNC is the naming authority. Where HGNC reaches an accession, HGNC
//! supplies that accession's whole record and NCBI is not consulted for it.
//! NCBI names only the accessions HGNC does not reach. The two sources are
//! never merged field by field.

use crate::CommandError;
use flate2::read::GzDecoder;
use pangopup_index::gene_names::{
    self, GeneNameCounts, GeneNameEntry, GeneNameIndex, GeneNameSource, GeneNames, GeneNaming,
    UnnamedReason,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::Path,
};

const HGNC_PUBLISHER: &str = "HUGO Gene Nomenclature Committee (HGNC), EMBL-EBI";
const HGNC_URL_PREFIX: &str =
    "https://storage.googleapis.com/public-download-files/hgnc/archive/archive/monthly/tsv/";
const NCBI_PUBLISHER: &str =
    "National Center for Biotechnology Information (NCBI), U.S. National Library of Medicine";
const NCBI_URL: &str =
    "https://ftp.ncbi.nlm.nih.gov/gene/DATA/GENE_INFO/Mammalia/Homo_sapiens.gene_info.gz";

const APPROVED: &str = "Approved";
const HUMAN_TAX_ID: &str = "9606";
const ENSEMBL_PREFIX: &str = "Ensembl:";
const ABSENT_CELL: &str = "-";
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// One source release the index was built from.
#[derive(Clone, Debug, Serialize)]
pub struct GeneNameSourceReport {
    pub publisher: &'static str,
    pub file: String,
    pub url: String,
    pub bytes: u64,
    pub sha256: String,
    /// Data rows in the file as fetched, counted before the organism and
    /// crosslink guards. A maintainer reads it against the published release.
    pub rows: u64,
    /// Whether the recorded bytes can be fetched again. HGNC publishes
    /// immutable dated monthly releases. NCBI replaces its file at a fixed URL
    /// and publishes no dated archive of it.
    pub refetchable: bool,
}

/// What one build reports: the two sources it read and the reach it produced.
#[derive(Clone, Debug, Serialize)]
pub struct GeneNameIndexReport {
    pub index_file: String,
    pub index_bytes: u64,
    pub index_sha256: String,
    pub accessions: u64,
    pub named_by_hgnc: u64,
    pub named_by_ncbi: u64,
    pub unnamed: u64,
    pub hgnc: GeneNameSourceReport,
    pub ncbi: GeneNameSourceReport,
}

/// Build the gene-name index from one dated HGNC release and one NCBI gene
/// information file. Identical source bytes produce identical index bytes.
pub fn build_gene_name_index(
    hgnc: &Path,
    ncbi: &Path,
    output: &Path,
) -> Result<GeneNameIndexReport, CommandError> {
    let hgnc_bytes = read_source(hgnc)?;
    let ncbi_bytes = read_source(ncbi)?;
    let (hgnc_rows, hgnc_records) = parse_hgnc(&decode_utf8(&hgnc_bytes)?)?;
    let (ncbi_rows, ncbi_records) = parse_ncbi(&decode_utf8(&decompress(&ncbi_bytes)?)?)?;

    let entries = merge(&hgnc_records, &ncbi_records);
    let counts = gene_names::write(output, &entries)
        .map_err(|error| failed(format!("gene-name index write failed: {error}")))?;
    let member = fs::read(output).map_err(|error| failed(error.to_string()))?;

    Ok(GeneNameIndexReport {
        index_file: file_name(output)?,
        index_bytes: member.len() as u64,
        index_sha256: sha256(&member),
        accessions: counts.accessions,
        named_by_hgnc: counts.named_by_hgnc,
        named_by_ncbi: counts.named_by_ncbi,
        unnamed: counts.unnamed,
        hgnc: GeneNameSourceReport {
            publisher: HGNC_PUBLISHER,
            url: format!("{HGNC_URL_PREFIX}{}", file_name(hgnc)?),
            file: file_name(hgnc)?,
            bytes: hgnc_bytes.len() as u64,
            sha256: sha256(&hgnc_bytes),
            rows: hgnc_rows,
            refetchable: true,
        },
        ncbi: GeneNameSourceReport {
            publisher: NCBI_PUBLISHER,
            file: file_name(ncbi)?,
            url: NCBI_URL.to_owned(),
            bytes: ncbi_bytes.len() as u64,
            sha256: sha256(&ncbi_bytes),
            rows: ncbi_rows,
            refetchable: false,
        },
    })
}

/// The record the repository keeps beside the committed index. It states the
/// index's own byte count and digest, and what each source release was.
pub fn provenance_document(report: &GeneNameIndexReport) -> Result<String, CommandError> {
    #[derive(Serialize)]
    struct Index<'a> {
        file: &'a str,
        bytes: u64,
        sha256: &'a str,
    }
    #[derive(Serialize)]
    struct Provenance<'a> {
        index: Index<'a>,
        counts: GeneNameCounts,
        sources: [&'a GeneNameSourceReport; 2],
    }

    let mut document = serde_json::to_string_pretty(&Provenance {
        index: Index {
            file: &report.index_file,
            bytes: report.index_bytes,
            sha256: &report.index_sha256,
        },
        counts: GeneNameCounts {
            accessions: report.accessions,
            named_by_hgnc: report.named_by_hgnc,
            named_by_ncbi: report.named_by_ncbi,
            unnamed: report.unnamed,
        },
        sources: [&report.hgnc, &report.ncbi],
    })
    .map_err(|error| failed(format!("provenance record is not serializable: {error}")))?;
    document.push('\n');
    Ok(document)
}

/// Report what the committed index holds for the accessions a maintainer
/// names, followed by one total line.
pub fn inspect_gene_name_index(
    index: &Path,
    accessions: &[String],
    output: &mut dyn Write,
) -> Result<(), CommandError> {
    let index = GeneNameIndex::open(index)
        .map_err(|error| failed(format!("gene-name index cannot be opened: {error}")))?;
    for accession in accessions {
        match index.naming(accession) {
            Some(GeneNaming::Named(names)) => writeln!(
                output,
                "gene={accession} source={} hgnc={} symbol={} ncbi={} prev={} alias={}",
                names.source.as_str(),
                names.hgnc_id.as_deref().unwrap_or(ABSENT_CELL),
                names.symbol,
                names
                    .ncbi_gene_id
                    .map_or_else(|| ABSENT_CELL.to_owned(), |value| value.to_string()),
                joined(&names.prev_symbols),
                joined(&names.alias_symbols),
            ),
            Some(GeneNaming::Unnamed(reason)) => {
                writeln!(output, "gene={accession} unnamed={}", reason.as_str())
            }
            None => writeln!(output, "gene={accession} absent"),
        }
        .map_err(write_failed)?;
    }
    let counts = index.counts();
    writeln!(
        output,
        "total accessions={} named_by_hgnc={} named_by_ncbi={} unnamed={}",
        counts.accessions, counts.named_by_hgnc, counts.named_by_ncbi, counts.unnamed,
    )
    .map_err(write_failed)
}

/// One record per accession, from HGNC where HGNC reaches it and from NCBI
/// otherwise. An accession the naming source carries more than once reports no
/// name, and so does a clone-derived placeholder symbol.
fn merge(
    hgnc: &BTreeMap<u64, Vec<GeneNames>>,
    ncbi: &BTreeMap<u64, Vec<GeneNames>>,
) -> Vec<GeneNameEntry> {
    let keys: BTreeSet<u64> = hgnc.keys().chain(ncbi.keys()).copied().collect();
    keys.into_iter()
        .map(|key| {
            let (source, records) = match hgnc.get(&key) {
                Some(records) => (GeneNameSource::Hgnc, records),
                None => (
                    GeneNameSource::Ncbi,
                    ncbi.get(&key)
                        .expect("the key came from one of the sources"),
                ),
            };
            GeneNameEntry {
                accession: accession_text(key),
                source,
                naming: admit(records),
            }
        })
        .collect()
}

fn admit(records: &[GeneNames]) -> GeneNaming {
    let [names] = records else {
        return GeneNaming::Unnamed(UnnamedReason::ConflictingRecords);
    };
    if names.symbol.is_empty() || names.symbol.contains('.') {
        return GeneNaming::Unnamed(UnnamedReason::PlaceholderSymbol);
    }
    GeneNaming::Named(names.clone())
}

/// Parse the tab-separated HGNC complete set. HGNC quotes a cell that holds
/// more than one pipe-separated value and leaves a single value bare. The
/// published 2026-09-04 release quotes 11,046 of its 45,045 alias cells, so a
/// reader that splits on `|` without stripping the quotes yields values such
/// as `"T4` and `Leu-3`.
fn parse_hgnc(text: &str) -> Result<(u64, BTreeMap<u64, Vec<GeneNames>>), CommandError> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines
        .next()
        .ok_or_else(|| invalid("HGNC source is empty"))?
        .split('\t')
        .collect();
    let hgnc_id = column(&header, "hgnc_id")?;
    let symbol = column(&header, "symbol")?;
    let status = column(&header, "status")?;
    let alias_symbol = column(&header, "alias_symbol")?;
    let prev_symbol = column(&header, "prev_symbol")?;
    let entrez_id = column(&header, "entrez_id")?;
    let accession_column = column(&header, "ensembl_gene_id")?;

    let mut rows = 0;
    let mut records: BTreeMap<u64, Vec<GeneNames>> = BTreeMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != header.len() {
            return Err(invalid("HGNC source row has an unexpected width"));
        }
        rows += 1;
        if scalar(cells[status]) != APPROVED {
            continue;
        }
        let accession = scalar(cells[accession_column]);
        if accession.is_empty() {
            continue;
        }
        records
            .entry(accession_key(accession)?)
            .or_default()
            .push(GeneNames {
                symbol: scalar(cells[symbol]).to_owned(),
                source: GeneNameSource::Hgnc,
                hgnc_id: Some(scalar(cells[hgnc_id]).to_owned()),
                ncbi_gene_id: gene_identifier(cells[entrez_id])?,
                prev_symbols: values(cells[prev_symbol]),
                alias_symbols: values(cells[alias_symbol]),
            });
    }
    Ok((rows, records))
}

/// Parse the NCBI gene information file. A row enters the index only where its
/// organism is human and its cross-references carry an Ensembl gene accession.
/// A row carries more than one such cross-reference 31 times in the file the
/// committed index was built from, and every one of them reaches its accession.
fn parse_ncbi(text: &str) -> Result<(u64, BTreeMap<u64, Vec<GeneNames>>), CommandError> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines
        .next()
        .ok_or_else(|| invalid("NCBI source is empty"))?
        .split('\t')
        .map(|name| name.trim_start_matches('#'))
        .collect();
    let tax_id = column(&header, "tax_id")?;
    let gene_id = column(&header, "GeneID")?;
    let symbol = column(&header, "Symbol")?;
    let synonyms = column(&header, "Synonyms")?;
    let cross_references = column(&header, "dbXrefs")?;

    let mut rows = 0;
    let mut records: BTreeMap<u64, Vec<GeneNames>> = BTreeMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != header.len() {
            return Err(invalid("NCBI source row has an unexpected width"));
        }
        rows += 1;
        if cells[tax_id] != HUMAN_TAX_ID {
            continue;
        }
        let names = GeneNames {
            symbol: cells[symbol].to_owned(),
            source: GeneNameSource::Ncbi,
            hgnc_id: None,
            ncbi_gene_id: gene_identifier(cells[gene_id])?,
            prev_symbols: Vec::new(),
            alias_symbols: absent_or(cells[synonyms]),
        };
        for accession in cells[cross_references]
            .split('|')
            .filter_map(|reference| reference.strip_prefix(ENSEMBL_PREFIX))
        {
            records
                .entry(accession_key(accession)?)
                .or_default()
                .push(names.clone());
        }
    }
    Ok((rows, records))
}

fn read_source(path: &Path) -> Result<Vec<u8>, CommandError> {
    fs::read(path).map_err(|error| failed(format!("{}: {error}", path.display())))
}

fn decompress(bytes: &[u8]) -> Result<Vec<u8>, CommandError> {
    if bytes.get(..2) != Some(GZIP_MAGIC.as_slice()) {
        return Ok(bytes.to_vec());
    }
    let mut decoded = Vec::new();
    GzDecoder::new(bytes)
        .read_to_end(&mut decoded)
        .map_err(|error| invalid(format!("NCBI source is not readable gzip: {error}")))?;
    Ok(decoded)
}

fn decode_utf8(bytes: &[u8]) -> Result<String, CommandError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| invalid("naming source is not valid UTF-8"))
}

fn file_name(path: &Path) -> Result<String, CommandError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| invalid("path has no file name"))
}

fn column(header: &[&str], name: &str) -> Result<usize, CommandError> {
    header
        .iter()
        .position(|candidate| *candidate == name)
        .ok_or_else(|| invalid(format!("naming source has no {name} column")))
}

/// The numeric part of an Ensembl gene accession. Any other shape is a source
/// change the builder refuses rather than silently keys on.
fn accession_key(accession: &str) -> Result<u64, CommandError> {
    accession
        .strip_prefix("ENSG")
        .filter(|digits| digits.len() == 11 && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|digits| digits.parse().ok())
        .ok_or_else(|| invalid(format!("{accession} is not ENSG followed by 11 digits")))
}

fn accession_text(key: u64) -> String {
    format!("ENSG{key:011}")
}

/// One value from a cell that carries at most one. HGNC leaves a single value
/// bare, so the quotes are stripped only where they surround the whole cell.
fn scalar(cell: &str) -> &str {
    cell.strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(cell)
}

/// Every value a multi-valued HGNC cell carries, in source order.
fn values(cell: &str) -> Vec<String> {
    let unquoted = scalar(cell);
    if unquoted.is_empty() {
        return Vec::new();
    }
    unquoted.split('|').map(ToOwned::to_owned).collect()
}

/// Every value a multi-valued NCBI cell carries. NCBI writes `-` where it
/// supplies none.
fn absent_or(cell: &str) -> Vec<String> {
    if cell.is_empty() || cell == ABSENT_CELL {
        return Vec::new();
    }
    cell.split('|').map(ToOwned::to_owned).collect()
}

fn gene_identifier(cell: &str) -> Result<Option<u32>, CommandError> {
    let value = scalar(cell);
    if value.is_empty() || value == ABSENT_CELL {
        return Ok(None);
    }
    value
        .parse()
        .map(Some)
        .map_err(|_| invalid("naming source NCBI Gene identifier is not a number"))
}

/// Every value of a multi-valued field, in source order. A field the source
/// does not supply reports `-`.
fn joined(values: &[String]) -> String {
    if values.is_empty() {
        ABSENT_CELL.to_owned()
    } else {
        values.join("|")
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::new("NAMING_SOURCE_INVALID", message)
}

fn failed(message: impl Into<String>) -> CommandError {
    CommandError::new("NAMING_BUILD_FAILED", message)
}

fn write_failed(error: std::io::Error) -> CommandError {
    CommandError::new("OUTPUT_IO", error.to_string())
}
