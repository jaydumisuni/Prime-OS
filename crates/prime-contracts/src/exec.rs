use serde::{Deserialize, Serialize};

pub const EXEC_INSPECTION_SCHEMA: &str = "prime.exec-inspection.v1";

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
