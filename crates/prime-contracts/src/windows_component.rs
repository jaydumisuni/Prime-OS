use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

pub const WINDOWS_COMPONENT_SCHEMA: &str = "prime.windows-component.v1";
pub const WINDOWS_COMPONENT_EVIDENCE_SCHEMA: &str = "prime.windows-component-evidence.v1";
const WINDOWS_COMPONENT_PREFIX: &str = "windows-component:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsComponentReference {
    pub component_id: String,
    pub revision: u64,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsComponentReferenceError {
    Prefix,
    ComponentId,
    Revision,
    Digest,
    Structure,
}

impl fmt::Display for WindowsComponentReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Prefix => "reference does not use windows-component prefix",
            Self::ComponentId => "component id is invalid",
            Self::Revision => "component revision is invalid",
            Self::Digest => "component digest is invalid",
            Self::Structure => "component reference structure is invalid",
        };
        f.write_str(message)
    }
}

impl std::error::Error for WindowsComponentReferenceError {}

impl WindowsComponentReference {
    pub fn parse(raw: &str) -> Result<Self, WindowsComponentReferenceError> {
        let body = raw
            .strip_prefix(WINDOWS_COMPONENT_PREFIX)
            .ok_or(WindowsComponentReferenceError::Prefix)?;
        if body.matches('#').count() != 1 || body.matches('@').count() != 1 {
            return Err(WindowsComponentReferenceError::Structure);
        }
        let (identity_revision, digest) = body
            .split_once('#')
            .ok_or(WindowsComponentReferenceError::Structure)?;
        let (component_id, revision_raw) = identity_revision
            .split_once('@')
            .ok_or(WindowsComponentReferenceError::Structure)?;
        if !valid_component_id(component_id) {
            return Err(WindowsComponentReferenceError::ComponentId);
        }
        if revision_raw.is_empty()
            || !revision_raw.bytes().all(|byte| byte.is_ascii_digit())
            || (revision_raw.len() > 1 && revision_raw.starts_with('0'))
        {
            return Err(WindowsComponentReferenceError::Revision);
        }
        let revision = revision_raw
            .parse::<u64>()
            .map_err(|_| WindowsComponentReferenceError::Revision)?;
        if revision == 0 {
            return Err(WindowsComponentReferenceError::Revision);
        }
        if !valid_sha256_label(digest) {
            return Err(WindowsComponentReferenceError::Digest);
        }
        Ok(Self {
            component_id: component_id.to_owned(),
            revision,
            digest: digest.to_owned(),
        })
    }
}

impl fmt::Display for WindowsComponentReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{WINDOWS_COMPONENT_PREFIX}{}@{}#{}",
            self.component_id, self.revision, self.digest
        )
    }
}

