//! Offline gene naming.
//!
//! A score record identifies its gene by Ensembl accession. A naming source
//! supplies the labels a consumer displays beside that accession. The
//! accession stays the identity. A symbol is a label. Symbols get renamed and
//! accessions do not.
//!
//! The component installs beside `bundles/` and `runtime/` under the data
//! directory. It must not enter the runtime profile: `RuntimeProfile` hashes
//! to `runtime_profile_id`, which is one of the three inputs to
//! `ActiveScoringIdentityPreimage`, so a naming field there would move
//! `scoring_identity` on every install, update and removal.

use super::{AssetError, AssetErrorKind, local};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

/// The published 2026-09-04 release is 16,903,161 bytes. The cap bounds a
/// hostile local file without rejecting a plausible later release.
const MAX_SOURCE_BYTES: u64 = 128 * 1024 * 1024;
const NAMING_DIRECTORY: &str = "naming";
const SOURCE_MEMBER: &str = "source.tsv";
const ACTIVE_MEMBER: &str = "active.json";
const STAGED_SOURCE_MEMBER: &str = "source.tsv.staged";
const STAGED_ACTIVE_MEMBER: &str = "active.json.staged";
const SOURCE_MODE: u32 = 0o400;
const ACTIVE_MODE: u32 = 0o600;
const FILE_PREFIX: &str = "hgnc_complete_set_";
const FILE_SUFFIX: &str = ".tsv";
const RELEASE_PREFIX: &str = "hgnc-";

const ACCESSION_COLUMN: &str = "ensembl_gene_id";
const APPROVED: &str = "Approved";

/// The labels one named gene carries. `symbol` and `hgnc_id` are always
/// present: the source supplies both for every joined gene. The remaining
/// fields are absent when the source supplies nothing for that gene.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneNames {
    pub symbol: String,
    pub hgnc_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ncbi_gene_id: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub prev_symbols: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alias_symbols: Vec<String>,
}

/// Why an accession the source carries still reports no name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnnamedReason {
    /// The accession carries more than one approved record. Picking one would
    /// be invisible to a consumer.
    ConflictingRecords,
    /// The approved symbol is a clone-derived string rather than a symbol. No
    /// approved symbol in the 2026-09-04 release contains a period, and a
    /// clone-derived name such as `AC092143.1` carries the period of its
    /// accession version.
    PlaceholderSymbol,
}

impl UnnamedReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConflictingRecords => "conflicting_records",
            Self::PlaceholderSymbol => "placeholder_symbol",
        }
    }
}

/// What one Ensembl accession yields from a naming source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneNaming {
    Named(GeneNames),
    Unnamed(UnnamedReason),
}

/// One dated naming release, read into memory and keyed by Ensembl accession.
#[derive(Clone, Debug)]
pub struct NamingSource {
    release: String,
    rows: usize,
    genes: BTreeMap<String, GeneNaming>,
}

impl NamingSource {
    /// Read one dated release file. The release identity comes from the file
    /// name, so the caller supplies the published member rather than a copy
    /// under an arbitrary name.
    pub fn read(source: &Path) -> Result<Self, AssetError> {
        let release = release_from_file_name(source)?;
        Self::parse(release, &read_source_file(source)?)
    }

