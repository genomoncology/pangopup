//! Artifact-local, domain-separated builder provenance.
//!
//! The evidence bytes are compiled into the builder. Artifact construction
//! therefore never reads a checkout or starts Cargo recursively.

use sha2::{Digest, Sha256};
use std::fmt;

const ALGORITHM: &[u8] = include_bytes!("source_fingerprint/algorithm.v1");
const SNV_INVENTORY_DECLARATION: &[u8] = include_bytes!("source_fingerprint/snv-inventory.v1");
const REFERENCE_INVENTORY_DECLARATION: &[u8] =
    include_bytes!("source_fingerprint/reference-inventory.v1");
const SNV_DOMAIN: &[u8] = b"pangopup.snv-builder-source.v1";
const REFERENCE_DOMAIN: &[u8] = b"pangopup.reference-builder-source.v1";

#[derive(Clone, Copy)]
struct Entry<'a> {
    path: &'a str,
    bytes: &'a [u8],
}

const SNV_ENTRIES: &[Entry<'static>] = &[
    Entry {
        path: "NOTICE",
        bytes: include_bytes!("../../../NOTICE"),
    },
    Entry {
        path: "crates/pangopup-assets/src/error.rs",
        bytes: include_bytes!("../../pangopup-assets/src/error.rs"),
    },
    Entry {
        path: "crates/pangopup-assets/src/input_audit.rs",
        bytes: include_bytes!("../../pangopup-assets/src/input_audit.rs"),
    },
    Entry {
        path: "crates/pangopup-assets/src/snv.rs",
        bytes: include_bytes!("../../pangopup-assets/src/snv.rs"),
    },
    Entry {
        path: "crates/pangopup-build/src/command_error.rs",
        bytes: include_bytes!("command_error.rs"),
    },
    Entry {
        path: "crates/pangopup-build/src/production.rs",
        bytes: include_bytes!("production.rs"),
    },
    Entry {
        path: "crates/pangopup-build/src/snv.rs",
        bytes: include_bytes!("snv.rs"),
    },
    Entry {
        path: "crates/pangopup-core/src/lib.rs",
        bytes: include_bytes!("../../pangopup-core/src/lib.rs"),
    },
    Entry {
        path: "crates/pangopup-index/src/snv.rs",
        bytes: include_bytes!("../../pangopup-index/src/snv.rs"),
    },
    Entry {
        path: "dependencies/snv-builder-cargo-lock.v1",
        bytes: include_bytes!("source_fingerprint/snv-builder-cargo-lock.v1"),
    },
    Entry {
        path: "dependencies/snv-builder-linux-lock.v1",
        bytes: include_bytes!("source_fingerprint/snv-builder-linux-lock.v1"),
    },
    Entry {
        path: "dependencies/snv-builder-roots.v1",
        bytes: include_bytes!("source_fingerprint/snv-builder-roots.v1"),
    },
    Entry {
        path: "wiring/snv-root-wiring.v1",
        bytes: include_bytes!("source_fingerprint/snv-root-wiring.v1"),
    },
];

const REFERENCE_ENTRIES: &[Entry<'static>] = &[
    Entry {
        path: "crates/pangopup-build/src/command_error.rs",
        bytes: include_bytes!("command_error.rs"),
    },
    Entry {
        path: "crates/pangopup-build/src/reference.rs",
        bytes: include_bytes!("reference.rs"),
    },
    Entry {
        path: "crates/pangopup-core/src/lib.rs",
        bytes: include_bytes!("../../pangopup-core/src/lib.rs"),
    },
    Entry {
        path: "crates/pangopup-index/src/reference.rs",
        bytes: include_bytes!("../../pangopup-index/src/reference.rs"),
    },
    Entry {
        path: "dependencies/reference-builder-cargo-lock.v1",
        bytes: include_bytes!("source_fingerprint/reference-builder-cargo-lock.v1"),
    },
    Entry {
        path: "dependencies/reference-builder-linux-lock.v1",
        bytes: include_bytes!("source_fingerprint/reference-builder-linux-lock.v1"),
    },
    Entry {
        path: "dependencies/reference-builder-roots.v1",
        bytes: include_bytes!("source_fingerprint/reference-builder-roots.v1"),
    },
    Entry {
        path: "tests/fixtures/pangolin-compat-v1/cases.jsonl",
        bytes: include_bytes!("../../../tests/fixtures/pangolin-compat-v1/cases.jsonl"),
    },
    Entry {
        path: "wiring/reference-root-wiring.v1",
        bytes: include_bytes!("source_fingerprint/reference-root-wiring.v1"),
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FingerprintError {
    EmptyDomain,
    EmptyPath,
    DuplicatePath,
}

impl fmt::Display for FingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyDomain => "fingerprint domain is empty",
            Self::EmptyPath => "fingerprint logical path is empty",
            Self::DuplicatePath => "fingerprint logical paths contain a duplicate",
        })
    }
}