fn valid_component_id(value: &str) -> bool {
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

pub fn valid_sha256_label(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WindowsComponentKind {
    Runtime,
    ApplicationInstaller,
    Support,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WindowsInstallerKind {
    Msi,
    Exe,
    Builtin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WindowsVerificationProbe {
    FileExists {
        path: String,
    },
    FileSha256 {
        path: String,
        sha256: String,
    },
    RegistryValueEquals {
        hive: String,
        key: String,
        name: String,
        value: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowsComponentManifest {
    pub schema: String,
    pub component_id: String,
    pub revision: u64,
    pub digest: String,
    pub display_name: String,
    pub kind: WindowsComponentKind,
    #[serde(default)]
    pub workload_arches: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub installer_kind: WindowsInstallerKind,
    pub artifact_identity: Option<String>,
    pub artifact_path: Option<String>,
    #[serde(default)]
    pub installer_args: Vec<String>,
    #[serde(default)]
    pub accepted_exit_codes: Vec<i32>,
    pub restart_compatibility_environment: bool,
    #[serde(default)]
    pub verification: Vec<WindowsVerificationProbe>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WindowsComponentOperation {
    Verify,
    Install,
    Recover,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WindowsComponentOutcome {
    Admitted,
    Installed,
    AlreadySatisfied,
    InstallerFailed,
    VerificationFailed,
    ComponentUnavailable,
    PolicyDenied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowsProbeEvidence {
    pub kind: String,
    pub target: String,
    pub passed: bool,
    pub observed: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowsComponentEvidence {
    pub schema: String,
    pub transaction_id: Uuid,
    pub host_id: Uuid,
    pub generation_id: String,
    pub application_id: Uuid,
    pub profile_revision: u64,
    pub profile_digest: String,
    pub component_id: String,
    pub component_revision: u64,
    pub component_digest: String,
    pub provider_id: String,
    pub provider_revision: u64,
    pub donor_fingerprint: String,
    pub installer_artifact_identity: Option<String>,
    pub operation: WindowsComponentOperation,
    pub outcome: WindowsComponentOutcome,
    pub installer_exit_code: Option<i32>,
    pub requested_at: String,
    pub completed_at: Option<String>,
    #[serde(default)]
    pub verification: Vec<WindowsProbeEvidence>,
    #[serde(default)]
    pub enforcement_properties: Vec<crate::LaunchEnforcementProperty>,
    pub resulting_marker_digest: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn windows_component_reference_round_trips_exact_pin() {
        let raw = format!("windows-component:msvc.2015-2022.x64@1#{DIGEST}");
        let parsed = WindowsComponentReference::parse(&raw).expect("valid W2 component reference");
        assert_eq!(parsed.component_id, "msvc.2015-2022.x64");
        assert_eq!(parsed.revision, 1);
        assert_eq!(parsed.digest, DIGEST);
        assert_eq!(parsed.to_string(), raw);
    }

    #[test]
    fn windows_component_reference_rejects_malformed_identity_or_pin() {
        let cases = [
            format!("windows-component:MSVC@1#{DIGEST}"),
            format!("windows-component:-bad@1#{DIGEST}"),
            format!("windows-component:good@0#{DIGEST}"),
            format!("windows-component:good@+1#{DIGEST}"),
            "windows-component:good@1#sha256:ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_owned(),
            "windows-component:good@1#sha256:abcd".to_owned(),
            format!("windows-component:good@1@2#{DIGEST}"),
            format!("windows-component:good@1#{DIGEST}#extra"),
        ];
        for raw in cases {
            assert!(
                WindowsComponentReference::parse(&raw).is_err(),
                "accepted {raw}"
            );
        }
    }

    #[test]
    fn windows_component_manifest_serializes_typed_installer_and_probes() {
        let manifest = WindowsComponentManifest {
            schema: WINDOWS_COMPONENT_SCHEMA.to_owned(),
            component_id: "runtime.test.x64".to_owned(),
            revision: 1,
            digest: DIGEST.to_owned(),
            display_name: "Runtime Test".to_owned(),
            kind: WindowsComponentKind::Runtime,
            workload_arches: vec!["x86_64".to_owned()],
            depends_on: vec![],
            installer_kind: WindowsInstallerKind::Msi,
            artifact_identity: Some(DIGEST.to_owned()),
            artifact_path: Some(
                "/usr/lib/prime/windows-components/artifacts/runtime-test.msi".to_owned(),
            ),
            installer_args: vec!["/qn".to_owned(), "/norestart".to_owned()],
            accepted_exit_codes: vec![0, 3010],
            restart_compatibility_environment: true,
            verification: vec![
                WindowsVerificationProbe::FileExists {
                    path: "drive_c/PrimeW2/installed.txt".to_owned(),
                },
                WindowsVerificationProbe::RegistryValueEquals {
                    hive: "HKLM".to_owned(),
                    key: r"Software\PrimeW2".to_owned(),
                    name: "Installed".to_owned(),
                    value: "1".to_owned(),
                },
            ],
            limitations: vec![],
        };
        let json = serde_json::to_value(&manifest).expect("serialize manifest");
        assert_eq!(json["installer_kind"], "MSI");
        assert_eq!(json["verification"][0]["kind"], "FILE_EXISTS");
        assert_eq!(json["verification"][1]["kind"], "REGISTRY_VALUE_EQUALS");
    }

    #[test]
    fn windows_component_evidence_has_explicit_install_outcomes() {
        let outcomes = [
            WindowsComponentOutcome::Admitted,
            WindowsComponentOutcome::Installed,
            WindowsComponentOutcome::AlreadySatisfied,
            WindowsComponentOutcome::InstallerFailed,
            WindowsComponentOutcome::VerificationFailed,
        ];
        let encoded = serde_json::to_value(outcomes).expect("serialize outcomes");
        assert_eq!(encoded[0], "ADMITTED");
        assert_eq!(encoded[1], "INSTALLED");
        assert_eq!(encoded[2], "ALREADY_SATISFIED");
        assert_eq!(encoded[3], "INSTALLER_FAILED");
        assert_eq!(encoded[4], "VERIFICATION_FAILED");
    }
}