    /// Parse the tab-separated complete set. HGNC quotes a cell that holds
    /// more than one pipe-separated value and leaves a single value bare. The
    /// published 2026-09-04 release quotes 11,046 of its 45,045 alias cells,
    /// so a reader that splits on `|` without stripping the quotes yields
    /// values such as `"T4` and `Leu-3"`.
    pub fn parse(release: String, bytes: &[u8]) -> Result<Self, AssetError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| naming_invalid("naming source is not valid UTF-8"))?;
        let mut lines = text.lines();
        let header: Vec<&str> = lines
            .next()
            .ok_or_else(|| naming_invalid("naming source is empty"))?
            .split('\t')
            .collect();
        let accession_column = column(&header, ACCESSION_COLUMN)?;
        let hgnc_id = column(&header, "hgnc_id")?;
        let symbol = column(&header, "symbol")?;
        let status = column(&header, "status")?;
        let alias_symbol = column(&header, "alias_symbol")?;
        let prev_symbol = column(&header, "prev_symbol")?;
        let entrez_id = column(&header, "entrez_id")?;

        let mut rows = 0;
        let mut records: BTreeMap<String, Vec<GeneNames>> = BTreeMap::new();
        for line in lines {
            if line.is_empty() {
                continue;
            }
            let cells: Vec<&str> = line.split('\t').collect();
            if cells.len() != header.len() {
                return Err(naming_invalid("naming source row has an unexpected width"));
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
                .entry(accession.to_owned())
                .or_default()
                .push(GeneNames {
                    symbol: scalar(cells[symbol]).to_owned(),
                    hgnc_id: scalar(cells[hgnc_id]).to_owned(),
                    ncbi_gene_id: gene_identifier(cells[entrez_id])?,
                    prev_symbols: values(cells[prev_symbol]),
                    alias_symbols: values(cells[alias_symbol]),
                });
        }

        let genes = records
            .into_iter()
            .map(|(accession, mut carried)| {
                let naming = if carried.len() > 1 {
                    GeneNaming::Unnamed(UnnamedReason::ConflictingRecords)
                } else {
                    let names = carried.pop().expect("one approved record");
                    if names.symbol.contains('.') || names.symbol.is_empty() {
                        GeneNaming::Unnamed(UnnamedReason::PlaceholderSymbol)
                    } else {
                        GeneNaming::Named(names)
                    }
                };
                (accession, naming)
            })
            .collect();
        Ok(Self {
            release,
            rows,
            genes,
        })
    }

    pub fn release(&self) -> &str {
        &self.release
    }

    pub const fn rows(&self) -> usize {
        self.rows
    }

    pub fn accessions(&self) -> usize {
        self.genes.len()
    }

    pub fn named(&self) -> usize {
        self.genes
            .values()
            .filter(|naming| matches!(naming, GeneNaming::Named(_)))
            .count()
    }

    pub fn unnamed(&self) -> usize {
        self.accessions() - self.named()
    }

    /// Every accession the source carries, in accession order.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &GeneNaming)> {
        self.genes
            .iter()
            .map(|(accession, naming)| (accession.as_str(), naming))
    }

    /// The labels for one stable Ensembl accession. An accession the source
    /// cannot name reports nothing.
    pub fn names(&self, accession: &str) -> Option<&GeneNames> {
        match self.genes.get(accession) {
            Some(GeneNaming::Named(names)) => Some(names),
            Some(GeneNaming::Unnamed(_)) | None => None,
        }
    }
}

/// The receipt one install leaves beside the published source.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ActiveNaming {
    release: String,
    source_bytes: u64,
    source_sha256: String,
    named_genes: usize,
}

/// What one install reports.
#[derive(Debug, Serialize)]
pub struct NamingInstallOutcome {
    pub status: &'static str,
    pub release: String,
    pub source_bytes: u64,
    pub source_sha256: String,
    pub named_genes: usize,
}

/// Install one dated naming release from a local file. The path never reaches
/// the network. A second install of the same bytes reuses what is published.
pub fn install_naming_source(
    source: &Path,
    data_root: &Path,
) -> Result<NamingInstallOutcome, AssetError> {
    let release = release_from_file_name(source)?;
    let bytes = read_source_file(source)?;
    let parsed = NamingSource::parse(release.clone(), &bytes)?;
    let receipt = ActiveNaming {
        release,
        source_bytes: bytes.len() as u64,
        source_sha256: sha256(&bytes),
        named_genes: parsed.named(),
    };

    let root = local::open_root(data_root, true)?.ok_or_else(|| {
        AssetError::new(
            AssetErrorKind::AssetIo,
            "data root is missing after creation",
        )
    })?;
    let naming = local::ensure_private_dir(&root.dir, NAMING_DIRECTORY, &root)?;
    let status = if published_matches(&naming, &root, &receipt)? {
        "reused"
    } else {
        publish(&naming, &root, &bytes, &receipt)?;
        "installed"
    };
    Ok(NamingInstallOutcome {
        status,
        release: receipt.release,
        source_bytes: receipt.source_bytes,
        source_sha256: receipt.source_sha256,
        named_genes: receipt.named_genes,
    })
}

