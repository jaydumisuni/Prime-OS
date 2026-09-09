use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const EXEC_INSPECTION_SCHEMA: &str = "prime.exec-inspection.v1";
pub const NATIVE_LAUNCH_REQUEST_SCHEMA: &str = "prime.native-launch-request.v1";
pub const NATIVE_LAUNCH_EVIDENCE_SCHEMA: &str = "prime.native-launch-evidence.v1";
pub const WINDOWS_PROVIDER_MANIFEST_SCHEMA: &str = "prime.windows-provider-manifest.v1";
pub const WINDOWS_LAUNCH_EVIDENCE_SCHEMA: &str = "prime.windows-launch-evidence.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArtifactFormat {
    #[serde(rename = "ELF")]
    Elf,
    #[serde(rename = "PE32")]
    Pe32,
    #[serde(rename = "PE32+")]
    Pe32Plus,
    #[serde(rename = "JAR")]
    Jar,
    #[serde(rename = "CLASS")]
    Class,
    #[serde(rename = "APK")]
    Apk,
    #[serde(rename = "DEX")]
    Dex,
    #[serde(rename = "WASM")]
    Wasm,
    #[serde(rename = "MACHO")]
    MachO,
    #[serde(rename = "APP_BUNDLE")]
    AppBundle,
    #[serde(rename = "IPA")]
    Ipa,
    #[serde(rename = "OTHER")]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeFamily {
    #[serde(rename = "NATIVE_LINUX")]
    NativeLinux,
    #[serde(rename = "WINDOWS")]
    Windows,
    #[serde(rename = "JVM")]
    Jvm,
    #[serde(rename = "ANDROID")]
    Android,
    #[serde(rename = "WASM")]
    Wasm,
    #[serde(rename = "DARWIN")]
    Darwin,
    #[serde(rename = "IOS")]
    Ios,
    #[serde(rename = "OTHER")]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionBackend {
    #[serde(rename = "NATIVE")]
    Native,
    #[serde(rename = "PERSONALITY")]
    Personality,
    #[serde(rename = "CONTAINER")]
    Container,
    #[serde(rename = "VM")]
    Vm,
    #[serde(rename = "REMOTE_PROVIDER")]
    RemoteProvider,
    #[serde(rename = "SPECIALIZED_PROVIDER")]
    SpecializedProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MechanicalCompatibilityState {
    #[serde(rename = "UNKNOWN")]
    Unknown,
    #[serde(rename = "RECOGNIZED")]
    Recognized,
    #[serde(rename = "INSTALLABLE")]
    Installable,
    #[serde(rename = "LAUNCHES")]
    Launches,
    #[serde(rename = "PARTIALLY_FUNCTIONAL")]
    PartiallyFunctional,
    #[serde(rename = "FUNCTIONAL")]
    Functional,
    #[serde(rename = "BROKEN")]
    Broken,
    #[serde(rename = "UNSUPPORTED")]
    Unsupported,
    #[serde(rename = "REQUIRES_VM")]
    RequiresVm,
    #[serde(rename = "REQUIRES_REMOTE_PROVIDER")]
    RequiresRemoteProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecInspection {
    pub schema: String,
    pub artifact_identity: String,
    pub size_bytes: u64,
    pub format: ArtifactFormat,
    pub runtime_family: RuntimeFamily,
    pub workload_arch: Option<String>,
    pub suggested_backend: Option<ExecutionBackend>,
    pub native_compatible: bool,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeLaunchRequest {
    pub schema: String,
    pub application_id: Uuid,
    pub artifact_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaunchEnforcementProperty {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NativeLaunchOutcome {
    Admitted,
    ExitedSuccess,
    SystemdOrWorkloadFailure,
    LauncherFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowsProviderManifest {
    pub schema: String,
    pub provider_id: String,
    pub provider_revision: u64,
    pub adapter_path: String,
    #[serde(default)]
    pub formats: Vec<ArtifactFormat>,
    #[serde(default)]
    pub workload_arches: Vec<String>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeLaunchEvidence {
    pub schema: String,
    pub launch_id: Uuid,
    pub host_id: Uuid,
    pub generation_id: String,
    pub application_id: Uuid,
    pub profile_revision: u64,
    pub profile_digest: String,
    pub policy_id: Uuid,
    pub policy_revision: u64,
    pub policy_digest: String,
    pub artifact_identity: String,
    pub staged_artifact_path: String,
    pub unit_name: String,
    pub requested_at: String,
    pub completed_at: Option<String>,
    pub outcome: NativeLaunchOutcome,
    pub launcher_exit_code: Option<i32>,
    #[serde(default)]
    pub enforcement_properties: Vec<LaunchEnforcementProperty>,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PersonalityLaunchOutcome {
    Admitted,
    ExitedSuccess,
    SystemdOrWorkloadFailure,
    ProviderFailure,
    LauncherFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowsLaunchEvidence {
    pub schema: String,
    pub launch_id: Uuid,
    pub host_id: Uuid,
    pub generation_id: String,
    pub application_id: Uuid,
    pub profile_revision: u64,
    pub profile_digest: String,
    pub policy_id: Uuid,
    pub policy_revision: u64,
    pub policy_digest: String,
    pub artifact_identity: String,
    pub staged_artifact_path: String,
    pub execution_backend: ExecutionBackend,
    pub runtime_family: RuntimeFamily,
    pub provider_id: String,
    pub provider_revision: u64,
    pub unit_name: String,
    pub requested_at: String,
    pub completed_at: Option<String>,
    pub outcome: PersonalityLaunchOutcome,
    pub launcher_exit_code: Option<i32>,
    #[serde(default)]
    pub enforcement_properties: Vec<LaunchEnforcementProperty>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum LaunchEvidence {
    Native(NativeLaunchEvidence),
    Windows(WindowsLaunchEvidence),
}

#[cfg(test)]
mod windows_contract_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn windows_provider_manifest_serializes_exact_w1_contract() {
        let manifest = WindowsProviderManifest {
            schema: WINDOWS_PROVIDER_MANIFEST_SCHEMA.to_owned(),
            provider_id: "prime.windows.compat-a".to_owned(),
            provider_revision: 1,
            adapter_path: "/usr/libexec/prime/provider-a".to_owned(),
            formats: vec![ArtifactFormat::Pe32, ArtifactFormat::Pe32Plus],
            workload_arches: vec!["x86".to_owned(), "x86_64".to_owned()],
            limitations: vec!["W1_PORTABLE_ONLY".to_owned()],
        };

        let value = serde_json::to_value(&manifest).expect("serialize manifest");
        assert_eq!(value["schema"], WINDOWS_PROVIDER_MANIFEST_SCHEMA);
        assert_eq!(value["provider_id"], "prime.windows.compat-a");
        assert_eq!(value["provider_revision"], 1);
        assert_eq!(value["adapter_path"], "/usr/libexec/prime/provider-a");
        assert_eq!(value["formats"], json!(["PE32", "PE32+"]));
        assert_eq!(value["workload_arches"], json!(["x86", "x86_64"]));
    }

    fn native_evidence() -> NativeLaunchEvidence {
        NativeLaunchEvidence {
            schema: NATIVE_LAUNCH_EVIDENCE_SCHEMA.to_owned(),
            launch_id: Uuid::nil(),
            host_id: Uuid::nil(),
            generation_id: "generation-a".to_owned(),
            application_id: Uuid::nil(),
            profile_revision: 1,
            profile_digest: "sha256:profile".to_owned(),
            policy_id: Uuid::nil(),
            policy_revision: 1,
            policy_digest: "sha256:policy".to_owned(),
            artifact_identity: "sha256:artifact".to_owned(),
            staged_artifact_path: "/var/lib/prime/artifacts/sha256/artifact".to_owned(),
            unit_name: "prime-app-native.service".to_owned(),
            requested_at: "2026-09-10T00:00:00Z".to_owned(),
            completed_at: Some("2026-09-10T00:00:01Z".to_owned()),
            outcome: NativeLaunchOutcome::ExitedSuccess,
            launcher_exit_code: Some(0),
            enforcement_properties: vec![],
        }
    }

    #[test]
    fn launch_evidence_wrapper_preserves_native_wire_shape() {
        let direct = serde_json::to_value(native_evidence()).expect("serialize native");
        let wrapped = serde_json::to_value(LaunchEvidence::Native(native_evidence()))
            .expect("serialize wrapped native");
        assert_eq!(wrapped, direct);
    }

    #[test]
    fn windows_launch_evidence_names_personality_windows_and_provider() {
        let evidence = WindowsLaunchEvidence {
            schema: WINDOWS_LAUNCH_EVIDENCE_SCHEMA.to_owned(),
            launch_id: Uuid::nil(),
            host_id: Uuid::nil(),
            generation_id: "generation-w1".to_owned(),
            application_id: Uuid::nil(),
            profile_revision: 3,
            profile_digest: "sha256:profile".to_owned(),
            policy_id: Uuid::nil(),
            policy_revision: 4,
            policy_digest: "sha256:policy".to_owned(),
            artifact_identity: "sha256:pe".to_owned(),
            staged_artifact_path: "/var/lib/prime/artifacts/sha256/pe".to_owned(),
            execution_backend: ExecutionBackend::Personality,
            runtime_family: RuntimeFamily::Windows,
            provider_id: "prime.windows.compat-a".to_owned(),
            provider_revision: 1,
            unit_name: "prime-win-app.service".to_owned(),
            requested_at: "2026-09-10T00:00:00Z".to_owned(),
            completed_at: None,
            outcome: PersonalityLaunchOutcome::Admitted,
            launcher_exit_code: None,
            enforcement_properties: vec![],
        };

        let value = serde_json::to_value(LaunchEvidence::Windows(evidence))
            .expect("serialize windows evidence");
        assert_eq!(value["schema"], WINDOWS_LAUNCH_EVIDENCE_SCHEMA);
        assert_eq!(value["execution_backend"], "PERSONALITY");
        assert_eq!(value["runtime_family"], "WINDOWS");
        assert_eq!(value["provider_id"], "prime.windows.compat-a");
        assert_eq!(value["provider_revision"], 1);
        assert_eq!(value["outcome"], "ADMITTED");
    }
}
