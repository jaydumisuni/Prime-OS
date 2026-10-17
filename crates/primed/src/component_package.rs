use prime_contracts::{
    valid_component_identifier, valid_component_version, validate_component_manifest,
    ComponentManifestError, PersistentDataPolicy, PrimeComponentManifest,
    APPLICATIONS_PROJECTION_SCHEMA, CAPABILITY_INTERFACE_VERSION,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledComponent {
    pub component_id: String,
    pub revision: u64,
    pub package_digest: String,
}

#[derive(Debug)]
pub struct ComponentAdmissionContext<'a> {
    pub host_arch: &'a str,
    pub prime_generation: &'a str,
    pub capability_interface_version: &'a str,
    pub application_profile_schema: &'a str,
    pub installed: &'a [InstalledComponent],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedComponentPackage {
    pub package_path: PathBuf,
    pub package_digest: String,
    pub component_id: String,
    pub revision: u64,
    pub version: String,
    pub publisher_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentTransactionKind {
    Install,
    Update {
        previous_revision: u64,
        previous_package_digest: String,
    },
    Remove {
        previous_revision: u64,
        previous_package_digest: String,
    },
    Rollback {
        previous_revision: u64,
        previous_package_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentTransactionPlan {
    pub component_id: String,
    pub target_revision: u64,
    pub target_package_digest: String,
    pub kind: ComponentTransactionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentTransactionState {
    Planned,
    Applying,
    Applied,
    Failed { reason: String },
    RecoveryRequired { reason: String },
    RolledBack,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("invalid component transaction state transition")]
pub struct ComponentTransactionTransitionError;

impl ComponentTransactionState {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Applied | Self::RolledBack)
    }

    // A terminal state cannot be reached directly from planning or failure.
    // Callers must record application and, when needed, recovery first.
    pub fn advance(self, next: Self) -> Result<Self, ComponentTransactionTransitionError> {
        let permitted = matches!(
            (&self, &next),
            (Self::Planned, Self::Applying)
                | (Self::Applying, Self::Applied)
                | (Self::Applying, Self::Failed { .. })
                | (Self::Applying, Self::RecoveryRequired { .. })
                | (Self::Failed { .. }, Self::RecoveryRequired { .. })
                | (Self::RecoveryRequired { .. }, Self::RolledBack)
        );
        let reason_valid = |state: &Self| match state {
            Self::Failed { reason } | Self::RecoveryRequired { reason } => {
                // Invisible format controls, separators, and bidi marks can spoof
                // durable failure/recovery log evidence.
                !reason.trim().is_empty()
                    && reason.trim() == reason
                    && !reason.chars().any(|c| {
                        c.is_control()
                            || (c.is_whitespace() && c != ' ')
                            || matches!(
                                c,
                                '\u{2028}' | '\u{2029}' | '\u{202A}'..='\u{202E}'
                                    | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}'
                                    | '\u{061C}'
                            )
                            || matches!(
                                c as u32,
                                0x00AD | 0x034F | 0x115F | 0x1160 | 0x180B..=0x180F | 0x200B
                                    | 0x200C | 0x200D | 0x2060 | 0x206A..=0x206F | 0x3164
                                    | 0xFE00..=0xFE0F | 0xFEFF | 0xFFA0 | 0xE0001
                                    | 0xE0020..=0xE007F
                            )
                    })
            }
            _ => true,
        };
        if permitted && reason_valid(&self) && reason_valid(&next) {
            Ok(next)
        } else {
            Err(ComponentTransactionTransitionError)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentDataDisposition {
    Retain,
    Migrated,
    DiscardEphemeral,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentAdmissionOutcome {
    Ready(VerifiedComponentPackage),
    GenerationHandoff {
        component_id: String,
        requirements: Vec<String>,
    },
}

#[derive(Debug, Error)]
pub enum ComponentAdmissionError {
    #[error("component manifest is invalid: {0}")]
    Manifest(#[from] ComponentManifestError),
    #[error("component package I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("component package path is not a regular non-symlink file")]
    UnsafePackagePath,
    #[error("component package digest does not match manifest")]
    PackageDigestMismatch,
    #[error("target component {0} has a noncanonical identity")]
    InvalidTargetComponentId(String),
    #[error("installed component {0} has a noncanonical identity")]
    InvalidInstalledComponentId(String),
    #[error("installed component {0} has an invalid zero revision")]
    InvalidInstalledRevision(String),
    #[error("installed component {0} has a noncanonical package digest")]
    InvalidInstalledPackageDigest(String),
    #[error("target component {0} has a noncanonical package digest")]
    InvalidTargetPackageDigest(String),
    #[error("component publisher signature was not admitted")]
    SignatureRejected,
    #[error("component does not support host architecture {0}")]
    ArchitectureMismatch(String),
    #[error(
        "component requires unsupported capability interface {required}; host provides {host}"
    )]
    CapabilityInterfaceMismatch { required: String, host: String },
    #[error("component requires unsupported application profile schema {required}; host provides {host}")]
    ApplicationProfileSchemaMismatch { required: String, host: String },
    #[error("component is incompatible with current Prime generation")]
    GenerationIncompatible,
    #[error("required component dependency {0} is missing")]
    DependencyMissing(String),
    #[error("installed component identity {0} is ambiguous")]
    DuplicateInstalledComponent(String),
    #[error("component dependency {component_id} revision is below required {minimum_revision}")]
    DependencyRevision {
        component_id: String,
        minimum_revision: u64,
    },
    #[error("component dependency {0} package digest does not match exact pin")]
    DependencyDigest(String),
    #[error("component transaction for {0} does not advance the installed revision")]
    RevisionNotAdvanced(String),
    #[error("component {0} requires explicit persistent-data migration before removal")]
    PersistentDataMigrationRequired(String),
}

pub fn plan_component_transaction(
    package: &VerifiedComponentPackage,
    installed: &[InstalledComponent],
) -> Result<ComponentTransactionPlan, ComponentAdmissionError> {
    if !valid_component_identifier(&package.component_id) {
        return Err(ComponentAdmissionError::InvalidTargetComponentId(
            package.component_id.clone(),
        ));
    }
    if !canonical_component_digest(&package.package_digest) {
        return Err(ComponentAdmissionError::InvalidTargetPackageDigest(
            package.component_id.clone(),
        ));
    }
    // VerifiedComponentPackage is public; fail closed if a caller constructs it
    // without passing through manifest admission.
    if !valid_component_identifier(&package.publisher_id) {
        return Err(ComponentAdmissionError::Manifest(
            ComponentManifestError::PublisherId,
        ));
    }
    if !valid_component_version(&package.version) {
        return Err(ComponentAdmissionError::Manifest(
            ComponentManifestError::Version,
        ));
    }
    if package.revision == 0 {
        return Err(ComponentAdmissionError::RevisionNotAdvanced(
            package.component_id.clone(),
        ));
    }
    let by_id = index_installed(installed)?;
    let current = by_id.get(package.component_id.as_str()).copied();

    let kind = match current {
        None => ComponentTransactionKind::Install,
        Some(current) if package.revision > current.revision => ComponentTransactionKind::Update {
            previous_revision: current.revision,
            previous_package_digest: current.package_digest.clone(),
        },
        Some(_) => {
            return Err(ComponentAdmissionError::RevisionNotAdvanced(
                package.component_id.clone(),
            ))
        }
    };

    Ok(ComponentTransactionPlan {
        component_id: package.component_id.clone(),
        target_revision: package.revision,
        target_package_digest: package.package_digest.clone(),
        kind,
    })
}

pub fn plan_component_removal(
    component_id: &str,
    installed: &[InstalledComponent],
) -> Result<ComponentTransactionPlan, ComponentAdmissionError> {
    if !valid_component_identifier(component_id) {
        return Err(ComponentAdmissionError::InvalidTargetComponentId(
            component_id.to_owned(),
        ));
    }
    let by_id = index_installed(installed)?;
    let current = by_id
        .get(component_id)
        .copied()
        .ok_or_else(|| ComponentAdmissionError::DependencyMissing(component_id.to_owned()))?;

    Ok(ComponentTransactionPlan {
        component_id: current.component_id.clone(),
        target_revision: 0,
        target_package_digest: String::new(),
        kind: ComponentTransactionKind::Remove {
            previous_revision: current.revision,
            previous_package_digest: current.package_digest.clone(),
        },
    })
}

pub fn plan_component_data_removal(
    manifest: &PrimeComponentManifest,
    current: &InstalledComponent,
    explicit_migration_completed: bool,
) -> Result<ComponentDataDisposition, ComponentAdmissionError> {
    validate_component_manifest(manifest)?;
    if manifest.component_id != current.component_id || manifest.revision != current.revision {
        return Err(ComponentAdmissionError::RevisionNotAdvanced(
            current.component_id.clone(),
        ));
    }
    if manifest.package_digest != current.package_digest {
        return Err(ComponentAdmissionError::PackageDigestMismatch);
    }

    match manifest.persistent_data_policy {
        PersistentDataPolicy::RetainOnRemove => Ok(ComponentDataDisposition::Retain),
        PersistentDataPolicy::ExplicitMigration if explicit_migration_completed => {
            Ok(ComponentDataDisposition::Migrated)
        }
        PersistentDataPolicy::ExplicitMigration => Err(
            ComponentAdmissionError::PersistentDataMigrationRequired(current.component_id.clone()),
        ),
        PersistentDataPolicy::Ephemeral => Ok(ComponentDataDisposition::DiscardEphemeral),
    }
}

pub fn plan_component_rollback(
    current: &InstalledComponent,
    retained: &InstalledComponent,
) -> Result<ComponentTransactionPlan, ComponentAdmissionError> {
    for component in [current, retained] {
        if !valid_component_identifier(&component.component_id) {
            return Err(ComponentAdmissionError::InvalidInstalledComponentId(
                component.component_id.clone(),
            ));
        }
    }
    if current.component_id != retained.component_id
        || retained.revision == 0
        || retained.revision >= current.revision
    {
        return Err(ComponentAdmissionError::RevisionNotAdvanced(
            current.component_id.clone(),
        ));
    }
    for component in [current, retained] {
        if !canonical_component_digest(&component.package_digest) {
            return Err(ComponentAdmissionError::InvalidInstalledPackageDigest(
                component.component_id.clone(),
            ));
        }
    }

    Ok(ComponentTransactionPlan {
        component_id: current.component_id.clone(),
        target_revision: retained.revision,
        target_package_digest: retained.package_digest.clone(),
        kind: ComponentTransactionKind::Rollback {
            previous_revision: current.revision,
            previous_package_digest: current.package_digest.clone(),
        },
    })
}

pub fn verify_offline_component_package<S, G>(
    package_path: &Path,
    manifest: &PrimeComponentManifest,
    context: &ComponentAdmissionContext<'_>,
    signature_admitted: S,
    generation_compatible: G,
) -> Result<ComponentAdmissionOutcome, ComponentAdmissionError>
where
    S: FnOnce(&PrimeComponentManifest) -> bool,
    G: FnOnce(&str, &str) -> bool,
{
    validate_component_manifest(manifest)?;
    validate_package_path(package_path)?;
    let original_metadata = fs::metadata(package_path)?;

    let observed_digest = sha256_file(package_path)?;
    if observed_digest != manifest.package_digest {
        return Err(ComponentAdmissionError::PackageDigestMismatch);
    }

    if !signature_admitted(manifest) {
        return Err(ComponentAdmissionError::SignatureRejected);
    }

    if !manifest.architectures.is_empty()
        && !manifest
            .architectures
            .iter()
            .any(|arch| arch == context.host_arch)
    {
        return Err(ComponentAdmissionError::ArchitectureMismatch(
            context.host_arch.to_owned(),
        ));
    }

    if let Some(required) = &manifest.required_capability_interface {
        if required != context.capability_interface_version
            || required != CAPABILITY_INTERFACE_VERSION
        {
            return Err(ComponentAdmissionError::CapabilityInterfaceMismatch {
                required: required.clone(),
                host: context.capability_interface_version.to_owned(),
            });
        }
    }

    if let Some(required) = &manifest.required_application_profile_schema {
        if required != context.application_profile_schema
            || required != APPLICATIONS_PROJECTION_SCHEMA
        {
            return Err(ComponentAdmissionError::ApplicationProfileSchemaMismatch {
                required: required.clone(),
                host: context.application_profile_schema.to_owned(),
            });
        }
    }

    if let Some(required_generation) = &manifest.minimum_prime_generation {
        if !generation_compatible(required_generation, context.prime_generation) {
            return Err(ComponentAdmissionError::GenerationIncompatible);
        }
    }

    validate_dependencies(manifest, context.installed)?;

    // Callbacks can replace the pathname after the initial digest was checked.
    // Revalidate the path and the bytes before returning any admission outcome.
    validate_package_path(package_path)?;
    let current_metadata = fs::metadata(package_path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if current_metadata.dev() != original_metadata.dev()
            || current_metadata.ino() != original_metadata.ino()
        {
            return Err(ComponentAdmissionError::UnsafePackagePath);
        }
    }
    if current_metadata.len() != original_metadata.len()
        || sha256_file(package_path)? != observed_digest
    {
        return Err(ComponentAdmissionError::PackageDigestMismatch);
    }

    if !manifest.base_system_requirements.is_empty() {
        return Ok(ComponentAdmissionOutcome::GenerationHandoff {
            component_id: manifest.component_id.clone(),
            requirements: manifest.base_system_requirements.clone(),
        });
    }

    Ok(ComponentAdmissionOutcome::Ready(VerifiedComponentPackage {
        package_path: package_path.to_path_buf(),
        package_digest: observed_digest,
        component_id: manifest.component_id.clone(),
        revision: manifest.revision,
        version: manifest.version.clone(),
        publisher_id: manifest.publisher_id.clone(),
    }))
}

fn canonical_component_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn index_installed(
    installed: &[InstalledComponent],
) -> Result<BTreeMap<&str, &InstalledComponent>, ComponentAdmissionError> {
    let mut by_id = BTreeMap::new();
    for component in installed {
        if !valid_component_identifier(&component.component_id) {
            return Err(ComponentAdmissionError::InvalidInstalledComponentId(
                component.component_id.clone(),
            ));
        }
        if component.revision == 0 {
            return Err(ComponentAdmissionError::InvalidInstalledRevision(
                component.component_id.clone(),
            ));
        }
        if !canonical_component_digest(&component.package_digest) {
            return Err(ComponentAdmissionError::InvalidInstalledPackageDigest(
                component.component_id.clone(),
            ));
        }
        if by_id
            .insert(component.component_id.as_str(), component)
            .is_some()
        {
            return Err(ComponentAdmissionError::DuplicateInstalledComponent(
                component.component_id.clone(),
            ));
        }
    }
    Ok(by_id)
}

fn validate_dependencies(
    manifest: &PrimeComponentManifest,
    installed: &[InstalledComponent],
) -> Result<(), ComponentAdmissionError> {
    let by_id = index_installed(installed)?;

    for dependency in &manifest.dependencies {
        let Some(observed) = by_id.get(dependency.component_id.as_str()) else {
            return Err(ComponentAdmissionError::DependencyMissing(
                dependency.component_id.clone(),
            ));
        };
        if observed.revision < dependency.minimum_revision {
            return Err(ComponentAdmissionError::DependencyRevision {
                component_id: dependency.component_id.clone(),
                minimum_revision: dependency.minimum_revision,
            });
        }
        if let Some(required_digest) = &dependency.exact_package_digest {
            if observed.package_digest != *required_digest {
                return Err(ComponentAdmissionError::DependencyDigest(
                    dependency.component_id.clone(),
                ));
            }
        }
    }

    Ok(())
}

fn validate_package_path(path: &Path) -> Result<(), ComponentAdmissionError> {
    if !path.is_absolute() {
        return Err(ComponentAdmissionError::UnsafePackagePath);
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ComponentAdmissionError::UnsafePackagePath);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(ComponentAdmissionError::UnsafePackagePath);
        }
    }
    // A regular final file can still be reached through a symlinked parent.
    // Reject aliases and traversal so the admitted package path is canonical.
    if fs::canonicalize(path)? != path {
        return Err(ComponentAdmissionError::UnsafePackagePath);
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, ComponentAdmissionError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(7 + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prime_contracts::{
        ComponentClass, ComponentDependency, PersistentDataPolicy, PRIME_COMPONENT_SCHEMA,
    };
    use tempfile::tempdir;

    const DIGEST_A: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_B: &str =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn package_digest(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);
        format!("sha256:{digest:x}")
    }

    fn manifest(bytes: &[u8]) -> PrimeComponentManifest {
        PrimeComponentManifest {
            schema: PRIME_COMPONENT_SCHEMA.to_owned(),
            component_id: "origins.runtime".to_owned(),
            revision: 2,
            version: "2.0.0".to_owned(),
            class: ComponentClass::Runtime,
            package_digest: package_digest(bytes),
            publisher_id: "thetechguy.origins".to_owned(),
            publisher_key_id: DIGEST_B.to_owned(),
            signature: format!("ed25519:{}", "a".repeat(128)),
            architectures: vec!["x86_64".to_owned()],
            dependencies: vec![],
            minimum_prime_generation: Some("prime-p2".to_owned()),
            required_capability_interface: Some(CAPABILITY_INTERFACE_VERSION.to_owned()),
            required_application_profile_schema: Some(APPLICATIONS_PROJECTION_SCHEMA.to_owned()),
            persistent_data_policy: PersistentDataPolicy::RetainOnRemove,
            application_profiles: vec![],
            services: vec!["originsd".to_owned()],
            capabilities: vec!["origins.factory".to_owned()],
            base_system_requirements: vec![],
            limitations: vec![],
        }
    }

    fn context<'a>(installed: &'a [InstalledComponent]) -> ComponentAdmissionContext<'a> {
        ComponentAdmissionContext {
            host_arch: "x86_64",
            prime_generation: "prime-p2-current",
            capability_interface_version: CAPABILITY_INTERFACE_VERSION,
            application_profile_schema: APPLICATIONS_PROJECTION_SCHEMA,
            installed,
        }
    }

    fn write_package(bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("component.primepkg");
        fs::write(&path, bytes).unwrap();
        (dir, path)
    }

    #[test]
    fn exact_offline_package_can_be_admitted_without_network_or_store() {
        let bytes = b"origins-package-v2";
        let (_dir, path) = write_package(bytes);
        let manifest = manifest(bytes);
        let outcome = verify_offline_component_package(
            &path,
            &manifest,
            &context(&[]),
            |_| true,
            |required, current| required == "prime-p2" && current == "prime-p2-current",
        )
        .unwrap();

        let ComponentAdmissionOutcome::Ready(verified) = outcome else {
            panic!("package unexpectedly required generation handoff");
        };
        assert_eq!(verified.component_id, "origins.runtime");
        assert_eq!(verified.package_digest, manifest.package_digest);
        assert_eq!(verified.package_path, path);
    }

    #[test]
    fn tampered_package_or_untrusted_signature_fails_closed() {
        let bytes = b"origins-package-v2";
        let (_dir, path) = write_package(b"tampered");
        let manifest = manifest(bytes);
        assert!(matches!(
            verify_offline_component_package(
                &path,
                &manifest,
                &context(&[]),
                |_| true,
                |_, _| true,
            ),
            Err(ComponentAdmissionError::PackageDigestMismatch)
        ));

        let (_dir, path) = write_package(bytes);
        assert!(matches!(
            verify_offline_component_package(
                &path,
                &manifest,
                &context(&[]),
                |_| false,
                |_, _| true,
            ),
            Err(ComponentAdmissionError::SignatureRejected)
        ));
    }

    #[test]
    fn architecture_interface_and_generation_checks_are_explicit() {
        let bytes = b"origins-package-v2";
        let (_dir, path) = write_package(bytes);
        let mut item = manifest(bytes);
        item.architectures = vec!["aarch64".to_owned()];
        assert!(matches!(
            verify_offline_component_package(&path, &item, &context(&[]), |_| true, |_, _| true,),
            Err(ComponentAdmissionError::ArchitectureMismatch(_))
        ));

        let mut item = manifest(bytes);
        item.required_capability_interface = Some("2.0".to_owned());
        assert!(matches!(
            verify_offline_component_package(&path, &item, &context(&[]), |_| true, |_, _| true,),
            Err(ComponentAdmissionError::CapabilityInterfaceMismatch { .. })
        ));

        let item = manifest(bytes);
        assert!(matches!(
            verify_offline_component_package(&path, &item, &context(&[]), |_| true, |_, _| false,),
            Err(ComponentAdmissionError::GenerationIncompatible)
        ));
    }

    #[test]
    fn dependencies_require_revision_and_exact_digest_when_pinned() {
        let bytes = b"origins-package-v2";
        let (_dir, path) = write_package(bytes);
        let mut item = manifest(bytes);
        item.dependencies = vec![ComponentDependency {
            component_id: "runtime.python".to_owned(),
            minimum_revision: 3,
            exact_package_digest: Some(DIGEST_B.to_owned()),
        }];

        assert!(matches!(
            verify_offline_component_package(&path, &item, &context(&[]), |_| true, |_, _| true,),
            Err(ComponentAdmissionError::DependencyMissing(_))
        ));

        let too_old = [InstalledComponent {
            component_id: "runtime.python".to_owned(),
            revision: 2,
            package_digest: DIGEST_B.to_owned(),
        }];
        assert!(matches!(
            verify_offline_component_package(
                &path,
                &item,
                &context(&too_old),
                |_| true,
                |_, _| true,
            ),
            Err(ComponentAdmissionError::DependencyRevision { .. })
        ));

        let wrong_digest = [InstalledComponent {
            component_id: "runtime.python".to_owned(),
            revision: 3,
            package_digest:
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned(),
        }];
        assert!(matches!(
            verify_offline_component_package(
                &path,
                &item,
                &context(&wrong_digest),
                |_| true,
                |_, _| true,
            ),
            Err(ComponentAdmissionError::DependencyDigest(_))
        ));

        let exact = [InstalledComponent {
            component_id: "runtime.python".to_owned(),
            revision: 3,
            package_digest: DIGEST_B.to_owned(),
        }];
        assert!(matches!(
            verify_offline_component_package(
                &path,
                &item,
                &context(&exact),
                |_| true,
                |_, _| true,
            )
            .unwrap(),
            ComponentAdmissionOutcome::Ready(_)
        ));
    }

    #[test]
    fn duplicate_installed_component_identity_fails_closed() {
        let bytes = b"origins-package-v2";
        let (_dir, path) = write_package(bytes);
        let mut item = manifest(bytes);
        item.dependencies = vec![ComponentDependency {
            component_id: "runtime.python".to_owned(),
            minimum_revision: 3,
            exact_package_digest: None,
        }];
        let ambiguous = [
            InstalledComponent {
                component_id: "runtime.python".to_owned(),
                revision: 3,
                package_digest: DIGEST_A.to_owned(),
            },
            InstalledComponent {
                component_id: "runtime.python".to_owned(),
                revision: 4,
                package_digest: DIGEST_B.to_owned(),
            },
        ];

        assert!(matches!(
            verify_offline_component_package(
                &path,
                &item,
                &context(&ambiguous),
                |_| true,
                |_, _| true,
            ),
            Err(ComponentAdmissionError::DuplicateInstalledComponent(component_id))
                if component_id == "runtime.python"
        ));
    }

    #[test]
    fn base_system_requirement_routes_to_generation_handoff_not_live_patch() {
        let bytes = b"driver-package";
        let (_dir, path) = write_package(bytes);
        let mut item = manifest(bytes);
        item.base_system_requirements = vec!["kernel.module.example".to_owned()];

        let outcome =
            verify_offline_component_package(&path, &item, &context(&[]), |_| true, |_, _| true)
                .unwrap();

        assert_eq!(
            outcome,
            ComponentAdmissionOutcome::GenerationHandoff {
                component_id: "origins.runtime".to_owned(),
                requirements: vec!["kernel.module.example".to_owned()],
            }
        );
    }

    #[test]
    fn transaction_plans_reject_ambiguous_installed_inventory() {
        let package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/component.primepkg"),
            package_digest: DIGEST_B.to_owned(),
            component_id: "origins.runtime".to_owned(),
            revision: 3,
            version: "3.0.0".to_owned(),
            publisher_id: "thetechguy.origins".to_owned(),
        };
        for duplicated_id in ["origins.runtime", "runtime.python"] {
            let ambiguous = [
                InstalledComponent {
                    component_id: duplicated_id.to_owned(),
                    revision: 1,
                    package_digest: DIGEST_A.to_owned(),
                },
                InstalledComponent {
                    component_id: duplicated_id.to_owned(),
                    revision: 2,
                    package_digest: DIGEST_B.to_owned(),
                },
            ];
            assert!(matches!(
                plan_component_transaction(&package, &ambiguous),
                Err(ComponentAdmissionError::DuplicateInstalledComponent(id))
                    if id == duplicated_id
            ));
            assert!(matches!(
                plan_component_removal(duplicated_id, &ambiguous),
                Err(ComponentAdmissionError::DuplicateInstalledComponent(id))
                    if id == duplicated_id
            ));
        }
    }

    #[test]
    fn transaction_plan_rejects_noncanonical_target_package_digests() {
        let mut package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/component.primepkg"),
            package_digest: DIGEST_B.to_owned(),
            component_id: "origins.runtime".to_owned(),
            revision: 3,
            version: "3.0.0".to_owned(),
            publisher_id: "thetechguy.origins".to_owned(),
        };
        let current = [InstalledComponent {
            component_id: package.component_id.clone(),
            revision: 2,
            package_digest: DIGEST_A.to_owned(),
        }];
        for invalid in [
            "",
            "latest",
            "sha256:NOT-CANONICAL",
            "sha256:abc",
            "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ] {
            package.package_digest = invalid.to_owned();
            for installed in [&[][..], &current[..]] {
                assert!(matches!(
                    plan_component_transaction(&package, installed),
                    Err(ComponentAdmissionError::InvalidTargetPackageDigest(id)) if id == "origins.runtime"
                ));
            }
        }
    }

    #[test]
    fn transaction_plan_rejects_forged_public_package_metadata() {
        let mut package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/component.primepkg"),
            package_digest: DIGEST_B.to_owned(),
            component_id: "origins.runtime".to_owned(),
            revision: 3,
            version: "3.0.0".to_owned(),
            publisher_id: "thetechguy.origins".to_owned(),
        };
        for invalid in ["", "TheTechGuy.origins", "thetechguy..origins"] {
            package.publisher_id = invalid.to_owned();
            assert!(matches!(
                plan_component_transaction(&package, &[]),
                Err(ComponentAdmissionError::Manifest(
                    ComponentManifestError::PublisherId
                ))
            ));
        }
        package.publisher_id = "thetechguy.origins".to_owned();
        for invalid in [
            "",
            " 3.0.0",
            "3.0.0\nspoof",
            "3.0\u{202E}spoof",
            "3.0\u{200B}spoof",
            "3.0\u{034F}spoof",
        ] {
            package.version = invalid.to_owned();
            assert!(matches!(
                plan_component_transaction(&package, &[]),
                Err(ComponentAdmissionError::Manifest(
                    ComponentManifestError::Version
                ))
            ));
        }
    }

    #[test]
    fn transaction_plan_rejects_zero_revision_even_without_installed_component() {
        let package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/component.primepkg"),
            package_digest: DIGEST_B.to_owned(),
            component_id: "origins.runtime".to_owned(),
            revision: 0,
            version: "0.0.0".to_owned(),
            publisher_id: "thetechguy.origins".to_owned(),
        };
        assert!(matches!(
            plan_component_transaction(&package, &[]),
            Err(ComponentAdmissionError::RevisionNotAdvanced(id)) if id == "origins.runtime"
        ));
    }

    #[test]
    fn installed_inventory_rejects_reserved_zero_revision_for_all_plans() {
        let package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/component.primepkg"),
            package_digest: DIGEST_B.to_owned(),
            component_id: "origins.runtime".to_owned(),
            revision: 3,
            version: "3.0.0".to_owned(),
            publisher_id: "thetechguy.origins".to_owned(),
        };
        for component_id in ["origins.runtime", "runtime.python"] {
            let installed = [InstalledComponent {
                component_id: component_id.to_owned(),
                revision: 0,
                package_digest: DIGEST_A.to_owned(),
            }];
            assert!(matches!(
                plan_component_transaction(&package, &installed),
                Err(ComponentAdmissionError::InvalidInstalledRevision(id)) if id == component_id
            ));
            assert!(matches!(
                plan_component_removal(component_id, &installed),
                Err(ComponentAdmissionError::InvalidInstalledRevision(id)) if id == component_id
            ));
        }
    }

    #[test]
    fn transaction_plan_distinguishes_install_from_rollback_capable_update() {
        let bytes = b"origins-package-v2";
        let (_dir, path) = write_package(bytes);
        let manifest = manifest(bytes);
        let ComponentAdmissionOutcome::Ready(package) = verify_offline_component_package(
            &path,
            &manifest,
            &context(&[]),
            |_| true,
            |_, _| true,
        )
        .unwrap() else {
            panic!("expected admitted package");
        };

        let install = plan_component_transaction(&package, &[]).unwrap();
        assert_eq!(install.kind, ComponentTransactionKind::Install);

        let current = [InstalledComponent {
            component_id: package.component_id.clone(),
            revision: 1,
            package_digest: DIGEST_B.to_owned(),
        }];
        let update = plan_component_transaction(&package, &current).unwrap();
        assert_eq!(
            update.kind,
            ComponentTransactionKind::Update {
                previous_revision: 1,
                previous_package_digest: DIGEST_B.to_owned(),
            }
        );
    }

    #[test]
    fn transaction_plan_rejects_same_revision_and_downgrade() {
        let package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/component.primepkg"),
            package_digest: DIGEST_B.to_owned(),
            component_id: "origins.runtime".to_owned(),
            revision: 2,
            version: "2.0.0".to_owned(),
            publisher_id: "thetechguy.origins".to_owned(),
        };
        for revision in [2, 3] {
            let current = [InstalledComponent {
                component_id: package.component_id.clone(),
                revision,
                package_digest: DIGEST_B.to_owned(),
            }];
            assert!(matches!(
                plan_component_transaction(&package, &current),
                Err(ComponentAdmissionError::RevisionNotAdvanced(_))
            ));
        }
    }

    #[test]
    fn removal_and_rollback_plans_preserve_previous_identity() {
        let current = InstalledComponent {
            component_id: "origins.runtime".to_owned(),
            revision: 3,
            package_digest: DIGEST_B.to_owned(),
        };
        let retained = InstalledComponent {
            component_id: current.component_id.clone(),
            revision: 2,
            package_digest: DIGEST_A.to_owned(),
        };

        let removal =
            plan_component_removal(&current.component_id, std::slice::from_ref(&current)).unwrap();
        assert_eq!(removal.target_revision, 0);
        assert_eq!(
            removal.kind,
            ComponentTransactionKind::Remove {
                previous_revision: 3,
                previous_package_digest: DIGEST_B.to_owned(),
            }
        );

        let rollback = plan_component_rollback(&current, &retained).unwrap();
        assert_eq!(rollback.target_revision, 2);
        assert_eq!(rollback.target_package_digest, DIGEST_A);
        assert_eq!(
            rollback.kind,
            ComponentTransactionKind::Rollback {
                previous_revision: 3,
                previous_package_digest: DIGEST_B.to_owned(),
            }
        );
    }

    #[test]
    fn rollback_rejects_cross_component_or_non_previous_target() {
        let current = InstalledComponent {
            component_id: "origins.runtime".to_owned(),
            revision: 3,
            package_digest: DIGEST_B.to_owned(),
        };
        for retained in [
            InstalledComponent {
                component_id: current.component_id.clone(),
                revision: 0,
                package_digest: DIGEST_A.to_owned(),
            },
            InstalledComponent {
                component_id: "other.runtime".to_owned(),
                revision: 2,
                package_digest: DIGEST_A.to_owned(),
            },
            InstalledComponent {
                component_id: current.component_id.clone(),
                revision: 3,
                package_digest: DIGEST_A.to_owned(),
            },
            InstalledComponent {
                component_id: current.component_id.clone(),
                revision: 4,
                package_digest: DIGEST_A.to_owned(),
            },
        ] {
            assert!(plan_component_rollback(&current, &retained).is_err());
        }
        assert!(plan_component_removal("missing.runtime", std::slice::from_ref(&current)).is_err());
    }

    #[test]
    fn removal_rejects_noncanonical_request_identity_before_inventory_lookup() {
        let installed = [InstalledComponent {
            component_id: "origins.runtime".to_owned(),
            revision: 2,
            package_digest: DIGEST_A.to_owned(),
        }];
        for id in [
            "Origins.runtime",
            "origins..runtime",
            "origins/runtime",
            " origins.runtime",
        ] {
            for inventory in [&[][..], &installed[..]] {
                assert!(matches!(
                    plan_component_removal(id, inventory),
                    Err(ComponentAdmissionError::InvalidTargetComponentId(actual)) if actual == id
                ));
            }
        }
    }

    #[test]
    fn installed_inventory_rejects_noncanonical_component_id() {
        for id in [
            "Origins.runtime",
            "origins..runtime",
            "origins/runtime",
            " origins.runtime",
        ] {
            let installed = [InstalledComponent {
                component_id: id.to_owned(),
                revision: 2,
                package_digest: DIGEST_A.to_owned(),
            }];
            assert!(
                plan_component_removal(id, &installed).is_err(),
                "noncanonical installed identity was accepted: {id}"
            );
        }
    }

    #[test]
    fn transaction_and_rollback_reject_noncanonical_component_id() {
        let package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/component.primepkg"),
            package_digest: DIGEST_B.to_owned(),
            component_id: "Origins.runtime".to_owned(),
            revision: 3,
            version: "3.0.0".to_owned(),
            publisher_id: "thetechguy.origins".to_owned(),
        };
        assert!(matches!(
            plan_component_transaction(&package, &[]),
            Err(ComponentAdmissionError::InvalidTargetComponentId(_))
        ));
        let current = InstalledComponent {
            component_id: package.component_id,
            revision: 3,
            package_digest: DIGEST_B.to_owned(),
        };
        let retained = InstalledComponent {
            revision: 2,
            package_digest: DIGEST_A.to_owned(),
            ..current.clone()
        };
        assert!(matches!(
            plan_component_rollback(&current, &retained),
            Err(ComponentAdmissionError::InvalidInstalledComponentId(_))
        ));
    }

    #[test]
    fn transaction_and_removal_reject_untrusted_installed_digest_labels() {
        let package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/component.primepkg"),
            package_digest: DIGEST_B.to_owned(),
            component_id: "origins.runtime".to_owned(),
            revision: 3,
            version: "3.0.0".to_owned(),
            publisher_id: "thetechguy.origins".to_owned(),
        };
        for invalid in [
            "",
            "sha256:abc",
            "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "sha256:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
        ] {
            let installed = [InstalledComponent {
                component_id: package.component_id.clone(),
                revision: 2,
                package_digest: invalid.to_owned(),
            }];
            assert!(matches!(
                plan_component_transaction(&package, &installed),
                Err(ComponentAdmissionError::InvalidInstalledPackageDigest(_))
            ));
            assert!(matches!(
                plan_component_removal(&package.component_id, &installed),
                Err(ComponentAdmissionError::InvalidInstalledPackageDigest(_))
            ));
        }
    }

    #[test]
    fn rollback_rejects_untrusted_installed_digest_labels() {
        let current = InstalledComponent {
            component_id: "origins.runtime".to_owned(),
            revision: 3,
            package_digest: DIGEST_B.to_owned(),
        };
        let retained = InstalledComponent {
            component_id: current.component_id.clone(),
            revision: 2,
            package_digest: DIGEST_A.to_owned(),
        };
        assert!(plan_component_rollback(&current, &retained).is_ok());
        for invalid in [
            "",
            "sha256:abc",
            "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "sha256:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
        ] {
            let bad_retained = InstalledComponent {
                package_digest: invalid.to_owned(),
                ..retained.clone()
            };
            assert!(matches!(
                plan_component_rollback(&current, &bad_retained),
                Err(ComponentAdmissionError::InvalidInstalledPackageDigest(_))
            ));
            let bad_current = InstalledComponent {
                package_digest: invalid.to_owned(),
                ..current.clone()
            };
            assert!(matches!(
                plan_component_rollback(&bad_current, &retained),
                Err(ComponentAdmissionError::InvalidInstalledPackageDigest(_))
            ));
        }
    }

    #[test]
    fn transaction_failure_and_recovery_states_never_claim_success() {
        for state in [
            ComponentTransactionState::Planned,
            ComponentTransactionState::Applying,
            ComponentTransactionState::Failed {
                reason: "write failed".to_owned(),
            },
            ComponentTransactionState::RecoveryRequired {
                reason: "previous package must be restored".to_owned(),
            },
        ] {
            assert!(!state.is_success());
        }
        assert!(ComponentTransactionState::Applied.is_success());
        assert!(ComponentTransactionState::RolledBack.is_success());
    }

    #[test]
    fn transaction_state_transitions_require_application_and_recovery() {
        use ComponentTransactionState as State;

        for (from, to) in [
            (State::Planned, State::Applied),
            (State::Planned, State::RolledBack),
            (
                State::Planned,
                State::RecoveryRequired {
                    reason: "skip apply".into(),
                },
            ),
            (
                State::Failed {
                    reason: "failed".into(),
                },
                State::Applied,
            ),
            (
                State::Failed {
                    reason: "failed".into(),
                },
                State::RolledBack,
            ),
            (State::Applied, State::Applying),
            (State::RolledBack, State::Applied),
            (State::Applying, State::RolledBack),
        ] {
            assert_eq!(from.advance(to), Err(ComponentTransactionTransitionError));
        }

        let applied = State::Planned
            .advance(State::Applying)
            .unwrap()
            .advance(State::Applied)
            .unwrap();
        assert!(applied.is_success());

        let recovered = State::Planned
            .advance(State::Applying)
            .unwrap()
            .advance(State::Failed {
                reason: "write interrupted".into(),
            })
            .unwrap()
            .advance(State::RecoveryRequired {
                reason: "restore prior package".into(),
            })
            .unwrap()
            .advance(State::RolledBack)
            .unwrap();
        assert!(recovered.is_success());
    }

    #[test]
    fn transaction_failure_states_require_meaningful_recovery_reasons() {
        use ComponentTransactionState as State;

        for invalid in [
            "",
            "   ",
            "\n",
            "failure\nforged",
            "failure\u{2028}forged",
            "failure\u{2029}forged",
            "failure\u{202E}forged",
            "failure\u{2066}forged",
            "failure\u{200F}forged",
            "failure\u{061C}forged",
            "failure\u{200B}forged",
            "failure\u{2060}forged",
            "failure\u{FE0F}forged",
            " disk failure",
            "disk failure ",
            "\u{00A0}disk failure",
            "disk failure\u{00A0}",
        ] {
            for next in [
                State::Failed {
                    reason: invalid.to_owned(),
                },
                State::RecoveryRequired {
                    reason: invalid.to_owned(),
                },
            ] {
                assert_eq!(
                    State::Applying.advance(next),
                    Err(ComponentTransactionTransitionError),
                    "invalid failure/recovery reason was accepted: {invalid:?}"
                );
            }
        }

        for invalid in ["", "   ", "\n"] {
            assert_eq!(
                (State::Failed {
                    reason: invalid.into()
                })
                .advance(State::RecoveryRequired {
                    reason: "restore previous package".into()
                }),
                Err(ComponentTransactionTransitionError)
            );
            assert_eq!(
                (State::RecoveryRequired {
                    reason: invalid.into()
                })
                .advance(State::RolledBack),
                Err(ComponentTransactionTransitionError)
            );
        }

        assert!(State::Applying
            .advance(State::Failed {
                reason: "disk write interrupted".into()
            })
            .is_ok());
    }

    #[test]
    fn transaction_reasons_reject_noncanonical_internal_whitespace() {
        use ComponentTransactionState as State;
        for separator in ['\u{2009}', '\u{205F}', '\u{3000}', '\u{00A0}'] {
            let reason = format!("disk{separator}failure");
            assert_eq!(
                State::Applying.advance(State::Failed {
                    reason: reason.clone(),
                }),
                Err(ComponentTransactionTransitionError),
                "noncanonical failure separator accepted: {reason:?}"
            );
            assert_eq!(
                (State::Failed {
                    reason: reason.clone(),
                })
                .advance(State::RecoveryRequired {
                    reason: "restore previous package".into(),
                }),
                Err(ComponentTransactionTransitionError),
                "noncanonical prior-state separator accepted: {reason:?}"
            );
        }
    }

    #[test]
    fn transaction_reasons_reject_invisible_hangul_fillers() {
        use ComponentTransactionState as State;
        for filler in ['\u{115F}', '\u{1160}', '\u{3164}', '\u{FFA0}'] {
            for reason in [filler.to_string(), format!("disk{filler}failure")] {
                for next in [
                    State::Failed {
                        reason: reason.clone(),
                    },
                    State::RecoveryRequired {
                        reason: reason.clone(),
                    },
                ] {
                    assert_eq!(
                        State::Applying.advance(next),
                        Err(ComponentTransactionTransitionError),
                        "invisible Hangul filler accepted: {reason:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn transaction_reasons_reject_invisible_mongolian_and_tag_formatting() {
        use ComponentTransactionState as State;
        for mark in [
            '\u{180B}',
            '\u{180C}',
            '\u{180D}',
            '\u{180F}',
            '\u{E0001}',
            '\u{E0061}',
            '\u{E007F}',
        ] {
            for reason in [mark.to_string(), format!("disk{mark}failure")] {
                for next in [
                    State::Failed {
                        reason: reason.clone(),
                    },
                    State::RecoveryRequired {
                        reason: reason.clone(),
                    },
                ] {
                    assert_eq!(
                        State::Applying.advance(next),
                        Err(ComponentTransactionTransitionError),
                        "invisible formatting mark accepted: {reason:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn removal_data_policy_is_explicit_and_fail_closed() {
        let bytes = b"origins-package-v2";
        let mut manifest = manifest(bytes);
        let current = InstalledComponent {
            component_id: manifest.component_id.clone(),
            revision: manifest.revision,
            package_digest: manifest.package_digest.clone(),
        };

        let mut invalid_manifest = manifest.clone();
        invalid_manifest.schema = "prime.component.v0".to_owned();
        assert!(matches!(
            plan_component_data_removal(&invalid_manifest, &current, false),
            Err(ComponentAdmissionError::Manifest(
                ComponentManifestError::Schema
            ))
        ));

        manifest.persistent_data_policy = PersistentDataPolicy::RetainOnRemove;
        assert_eq!(
            plan_component_data_removal(&manifest, &current, false).unwrap(),
            ComponentDataDisposition::Retain
        );

        manifest.persistent_data_policy = PersistentDataPolicy::ExplicitMigration;
        assert!(matches!(
            plan_component_data_removal(&manifest, &current, false),
            Err(ComponentAdmissionError::PersistentDataMigrationRequired(_))
        ));
        assert_eq!(
            plan_component_data_removal(&manifest, &current, true).unwrap(),
            ComponentDataDisposition::Migrated
        );

        manifest.persistent_data_policy = PersistentDataPolicy::Ephemeral;
        assert_eq!(
            plan_component_data_removal(&manifest, &current, false).unwrap(),
            ComponentDataDisposition::DiscardEphemeral
        );

        let wrong = InstalledComponent {
            revision: current.revision + 1,
            ..current.clone()
        };
        assert!(plan_component_data_removal(&manifest, &wrong, false).is_err());

        let wrong_digest = InstalledComponent {
            package_digest: DIGEST_B.to_owned(),
            ..current
        };
        assert!(matches!(
            plan_component_data_removal(&manifest, &wrong_digest, false),
            Err(ComponentAdmissionError::PackageDigestMismatch)
        ));
    }

    #[test]
    fn relative_or_symlink_package_path_is_rejected() {
        let bytes = b"origins-package-v2";
        let (dir, path) = write_package(bytes);
        let manifest = manifest(bytes);

        assert!(matches!(
            verify_offline_component_package(
                Path::new("relative.primepkg"),
                &manifest,
                &context(&[]),
                |_| true,
                |_, _| true,
            ),
            Err(ComponentAdmissionError::UnsafePackagePath)
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link = dir.path().join("component-link.primepkg");
            symlink(&path, &link).unwrap();
            assert!(matches!(
                verify_offline_component_package(
                    &link,
                    &manifest,
                    &context(&[]),
                    |_| true,
                    |_, _| true,
                ),
                Err(ComponentAdmissionError::UnsafePackagePath)
            ));
        }
    }
    #[cfg(unix)]
    #[test]
    fn offline_package_rejects_hardlinked_external_alias() {
        let bytes = b"origins-package-v2";
        let (dir, path) = write_package(bytes);
        let manifest = manifest(bytes);
        let external = dir.path().join("external.primepkg");
        fs::hard_link(&path, &external).unwrap();
        assert!(matches!(
            verify_offline_component_package(
                &path,
                &manifest,
                &context(&[]),
                |_| true,
                |_, _| true,
            ),
            Err(ComponentAdmissionError::UnsafePackagePath)
        ));
        assert_eq!(fs::read(&external).unwrap(), bytes);
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_rejects_hardlink_alias_as_requested_path() {
        let bytes = b"origins-package-v2";
        let (dir, original) = write_package(bytes);
        let alias = dir.path().join("alias.primepkg");
        fs::hard_link(&original, &alias).unwrap();
        let manifest = manifest(bytes);
        assert!(matches!(
            verify_offline_component_package(
                &alias,
                &manifest,
                &context(&[]),
                |_| true,
                |_, _| true,
            ),
            Err(ComponentAdmissionError::UnsafePackagePath)
        ));
        assert_eq!(fs::read(&original).unwrap(), bytes);
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_rejects_path_replaced_after_digest_verification() {
        let bytes = b"origins-package-v2";
        let (_dir, path) = write_package(bytes);
        let manifest = manifest(bytes);
        let original = path.with_extension("original");
        let result = verify_offline_component_package(
            &path,
            &manifest,
            &context(&[]),
            |_| {
                fs::rename(&path, &original).unwrap();
                fs::write(&path, b"unverified-replacement").unwrap();
                true
            },
            |_, _| true,
        );
        assert!(matches!(
            result,
            Err(ComponentAdmissionError::UnsafePackagePath)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_rejects_same_digest_inode_replacement_during_signature() {
        let bytes = b"origins-package-v2";
        let (_dir, path) = write_package(bytes);
        let manifest = manifest(bytes);
        let original = path.with_extension("original");
        let result = verify_offline_component_package(
            &path,
            &manifest,
            &context(&[]),
            |_| {
                fs::rename(&path, &original).unwrap();
                fs::write(&path, bytes).unwrap();
                true
            },
            |_, _| true,
        );
        assert!(matches!(
            result,
            Err(ComponentAdmissionError::UnsafePackagePath)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn package_path_rejects_symlinked_parent_and_dotdot_alias() {
        use std::os::unix::fs::symlink;

        let bytes = b"origins-package-v2";
        let (dir, path) = write_package(bytes);
        let manifest = manifest(bytes);
        let alias = dir.path().join("alias");
        symlink(dir.path(), &alias).unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();

        for candidate in [
            alias.join("component.primepkg"),
            dir.path()
                .join("nested")
                .join("..")
                .join("component.primepkg"),
        ] {
            assert!(matches!(
                verify_offline_component_package(
                    &candidate,
                    &manifest,
                    &context(&[]),
                    |_| true,
                    |_, _| true,
                ),
                Err(ComponentAdmissionError::UnsafePackagePath)
            ));
        }
        assert!(matches!(
            verify_offline_component_package(
                &path,
                &manifest,
                &context(&[]),
                |_| true,
                |_, _| true
            ),
            Ok(ComponentAdmissionOutcome::Ready(_))
        ));
    }

    #[test]
    fn application_profile_schema_compatibility_fails_closed() {
        let bytes = b"origins-package-v2";
        let (_dir, path) = write_package(bytes);
        let manifest = manifest(bytes);
        let installed = [];
        let mut ctx = context(&installed);
        ctx.application_profile_schema = "prime.applications.v0";
        let result = verify_offline_component_package(
            &path,
            &manifest,
            &ctx,
            |_| true,
            |required, current| current.starts_with(required),
        );
        assert!(matches!(
            result,
            Err(ComponentAdmissionError::ApplicationProfileSchemaMismatch { .. })
        ));
    }
}
