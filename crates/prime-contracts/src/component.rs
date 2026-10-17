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
    Limitation,
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
            Self::Signature => "component signature must be canonical Ed25519",
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
            Self::Limitation => "component limitation is empty, non-canonical, or duplicated",
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
    if !valid_component_identifier(&manifest.component_id) {
        return Err(ComponentManifestError::ComponentId);
    }
    if manifest.revision == 0 {
        return Err(ComponentManifestError::Revision);
    }
    if !valid_component_version(&manifest.version) {
        return Err(ComponentManifestError::Version);
    }
    if !valid_sha256_label(&manifest.package_digest) {
        return Err(ComponentManifestError::PackageDigest);
    }
    if !valid_component_identifier(&manifest.publisher_id) {
        return Err(ComponentManifestError::PublisherId);
    }
    if !valid_sha256_label(&manifest.publisher_key_id) {
        return Err(ComponentManifestError::PublisherKeyId);
    }
    if !valid_ed25519_signature(&manifest.signature) {
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
        if !valid_requirement(requirement) {
            return Err(ComponentManifestError::CompatibilityRequirement);
        }
    }

    let mut dependencies = BTreeSet::new();
    for dependency in &manifest.dependencies {
        if !valid_component_identifier(&dependency.component_id)
            || dependency.component_id == manifest.component_id
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

    let mut registrations = BTreeSet::new();
    for registration_group in [
        &manifest.application_profiles,
        &manifest.services,
        &manifest.capabilities,
    ] {
        for registration in registration_group {
            if !valid_component_identifier(registration)
                || !registrations.insert(registration.as_str())
            {
                return Err(ComponentManifestError::RegistrationIdentity);
            }
        }
    }

    let mut base_requirements = BTreeSet::new();
    for requirement in &manifest.base_system_requirements {
        if !valid_bounded_text(requirement) || !base_requirements.insert(requirement.as_str()) {
            return Err(ComponentManifestError::BaseSystemRequirement);
        }
    }

    let mut limitations = BTreeSet::new();
    for limitation in &manifest.limitations {
        if !valid_bounded_text(limitation) || !limitations.insert(limitation.as_str()) {
            return Err(ComponentManifestError::Limitation);
        }
    }

    Ok(())
}

pub fn valid_component_version(value: &str) -> bool {
    valid_bounded_text(value)
        && !value.chars().any(|c| {
            matches!(
                c,
                '\u{2028}' | '\u{2029}' | '\u{202A}'..='\u{202E}'
                    | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}'
                    | '\u{061C}'
            )
        })
}

fn valid_bounded_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value == value.trim()
        && value.len() <= 128
        && !value.chars().any(char::is_control)
        && !value.chars().any(|c| matches!(c as u32, 0x00A0 | 0x00AD | 0x034F | 0x180E | 0x2007 | 0x200B | 0x202F | 0x200C | 0x200D | 0x2060..=0x2064 | 0x206A..=0x206F | 0xFE00..=0xFE0F | 0xFEFF))
        && !value.chars().any(|c| matches!(c as u32, 0x115F | 0x1160 | 0x3164 | 0xFFA0 | 0x2800 | 0xE0000..=0xE007F))
        && !value.chars().any(|c| matches!(c, '\u{2028}' | '\u{2029}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}'))
}

fn validate_unique_identities(
    values: &[String],
    error: ComponentManifestError,
) -> Result<(), ComponentManifestError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !valid_component_identifier(value) || !seen.insert(value.as_str()) {
            return Err(error);
        }
    }
    Ok(())
}

pub fn valid_component_identifier(value: &str) -> bool {
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
    let mut previous_was_separator = false;
    for byte in bytes {
        let is_separator = matches!(byte, b'.' | b'_' | b'-');
        if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || is_separator)
            || (is_separator && previous_was_separator)
        {
            return false;
        }
        previous_was_separator = is_separator;
    }
    !previous_was_separator
}

fn valid_requirement(value: &str) -> bool {
    valid_bounded_text(value)
}

