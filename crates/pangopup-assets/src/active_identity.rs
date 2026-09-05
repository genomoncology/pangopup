//! Canonical identity for one active HTTP scoring environment.

use crate::RuntimeProfileId;
use pangopup_model::CpuPolicy;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt;

pub const ACTIVE_SCORING_IDENTITY_SCHEMA: &str = "pangopup.active-scoring-identity.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ActiveScoringIdentityPreimage {
    schema: &'static str,
    software_version: String,
    runtime_profile_id: String,
    effective_cpu_policy: String,
}

impl ActiveScoringIdentityPreimage {
    pub fn new(
        software_version: impl Into<String>,
        runtime_profile_id: &RuntimeProfileId,
        effective_cpu_policy: CpuPolicy,
    ) -> Self {
        Self {
            schema: ACTIVE_SCORING_IDENTITY_SCHEMA,
            software_version: software_version.into(),
            runtime_profile_id: runtime_profile_id.as_str().to_owned(),
            effective_cpu_policy: effective_cpu_policy.to_string(),
        }
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_jcs::to_vec(self).expect("active scoring identity contains only serializable fields")
    }

    pub fn identity(&self) -> ActiveScoringIdentity {
        ActiveScoringIdentity(format!(
            "sha256:{:x}",
            Sha256::digest(self.canonical_bytes())
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ActiveScoringIdentity(String);

impl ActiveScoringIdentity {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ActiveScoringIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{canonical_runtime_profile_bytes, production_runtime_profile, runtime_profile_id};
    use pangopup_model::{CpuExecutionMode, IntraOpThreads};
    use std::num::NonZeroUsize;

    fn runtime_id() -> RuntimeProfileId {
        let bytes =
            canonical_runtime_profile_bytes(&production_runtime_profile()).expect("profile");
        runtime_profile_id(&bytes).expect("runtime identity")
    }

    fn policy(threads: usize) -> CpuPolicy {
        CpuPolicy::new(
            CpuExecutionMode::Sequential,
            IntraOpThreads::Fixed(NonZeroUsize::new(threads).expect("positive")),
            NonZeroUsize::MIN,
        )
        .expect("policy")
    }

    #[test]
    fn active_scoring_identity_has_one_pinned_canonical_preimage() {
        let preimage = ActiveScoringIdentityPreimage::new("0.3.0", &runtime_id(), policy(1));
        assert_eq!(
            String::from_utf8(preimage.canonical_bytes()).expect("UTF-8"),
            concat!(
                "{\"effective_cpu_policy\":\"sequential:1/1\",",
                "\"runtime_profile_id\":\"sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c\",",
                "\"schema\":\"pangopup.active-scoring-identity.v1\",",
                "\"software_version\":\"0.3.0\"}"
            )
        );
        assert_eq!(
            preimage.identity().as_str(),
            "sha256:c0e2e1fd77821555a868b5f70514769d144a15aeb160e71aea17d6099839328f"
        );
        assert_eq!(preimage.identity(), preimage.identity());
    }

    #[test]
    fn every_declared_input_changes_the_active_identity() {
        let runtime_id = runtime_id();
        let baseline =
            ActiveScoringIdentityPreimage::new("0.3.0", &runtime_id, policy(1)).identity();
        let changed_version =
            ActiveScoringIdentityPreimage::new("0.3.1", &runtime_id, policy(1)).identity();
        let changed_policy =
            ActiveScoringIdentityPreimage::new("0.3.0", &runtime_id, policy(2)).identity();
        let mut changed_profile = production_runtime_profile();
        changed_profile.scoring.masking_policy.push_str("-changed");
        let changed_profile_bytes =
            canonical_runtime_profile_bytes(&changed_profile).expect("changed profile");
        let changed_runtime_id = runtime_profile_id(&changed_profile_bytes).expect("profile id");
        let changed_runtime =
            ActiveScoringIdentityPreimage::new("0.3.0", &changed_runtime_id, policy(1)).identity();
        assert_ne!(baseline, changed_version);
        assert_ne!(baseline, changed_policy);
        assert_ne!(baseline, changed_runtime);
    }
}
