use std::ffi::{OsStr, OsString};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Leaf {
    Inspect,
    PrototypeRoundtrip,
    PrototypeOpen,
    BenchmarkCorpus,
    Build,
    Verify,
    ReferenceBuild,
    ReferenceInspect,
    ReferenceWindow,
    TransportPack,
    TransportVerify,
    TransportUnpack,
    ReleasePrepare,
    ReleaseUploadAsset,
    CompatibilityInspect,
    CompatibilityCapture,
    ModelEvidence,
    ModelConvert,
    ModelInspect,
    ModelQualify,
    RuntimeProfilePrepare,
    RuntimeTransportPack,
    RuntimeTransportVerify,
    RuntimeTransportUnpack,
}

impl Leaf {
    #[cfg(test)]
    const ALL: [Self; 24] = [
        Self::Inspect,
        Self::PrototypeRoundtrip,
        Self::PrototypeOpen,
        Self::BenchmarkCorpus,
        Self::Build,
        Self::Verify,
        Self::ReferenceBuild,
        Self::ReferenceInspect,
        Self::ReferenceWindow,
        Self::TransportPack,
        Self::TransportVerify,
        Self::TransportUnpack,
        Self::ReleasePrepare,
        Self::ReleaseUploadAsset,
        Self::CompatibilityInspect,
        Self::CompatibilityCapture,
        Self::ModelEvidence,
        Self::ModelConvert,
        Self::ModelInspect,
        Self::ModelQualify,
        Self::RuntimeProfilePrepare,
        Self::RuntimeTransportPack,
        Self::RuntimeTransportVerify,
        Self::RuntimeTransportUnpack,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Entry {
    pub leaf: Leaf,
    pub namespace: Option<&'static str>,
    pub action: &'static str,
    pub synopsis: &'static str,
    pub summary: &'static str,
}

const ENTRIES: &[Entry] = &[
    Entry {
        leaf: Leaf::Inspect,
        namespace: None,
        action: "inspect",
        synopsis: "inspect <SOURCE_DIR>",
        summary: "Validate and summarize a Pangolin precomputed-score source directory.",
    },
    Entry {
        leaf: Leaf::PrototypeRoundtrip,
        namespace: None,
        action: "prototype-roundtrip",
        synopsis: "prototype-roundtrip <SOURCE_DIR> <OUTPUT>",
        summary: "Build and verify the retained fixed-11 prototype artifact.",
    },
    Entry {
        leaf: Leaf::PrototypeOpen,
        namespace: None,
        action: "prototype-open",
        synopsis: "prototype-open <ARTIFACT>",
        summary: "Structurally open a retained fixed-11 prototype artifact.",
    },
    Entry {
        leaf: Leaf::BenchmarkCorpus,
        namespace: None,
        action: "benchmark-corpus",
        synopsis: "benchmark-corpus <SOURCE_DIR> <OUTPUT> <SELECTED_MANIFEST>",
        summary: "Prepare the checked source-derived benchmark corpus.",
    },
    Entry {
        leaf: Leaf::Build,
        namespace: None,
        action: "build",
        synopsis: "build --source <SOURCE_DIR> --reference <GRCH38_FASTA_OR_GZIP> --output <NEW_BUNDLE>",
        summary: "Build the deterministic complete SNV lookup bundle.",
    },
    Entry {
        leaf: Leaf::Verify,
        namespace: None,
        action: "verify",
        synopsis: "verify <BUNDLE>",
        summary: "Perform complete offline certification of an SNV lookup bundle.",
    },
    Entry {
        leaf: Leaf::ReferenceBuild,
        namespace: Some("reference"),
        action: "build",
        synopsis: "reference build --profile <refseq-grch38p14-primary-v1|pangopup-reference-mini-v1|pangopup-reference-route-test-v1> --source <FASTA_OR_GZIP> --assembly-report <ASSEMBLY_REPORT> --output <NEW_BUNDLE>",
        summary: "Build and privately certify a compact reference bundle.",
    },
    Entry {
        leaf: Leaf::ReferenceInspect,
        namespace: Some("reference"),
        action: "inspect",
        synopsis: "reference inspect --bundle <BUNDLE>",
        summary: "Inspect a compact reference bundle without reading its complete payload.",
    },
    Entry {
        leaf: Leaf::ReferenceWindow,
        namespace: Some("reference"),
        action: "window",
        synopsis: "reference window --bundle <BUNDLE> --contig <GRCH38_CONTIG_OR_REFSEQ_ACCESSION> --start <POSITIVE_1_BASED_POSITION> --length <1..1048576>",
        summary: "Read one bounded sequence window from a compact reference bundle.",
    },
    Entry {
        leaf: Leaf::TransportPack,
        namespace: Some("transport"),
        action: "pack",
        synopsis: "transport pack --bundle <BUNDLE> --output <ABSENT_DIR>",
        summary: "Pack an SNV bundle into deterministic split transport members.",
    },
    Entry {
        leaf: Leaf::TransportVerify,
        namespace: Some("transport"),
        action: "verify",
        synopsis: "transport verify --transport <TRANSPORT_DIR>",
        summary: "Verify a complete SNV transport.",
    },
    Entry {
        leaf: Leaf::TransportUnpack,
        namespace: Some("transport"),
        action: "unpack",
        synopsis: "transport unpack --transport <TRANSPORT_DIR> --output <ABSENT_DIR>",
        summary: "Reconstruct an SNV bundle from a verified transport.",
    },
    Entry {
        leaf: Leaf::ReleasePrepare,
        namespace: Some("release"),
        action: "prepare",
        synopsis: "release prepare --transport <TRANSPORT_DIR> --receipt <PROOF_RECEIPT_JSON> --output <ABSENT_DIR>",
        summary: "Prepare bounded, pinned SNV release metadata.",
    },
    Entry {
        leaf: Leaf::ReleaseUploadAsset,
        namespace: Some("release"),
        action: "upload-asset",
        synopsis: "release upload-asset --transport <TRANSPORT_DIR> --prepared <PREPARED_DIR> --gh <ABSOLUTE_PINNED_GH_BINARY> --release-id <POSITIVE_GITHUB_ID> --asset <EXACT_ASSET_NAME>",
        summary: "Upload one exact reviewed SNV release asset.",
    },
    Entry {
        leaf: Leaf::CompatibilityInspect,
        namespace: Some("compatibility"),
        action: "inspect",
        synopsis: "compatibility inspect --corpus <CORPUS_DIR>",
        summary: "Inspect the checked Pangolin compatibility corpus.",
    },
    Entry {
        leaf: Leaf::CompatibilityCapture,
        namespace: Some("compatibility"),
        action: "capture",
        synopsis: "compatibility capture --upstream <PANGOLIN_DIR> --python <PYTHON> --reference-source <REFSEQ_FASTA_GZIP> --assembly-report <ASSEMBLY_REPORT> --reference <DERIVED_FASTA> --annotation-db <GENCODE_DB> --annotation-gtf <GENCODE_GTF_GZIP> --output <ABSENT_DIR>",
        summary: "Capture the pinned Pangolin compatibility corpus.",
    },
    Entry {
        leaf: Leaf::ModelEvidence,
        namespace: Some("model"),
        action: "evidence",
        synopsis: "model evidence --upstream <PANGOLIN_DIR> --python <PYTHON> --corpus <CORPUS_DIR> --output <ABSENT_DIR>",
        summary: "Capture authenticated checkpoint conversion evidence.",
    },
    Entry {
        leaf: Leaf::ModelConvert,
        namespace: Some("model"),
        action: "convert",
        synopsis: "model convert --upstream <PANGOLIN_DIR> --python <PYTHON> --evidence <EVIDENCE_DIR> --output <ABSENT_DIR> --representation <singleton|zero-padded-batch|paired-strand-batch>",
        summary: "Convert authenticated Pangolin checkpoints into an ONNX bundle.",
    },
    Entry {
        leaf: Leaf::ModelInspect,
        namespace: Some("model"),
        action: "inspect",
        synopsis: "model inspect --bundle <MODEL_BUNDLE>",
        summary: "Inspect an authenticated ONNX model bundle.",
    },
    Entry {
        leaf: Leaf::ModelQualify,
        namespace: Some("model"),
        action: "qualify",
        synopsis: "model qualify --bundle <MODEL_BUNDLE> --evidence <EVIDENCE_DIR>",
        summary: "Qualify an ONNX model bundle against authenticated evidence.",
    },
    Entry {
        leaf: Leaf::RuntimeProfilePrepare,
        namespace: Some("runtime-profile"),
        action: "prepare",
        synopsis: "runtime-profile prepare --snv-bundle <SNV_BUNDLE> --model-bundle <MODEL_BUNDLE> --reference-bundle <REFERENCE_BUNDLE> --mask <MASK_FILE> --output <PROFILE_JSON>",
        summary: "Bind the exact qualified four-asset runtime tuple.",
    },
    Entry {
        leaf: Leaf::RuntimeTransportPack,
        namespace: Some("runtime-transport"),
        action: "pack",
        synopsis: "runtime-transport pack --profile <FILE> --model-bundle <DIR> --reference-bundle <DIR> --mask <FILE> --output <ABSENT_DIR>",
        summary: "Package model-side runtime assets into a deterministic local transport.",
    },
    Entry {
        leaf: Leaf::RuntimeTransportVerify,
        namespace: Some("runtime-transport"),
        action: "verify",
        synopsis: "runtime-transport verify --transport <DIR>",
        summary: "Stream and authenticate a complete model-side runtime transport.",
    },
    Entry {
        leaf: Leaf::RuntimeTransportUnpack,
        namespace: Some("runtime-transport"),
        action: "unpack",
        synopsis: "runtime-transport unpack --transport <DIR> --output <ABSENT_DIR>",
        summary: "Reconstruct model-side runtime assets with atomic publication.",
    },
];

pub(crate) fn resolve(arguments: &[OsString]) -> Option<(Leaf, &[OsString])> {
    let first = arguments.first()?.to_str()?;
    if let Some(entry) = ENTRIES
        .iter()
        .find(|entry| entry.namespace.is_none() && entry.action == first)
    {
        return Some((entry.leaf, &arguments[1..]));
    }
    let second = arguments.get(1)?.to_str()?;
    ENTRIES
        .iter()
        .find(|entry| entry.namespace == Some(first) && entry.action == second)
        .map(|entry| (entry.leaf, &arguments[2..]))
}

pub(crate) fn namespace(arguments: &[OsString]) -> Option<&'static str> {
    let first = arguments.first()?.to_str()?;
    namespaces().find(|candidate| *candidate == first)
}

pub(crate) enum Information {
    HelpRoot,
    HelpNamespace(&'static str),
    HelpLeaf(Leaf),
    Version,
}

pub(crate) fn information(arguments: &[OsString]) -> Option<Information> {
    match arguments {
        [flag] if help_flag(flag) => Some(Information::HelpRoot),
        [flag] if version_flag(flag) => Some(Information::Version),
        [path, flag] if help_flag(flag) => {
            let path = path.to_str()?;
            ENTRIES
                .iter()
                .find(|entry| entry.namespace.is_none() && entry.action == path)
                .map(|entry| Information::HelpLeaf(entry.leaf))
                .or_else(|| {
                    namespaces()
                        .find(|namespace| *namespace == path)
                        .map(Information::HelpNamespace)
                })
        }
        [namespace, action, flag] if help_flag(flag) => {
            let namespace = namespace.to_str()?;
            let action = action.to_str()?;
            ENTRIES
                .iter()
                .find(|entry| entry.namespace == Some(namespace) && entry.action == action)
                .map(|entry| Information::HelpLeaf(entry.leaf))
        }
        _ => None,
    }
}

pub(crate) fn render(information: Information) -> String {
    match information {
        Information::Version => format!("pangopup-build {}", env!("CARGO_PKG_VERSION")),
        Information::HelpRoot => {
            let mut output = String::from("Usage: pangopup-build <COMMAND>\n\nCommands:\n");
            for entry in ENTRIES {
                output.push_str("  pangopup-build ");
                output.push_str(entry.synopsis);
                output.push('\n');
            }
            output
        }
        Information::HelpNamespace(namespace) => {
            let mut output = format!("Usage: pangopup-build {namespace} <ACTION>\n\nActions:\n");
            for entry in ENTRIES
                .iter()
                .filter(|entry| entry.namespace == Some(namespace))
            {
                output.push_str("  pangopup-build ");
                output.push_str(entry.synopsis);
                output.push('\n');
            }
            output
        }
        Information::HelpLeaf(leaf) => {
            let entry = entry(leaf);
            format!(
                "Usage: pangopup-build {}\n\n{}",
                entry.synopsis, entry.summary
            )
        }
    }
}

fn entry(leaf: Leaf) -> &'static Entry {
    ENTRIES
        .iter()
        .find(|entry| entry.leaf == leaf)
        .expect("every leaf is cataloged")
}

fn namespaces() -> impl Iterator<Item = &'static str> {
    ENTRIES.iter().enumerate().filter_map(|(index, entry)| {
        entry.namespace.filter(|namespace| {
            !ENTRIES[..index]
                .iter()
                .any(|prior| prior.namespace == Some(*namespace))
        })
    })
}

