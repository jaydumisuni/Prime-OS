use prime_contracts::{
    validate_component_manifest, ComponentManifestError, PrimeComponentManifest,
    CAPABILITY_INTERFACE_VERSION,
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
    #[error("component publisher signature was not admitted")]
    SignatureRejected,
    #[error("component does not support host architecture {0}")]
    ArchitectureMismatch(String),
    #[error(
        "component requires unsupported capability interface {required}; host provides {host}"
    )]
    CapabilityInterfaceMismatch { required: String, host: String },
    #[error("component is incompatible with current Prime generation")]
    GenerationIncompatible,
    #[error("required component dependency {0} is missing")]
    DependencyMissing(String),
    #[error("component dependency {component_id} revision is below required {minimum_revision}")]
    DependencyRevision {
        component_id: String,
        minimum_revision: u64,
    },
    #[error("component dependency {0} package digest does not match exact pin")]
    DependencyDigest(String),
    #[error("component transaction for {0} does not advance the installed revision")]
    RevisionNotAdvanced(String),
}

pub fn plan_component_transaction(
    package: &VerifiedComponentPackage,
    installed: &[InstalledComponent],
) -> Result<ComponentTransactionPlan, ComponentAdmissionError> {
    let current = installed
        .iter()
        .find(|component| component.component_id == package.component_id);

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
    let current = installed
        .iter()
        .find(|component| component.component_id == component_id)
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

pub fn plan_component_rollback(
    current: &InstalledComponent,
    retained: &InstalledComponent,
) -> Result<ComponentTransactionPlan, ComponentAdmissionError> {
    if current.component_id != retained.component_id || retained.revision >= current.revision {
        return Err(ComponentAdmissionError::RevisionNotAdvanced(
            current.component_id.clone(),
        ));
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

    if let Some(required_generation) = &manifest.minimum_prime_generation {
        if !generation_compatible(required_generation, context.prime_generation) {
            return Err(ComponentAdmissionError::GenerationIncompatible);
        }
    }

    validate_dependencies(manifest, context.installed)?;

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

fn validate_dependencies(
    manifest: &PrimeComponentManifest,
    installed: &[InstalledComponent],
) -> Result<(), ComponentAdmissionError> {
    let by_id: BTreeMap<&str, &InstalledComponent> = installed
        .iter()
        .map(|component| (component.component_id.as_str(), component))
        .collect();

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
            signature: "ed25519:fixture-signature".to_owned(),
            architectures: vec!["x86_64".to_owned()],
            dependencies: vec![],
            minimum_prime_generation: Some("prime-p2".to_owned()),
            required_capability_interface: Some(CAPABILITY_INTERFACE_VERSION.to_owned()),
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
}