/// Open the installed naming source, or report that none is installed.
pub fn open_installed_naming_source(data_root: &Path) -> Result<Option<NamingSource>, AssetError> {
    let Some(root) = local::open_root(data_root, false)? else {
        return Ok(None);
    };
    let Some(naming) = local::open_owned_dir_optional(&root.dir, NAMING_DIRECTORY, &root)? else {
        return Ok(None);
    };
    let Some(receipt) = read_receipt(&naming, &root)? else {
        return Ok(None);
    };
    let bytes = read_published_source(&naming, &root, &receipt)?
        .ok_or_else(|| naming_state_invalid("installed naming source is missing"))?;
    NamingSource::parse(receipt.release, &bytes).map(Some)
}

fn read_receipt(
    naming: &local::Dir,
    root: &local::Root,
) -> Result<Option<ActiveNaming>, AssetError> {
    let Some(file) = local::open_owned_file_optional(
        naming,
        ACTIVE_MEMBER,
        ACTIVE_MODE,
        root,
        AssetErrorKind::ManifestInvalid,
    )
    .map_err(|_| naming_state_invalid("installed naming receipt is not a private regular file"))?
    else {
        return Ok(None);
    };
    let bytes = local::read_bounded_handle_ref(&file, 4_096, AssetErrorKind::ManifestInvalid)
        .map_err(|_| naming_state_invalid("installed naming receipt cannot be read"))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| naming_state_invalid("installed naming receipt is invalid"))
}

/// Read the published source and authenticate it against its receipt.
fn read_published_source(
    naming: &local::Dir,
    root: &local::Root,
    receipt: &ActiveNaming,
) -> Result<Option<Vec<u8>>, AssetError> {
    let Some(file) = local::open_owned_file_optional(
        naming,
        SOURCE_MEMBER,
        SOURCE_MODE,
        root,
        AssetErrorKind::ManifestInvalid,
    )
    .map_err(|_| naming_state_invalid("installed naming source is not a private regular file"))?
    else {
        return Ok(None);
    };
    let bytes =
        local::read_bounded_handle_ref(&file, MAX_SOURCE_BYTES, AssetErrorKind::ManifestInvalid)
            .map_err(|_| naming_state_invalid("installed naming source cannot be read"))?;
    if bytes.len() as u64 != receipt.source_bytes || sha256(&bytes) != receipt.source_sha256 {
        return Err(naming_state_invalid(
            "installed naming source does not match its receipt",
        ));
    }
    Ok(Some(bytes))
}

/// Re-authenticate what is already published against the supplied source. A
/// receipt without its source, or a source that no longer matches, reinstalls
/// rather than reporting a reuse that is not there.
fn published_matches(
    naming: &local::Dir,
    root: &local::Root,
    receipt: &ActiveNaming,
) -> Result<bool, AssetError> {
    let Some(published) = read_receipt(naming, root)? else {
        return Ok(false);
    };
    if published.release != receipt.release
        || published.source_bytes != receipt.source_bytes
        || published.source_sha256 != receipt.source_sha256
    {
        return Ok(false);
    }
    Ok(read_published_source(naming, root, &published)?.is_some())
}

fn publish(
    naming: &local::Dir,
    root: &local::Root,
    bytes: &[u8],
    receipt: &ActiveNaming,
) -> Result<(), AssetError> {
    let encoded = serde_json::to_vec(receipt)
        .map_err(|_| naming_invalid("naming receipt is not serializable"))?;
    write_staged(naming, root, STAGED_SOURCE_MEMBER, bytes, SOURCE_MODE)?;
    write_staged(naming, root, STAGED_ACTIVE_MEMBER, &encoded, ACTIVE_MODE)?;
    local::rename_owned_replace(naming, STAGED_SOURCE_MEMBER, naming, SOURCE_MEMBER)?;
    local::rename_owned_replace(naming, STAGED_ACTIVE_MEMBER, naming, ACTIVE_MEMBER)?;
    naming
        .file
        .sync_all()
        .map_err(|_| AssetError::new(AssetErrorKind::AssetIo, "sync naming directory"))
}