fn help_flag(value: &OsStr) -> bool {
    value == "--help" || value == "-h"
}

fn version_flag(value: &OsStr) -> bool {
    value == "--version" || value == "-V"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn catalog_has_one_entry_for_every_leaf_and_unique_paths() {
        assert_eq!(ENTRIES.len(), Leaf::ALL.len());
        let leaves: HashSet<_> = ENTRIES.iter().map(|entry| entry.leaf).collect();
        let paths: HashSet<_> = ENTRIES
            .iter()
            .map(|entry| (entry.namespace, entry.action))
            .collect();
        assert_eq!(leaves.len(), ENTRIES.len());
        assert_eq!(leaves, Leaf::ALL.into_iter().collect());
        assert_eq!(paths.len(), ENTRIES.len());
        let discovered: Vec<_> = namespaces().collect();
        assert_eq!(
            discovered,
            [
                "reference",
                "transport",
                "release",
                "compatibility",
                "model",
                "runtime-profile",
                "runtime-transport"
            ]
        );
        assert!(
            ENTRIES
                .iter()
                .filter_map(|entry| entry.namespace)
                .all(|namespace| discovered.contains(&namespace))
        );
        for namespace in discovered {
            assert_eq!(
                super::namespace(&args(&[namespace, "unknown"])),
                Some(namespace)
            );
            assert!(matches!(
                information(&args(&[namespace, "--help"])),
                Some(Information::HelpNamespace(found)) if found == namespace
            ));
        }
    }

    #[test]
    fn exact_informational_positions_are_accepted() {
        assert!(matches!(
            information(&args(&["--help"])),
            Some(Information::HelpRoot)
        ));
        assert!(matches!(
            information(&args(&["-V"])),
            Some(Information::Version)
        ));
        assert!(matches!(
            information(&args(&["inspect", "-h"])),
            Some(Information::HelpLeaf(Leaf::Inspect))
        ));
        assert!(matches!(
            information(&args(&["model", "--help"])),
            Some(Information::HelpNamespace("model"))
        ));
        assert!(matches!(
            information(&args(&["model", "convert", "--help"])),
            Some(Information::HelpLeaf(Leaf::ModelConvert))
        ));
    }

    #[test]
    fn misplaced_or_extended_information_is_operational_input() {
        for values in [
            &["--help", "reference"][..],
            &["reference", "build", "--profile", "x", "--help"],
            &["inspect", "--help", "extra"],
            &["model", "convert", "--help", "extra"],
            &["unknown", "--help"],
            &["--version", "extra"],
        ] {
            assert!(information(&args(values)).is_none(), "{values:?}");
        }
    }

    #[test]
    fn resolver_uses_the_catalog_and_leaves_operands_untouched() {
        let arguments = args(&["model", "inspect", "--bundle", "somewhere"]);
        let (leaf, operands) = resolve(&arguments).expect("cataloged path");
        assert_eq!(leaf, Leaf::ModelInspect);
        assert_eq!(operands, args(&["--bundle", "somewhere"]));
        assert!(resolve(&args(&["model", "unknown"])).is_none());
        assert!(resolve(&args(&["unknown"])).is_none());
    }

    #[test]
    fn every_help_render_is_derived_from_its_catalog_entry() {
        for entry in ENTRIES {
            let rendered = render(Information::HelpLeaf(entry.leaf));
            assert!(rendered.contains(entry.synopsis));
            assert!(rendered.contains(entry.summary));
        }
    }

    #[test]
    fn current_state_documents_point_to_the_checked_catalog() {
        let readme = include_str!("../../../README.md");
        let faq = include_str!("../../../planning/faq.md");
        let frontier = include_str!("../../../planning/frontier.md");
        assert!(readme.contains("pangopup-build --help"));
        assert!(faq.contains("pangopup-build --help"));
        assert!(frontier.contains(
            "dispatch and successful root, namespace, and leaf help share one checked\ncatalog"
        ));
    }
}
