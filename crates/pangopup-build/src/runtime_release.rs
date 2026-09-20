//! Provenance-bound preparation of the inactive runtime-v2 outer authority.

use crate::CommandError;
use pangopup_assets::{PrepareRuntimeReleaseOutcome, prepare_runtime_v2_release};
use std::path::Path;

#[derive(Clone, Copy)]
struct BuildProvenance<'a> {
    commit: &'a str,
    clean: &'a str,
}

pub fn prepare_runtime_v2_release_from_clean_build(
    transport: &Path,
    tooling_commit: &str,
    release_target_commit: &str,
    output: &Path,
) -> Result<PrepareRuntimeReleaseOutcome, CommandError> {
    prepare_against_build(
        transport,
        tooling_commit,
        release_target_commit,
        output,
        BuildProvenance {
            commit: env!("PANGOPUP_GIT_COMMIT"),
            clean: env!("PANGOPUP_GIT_CLEAN"),
        },
    )
}

fn prepare_against_build(
    transport: &Path,
    tooling_commit: &str,
    release_target_commit: &str,
    output: &Path,
    build: BuildProvenance<'_>,
) -> Result<PrepareRuntimeReleaseOutcome, CommandError> {
    validate_build_provenance(tooling_commit, build)?;
    prepare_runtime_v2_release(transport, tooling_commit, release_target_commit, output)
        .map_err(|error| CommandError::new(error.kind().code(), error.to_string()))
}

fn validate_build_provenance(
    supplied: &str,
    compiled: BuildProvenance<'_>,
) -> Result<(), CommandError> {
    if !valid_commit(compiled.commit) {
        return Err(CommandError::new(
            "RUNTIME_RELEASE_BUILD",
            "compiled Git commit is unavailable or invalid",
        ));
    }
    match compiled.clean {
        "true" => {}
        "false" => {
            return Err(CommandError::new(
                "RUNTIME_RELEASE_BUILD",
                "release tooling was compiled from a dirty checkout",
            ));
        }
        _ => {
            return Err(CommandError::new(
                "RUNTIME_RELEASE_BUILD",
                "compiled Git cleanliness is unavailable",
            ));
        }
    }
    if supplied != compiled.commit {
        return Err(CommandError::new(
            "RUNTIME_RELEASE_BUILD",
            "tooling commit differs from the compiled Git commit",
        ));
    }
    Ok(())
}

fn valid_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn runtime_v2_tooling_commit_requires_matching_clean_compiled_provenance() {
        assert!(
            validate_build_provenance(
                COMMIT,
                BuildProvenance {
                    commit: COMMIT,
                    clean: "true",
                },
            )
            .is_ok()
        );
        for (supplied, commit, clean, message) in [
            (COMMIT, "unavailable", "true", "unavailable or invalid"),
            (COMMIT, COMMIT, "false", "dirty checkout"),
            (COMMIT, COMMIT, "unknown", "cleanliness is unavailable"),
            (
                "1111111111111111111111111111111111111111",
                COMMIT,
                "true",
                "differs from the compiled Git commit",
            ),
        ] {
            let error = validate_build_provenance(supplied, BuildProvenance { commit, clean })
                .expect_err("invalid provenance");
            assert!(error.to_string().contains(message), "{error}");
        }
    }
}