fn write_staged(
    naming: &local::Dir,
    root: &local::Root,
    name: &str,
    bytes: &[u8],
    mode: u32,
) -> Result<(), AssetError> {
    // A staged name left by an interrupted install would otherwise block the
    // exclusive create below.
    local::remove_owned_file_optional(naming, name)?;
    let mut file = local::create_owned_file(naming, name, mode, root)?;
    file.write_all(bytes)
        .map_err(|_| AssetError::new(AssetErrorKind::AssetIo, "write staged naming member"))?;
    local::set_mode(&file, mode)?;
    file.sync_all()
        .map_err(|_| AssetError::new(AssetErrorKind::AssetIo, "sync staged naming member"))
}

/// Read one caller-supplied naming source without following a symbolic link
/// and without admitting an unbounded file.
fn read_source_file(source: &Path) -> Result<Vec<u8>, AssetError> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(source)
        .map_err(|error| AssetError::new(AssetErrorKind::InputIo, error.to_string()))?;
    let metadata = file
        .metadata()
        .map_err(|error| AssetError::new(AssetErrorKind::InputIo, error.to_string()))?;
    if !metadata.file_type().is_file() {
        return Err(naming_invalid("naming source is not a regular file"));
    }
    if metadata.len() > MAX_SOURCE_BYTES {
        return Err(naming_invalid("naming source exceeds the admitted size"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| AssetError::new(AssetErrorKind::InputIo, error.to_string()))?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(naming_invalid("naming source exceeds the admitted size"));
    }
    Ok(bytes)
}

fn release_from_file_name(source: &Path) -> Result<String, AssetError> {
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| naming_invalid("naming source path has no file name"))?;
    let date = name
        .strip_prefix(FILE_PREFIX)
        .and_then(|rest| rest.strip_suffix(FILE_SUFFIX))
        .filter(|date| is_release_date(date))
        .ok_or_else(|| {
            naming_invalid(format!(
                "naming source must be named {FILE_PREFIX}<YYYY-MM-DD>{FILE_SUFFIX}"
            ))
        })?;
    Ok(format!("{RELEASE_PREFIX}{date}"))
}

fn is_release_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && [0, 1, 2, 3, 5, 6, 8, 9]
            .into_iter()
            .all(|index| bytes[index].is_ascii_digit())
}

fn column(header: &[&str], name: &str) -> Result<usize, AssetError> {
    header
        .iter()
        .position(|candidate| *candidate == name)
        .ok_or_else(|| naming_invalid(format!("naming source has no {name} column")))
}

/// One value from a cell that carries at most one. HGNC leaves a single value
/// bare, so the quotes are stripped only where they surround the whole cell.
fn scalar(cell: &str) -> &str {
    cell.strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(cell)
}

/// Every value a multi-valued cell carries, in source order.
fn values(cell: &str) -> Vec<String> {
    let unquoted = scalar(cell);
    if unquoted.is_empty() {
        return Vec::new();
    }
    unquoted.split('|').map(str::to_owned).collect()
}

fn gene_identifier(cell: &str) -> Result<Option<u32>, AssetError> {
    let value = scalar(cell);
    if value.is_empty() {
        return Ok(None);
    }
    value
        .parse()
        .map(Some)
        .map_err(|_| naming_invalid("naming source NCBI Gene identifier is not a number"))
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// A caller-supplied naming source that cannot be read as one.
fn naming_invalid(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::ManifestInvalid, message)
}