fn frame(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn fingerprint(
    algorithm: &[u8],
    domain: &[u8],
    inventory_declaration: &[u8],
    entries: &[Entry<'_>],
) -> Result<String, FingerprintError> {
    if domain.is_empty() {
        return Err(FingerprintError::EmptyDomain);
    }
    let mut ordered = entries.to_vec();
    ordered.sort_unstable_by(|left, right| left.path.cmp(right.path));
    if ordered.iter().any(|entry| entry.path.is_empty()) {
        return Err(FingerprintError::EmptyPath);
    }
    if ordered.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err(FingerprintError::DuplicatePath);
    }

    let mut hash = Sha256::new();
    frame(&mut hash, algorithm);
    frame(&mut hash, domain);
    frame(&mut hash, inventory_declaration);
    for entry in ordered {
        frame(&mut hash, entry.path.as_bytes());
        frame(&mut hash, entry.bytes);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn compiled_fingerprint(
    domain: &[u8],
    inventory_declaration: &[u8],
    entries: &[Entry<'_>],
) -> String {
    fingerprint(ALGORITHM, domain, inventory_declaration, entries)
        .unwrap_or_else(|error| panic!("compiled builder-source inventory is invalid: {error}"))
}

pub(crate) fn snv_source_sha256() -> String {
    compiled_fingerprint(SNV_DOMAIN, SNV_INVENTORY_DECLARATION, SNV_ENTRIES)
}

pub(crate) fn reference_source_sha256() -> String {
    compiled_fingerprint(
        REFERENCE_DOMAIN,
        REFERENCE_INVENTORY_DECLARATION,
        REFERENCE_ENTRIES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::PathBuf,
        sync::{
            OnceLock,
            atomic::{AtomicU64, Ordering},
        },
    };

    const EXPECTED_SNV_SHA256: &str =
        "85126cbb4bbc008a475b0b941447fb7a24f299abb1754a1c10582912a522eb2d";
    const EXPECTED_REFERENCE_SHA256: &str =
        "252f60fd8ea809fa0a3b583bf3a7ddb99601fef67b21a227264e8fa55b873e24";
    static RESOLVER_SERIAL: AtomicU64 = AtomicU64::new(0);
    static MANIFEST_MODEL: OnceLock<ManifestModel> = OnceLock::new();

    const SNV_PATHS: &[&str] = &[
        "NOTICE",
        "crates/pangopup-assets/src/error.rs",
        "crates/pangopup-assets/src/input_audit.rs",
        "crates/pangopup-assets/src/snv.rs",
        "crates/pangopup-build/src/command_error.rs",
        "crates/pangopup-build/src/production.rs",
        "crates/pangopup-build/src/snv.rs",
        "crates/pangopup-core/src/lib.rs",
        "crates/pangopup-index/src/snv.rs",
        "dependencies/snv-builder-cargo-lock.v1",
        "dependencies/snv-builder-linux-lock.v1",
        "dependencies/snv-builder-roots.v1",
        "wiring/snv-root-wiring.v1",
    ];
    const REFERENCE_PATHS: &[&str] = &[
        "crates/pangopup-build/src/command_error.rs",
        "crates/pangopup-build/src/reference.rs",
        "crates/pangopup-core/src/lib.rs",
        "crates/pangopup-index/src/reference.rs",
        "dependencies/reference-builder-cargo-lock.v1",
        "dependencies/reference-builder-linux-lock.v1",
        "dependencies/reference-builder-roots.v1",
        "tests/fixtures/pangolin-compat-v1/cases.jsonl",
        "wiring/reference-root-wiring.v1",
    ];
    const REPRESENTATIVE_EXCLUDED: &[&str] = &[
        "crates/pangopup-build/build.rs",
        "crates/pangopup-build/src/lib.rs",
        "crates/pangopup-index/src/mask.rs",
        "crates/pangopup-index/src/lib.rs",
        "crates/pangopup-assets/src/sync.rs",
        "crates/pangopup-assets/src/release.rs",
        "crates/pangopup-assets/src/lib.rs",
        "crates/pangopup-cli/src/main.rs",
        "dependencies/reference-builder-direct-uses.v1",
        "dependencies/snv-builder-direct-uses.v1",
    ];

    #[derive(Clone)]
    struct OwnedEntry {
        path: String,
        bytes: Vec<u8>,
    }

    fn owned(entries: &[Entry<'_>]) -> Vec<OwnedEntry> {
        entries
            .iter()
            .map(|entry| OwnedEntry {
                path: entry.path.to_owned(),
                bytes: entry.bytes.to_vec(),
            })
            .collect()
    }

    fn borrowed(entries: &[OwnedEntry]) -> Vec<Entry<'_>> {
        entries
            .iter()
            .map(|entry| Entry {
                path: &entry.path,
                bytes: &entry.bytes,
            })
            .collect()
    }

    fn candidate_universe() -> BTreeMap<String, Vec<u8>> {
        let mut universe = BTreeMap::new();
        for entry in SNV_ENTRIES.iter().chain(REFERENCE_ENTRIES) {
            if let Some(prior) = universe.insert(entry.path.to_owned(), entry.bytes.to_vec()) {
                assert_eq!(prior, entry.bytes, "shared candidate bytes");
            }
        }
        for path in REPRESENTATIVE_EXCLUDED {
            assert!(
                universe
                    .insert((*path).to_owned(), b"before".to_vec())
                    .is_none(),
                "excluded candidate must not overlap a declared input"
            );
        }
        universe
    }

    fn select(universe: &BTreeMap<String, Vec<u8>>, paths: &[&str]) -> Vec<OwnedEntry> {
        paths
            .iter()
            .map(|path| OwnedEntry {
                path: (*path).to_owned(),
                bytes: universe.get(*path).expect("candidate evidence").clone(),
            })
            .collect()
    }

    fn digest(
        algorithm: &[u8],
        domain: &[u8],
        declaration: &[u8],
        entries: &[OwnedEntry],
    ) -> String {
        fingerprint(algorithm, domain, declaration, &borrowed(entries))
            .expect("valid injected inventory")
    }

    fn oracle_digest(
        algorithm: &[u8],
        domain: &[u8],
        declaration: &[u8],
        entries: &[Entry<'_>],
    ) -> Result<String, FingerprintError> {
        if domain.is_empty() {
            return Err(FingerprintError::EmptyDomain);
        }
        let mut ordered = BTreeMap::new();
        for entry in entries {
            if entry.path.is_empty() {
                return Err(FingerprintError::EmptyPath);
            }
            if ordered.insert(entry.path, entry.bytes).is_some() {
                return Err(FingerprintError::DuplicatePath);
            }
        }

        let mut preimage = Vec::new();
        for bytes in [algorithm, domain, declaration] {
            preimage.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            preimage.extend_from_slice(bytes);
        }
        for (path, bytes) in ordered {
            let path = path.as_bytes();
            preimage.extend_from_slice(&(path.len() as u64).to_le_bytes());
            preimage.extend_from_slice(path);
            preimage.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            preimage.extend_from_slice(bytes);
        }
        Ok(format!("{:x}", Sha256::digest(&preimage)))
    }

    fn mutate(entries: &mut [OwnedEntry], path: &str) {
        let entry = entries
            .iter_mut()
            .find(|entry| entry.path == path)
            .expect("declared path");
        entry.bytes.push(0xa5);
    }

    fn declaration_paths(bytes: &[u8]) -> Vec<&str> {
        let text = std::str::from_utf8(bytes).expect("UTF-8 inventory declaration");
        assert!(text.ends_with('\n'));
        text.lines().collect()
    }

    #[derive(Debug, Eq, PartialEq)]
    struct ProjectedDependency {
        version: String,
        checksum: String,
        features: BTreeSet<String>,
    }

    fn dependency_projection(bytes: &[u8]) -> BTreeMap<String, ProjectedDependency> {
        let text = std::str::from_utf8(bytes).expect("UTF-8 dependency projection");
        let mut projection = BTreeMap::new();
        for line in text.lines() {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 4, "projection row shape");
            let features = if fields[3] == "-" {
                BTreeSet::new()
            } else {
                fields[3].split(',').map(ToOwned::to_owned).collect()
            };
            assert!(
                projection
                    .insert(
                        fields[0].to_owned(),
                        ProjectedDependency {
                            version: fields[1].to_owned(),
                            checksum: fields[2].to_owned(),
                            features,
                        },
                    )
                    .is_none(),
                "duplicate projected package"
            );
        }
        projection
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct DependencyRoot {
        requirement: String,
        resolved_version: String,
        default_features: bool,
        features: BTreeSet<String>,
    }

    fn dependency_roots(bytes: &[u8]) -> BTreeMap<String, DependencyRoot> {
        let text = std::str::from_utf8(bytes).expect("UTF-8 dependency roots");
        let mut roots = BTreeMap::new();
        for line in text.lines() {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 5, "root row shape");
            let default_features = match fields[3] {
                "default" => true,
                "no-default" => false,
                _ => panic!("root default-feature marker"),
            };
            let features = if fields[4] == "-" {
                BTreeSet::new()
            } else {
                fields[4].split(',').map(ToOwned::to_owned).collect()
            };
            assert!(
                roots
                    .insert(
                        fields[0].to_owned(),
                        DependencyRoot {
                            requirement: fields[1].to_owned(),
                            resolved_version: fields[2].to_owned(),
                            default_features,
                            features,
                        },
                    )
                    .is_none(),
                "duplicate dependency root"
            );
        }
        roots
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct DirectUse {
        path: String,
        needle: String,
    }

    fn direct_uses(bytes: &[u8]) -> BTreeMap<String, DirectUse> {
        let text = std::str::from_utf8(bytes).expect("UTF-8 direct uses");
        let mut uses = BTreeMap::new();
        for line in text.lines() {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 3, "direct-use row shape");
            assert!(
                uses.insert(
                    fields[0].to_owned(),
                    DirectUse {
                        path: fields[1].to_owned(),
                        needle: fields[2].to_owned(),
                    },
                )
                .is_none(),
                "duplicate direct-use root"
            );
        }
        uses
    }

    fn raw_literal_end(bytes: &[u8], start: usize) -> Option<usize> {
        let mut cursor = start;
        if matches!(bytes.get(cursor), Some(b'b' | b'c')) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'r') {
            return None;
        }
        cursor += 1;
        let mut hashes = 0;
        while bytes.get(cursor) == Some(&b'#') {
            cursor += 1;
            hashes += 1;
        }
        if bytes.get(cursor) != Some(&b'"') {
            return None;
        }
        cursor += 1;
        while cursor < bytes.len() {
            if bytes[cursor] == b'"'
                && bytes
                    .get(cursor + 1..cursor + 1 + hashes)
                    .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
            {
                return Some(cursor + 1 + hashes);
            }
            cursor += 1;
        }
        Some(bytes.len())
    }

    fn quoted_literal_end(bytes: &[u8], quote: usize) -> usize {
        let mut cursor = quote + 1;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'\\' => cursor = (cursor + 2).min(bytes.len()),
                b'"' => return cursor + 1,
                _ => cursor += 1,
            }
        }
        bytes.len()
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct RustToken {
        text: String,
        identifier: bool,
    }

    impl RustToken {
        fn identifier(&self) -> Option<&str> {
            self.identifier.then_some(self.text.as_str())
        }
    }

    fn rust_tokens(bytes: &[u8]) -> Vec<RustToken> {
        let mut tokens = Vec::new();
        let mut cursor = 0;
        while cursor < bytes.len() {
            if bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
                continue;
            }
            if bytes.get(cursor..cursor + 2) == Some(b"//") {
                cursor += 2;
                while cursor < bytes.len() && bytes[cursor] != b'\n' {
                    cursor += 1;
                }
                continue;
            }
            if bytes.get(cursor..cursor + 2) == Some(b"/*") {
                cursor += 2;
                let mut depth = 1_u64;
                while cursor < bytes.len() && depth != 0 {
                    if bytes.get(cursor..cursor + 2) == Some(b"/*") {
                        depth += 1;
                        cursor += 2;
                    } else if bytes.get(cursor..cursor + 2) == Some(b"*/") {
                        depth -= 1;
                        cursor += 2;
                    } else {
                        cursor += 1;
                    }
                }
                continue;
            }
            if let Some(end) = raw_literal_end(bytes, cursor) {
                tokens.push(RustToken {
                    text: String::from_utf8_lossy(&bytes[cursor..end]).into_owned(),
                    identifier: false,
                });
                cursor = end;
                continue;
            }
            if bytes[cursor] == b'"' {
                let end = quoted_literal_end(bytes, cursor);
                tokens.push(RustToken {
                    text: String::from_utf8_lossy(&bytes[cursor..end]).into_owned(),
                    identifier: false,
                });
                cursor = end;
                continue;
            }
            if matches!(bytes[cursor], b'b' | b'c') && bytes.get(cursor + 1) == Some(&b'"') {
                let end = quoted_literal_end(bytes, cursor + 1);
                tokens.push(RustToken {
                    text: String::from_utf8_lossy(&bytes[cursor..end]).into_owned(),
                    identifier: false,
                });
                cursor = end;
                continue;
            }
            if bytes[cursor].is_ascii_alphabetic() || bytes[cursor] == b'_' {
                let start = cursor;
                cursor += 1;
                while cursor < bytes.len()
                    && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_')
                {
                    cursor += 1;
                }
                tokens.push(RustToken {
                    text: std::str::from_utf8(&bytes[start..cursor])
                        .expect("Rust identifier UTF-8")
                        .to_owned(),
                    identifier: true,
                });
                continue;
            }
            let end = if bytes.get(cursor..cursor + 2) == Some(b"::") {
                cursor + 2
            } else {
                cursor + 1
            };
            tokens.push(RustToken {
                text: String::from_utf8_lossy(&bytes[cursor..end]).into_owned(),
                identifier: false,
            });
            cursor = end;
        }
        tokens
    }

    fn source_path_heads_from_tokens(tokens: &[RustToken]) -> BTreeSet<String> {
        tokens
            .windows(2)
            .filter_map(|pair| {
                (pair[1].text == "::")
                    .then(|| pair[0].identifier().map(ToOwned::to_owned))
                    .flatten()
            })
            .collect()
    }

    fn source_path_heads(bytes: &[u8]) -> BTreeSet<String> {
        source_path_heads_from_tokens(&rust_tokens(bytes))
    }

    fn source_import_roots_and_aliases(
        tokens: &[RustToken],
    ) -> (BTreeSet<String>, BTreeMap<String, BTreeSet<String>>) {
        let mut roots = BTreeSet::new();
        let mut aliases = BTreeMap::<String, BTreeSet<String>>::new();
        let mut cursor = 0;
        while cursor < tokens.len() {
            if tokens[cursor].identifier() == Some("use") {
                let mut item = cursor + 1;
                if tokens.get(item).is_some_and(|token| token.text == "::") {
                    item += 1;
                }
                let Some(origin) = tokens
                    .get(item)
                    .and_then(RustToken::identifier)
                    .map(ToOwned::to_owned)
                else {
                    cursor += 1;
                    continue;
                };
                roots.insert(origin.clone());
                let mut depth = 0_u64;
                cursor = item + 1;
                while cursor < tokens.len() {
                    match tokens[cursor].text.as_str() {
                        "{" | "(" | "[" => depth += 1,
                        "}" | ")" | "]" => depth = depth.saturating_sub(1),
                        ";" if depth == 0 => break,
                        "as" => {
                            if let Some(alias) = tokens
                                .get(cursor + 1)
                                .and_then(RustToken::identifier)
                                .filter(|alias| *alias != "_")
                            {
                                aliases
                                    .entry(alias.to_owned())
                                    .or_default()
                                    .insert(origin.clone());
                            }
                        }
                        _ => {}
                    }
                    cursor += 1;
                }
            } else if tokens[cursor].identifier() == Some("extern")
                && tokens.get(cursor + 1).and_then(RustToken::identifier) == Some("crate")
            {
                let Some(origin) = tokens
                    .get(cursor + 2)
                    .and_then(RustToken::identifier)
                    .map(ToOwned::to_owned)
                else {
                    cursor += 1;
                    continue;
                };
                roots.insert(origin.clone());
                if tokens.get(cursor + 3).and_then(RustToken::identifier) == Some("as")
                    && let Some(alias) = tokens
                        .get(cursor + 4)
                        .and_then(RustToken::identifier)
                        .filter(|alias| *alias != "_")
                {
                    aliases.entry(alias.to_owned()).or_default().insert(origin);
                }
            }
            cursor += 1;
        }
        (roots, aliases)
    }

    fn source_dependency_candidates(bytes: &[u8]) -> BTreeSet<String> {
        let tokens = rust_tokens(bytes);
        let mut candidates = source_path_heads_from_tokens(&tokens);
        let (roots, aliases) = source_import_roots_and_aliases(&tokens);
        candidates.extend(roots);
        let mut pending: Vec<_> = candidates.iter().cloned().collect();
        while let Some(candidate) = pending.pop() {
            if let Some(origins) = aliases.get(&candidate) {
                for origin in origins {
                    if candidates.insert(origin.clone()) {
                        pending.push(origin.clone());
                    }
                }
            }
        }
        candidates
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ManifestDependency {
        package_name: String,
        requirement: String,
        default_features: bool,
        features: BTreeSet<String>,
        target: Option<String>,
    }

    type ManifestDependencies = BTreeMap<String, BTreeMap<String, ManifestDependency>>;
    type WorkspaceDependencies = BTreeMap<String, BTreeMap<String, String>>;

    #[derive(Clone, Debug)]
    struct ManifestModel {
        registry: ManifestDependencies,
        workspace: WorkspaceDependencies,
    }

    fn load_manifest_model() -> ManifestModel {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output =
            std::process::Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
                .args([
                    "metadata",
                    "--locked",
                    "--no-deps",
                    "--filter-platform",
                    "x86_64-unknown-linux-gnu",
                    "--format-version",
                    "1",
                ])
                .current_dir(workspace)
                .output()
                .expect("read actual Pangopup Cargo manifests");
        assert!(
            output.status.success(),
            "Cargo manifest metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("Cargo manifest metadata JSON");
        let mut registry_packages = ManifestDependencies::new();
        let mut workspace_packages = WorkspaceDependencies::new();
        for package in metadata["packages"].as_array().expect("manifest packages") {
            let package_name = package["name"]
                .as_str()
                .expect("manifest package name")
                .to_owned();
            let manifest_path = package["manifest_path"].as_str().expect("manifest path");
            assert!(
                manifest_path.ends_with(&format!("crates/{package_name}/Cargo.toml")),
                "{package_name} must come from its actual Pangopup Cargo manifest"
            );
            let mut registry_dependencies = BTreeMap::new();
            let mut workspace_dependencies = BTreeMap::new();
            for dependency in package["dependencies"]
                .as_array()
                .expect("manifest dependencies")
            {
                if !dependency["kind"].is_null() {
                    continue;
                }
                let package_dependency = dependency["name"]
                    .as_str()
                    .expect("dependency package name");
                let import_name = dependency["rename"]
                    .as_str()
                    .unwrap_or(package_dependency)
                    .replace('-', "_");
                if dependency["source"].is_null()
                    && dependency["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with(&format!("crates/{package_dependency}")))
                {
                    assert!(
                        workspace_dependencies
                            .insert(import_name, package_dependency.to_owned())
                            .is_none(),
                        "duplicate workspace dependency import"
                    );
                    continue;
                }
                if !dependency["source"]
                    .as_str()
                    .is_some_and(|source| source.starts_with("registry+"))
                {
                    continue;
                }
                let features = dependency["features"]
                    .as_array()
                    .expect("manifest dependency features")
                    .iter()
                    .map(|feature| {
                        feature
                            .as_str()
                            .expect("manifest dependency feature")
                            .to_owned()
                    })
                    .collect();
                assert!(
                    registry_dependencies
                        .insert(
                            import_name,
                            ManifestDependency {
                                package_name: package_dependency.to_owned(),
                                requirement: dependency["req"]
                                    .as_str()
                                    .expect("dependency requirement")
                                    .to_owned(),
                                default_features: dependency["uses_default_features"]
                                    .as_bool()
                                    .expect("dependency default features"),
                                features,
                                target: dependency["target"].as_str().map(ToOwned::to_owned),
                            },
                        )
                        .is_none(),
                    "duplicate manifest dependency import"
                );
            }
            assert!(
                registry_packages
                    .insert(package_name.clone(), registry_dependencies)
                    .is_none(),
                "duplicate Pangopup package"
            );
            assert!(
                workspace_packages
                    .insert(package_name, workspace_dependencies)
                    .is_none(),
                "duplicate Pangopup workspace package"
            );
        }
        ManifestModel {
            registry: registry_packages,
            workspace: workspace_packages,
        }
    }

    fn actual_manifest_dependencies() -> ManifestDependencies {
        MANIFEST_MODEL
            .get_or_init(load_manifest_model)
            .registry
            .clone()
    }

    fn actual_workspace_dependencies() -> WorkspaceDependencies {
        MANIFEST_MODEL
            .get_or_init(load_manifest_model)
            .workspace
            .clone()
    }

    fn source_owner(path: &str) -> Result<&str, String> {
        let relative = path
            .strip_prefix("crates/")
            .ok_or_else(|| format!("{path} is not crate-owned Rust source"))?;
        let (owner, source) = relative
            .split_once('/')
            .ok_or_else(|| format!("{path} lacks a crate owner"))?;
        if !source.starts_with("src/") {
            return Err(format!("{path} is not crate source"));
        }
        Ok(owner)
    }

    #[derive(Clone, Debug)]
    struct RootSource {
        path: String,
        bytes: Vec<u8>,
    }

    #[derive(Clone, Debug)]
    enum RootItem {
        Module {
            name: String,
            public: bool,
            normalized: String,
        },
        Reexport {
            source: String,
            exports: BTreeSet<String>,
            wildcard: bool,
            normalized: String,
        },
    }

    fn actual_root_sources() -> BTreeMap<String, RootSource> {
        [
            (
                "pangopup-assets",
                "crates/pangopup-assets/src/lib.rs",
                include_bytes!("../../pangopup-assets/src/lib.rs").as_slice(),
            ),
            (
                "pangopup-build",
                "crates/pangopup-build/src/lib.rs",
                include_bytes!("lib.rs").as_slice(),
            ),
            (
                "pangopup-index",
                "crates/pangopup-index/src/lib.rs",
                include_bytes!("../../pangopup-index/src/lib.rs").as_slice(),
            ),
        ]
        .into_iter()
        .map(|(package, path, bytes)| {
            (
                package.to_owned(),
                RootSource {
                    path: path.to_owned(),
                    bytes: bytes.to_vec(),
                },
            )
        })
        .collect()
    }

    fn matching_group_end(tokens: &[RustToken], open: usize) -> Result<usize, String> {
        let (open_text, close_text) = match tokens.get(open).map(|token| token.text.as_str()) {
            Some("(") => ("(", ")"),
            Some("[") => ("[", "]"),
            Some("{") => ("{", "}"),
            _ => return Err("group does not start with a delimiter".to_owned()),
        };
        let mut depth = 0_u64;
        for (cursor, token) in tokens.iter().enumerate().skip(open) {
            if token.text == open_text {
                depth += 1;
            } else if token.text == close_text {
                depth -= 1;
                if depth == 0 {
                    return Ok(cursor);
                }
            }
        }
        Err("unterminated Rust group".to_owned())
    }

    fn normalized_tokens(tokens: &[RustToken]) -> String {
        tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn reexport_exports(tokens: &[RustToken]) -> Result<(String, BTreeSet<String>, bool), String> {
        let mut cursor = 0;
        if tokens.get(cursor).is_some_and(|token| token.text == "::") {
            cursor += 1;
        }
        let source = tokens
            .get(cursor)
            .and_then(RustToken::identifier)
            .ok_or_else(|| "root re-export lacks a source module".to_owned())?
            .to_owned();
        cursor += 1;
        if tokens.get(cursor).is_none_or(|token| token.text != "::") {
            return Err("root re-export must name a source item".to_owned());
        }
        cursor += 1;
        if tokens.get(cursor).is_some_and(|token| token.text == "*") {
            return Ok((source, BTreeSet::new(), true));
        }

        let mut exports = BTreeSet::new();
        if tokens.get(cursor).is_some_and(|token| token.text == "{") {
            let end = matching_group_end(tokens, cursor)?;
            let mut segment = cursor + 1;
            let mut depth = 0_u64;
            for boundary in cursor + 1..=end {
                match tokens[boundary].text.as_str() {
                    "{" | "(" | "[" => depth += 1,
                    "}" if depth == 0 => {
                        if segment < boundary {
                            collect_reexport_name(&tokens[segment..boundary], &mut exports)?;
                        }
                        break;
                    }
                    "}" | ")" | "]" => depth = depth.saturating_sub(1),
                    "," if depth == 0 => {
                        if segment < boundary {
                            collect_reexport_name(&tokens[segment..boundary], &mut exports)?;
                        }
                        segment = boundary + 1;
                    }
                    _ => {}
                }
            }
        } else {
            collect_reexport_name(&tokens[cursor..], &mut exports)?;
        }
        Ok((source, exports, false))
    }

    fn collect_reexport_name(
        tokens: &[RustToken],
        exports: &mut BTreeSet<String>,
    ) -> Result<(), String> {
        if tokens.is_empty() {
            return Ok(());
        }
        let alias = tokens.windows(2).find_map(|pair| {
            (pair[0].identifier() == Some("as"))
                .then(|| pair[1].identifier().map(ToOwned::to_owned))
                .flatten()
        });
        let name = alias.or_else(|| {
            tokens
                .iter()
                .rev()
                .find_map(RustToken::identifier)
                .filter(|name| *name != "self")
                .map(ToOwned::to_owned)
        });
        let Some(name) = name else {
            return Err("root re-export lacks an exported name".to_owned());
        };
        if !exports.insert(name) {
            return Err("root re-export repeats an exported name".to_owned());
        }
        Ok(())
    }

    fn skip_unknown_root_item(tokens: &[RustToken], start: usize) -> Result<usize, String> {
        let mut cursor = start;
        let mut parentheses = 0_u64;
        let mut brackets = 0_u64;
        while cursor < tokens.len() {
            match tokens[cursor].text.as_str() {
                "(" => parentheses += 1,
                ")" => parentheses = parentheses.saturating_sub(1),
                "[" => brackets += 1,
                "]" => brackets = brackets.saturating_sub(1),
                "{" if parentheses == 0 && brackets == 0 => {
                    let end = matching_group_end(tokens, cursor)?;
                    return Ok(end
                        + 1
                        + usize::from(tokens.get(end + 1).is_some_and(|token| token.text == ";")));
                }
                ";" if parentheses == 0 && brackets == 0 => return Ok(cursor + 1),
                _ => {}
            }
            cursor += 1;
        }
        Ok(tokens.len())
    }

    fn parse_root_items(bytes: &[u8]) -> Result<Vec<RootItem>, String> {
        let tokens = rust_tokens(bytes);
        let mut items = Vec::new();
        let mut cursor = 0;
        while cursor < tokens.len() {
            let start = cursor;
            while tokens.get(cursor).is_some_and(|token| token.text == "#") {
                cursor += 1;
                if tokens.get(cursor).is_some_and(|token| token.text == "!") {
                    cursor += 1;
                }
                if tokens.get(cursor).is_none_or(|token| token.text != "[") {
                    return Err("root attribute lacks a bracketed body".to_owned());
                }
                cursor = matching_group_end(&tokens, cursor)? + 1;
            }

            let mut public = false;
            if tokens.get(cursor).and_then(RustToken::identifier) == Some("pub") {
                public = true;
                cursor += 1;
                if tokens.get(cursor).is_some_and(|token| token.text == "(") {
                    public = false;
                    cursor = matching_group_end(&tokens, cursor)? + 1;
                }
            }

            if tokens.get(cursor).and_then(RustToken::identifier) == Some("mod") {
                let name = tokens
                    .get(cursor + 1)
                    .and_then(RustToken::identifier)
                    .ok_or_else(|| "root module lacks a name".to_owned())?
                    .to_owned();
                if tokens
                    .get(cursor + 2)
                    .is_some_and(|token| token.text == ";")
                {
                    let end = cursor + 2;
                    items.push(RootItem::Module {
                        name,
                        public,
                        normalized: normalized_tokens(&tokens[start..=end]),
                    });
                    cursor = end + 1;
                    continue;
                }
            } else if public && tokens.get(cursor).and_then(RustToken::identifier) == Some("use") {
                let mut end = cursor + 1;
                let mut braces = 0_u64;
                while end < tokens.len() {
                    match tokens[end].text.as_str() {
                        "{" => braces += 1,
                        "}" => braces = braces.saturating_sub(1),
                        ";" if braces == 0 => break,
                        _ => {}
                    }
                    end += 1;
                }
                if end == tokens.len() {
                    return Err("unterminated root re-export".to_owned());
                }
                let (source, exports, wildcard) = reexport_exports(&tokens[cursor + 1..end])?;
                items.push(RootItem::Reexport {
                    source,
                    exports,
                    wildcard,
                    normalized: normalized_tokens(&tokens[start..=end]),
                });
                cursor = end + 1;
                continue;
            }
            cursor = skip_unknown_root_item(&tokens, start)?;
        }
        Ok(items)
    }

    fn selected_root_modules(
        entries: &[Entry<'_>],
    ) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
        let mut modules = BTreeMap::<String, BTreeSet<String>>::new();
        for entry in entries.iter().filter(|entry| entry.path.ends_with(".rs")) {
            let owner = source_owner(entry.path)?;
            let source = entry
                .path
                .strip_prefix(&format!("crates/{owner}/src/"))
                .ok_or_else(|| format!("{} is outside its owner source", entry.path))?;
            if !source.contains('/') && source != "lib.rs" {
                modules
                    .entry(owner.to_owned())
                    .or_default()
                    .insert(source.trim_end_matches(".rs").to_owned());
            }
            let tokens = rust_tokens(entry.bytes);
            for triple in tokens.windows(3) {
                if triple[0].identifier() == Some("crate")
                    && triple[1].text == "::"
                    && let Some(module) = triple[2].identifier()
                {
                    modules
                        .entry(owner.to_owned())
                        .or_default()
                        .insert(module.to_owned());
                }
            }
        }
        Ok(modules)
    }

    fn alias_origins(head: &str, aliases: &BTreeMap<String, BTreeSet<String>>) -> BTreeSet<String> {
        let mut origins = BTreeSet::from([head.to_owned()]);
        let mut pending = vec![head.to_owned()];
        while let Some(candidate) = pending.pop() {
            if let Some(next) = aliases.get(&candidate) {
                for origin in next {
                    if origins.insert(origin.clone()) {
                        pending.push(origin.clone());
                    }
                }
            }
        }
        origins
    }

    fn root_symbols_after(tokens: &[RustToken], head: usize) -> BTreeSet<String> {
        let mut symbols = BTreeSet::new();
        let Some(next) = tokens.get(head + 2) else {
            return symbols;
        };
        if let Some(symbol) = next.identifier() {
            symbols.insert(symbol.to_owned());
            return symbols;
        }
        if next.text != "{" {
            return symbols;
        }
        let Ok(end) = matching_group_end(tokens, head + 2) else {
            return symbols;
        };
        let mut depth = 0_u64;
        let mut at_segment_start = true;
        for token in &tokens[head + 3..end] {
            match token.text.as_str() {
                "{" | "(" | "[" => depth += 1,
                "}" | ")" | "]" => depth = depth.saturating_sub(1),
                "," if depth == 0 => at_segment_start = true,
                _ if depth == 0 && at_segment_start => {
                    if let Some(symbol) = token.identifier()
                        && symbol != "self"
                    {
                        symbols.insert(symbol.to_owned());
                        at_segment_start = false;
                    }
                }
                _ => {}
            }
        }
        symbols
    }

    fn workspace_root_requests(
        entries: &[Entry<'_>],
        workspace: &WorkspaceDependencies,
    ) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
        let selected_roots: BTreeSet<_> = entries
            .iter()
            .filter_map(|entry| {
                entry
                    .path
                    .strip_prefix("crates/")
                    .and_then(|path| path.strip_suffix("/src/lib.rs"))
            })
            .collect();
        let mut requests = BTreeMap::<String, BTreeSet<String>>::new();
        for entry in entries.iter().filter(|entry| entry.path.ends_with(".rs")) {
            let owner = source_owner(entry.path)?;
            let declarations = workspace
                .get(owner)
                .ok_or_else(|| format!("{owner} workspace dependencies are absent"))?;
            let tokens = rust_tokens(entry.bytes);
            let (_, aliases) = source_import_roots_and_aliases(&tokens);
            for (head, pair) in tokens.windows(2).enumerate() {
                let Some(identifier) = pair[0].identifier() else {
                    continue;
                };
                if pair[1].text != "::" {
                    continue;
                }
                let symbols = root_symbols_after(&tokens, head);
                if symbols.is_empty() {
                    continue;
                }
                for origin in alias_origins(identifier, &aliases) {
                    let Some(package) = declarations.get(&origin) else {
                        continue;
                    };
                    if selected_roots.contains(package.as_str()) {
                        continue;
                    }
                    requests
                        .entry(package.clone())
                        .or_default()
                        .extend(symbols.iter().cloned());
                }
            }
        }
        Ok(requests)
    }

    fn derive_root_wiring(
        entries: &[Entry<'_>],
        roots: &BTreeMap<String, RootSource>,
        workspace: &WorkspaceDependencies,
    ) -> Result<BTreeSet<String>, String> {
        let required_modules = selected_root_modules(entries)?;
        let requests = workspace_root_requests(entries, workspace)?;
        let mut projection = BTreeSet::new();
        let mut parsed = BTreeMap::<String, Vec<RootItem>>::new();
        for package in required_modules.keys().chain(requests.keys()) {
            if parsed.contains_key(package) {
                continue;
            }
            let root = roots
                .get(package)
                .ok_or_else(|| format!("{package} root source is absent"))?;
            parsed.insert(package.clone(), parse_root_items(&root.bytes)?);
        }

        for (package, modules) in &required_modules {
            let root = &roots[package];
            let items = &parsed[package];
            for module in modules {
                let matches: Vec<_> = items
                    .iter()
                    .filter_map(|item| match item {
                        RootItem::Module {
                            name, normalized, ..
                        } if name == module => Some(normalized),
                        _ => None,
                    })
                    .collect();
                if matches.len() != 1 {
                    return Err(format!(
                        "{package} must have one root declaration for module {module}"
                    ));
                }
                projection.insert(format!("{}\t{}", root.path, matches[0]));
            }
        }

        for (package, symbols) in requests {
            let root = &roots[&package];
            let items = &parsed[&package];
            let selected_modules = required_modules.get(&package).cloned().unwrap_or_default();
            for symbol in symbols {
                let matches: Vec<_> = items
                    .iter()
                    .filter_map(|item| match item {
                        RootItem::Module {
                            name,
                            public,
                            normalized,
                        } if *public && name == &symbol => Some(normalized),
                        RootItem::Reexport {
                            source,
                            exports,
                            wildcard,
                            normalized,
                            ..
                        } if (*wildcard && selected_modules.contains(source))
                            || exports.contains(&symbol) =>
                        {
                            Some(normalized)
                        }
                        _ => None,
                    })
                    .collect();
                if matches.len() != 1 {
                    return Err(format!(
                        "{package} must expose {symbol} through one root wiring item"
                    ));
                }
                projection.insert(format!("{}\t{}", root.path, matches[0]));
            }
        }
        Ok(projection)
    }

    fn checked_root_wiring(bytes: &[u8]) -> BTreeSet<String> {
        let text = std::str::from_utf8(bytes).expect("UTF-8 root wiring projection");
        assert!(text.ends_with('\n'));
        let lines: Vec<_> = text.lines().collect();
        assert!(lines.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            lines.len(),
            lines.iter().copied().collect::<BTreeSet<_>>().len()
        );
        lines.into_iter().map(ToOwned::to_owned).collect()
    }

    fn derive_external_dependencies(
        entries: &[Entry<'_>],
        manifests: &ManifestDependencies,
    ) -> Result<BTreeMap<String, ManifestDependency>, String> {
        let mut derived = BTreeMap::new();
        for entry in entries.iter().filter(|entry| entry.path.ends_with(".rs")) {
            let owner = source_owner(entry.path)?;
            let declarations = manifests.get(owner).ok_or_else(|| {
                format!(
                    "{entry_path} owner manifest is absent",
                    entry_path = entry.path
                )
            })?;
            for head in source_dependency_candidates(entry.bytes) {
                let Some(dependency) = declarations.get(&head) else {
                    continue;
                };
                if dependency.target.is_some() {
                    return Err(format!(
                        "{} uses a target-qualified dependency unsupported by v1 derivation",
                        dependency.package_name
                    ));
                }
                if let Some(prior) =
                    derived.insert(dependency.package_name.clone(), dependency.clone())
                    && prior != *dependency
                {
                    return Err(format!(
                        "{} has inconsistent causal declarations",
                        dependency.package_name
                    ));
                }
            }
        }
        Ok(derived)
    }

    fn source_manifest_dependency_contract(
        roots: &BTreeMap<String, DependencyRoot>,
        entries: &[Entry<'_>],
        manifests: &ManifestDependencies,
    ) -> Result<BTreeMap<String, ManifestDependency>, String> {
        let derived = derive_external_dependencies(entries, manifests)?;
        if roots.keys().ne(derived.keys()) {
            return Err("derived external dependencies and roots differ".to_owned());
        }
        for (name, dependency) in &derived {
            let root = &roots[name];
            if root.requirement != dependency.requirement {
                return Err(format!("{name} version requirement differs"));
            }
            if root.default_features != dependency.default_features {
                return Err(format!("{name} default-features differs"));
            }
            if root.features != dependency.features {
                return Err(format!("{name} feature list differs"));
            }
        }
        Ok(derived)
    }

    fn diagnostic_uses_match(
        uses: &BTreeMap<String, DirectUse>,
        derived: &BTreeMap<String, ManifestDependency>,
        entries: &[Entry<'_>],
    ) -> Result<(), String> {
        if uses.keys().ne(derived.keys()) {
            return Err("diagnostic uses and derived dependencies differ".to_owned());
        }
        for (name, witness) in uses {
            let source = entries
                .iter()
                .find(|entry| entry.path == witness.path)
                .ok_or_else(|| format!("{name} diagnostic path is not inventoried"))?;
            if !source
                .bytes
                .windows(witness.needle.len())
                .any(|window| window == witness.needle.as_bytes())
            {
                return Err(format!("{name} diagnostic witness is absent"));
            }
        }
        Ok(())
    }

    fn cargo_lock_checksums(bytes: &str) -> BTreeMap<(String, String), String> {
        fn quoted(block: &str, key: &str) -> Option<String> {
            let prefix = format!("{key} = \"");
            block.lines().find_map(|line| {
                line.strip_prefix(&prefix)
                    .and_then(|value| value.strip_suffix('"'))
                    .map(ToOwned::to_owned)
            })
        }

        bytes
            .split("[[package]]")
            .filter_map(|block| {
                Some((
                    (quoted(block, "name")?, quoted(block, "version")?),
                    quoted(block, "checksum")?,
                ))
            })
            .collect()
    }

    fn resolver_manifest(package: &str, roots: &BTreeMap<String, DependencyRoot>) -> String {
        let mut manifest = format!(
            "[package]\nname = \"{package}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\n"
        );
        for (name, root) in roots {
            let features = root
                .features
                .iter()
                .map(|feature| format!("\"{feature}\""))
                .collect::<Vec<_>>()
                .join(", ");
            manifest.push_str(&format!(
                "{name} = {{ version = \"{}\", default-features = {}, features = [{}] }}\n",
                root.requirement, root.default_features, features
            ));
        }
        manifest.push_str("\n[workspace]\n");
        manifest
    }

    struct ResolverTemp(PathBuf);

    impl ResolverTemp {
        fn new(label: &str, manifest: &str, cargo_lock: &[u8]) -> Self {
            let serial = RESOLVER_SERIAL.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pangopup-builder-resolver-{}-{serial}-{label}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create isolated resolver");
            fs::create_dir(path.join("src")).expect("create isolated resolver source");
            fs::write(path.join("Cargo.toml"), manifest).expect("write isolated manifest");
            fs::write(path.join("Cargo.lock"), cargo_lock).expect("write isolated lock");
            fs::write(path.join("src/lib.rs"), b"").expect("write isolated target");
            Self(path)
        }
    }

    impl Drop for ResolverTemp {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated resolver");
        }
    }

    fn assert_projection_matches_isolated_resolution(
        package: &str,
        projection: &[u8],
        roots: &[u8],
        cargo_lock: &[u8],
    ) {
        let roots = dependency_roots(roots);
        let manifest = resolver_manifest(package, &roots);
        let temp = ResolverTemp::new(package, &manifest, cargo_lock);
        let output =
            std::process::Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
                .args([
                    "metadata",
                    "--locked",
                    "--offline",
                    "--filter-platform",
                    "x86_64-unknown-linux-gnu",
                    "--format-version",
                    "1",
                    "--manifest-path",
                ])
                .arg(temp.0.join("Cargo.toml"))
                .current_dir(&temp.0)
                .output()
                .expect("run Cargo's isolated resolver");
        assert!(
            output.status.success(),
            "isolated cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("Cargo metadata JSON");
        let packages = metadata["packages"].as_array().expect("metadata packages");
        let nodes = metadata["resolve"]["nodes"]
            .as_array()
            .expect("metadata resolve nodes");

        let mut package_by_id = BTreeMap::new();
        for package in packages {
            package_by_id.insert(
                package["id"].as_str().expect("package id").to_owned(),
                package,
            );
        }
        let mut node_by_id = BTreeMap::new();
        for node in nodes {
            node_by_id.insert(node["id"].as_str().expect("node id").to_owned(), node);
        }

        let root_package = packages
            .iter()
            .find(|candidate| {
                candidate["name"].as_str() == Some(package) && candidate["source"].is_null()
            })
            .expect("isolated root package");
        let root_id = root_package["id"].as_str().expect("isolated root id");
        let root_node = node_by_id.get(root_id).expect("isolated root node");
        let direct_names: BTreeSet<_> = root_node["deps"]
            .as_array()
            .expect("isolated direct dependencies")
            .iter()
            .map(|dependency| {
                let id = dependency["pkg"].as_str().expect("direct dependency id");
                package_by_id[id]["name"]
                    .as_str()
                    .expect("direct dependency name")
                    .to_owned()
            })
            .collect();
        assert_eq!(
            direct_names,
            roots.keys().cloned().collect(),
            "isolated direct dependency roots"
        );

        let mut closure = BTreeSet::new();
        let mut pending = vec![root_id.to_owned()];
        while let Some(id) = pending.pop() {
            if !closure.insert(id.clone()) {
                continue;
            }
            let node = node_by_id.get(&id).expect("resolved root node");
            for dependency in node["deps"].as_array().expect("resolved dependencies") {
                let dependency_id = dependency["pkg"].as_str().expect("dependency id");
                pending.push(dependency_id.to_owned());
            }
        }
        closure.remove(root_id);

        let projected = dependency_projection(projection);
        for (name, root) in &roots {
            assert_eq!(
                projected
                    .get(name)
                    .map(|dependency| dependency.version.as_str()),
                Some(root.resolved_version.as_str()),
                "{name} resolved root version"
            );
        }
        let isolated_lock =
            std::str::from_utf8(cargo_lock).expect("UTF-8 isolated Cargo lock evidence");
        let isolated_checksums = cargo_lock_checksums(isolated_lock);
        let workspace_checksums = cargo_lock_checksums(include_str!("../../../Cargo.lock"));
        let actual_names: BTreeSet<_> = closure
            .iter()
            .map(|id| {
                package_by_id[id]["name"]
                    .as_str()
                    .expect("dependency name")
                    .to_owned()
            })
            .collect();
        assert_eq!(
            actual_names,
            projected.keys().cloned().collect(),
            "projected package closure"
        );

        for id in closure {
            let package = package_by_id[&id];
            let name = package["name"].as_str().expect("package name");
            let expected = &projected[name];
            assert_eq!(
                package["version"].as_str(),
                Some(expected.version.as_str()),
                "{name} version"
            );
            assert_eq!(
                isolated_checksums.get(&(name.to_owned(), expected.version.clone())),
                Some(&expected.checksum),
                "{name} isolated checksum"
            );
            assert_eq!(
                workspace_checksums.get(&(name.to_owned(), expected.version.clone())),
                Some(&expected.checksum),
                "{name} workspace-lock checksum"
            );
            let actual_features: BTreeSet<_> = node_by_id[&id]["features"]
                .as_array()
                .expect("node features")
                .iter()
                .map(|feature| feature.as_str().expect("feature").to_owned())
                .collect();
            assert_eq!(actual_features, expected.features, "{name} features");
        }
    }

    #[test]
    fn source_fingerprint_compiled_inventories_are_canonical_complete_and_distinct() {
        let snv_paths: Vec<_> = SNV_ENTRIES.iter().map(|entry| entry.path).collect();
        let reference_paths: Vec<_> = REFERENCE_ENTRIES.iter().map(|entry| entry.path).collect();
        assert_eq!(snv_paths, SNV_PATHS);
        assert_eq!(reference_paths, REFERENCE_PATHS);
        assert_eq!(declaration_paths(SNV_INVENTORY_DECLARATION), SNV_PATHS);
        assert_eq!(
            declaration_paths(REFERENCE_INVENTORY_DECLARATION),
            REFERENCE_PATHS
        );
        assert!(snv_paths.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(reference_paths.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            snv_source_sha256(),
            EXPECTED_SNV_SHA256,
            "hard SNV source fingerprint"
        );
        assert_eq!(
            oracle_digest(
                ALGORITHM,
                SNV_DOMAIN,
                SNV_INVENTORY_DECLARATION,
                SNV_ENTRIES
            )
            .expect("SNV oracle"),
            EXPECTED_SNV_SHA256,
            "independent SNV oracle"
        );
        assert_eq!(
            reference_source_sha256(),
            EXPECTED_REFERENCE_SHA256,
            "hard reference source fingerprint"
        );
        assert_eq!(
            oracle_digest(
                ALGORITHM,
                REFERENCE_DOMAIN,
                REFERENCE_INVENTORY_DECLARATION,
                REFERENCE_ENTRIES
            )
            .expect("reference oracle"),
            EXPECTED_REFERENCE_SHA256,
            "independent reference oracle"
        );
        assert_ne!(snv_source_sha256(), reference_source_sha256());

        let declared: BTreeSet<_> = snv_paths
            .iter()
            .chain(reference_paths.iter())
            .copied()
            .collect();
        for path in REPRESENTATIVE_EXCLUDED {
            assert!(!declared.contains(path));
        }
    }

    #[test]
    fn source_fingerprint_family_inputs_are_discriminating() {
        let snv_baseline = snv_source_sha256();
        let reference_baseline = reference_source_sha256();
        let shared = [
            "crates/pangopup-build/src/command_error.rs",
            "crates/pangopup-core/src/lib.rs",
        ];

        for path in SNV_PATHS
            .iter()
            .copied()
            .filter(|path| !shared.contains(path))
        {
            let mut candidate = owned(SNV_ENTRIES);
            mutate(&mut candidate, path);
            assert_ne!(
                digest(ALGORITHM, SNV_DOMAIN, SNV_INVENTORY_DECLARATION, &candidate),
                snv_baseline,
                "{path}"
            );
            assert_eq!(reference_source_sha256(), reference_baseline, "{path}");
        }
        for path in REFERENCE_PATHS
            .iter()
            .copied()
            .filter(|path| !shared.contains(path))
        {
            let mut candidate = owned(REFERENCE_ENTRIES);
            mutate(&mut candidate, path);
            assert_ne!(
                digest(
                    ALGORITHM,
                    REFERENCE_DOMAIN,
                    REFERENCE_INVENTORY_DECLARATION,
                    &candidate
                ),
                reference_baseline,
                "{path}"
            );
            assert_eq!(snv_source_sha256(), snv_baseline, "{path}");
        }
        for path in shared {
            let mut snv = owned(SNV_ENTRIES);
            let mut reference = owned(REFERENCE_ENTRIES);
            mutate(&mut snv, path);
            mutate(&mut reference, path);
            assert_ne!(
                digest(ALGORITHM, SNV_DOMAIN, SNV_INVENTORY_DECLARATION, &snv),
                snv_baseline,
                "{path}"
            );
            assert_ne!(
                digest(
                    ALGORITHM,
                    REFERENCE_DOMAIN,
                    REFERENCE_INVENTORY_DECLARATION,
                    &reference
                ),
                reference_baseline,
                "{path}"
            );
        }

        let changed_algorithm = [ALGORITHM, b"changed"].concat();
        assert_ne!(
            digest(
                &changed_algorithm,
                SNV_DOMAIN,
                SNV_INVENTORY_DECLARATION,
                &owned(SNV_ENTRIES)
            ),
            snv_baseline
        );
        assert_ne!(
            digest(
                &changed_algorithm,
                REFERENCE_DOMAIN,
                REFERENCE_INVENTORY_DECLARATION,
                &owned(REFERENCE_ENTRIES)
            ),
            reference_baseline
        );
    }

    #[test]
    fn source_fingerprint_order_duplicate_and_inventory_edits_are_controlled() {
        let baseline = snv_source_sha256();
        let mut reordered = owned(SNV_ENTRIES);
        reordered.reverse();
        assert_eq!(
            digest(ALGORITHM, SNV_DOMAIN, SNV_INVENTORY_DECLARATION, &reordered),
            baseline
        );

        let mut duplicate = owned(SNV_ENTRIES);
        duplicate.push(duplicate[0].clone());
        assert_eq!(
            fingerprint(
                ALGORITHM,
                SNV_DOMAIN,
                SNV_INVENTORY_DECLARATION,
                &borrowed(&duplicate)
            ),
            Err(FingerprintError::DuplicatePath)
        );

        let mut added = owned(SNV_ENTRIES);
        added.push(OwnedEntry {
            path: "new/source".to_owned(),
            bytes: b"new".to_vec(),
        });
        assert_ne!(
            digest(ALGORITHM, SNV_DOMAIN, SNV_INVENTORY_DECLARATION, &added),
            baseline
        );

        let mut removed = owned(SNV_ENTRIES);
        removed.pop();
        assert_ne!(
            digest(ALGORITHM, SNV_DOMAIN, SNV_INVENTORY_DECLARATION, &removed),
            baseline
        );

        let mut renamed = owned(SNV_ENTRIES);
        renamed[0].path.push_str(".renamed");
        assert_ne!(
            digest(ALGORITHM, SNV_DOMAIN, SNV_INVENTORY_DECLARATION, &renamed),
            baseline
        );
        assert_ne!(
            fingerprint(
                ALGORITHM,
                b"pangopup.snv-builder-source.v2",
                SNV_INVENTORY_DECLARATION,
                SNV_ENTRIES
            )
            .expect("versioned domain"),
            baseline
        );
        let changed_declaration = [SNV_INVENTORY_DECLARATION, b"changed\n"].concat();
        assert_ne!(
            fingerprint(ALGORITHM, SNV_DOMAIN, &changed_declaration, SNV_ENTRIES)
                .expect("changed declaration"),
            baseline
        );
    }

    #[test]
    fn source_fingerprint_excluded_bytes_change_neither_family() {
        let mut universe = candidate_universe();
        let snv_baseline = digest(
            ALGORITHM,
            SNV_DOMAIN,
            SNV_INVENTORY_DECLARATION,
            &select(&universe, SNV_PATHS),
        );
        let reference_baseline = digest(
            ALGORITHM,
            REFERENCE_DOMAIN,
            REFERENCE_INVENTORY_DECLARATION,
            &select(&universe, REFERENCE_PATHS),
        );
        for path in REPRESENTATIVE_EXCLUDED {
            universe
                .get_mut(*path)
                .expect("injected excluded evidence")
                .push(0xa5);
            assert_eq!(
                digest(
                    ALGORITHM,
                    SNV_DOMAIN,
                    SNV_INVENTORY_DECLARATION,
                    &select(&universe, SNV_PATHS)
                ),
                snv_baseline,
                "{path}"
            );
            assert_eq!(
                digest(
                    ALGORITHM,
                    REFERENCE_DOMAIN,
                    REFERENCE_INVENTORY_DECLARATION,
                    &select(&universe, REFERENCE_PATHS)
                ),
                reference_baseline,
                "{path}"
            );
        }
    }

    #[test]
    fn source_fingerprint_dependency_projections_are_canonical() {
        for bytes in [
            include_bytes!("source_fingerprint/snv-builder-linux-lock.v1").as_slice(),
            include_bytes!("source_fingerprint/reference-builder-linux-lock.v1").as_slice(),
        ] {
            let text = std::str::from_utf8(bytes).expect("UTF-8 dependency projection");
            assert!(text.ends_with('\n'));
            let lines: Vec<_> = text.lines().collect();
            assert!(lines.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(lines.iter().all(|line| line.split('\t').count() == 4));
            assert_eq!(
                lines.len(),
                lines.iter().copied().collect::<BTreeSet<_>>().len()
            );
        }
        for bytes in [
            include_bytes!("source_fingerprint/snv-builder-roots.v1").as_slice(),
            include_bytes!("source_fingerprint/reference-builder-roots.v1").as_slice(),
        ] {
            let text = std::str::from_utf8(bytes).expect("UTF-8 dependency roots");
            assert!(text.ends_with('\n'));
            let lines: Vec<_> = text.lines().collect();
            assert!(lines.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(lines.iter().all(|line| line.split('\t').count() == 5));
            assert_eq!(
                lines.len(),
                lines.iter().copied().collect::<BTreeSet<_>>().len()
            );
        }
        for bytes in [
            include_bytes!("source_fingerprint/snv-builder-direct-uses.v1").as_slice(),
            include_bytes!("source_fingerprint/reference-builder-direct-uses.v1").as_slice(),
        ] {
            let text = std::str::from_utf8(bytes).expect("UTF-8 direct uses");
            assert!(text.ends_with('\n'));
            let lines: Vec<_> = text.lines().collect();
            assert!(lines.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(lines.iter().all(|line| line.split('\t').count() == 3));
            assert_eq!(
                lines.len(),
                lines.iter().copied().collect::<BTreeSet<_>>().len()
            );
        }
        assert_projection_matches_isolated_resolution(
            "pangopup-ticket013-snv-resolver",
            include_bytes!("source_fingerprint/snv-builder-linux-lock.v1"),
            include_bytes!("source_fingerprint/snv-builder-roots.v1"),
            include_bytes!("source_fingerprint/snv-builder-cargo-lock.v1"),
        );
        assert_projection_matches_isolated_resolution(
            "pangopup-ticket013-reference-resolver",
            include_bytes!("source_fingerprint/reference-builder-linux-lock.v1"),
            include_bytes!("source_fingerprint/reference-builder-roots.v1"),
            include_bytes!("source_fingerprint/reference-builder-cargo-lock.v1"),
        );
    }

    #[test]
    fn source_fingerprint_rust_path_enumerator_ignores_comments_and_literals() {
        let source = br##"
            // comment_only::Item
            /* outer::Item /* nested::Item */ */
            "string_only::Item";
            b"bytes_only::Item";
            r#"raw_only::Item"#;
            br#"raw_bytes_only::Item"#;
            actual_crate::Item;
        "##;
        assert_eq!(
            source_path_heads(source),
            BTreeSet::from(["actual_crate".to_owned()])
        );
    }

    #[test]
    fn source_fingerprint_dependency_aliases_are_source_derived_without_literal_noise() {
        let source = br###"
            use flate2 as gzip;
            use flate2::{self as tree_gzip};
            extern crate flate2 as extern_gzip;
            gzip::bufread::GzDecoder::new(input);
            tree_gzip::Compression::fast();
            extern_gzip::GzBuilder::new();

            // use serde_json as comment_alias; comment_alias::Value;
            /* extern crate serde_json as block_alias; block_alias::Value; */
            "use serde_json as string_alias; string_alias::Value";
            b"use serde_json as byte_alias; byte_alias::Value";
            c"use serde_json as c_alias; c_alias::Value";
            r#"use serde_json as raw_alias; raw_alias::Value"#;
            br#"use serde_json as raw_byte_alias; raw_byte_alias::Value"#;
        "###;
        let entries = [Entry {
            path: "crates/pangopup-build/src/alias_probe.rs",
            bytes: source,
        }];
        let manifests = actual_manifest_dependencies();
        let derived =
            derive_external_dependencies(&entries, &manifests).expect("alias-derived dependency");
        assert_eq!(
            derived.keys().cloned().collect::<BTreeSet<_>>(),
            BTreeSet::from(["flate2".to_owned()])
        );
        let candidates = source_dependency_candidates(source);
        assert!(candidates.contains("flate2"));
        assert!(!candidates.contains("serde_json"));
        assert!(
            !candidates
                .iter()
                .any(|candidate| candidate.ends_with("_alias"))
        );
    }

    #[test]
    fn source_fingerprint_alias_root_and_witness_omission_fails() {
        let source = b"use flate2 as gzip;\ngzip::bufread::GzDecoder::new(input);\n";
        let entries = [Entry {
            path: "crates/pangopup-build/src/alias_probe.rs",
            bytes: source,
        }];
        let manifests = actual_manifest_dependencies();
        let all_roots = dependency_roots(include_bytes!("source_fingerprint/snv-builder-roots.v1"));
        let mut roots = BTreeMap::from([("flate2".to_owned(), all_roots["flate2"].clone())]);
        let mut witnesses = BTreeMap::from([(
            "flate2".to_owned(),
            DirectUse {
                path: entries[0].path.to_owned(),
                needle: "gzip::".to_owned(),
            },
        )]);
        assert!(roots.remove("flate2").is_some());
        assert!(witnesses.remove("flate2").is_some());
        assert_eq!(
            roots.keys().collect::<Vec<_>>(),
            witnesses.keys().collect::<Vec<_>>(),
            "a root/witness-only check accepts simultaneous alias omission"
        );
        assert!(
            source_manifest_dependency_contract(&roots, &entries, &manifests).is_err(),
            "source alias derivation still finds the omitted flate2 root"
        );
    }

    fn replace_root_bytes(root: &mut RootSource, from: &str, to: &str) {
        let source = std::str::from_utf8(&root.bytes).expect("root source UTF-8");
        assert_eq!(source.matches(from).count(), 1, "unique root edit");
        root.bytes = source.replacen(from, to, 1).into_bytes();
    }

    #[test]
    fn source_fingerprint_root_wiring_is_derived_and_artifact_specific() {
        let roots = actual_root_sources();
        let workspace = actual_workspace_dependencies();
        assert_eq!(
            derive_root_wiring(SNV_ENTRIES, &roots, &workspace).expect("derived SNV root wiring"),
            checked_root_wiring(include_bytes!("source_fingerprint/snv-root-wiring.v1"))
        );
        assert_eq!(
            derive_root_wiring(REFERENCE_ENTRIES, &roots, &workspace)
                .expect("derived reference root wiring"),
            checked_root_wiring(include_bytes!(
                "source_fingerprint/reference-root-wiring.v1"
            ))
        );
    }

    #[test]
    fn source_fingerprint_causal_root_wiring_rebinds_fail_but_unrelated_edits_do_not() {
        let workspace = actual_workspace_dependencies();
        let expected = checked_root_wiring(include_bytes!("source_fingerprint/snv-root-wiring.v1"));

        let mut module_rebind = actual_root_sources();
        replace_root_bytes(
            module_rebind.get_mut("pangopup-build").expect("build root"),
            "mod snv;",
            "#[path = \"replacement.rs\"] mod snv;",
        );
        assert_ne!(
            derive_root_wiring(SNV_ENTRIES, &module_rebind, &workspace)
                .expect("module-rebound wiring"),
            expected,
            "a selected private-module rebind changes the checked projection"
        );

        let mut reexport_rebind = actual_root_sources();
        replace_root_bytes(
            reexport_rebind
                .get_mut("pangopup-index")
                .expect("index root"),
            "pub use snv::*;",
            "pub use reference::*;",
        );
        let rebound = derive_root_wiring(SNV_ENTRIES, &reexport_rebind, &workspace);
        assert!(
            rebound.is_err() || rebound.as_ref().is_ok_and(|actual| actual != &expected),
            "a selected cross-crate re-export rebind must fail or change the projection"
        );

        let mut unrelated = actual_root_sources();
        unrelated
            .get_mut("pangopup-index")
            .expect("index root")
            .bytes
            .extend_from_slice(b"\npub mod unrelated_root_edit;\n");
        assert_eq!(
            derive_root_wiring(SNV_ENTRIES, &unrelated, &workspace).expect("unrelated root edit"),
            expected,
            "an unrelated root item is outside the artifact projection"
        );
    }

    #[test]
    fn source_fingerprint_direct_roots_derive_from_sources_and_manifests() {
        let roots = dependency_roots(include_bytes!("source_fingerprint/snv-builder-roots.v1"));
        let manifests = actual_manifest_dependencies();
        let derived = source_manifest_dependency_contract(&roots, SNV_ENTRIES, &manifests)
            .expect("source- and manifest-derived SNV roots");
        assert!(!derived.contains_key("ureq"));
        assert!(!derived.contains_key("zstd"));

        let uses = direct_uses(include_bytes!(
            "source_fingerprint/snv-builder-direct-uses.v1"
        ));
        diagnostic_uses_match(&uses, &derived, SNV_ENTRIES)
            .expect("SNV diagnostic uses agree with derived roots");

        let reference_roots = dependency_roots(include_bytes!(
            "source_fingerprint/reference-builder-roots.v1"
        ));
        let reference_derived =
            source_manifest_dependency_contract(&reference_roots, REFERENCE_ENTRIES, &manifests)
                .expect("source- and manifest-derived reference roots");
        let reference_uses = direct_uses(include_bytes!(
            "source_fingerprint/reference-builder-direct-uses.v1"
        ));
        diagnostic_uses_match(&reference_uses, &reference_derived, REFERENCE_ENTRIES)
            .expect("reference diagnostic uses agree with derived roots");
    }

    #[test]
    fn source_fingerprint_simultaneous_root_and_witness_omission_fails() {
        let manifests = actual_manifest_dependencies();
        let roots = dependency_roots(include_bytes!("source_fingerprint/snv-builder-roots.v1"));
        let mut uses = direct_uses(include_bytes!(
            "source_fingerprint/snv-builder-direct-uses.v1"
        ));
        let mut missing_libc = roots.clone();
        assert!(missing_libc.remove("libc").is_some());
        assert!(uses.remove("libc").is_some());
        assert_eq!(
            missing_libc.keys().collect::<Vec<_>>(),
            uses.keys().collect::<Vec<_>>(),
            "the former declaration-to-witness comparison would accept both omissions"
        );
        assert!(
            source_manifest_dependency_contract(&missing_libc, SNV_ENTRIES, &manifests).is_err(),
            "actual source still derives libc when root and diagnostic are both omitted"
        );
    }

    #[test]
    fn source_fingerprint_manifest_version_requirement_drift_fails() {
        let roots = dependency_roots(include_bytes!("source_fingerprint/snv-builder-roots.v1"));
        let mut manifests = actual_manifest_dependencies();
        manifests
            .get_mut("pangopup-build")
            .expect("build manifest")
            .get_mut("flate2")
            .expect("flate2 declaration")
            .requirement = "^1.1.6".to_owned();
        assert!(
            source_manifest_dependency_contract(&roots, SNV_ENTRIES, &manifests).is_err(),
            "causal manifest version-requirement drift must fail"
        );
    }

    #[test]
    fn source_fingerprint_manifest_default_features_drift_fails() {
        let roots = dependency_roots(include_bytes!("source_fingerprint/snv-builder-roots.v1"));
        let mut manifests = actual_manifest_dependencies();
        let memmap2 = manifests
            .get_mut("pangopup-index")
            .expect("index manifest")
            .get_mut("memmap2")
            .expect("memmap2 declaration");
        memmap2.default_features = !memmap2.default_features;
        assert!(
            source_manifest_dependency_contract(&roots, SNV_ENTRIES, &manifests).is_err(),
            "causal manifest default-features drift must fail"
        );
    }

    #[test]
    fn source_fingerprint_manifest_feature_list_drift_fails() {
        let roots = dependency_roots(include_bytes!("source_fingerprint/snv-builder-roots.v1"));
        let mut manifests = actual_manifest_dependencies();
        assert!(
            manifests
                .get_mut("pangopup-build")
                .expect("build manifest")
                .get_mut("rustix")
                .expect("rustix declaration")
                .features
                .insert("process".to_owned())
        );
        assert!(
            source_manifest_dependency_contract(&roots, SNV_ENTRIES, &manifests).is_err(),
            "causal manifest feature-list drift must fail"
        );
    }
}
