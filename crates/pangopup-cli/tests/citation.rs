use serde_yaml_ng::{Mapping, Value};
use std::{fs, path::Path};

const REPOSITORY: &str = "https://github.com/genomoncology/pangopup";
const RELEASE: &str = "https://github.com/genomoncology/pangopup/releases/tag/v0.3.0";
const PAPER_DOI: &str = "https://doi.org/10.1186/s13059-022-02664-4";
const ZENODO_DOI: &str = "https://doi.org/10.5281/zenodo.15649338";
const PANGOLIN_REPOSITORY: &str = "https://github.com/tkzeng/Pangolin";

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
}

fn mapping(value: &Value) -> Result<&Mapping, String> {
    value
        .as_mapping()
        .ok_or_else(|| "citation document must be a YAML mapping".to_owned())
}

fn required_string<'a>(document: &'a Mapping, key: &str) -> Result<&'a str, String> {
    document
        .get(Value::String(key.to_owned()))
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing or non-string field {key}"))
}

fn validate_citation(source: &str) -> Result<(), String> {
    let parsed: Value = serde_yaml_ng::from_str(source)
        .map_err(|error| format!("CITATION.cff is not valid YAML: {error}"))?;
    let document = mapping(&parsed)?;

    let expected = [
        ("cff-version", "1.2.0"),
        ("title", "PangoPup"),
        ("type", "software"),
        ("version", "0.3.0"),
        ("date-released", "2026-08-05"),
        ("repository-code", REPOSITORY),
        ("repository-artifact", RELEASE),
        ("license", "GPL-3.0-only"),
    ];
    for (key, expected_value) in expected {
        let actual = required_string(document, key)?;
        if actual != expected_value {
            return Err(format!(
                "{key} must be {expected_value:?}, found {actual:?}"
            ));
        }
    }

    let authors = document
        .get(Value::String("authors".to_owned()))
        .and_then(Value::as_sequence)
        .ok_or_else(|| "authors must be a YAML sequence".to_owned())?;
    if authors.len() != 1 {
        return Err(format!(
            "authors must contain exactly Ian Maurer, found {} entries",
            authors.len()
        ));
    }
    let author = mapping(&authors[0])?;
    if required_string(author, "given-names")? != "Ian"
        || required_string(author, "family-names")? != "Maurer"
    {
        return Err("author must be Ian Maurer".to_owned());
    }
    if author.contains_key(Value::String("orcid".to_owned())) {
        return Err("do not invent an ORCID for Ian Maurer".to_owned());
    }

    Ok(())
}

#[test]
fn citation_cff_has_the_released_software_identity() {
    let source =
        fs::read_to_string(repository_root().join("CITATION.cff")).expect("read root CITATION.cff");
    validate_citation(&source).expect("valid stable citation metadata");
}

#[test]
fn citation_validation_rejects_missing_or_drifted_identity() {
    let source =
        fs::read_to_string(repository_root().join("CITATION.cff")).expect("read root CITATION.cff");

    let missing_release = source
        .lines()
        .filter(|line| !line.starts_with("repository-artifact:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        validate_citation(&missing_release)
            .expect_err("missing release must fail")
            .contains("repository-artifact")
    );

    let wrong_author = source.replace("given-names: Ian", "given-names: Pangolin");
    assert_eq!(
        validate_citation(&wrong_author).expect_err("author drift must fail"),
        "author must be Ian Maurer"
    );
}

#[test]
fn readme_links_directly_to_the_named_prior_art() {
    let readme = fs::read_to_string(repository_root().join("README.md")).expect("read README");
    for required in [PANGOLIN_REPOSITORY, PAPER_DOI, ZENODO_DOI] {
        assert!(
            readme.contains(required),
            "README must link directly to {required}"
        );
    }
    for creator in [
        "Tony Zeng",
        "Yang I. Li",
        "Nils Wagner",
        "Aleksandr Neverov",
    ] {
        assert!(readme.contains(creator), "README must name {creator}");
    }
}
