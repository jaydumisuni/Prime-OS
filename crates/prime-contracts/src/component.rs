use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

pub const PRIME_COMPONENT_SCHEMA: &str = "prime.component-manifest.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComponentClass {
    Application,
    Provider,
    Runtime,
    ToolchainSdk,
    CapabilityPack,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PersistentDataPolicy {
    RetainOnRemove,
    ExplicitMigration,
    Ephemeral,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentDependency {
    pub component_id: String,
    pub minimum_revision: u64,
    pub exact_package_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrimeComponentManifest {
    pub schema: String,
    pub component_id: String,
    pub revision: u64,
    pub version: String,
    pub class: ComponentClass,
    pub package_digest: String,
    pub publisher_id: String,
    pub publisher_key_id: String,
    pub signature: String,
    #[serde(default)]
    pub architectures: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<ComponentDependency>,
    pub minimum_prime_generation: Option<String>,
    pub required_capability_interface: Option<String>,
    pub required_application_profile_schema: Option<String>,
    pub persistent_data_policy: PersistentDataPolicy,
    #[serde(default)]
    pub application_profiles: Vec<String>,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub base_system_requirements: Vec<String>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentManifestError {
    Schema,
    ComponentId,
    Revision,
    Version,
    PackageDigest,
    PublisherId,
    PublisherKeyId,
    Signature,
    Architecture,
    Dependency,
    DuplicateDependency,
    RegistrationIdentity,
    BaseSystemRequirement,
    CompatibilityRequirement,
}

impl fmt::Display for ComponentManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Schema => "component schema is unsupported",
            Self::ComponentId => "component id is invalid",
            Self::Revision => "component revision must be positive",
            Self::Version => "component version is invalid",
            Self::PackageDigest => "package digest must be canonical SHA-256",
            Self::PublisherId => "publisher id is invalid",
            Self::PublisherKeyId => "publisher key id must be canonical SHA-256",
            Self::Signature => "component signature is missing",
            Self::Architecture => "component architecture is invalid or duplicated",
            Self::Dependency => "component dependency is invalid",
            Self::DuplicateDependency => "component dependency is duplicated",
            Self::RegistrationIdentity => {
                "component registration identity is invalid or duplicated"
            }
            Self::BaseSystemRequirement => "base-system requirement is empty or duplicated",
            Self::CompatibilityRequirement => {
                "component compatibility requirement is empty or non-canonical"
            }
        };
        f.write_str(message)
    }
}

impl std::error::Error for ComponentManifestError {}

pub fn validate_component_manifest(
    manifest: &PrimeComponentManifest,
) -> Result<(), ComponentManifestError> {
    if manifest.schema != PRIME_COMPONENT_SCHEMA {
        return Err(ComponentManifestError::Schema);
    }
    if !valid_identifier(&manifest.component_id) {
        return Err(ComponentManifestError::ComponentId);
    }
    if manifest.revision == 0 {
        return Err(ComponentManifestError::Revision);
    }
    if manifest.version.trim().is_empty() || manifest.version != manifest.version.trim() {
        return Err(ComponentManifestError::Version);
    }
    if !valid_sha256_label(&manifest.package_digest) {
        return Err(ComponentManifestError::PackageDigest);
    }
    if !valid_identifier(&manifest.publisher_id) {
        return Err(ComponentManifestError::PublisherId);
    }
    if !valid_sha256_label(&manifest.publisher_key_id) {
        return Err(ComponentManifestError::PublisherKeyId);
    }
    if manifest.signature.trim().is_empty() {
        return Err(ComponentManifestError::Signature);
    }

    validate_unique_identities(
        &manifest.architectures,
        ComponentManifestError::Architecture,
    )?;

    for requirement in [
        manifest.minimum_prime_generation.as_deref(),
        manifest.required_capability_interface.as_deref(),
        manifest.required_application_profile_schema.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if requirement.trim().is_empty() || requirement != requirement.trim() {
            return Err(ComponentManifestError::CompatibilityRequirement);
        }
    }

    let mut dependencies = BTreeSet::new();
    for dependency in &manifest.dependencies {
        if !valid_identifier(&dependency.component_id)
            || dependency.minimum_revision == 0
            || dependency
                .exact_package_digest
                .as_deref()
                .is_some_and(|digest| !valid_sha256_label(digest))
        {
            return Err(ComponentManifestError::Dependency);
        }
        if !dependencies.insert(dependency.component_id.as_str()) {
            return Err(ComponentManifestError::DuplicateDependency);
        }
    }

    for registrations in [
        &manifest.application_profiles,
        &manifest.services,
        &manifest.capabilities,
    ] {
        validate_unique_identities(registrations, ComponentManifestError::RegistrationIdentity)?;
    }

    let mut base_requirements = BTreeSet::new();
    for requirement in &manifest.base_system_requirements {
        if requirement.trim().is_empty()
            || requirement != requirement.trim()
            || !base_requirements.insert(requirement.as_str())
        {
            return Err(ComponentManifestError::BaseSystemRequirement);
        }
    }

    Ok(())
}