/// An installed naming component that no longer agrees with its receipt.
fn naming_state_invalid(message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::AssetStateInvalid, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/gene-naming-mini/hgnc_complete_set_2026-09-04.tsv"
    );

    fn fixture() -> NamingSource {
        NamingSource::read(Path::new(FIXTURE)).expect("miniature naming source")
    }

    #[test]
    fn a_quoted_multi_valued_cell_reports_its_values_without_the_quotes() {
        let source = fixture();
        let names = source.names("ENSG00000010610").expect("CD4 is named");
        assert_eq!(names.symbol, "CD4");
        assert_eq!(names.hgnc_id, "HGNC:1678");
        assert_eq!(names.ncbi_gene_id, Some(920));
        assert!(names.prev_symbols.is_empty());
        assert_eq!(names.alias_symbols, ["T4", "Leu-3"]);
    }

    #[test]
    fn a_hyphen_is_ordinary_and_a_period_in_the_symbol_withholds_the_name() {
        let source = fixture();
        let names = source.names("ENSG00000167034").expect("NKX3-1 is named");
        assert_eq!(names.symbol, "NKX3-1");
        assert_eq!(names.alias_symbols, ["NKX3.1", "BAPX2"]);
        assert_eq!(source.names("ENSG00000185974"), None);
        assert_eq!(
            source
                .entries()
                .find(|(accession, _)| *accession == "ENSG00000185974"),
            Some((
                "ENSG00000185974",
                &GeneNaming::Unnamed(UnnamedReason::PlaceholderSymbol)
            ))
        );
    }

    #[test]
    fn an_accession_with_two_approved_records_reports_no_name() {
        let source = fixture();
        assert_eq!(source.names("ENSG00000175658"), None);
        assert_eq!(
            source
                .entries()
                .find(|(accession, _)| *accession == "ENSG00000175658"),
            Some((
                "ENSG00000175658",
                &GeneNaming::Unnamed(UnnamedReason::ConflictingRecords)
            ))
        );
        assert_eq!(source.rows(), 10);
        assert_eq!(source.accessions(), 9);
        assert_eq!(source.named(), 7);
        assert_eq!(source.unnamed(), 2);
        assert_eq!(source.release(), "hgnc-2026-09-04");
    }

    #[test]
    fn an_unsupplied_field_is_absent_rather_than_empty() {
        let source = fixture();
        let names = source.names("ENSG00000141510").expect("TP53 is named");
        assert_eq!(
            serde_json::to_string(names).expect("naming JSON"),
            r#"{"symbol":"TP53","hgnc_id":"HGNC:11998","alias_symbols":["p53","LFS1"]}"#
        );
    }

    #[test]
    fn a_release_identity_comes_from_the_dated_file_name() {
        assert_eq!(
            release_from_file_name(Path::new("/tmp/hgnc_complete_set_2026-09-04.tsv"))
                .expect("dated release"),
            "hgnc-2026-09-04"
        );
        for rejected in [
            "/tmp/hgnc_complete_set.tsv",
            "/tmp/hgnc_complete_set_2026-09-04.txt",
            "/tmp/hgnc_complete_set_2026-9-04.tsv",
        ] {
            assert!(
                release_from_file_name(Path::new(rejected)).is_err(),
                "{rejected}"
            );
        }
    }

    #[test]
    fn installing_the_same_source_twice_publishes_once_and_then_reuses_it() {
        let root = tempfile::tempdir().expect("temporary root");
        let data = root.path().join("data");
        let first = install_naming_source(Path::new(FIXTURE), &data).expect("install");
        assert_eq!(first.status, "installed");
        assert_eq!(first.release, "hgnc-2026-09-04");
        assert_eq!(first.source_bytes, 6_092);
        assert_eq!(first.named_genes, 7);
        let second = install_naming_source(Path::new(FIXTURE), &data).expect("reinstall");
        assert_eq!(second.status, "reused");
        assert_eq!(second.source_sha256, first.source_sha256);

        let opened = open_installed_naming_source(&data)
            .expect("open")
            .expect("installed naming source");
        assert_eq!(opened.release(), "hgnc-2026-09-04");
        assert_eq!(
            opened
                .names("ENSG00000157764")
                .map(|names| names.symbol.as_str()),
            Some("BRAF")
        );
    }

    #[test]
    fn a_relaxed_member_mode_is_refused_and_the_message_names_the_naming_component() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("temporary root");
        let data = root.path().join("data");
        install_naming_source(Path::new(FIXTURE), &data).expect("install");
        let source = data.join(NAMING_DIRECTORY).join(SOURCE_MEMBER);
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
            .expect("relax the published source mode");

        let error = open_installed_naming_source(&data).expect_err("a relaxed mode is refused");
        assert_eq!(error.kind(), AssetErrorKind::AssetStateInvalid);
        assert_eq!(
            error.to_string(),
            "installed naming source is not a private regular file"
        );
    }

    #[test]
    fn an_absent_component_reports_no_naming_source() {
        let root = tempfile::tempdir().expect("temporary root");
        assert!(
            open_installed_naming_source(&root.path().join("missing"))
                .expect("absent root")
                .is_none()
        );
        let data = root.path().join("data");
        local::open_root(&data, true).expect("create data root");
        assert!(
            open_installed_naming_source(&data)
                .expect("absent component")
                .is_none()
        );
    }
}