fn valid_ed25519_signature(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("ed25519:") else {
        return false;
    };
    hex.len() == 128
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
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
            signature: format!("ed25519:{}", "a".repeat(128)),
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
    fn version_must_be_bounded_and_single_line() {
        for invalid in [
            "",
            " 1.0.0",
            "1.0.0 ",
            "1.0\nspoof",
            "1.0\tspoof",
            "1.0\u{202E}spoof",
            "1.0\u{2028}spoof",
            "1.0\u{2066}spoof",
            "1.0\u{200B}spoof",
            "1.0\u{200D}spoof",
            "1.0\u{2060}spoof",
            "1.0\u{FEFF}spoof",
            "1.0\u{00AD}spoof",
            "1.0\u{206A}spoof",
        ] {
            let mut item = manifest();
            item.version = invalid.to_owned();
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::Version)
            );
        }

        let mut item = manifest();
        item.version = "v".repeat(129);
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::Version)
        );
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
    fn signature_must_be_canonical_ed25519() {
        for invalid in ["", "fixture-signature", "ed25519:fixture-signature", "ed25519:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"] {
            let mut item = manifest();
            item.signature = invalid.to_owned();
            assert_eq!(validate_component_manifest(&item), Err(ComponentManifestError::Signature));
        }
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
    fn identifiers_reject_ambiguous_separator_forms() {
        for invalid in [
            "origins..runtime",
            "origins__runtime",
            "origins.-runtime",
            "origins.runtime.",
        ] {
            let mut item = manifest();
            item.component_id = invalid.to_owned();
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::ComponentId)
            );
        }
    }

    #[test]
    fn registration_identity_cannot_alias_across_namespaces() {
        let mut item = manifest();
        item.application_profiles = vec!["shared.identity".to_owned()];
        item.services = vec!["shared.identity".to_owned()];
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::RegistrationIdentity)
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
    fn component_cannot_depend_on_itself() {
        let mut item = manifest();
        item.dependencies = vec![ComponentDependency {
            component_id: item.component_id.clone(),
            minimum_revision: 1,
            exact_package_digest: None,
        }];
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::Dependency)
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

        for invalid in [
            "generation\nspoof",
            "generation\tspoof",
            "generation\u{200B}spoof",
        ] {
            let mut item = manifest();
            item.minimum_prime_generation = Some(invalid.to_owned());
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::CompatibilityRequirement)
            );
        }

        let mut item = manifest();
        item.required_application_profile_schema = Some("s".repeat(129));
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::CompatibilityRequirement)
        );
    }

    #[test]
    fn deprecated_format_controls_cannot_spoof_component_text() {
        for hidden in [
            char::from_u32(0x00AD).unwrap(),
            char::from_u32(0x206A).unwrap(),
            char::from_u32(0x206F).unwrap(),
        ] {
            let mut item = manifest();
            item.base_system_requirements = vec![format!("kernel{hidden}module")];
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::BaseSystemRequirement)
            );

            let mut item = manifest();
            item.required_capability_interface = Some(format!("capability{hidden}v1"));
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::CompatibilityRequirement)
            );

            let mut item = manifest();
            item.limitations = vec![format!("requires{hidden}restart")];
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::Limitation)
            );
        }
    }

    #[test]
    fn invisible_grapheme_modifiers_cannot_spoof_component_text() {
        for hidden in ['\u{034F}', '\u{180E}', '\u{FE0F}'] {
            let mut item = manifest();
            item.version = format!("1.0{hidden}.0");
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::Version)
            );

            let mut item = manifest();
            item.required_capability_interface = Some(format!("capability{hidden}v1"));
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::CompatibilityRequirement)
            );

            let mut item = manifest();
            item.base_system_requirements = vec![format!("kernel{hidden}module")];
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::BaseSystemRequirement)
            );
        }
    }

    #[test]
    fn invisible_hangul_and_tag_characters_cannot_spoof_manifest_text() {
        for hidden in ['\u{115F}', '\u{1160}', '\u{3164}', '\u{FFA0}', '\u{E0001}'] {
            let mut item = manifest();
            item.version = format!("1.0{hidden}.0");
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::Version)
            );

            let mut item = manifest();
            item.required_capability_interface = Some(format!("capability{hidden}v1"));
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::CompatibilityRequirement)
            );

            let mut item = manifest();
            item.base_system_requirements = vec![format!("kernel{hidden}module")];
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::BaseSystemRequirement)
            );

            let mut item = manifest();
            item.limitations = vec![format!("requires{hidden}restart")];
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::Limitation)
            );
        }
    }

    #[test]
    fn nonbreaking_spaces_cannot_spoof_component_manifest_text() {
        for hidden in ['\u{00A0}', '\u{2007}', '\u{202F}'] {
            let mut item = manifest();
            item.version = format!("1.0{hidden}.0");
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::Version)
            );

            let mut item = manifest();
            item.required_capability_interface = Some(format!("capability{hidden}v1"));
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::CompatibilityRequirement)
            );

            let mut item = manifest();
            item.base_system_requirements = vec![format!("kernel{hidden}module")];
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::BaseSystemRequirement)
            );

            let mut item = manifest();
            item.limitations = vec![format!("requires{hidden}restart")];
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::Limitation)
            );
        }
    }

    #[test]
    fn invisible_mathematical_operators_cannot_spoof_component_manifest_text() {
        for hidden in ['\u{2061}', '\u{2062}', '\u{2063}', '\u{2064}'] {
            let mut item = manifest();
            item.version = format!("1.0{hidden}.0");
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::Version)
            );

            let mut item = manifest();
            item.required_capability_interface = Some(format!("capability{hidden}v1"));
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::CompatibilityRequirement)
            );

            let mut item = manifest();
            item.base_system_requirements = vec![format!("kernel{hidden}module")];
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::BaseSystemRequirement)
            );

            let mut item = manifest();
            item.limitations = vec![format!("requires{hidden}restart")];
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::Limitation)
            );
        }
    }

    #[test]
    fn braille_blank_cannot_spoof_component_manifest_text() {
        let hidden = '\u{2800}';

        let mut item = manifest();
        item.version = format!("1.0{hidden}.0");
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::Version)
        );

        let mut item = manifest();
        item.required_capability_interface = Some(format!("capability{hidden}v1"));
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::CompatibilityRequirement)
        );

        let mut item = manifest();
        item.base_system_requirements = vec![format!("kernel{hidden}module")];
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::BaseSystemRequirement)
        );

        let mut item = manifest();
        item.limitations = vec![format!("requires{hidden}restart")];
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::Limitation)
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
    #[test]
    fn limitations_must_be_nonempty_canonical_and_unique() {
        let mut value = manifest();
        value.limitations = vec!["requires-restart".to_owned()];
        assert_eq!(validate_component_manifest(&value), Ok(()));

        value.limitations = vec![" requires-restart".to_owned()];
        assert_eq!(
            validate_component_manifest(&value),
            Err(ComponentManifestError::Limitation)
        );

        value.limitations = vec!["requires-restart".to_owned(), "requires-restart".to_owned()];
        assert_eq!(
            validate_component_manifest(&value),
            Err(ComponentManifestError::Limitation)
        );

        value.limitations = vec!["x".repeat(129)];
        assert_eq!(
            validate_component_manifest(&value),
            Err(ComponentManifestError::Limitation)
        );

        value.limitations = vec!["requires\u{2060}restart".to_owned()];
        assert_eq!(
            validate_component_manifest(&value),
            Err(ComponentManifestError::Limitation)
        );

        value.limitations = vec!["requires\nrestart".to_owned()];
        assert_eq!(
            validate_component_manifest(&value),
            Err(ComponentManifestError::Limitation)
        );
    }

    #[test]
    fn architecture_identity_must_be_canonical_and_unique() {
        for invalid in ["X86_64", "x86__64", "x86_64 ", "x86_64\nspoof"] {
            let mut item = manifest();
            item.architectures = vec![invalid.to_owned()];
            assert_eq!(
                validate_component_manifest(&item),
                Err(ComponentManifestError::Architecture)
            );
        }

        let mut item = manifest();
        item.architectures = vec!["x86_64".to_owned(), "x86_64".to_owned()];
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::Architecture)
        );
    }

    #[test]
    fn dependency_revision_and_digest_fail_closed() {
        let mut item = manifest();
        item.dependencies = vec![ComponentDependency {
            component_id: "runtime.python".to_owned(),
            minimum_revision: 0,
            exact_package_digest: None,
        }];
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::Dependency)
        );

        item.dependencies[0].minimum_revision = 1;
        item.dependencies[0].exact_package_digest = Some("sha256:NOT-CANONICAL".to_owned());
        assert_eq!(
            validate_component_manifest(&item),
            Err(ComponentManifestError::Dependency)
        );

        item.dependencies[0].exact_package_digest = Some(DIGEST_B.to_owned());
        assert_eq!(validate_component_manifest(&item), Ok(()));
    }

    #[test]
    fn base_system_requirements_are_bounded_and_control_free() {
        let mut value = manifest();
        value.base_system_requirements = vec!["kernel.module.example".to_owned()];
        assert_eq!(validate_component_manifest(&value), Ok(()));

        value.base_system_requirements = vec!["x".repeat(129)];
        assert_eq!(
            validate_component_manifest(&value),
            Err(ComponentManifestError::BaseSystemRequirement)
        );

        value.base_system_requirements = vec!["kernel\tmodule".to_owned()];
        assert_eq!(
            validate_component_manifest(&value),
            Err(ComponentManifestError::BaseSystemRequirement)
        );

        value.base_system_requirements = vec![" kernel.module.example".to_owned()];
        assert_eq!(
            validate_component_manifest(&value),
            Err(ComponentManifestError::BaseSystemRequirement)
        );

        value.base_system_requirements = vec!["kernel.module.example ".to_owned()];
        assert_eq!(
            validate_component_manifest(&value),
            Err(ComponentManifestError::BaseSystemRequirement)
        );

        value.base_system_requirements = vec![
            "kernel.module.example".to_owned(),
            "kernel.module.example".to_owned(),
        ];
        assert_eq!(
            validate_component_manifest(&value),
            Err(ComponentManifestError::BaseSystemRequirement)
        );
    }
}