fn validate_unique_identities(
    values: &[String],
    error: ComponentManifestError,
) -> Result<(), ComponentManifestError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !valid_identifier(value) || !seen.insert(value.as_str()) {
            return Err(error);
        }
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 {
        return false;
    }
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    bytes.all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
    })
}

fn valid_sha256_label(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::APPLICATIONS_PROJECTION_SCHEMA;

    const DIGEST_A: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_B: &str =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn manifest() -> PrimeComponentManifest {
        PrimeComponentManifest {
            schema: PRIME_COMPONENT_SCHEMA.to_owned(),
            component_id: "origins.runtime".to_owned(),
            revision: 1,
            version: "1.0.0".to_owned(),
            class: ComponentClass::Runtime,
            package_digest: DIGEST_A.to_owned(),
            publisher_id: "thetechguy.origins".to_owned(),
            publisher_key_id: DIGEST_B.to_owned(),
            signature: "ed25519:fixture-signature".to_owned(),
            architectures: vec!["x86_64".to_owned()],
            dependencies: vec![],
            minimum_prime_generation: Some("prime-generation-p2".to_owned()),
            required_capability_interface: Some("1.0".to_owned()),
            required_application_profile_schema: Some(APPLICATIONS_PROJECTION_SCHEMA.to_owned()),
            persistent_data_policy: PersistentDataPolicy::RetainOnRemove,
            application_profiles: vec![],
            services: vec!["originsd".to_owned()],
            capabilities: vec!["origins.factory".to_owned()],
            base_system_requirements: vec![],
            limitations: vec![],
        }
    }

    #[test]
    fn generic_component_contract_accepts_exact_origins_shape() {
        validate_component_manifest(&manifest()).unwrap();
    }

    #[test]
    fn package_identity_and_publisher_key_must_be_exact_sha256() {
        let mut item = manifest();
        item.package_digest = "latest".to_owned();
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::PackageDigest)
        );

        let mut item = manifest();
        item.publisher_key_id = "publisher-key".to_owned();
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::PublisherKeyId)
        );
    }

    #[test]
    fn component_and_registration_ids_are_canonical_and_unique() {
        let mut item = manifest();
        item.services = vec!["originsd".to_owned(), "originsd".to_owned()];
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::RegistrationIdentity)
        );

        let mut item = manifest();
        item.component_id = "Origins Runtime".to_owned();
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::ComponentId)
        );
    }

    #[test]
    fn dependencies_are_exact_and_nonduplicated() {
        let mut item = manifest();
        item.dependencies = vec![
            ComponentDependency {
                component_id: "runtime.python".to_owned(),
                minimum_revision: 3,
                exact_package_digest: Some(DIGEST_B.to_owned()),
            },
            ComponentDependency {
                component_id: "runtime.python".to_owned(),
                minimum_revision: 4,
                exact_package_digest: None,
            },
        ];
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::DuplicateDependency)
        );
    }

    #[test]
    fn compatibility_requirements_must_be_nonempty_and_canonical() {
        for invalid in ["", " capability-v1", "profile-v1 "] {
            let mut item = manifest();
            item.required_capability_interface = Some(invalid.to_owned());
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::CompatibilityRequirement)
            );
        }

        let mut item = manifest();
        item.minimum_prime_generation = Some(" ".to_owned());
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::CompatibilityRequirement)
        );
    }

    #[test]
    fn persistent_data_ownership_is_explicit_in_serialized_contract() {
        let value = serde_json::to_value(manifest()).unwrap();
        assert_eq!(value["persistent_data_policy"], "RETAIN_ON_REMOVE");
        assert_eq!(value["schema"], PRIME_COMPONENT_SCHEMA);
        assert_eq!(value["class"], "RUNTIME");
    }

    #[test]
    fn unsigned_or_zero_revision_manifest_fails_closed() {
        let mut item = manifest();
        item.signature.clear();
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::Signature)
        );

        let mut item = manifest();
        item.revision = 0;
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::Revision)
        );
    }
}
