use prime_contracts::{
    valid_sha256_label, WindowsComponentManifest, WindowsComponentReference,
    WindowsComponentReferenceError, WindowsInstallerKind, WINDOWS_COMPONENT_SCHEMA,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WindowsComponentRegistryError {
    #[error("WINDOWS_COMPONENT_REFERENCE_INVALID: {0}")]
    Reference(#[from] WindowsComponentReferenceError),
    #[error("WINDOWS_COMPONENT_REGISTRY_IO: {0}")]
    Io(#[from] io::Error),
    #[error("WINDOWS_COMPONENT_REGISTRY_JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("WINDOWS_COMPONENT_SCHEMA_INVALID: {0}")]
    Schema(String),
    #[error("WINDOWS_COMPONENT_MANIFEST_INVALID: {0}")]
    Manifest(&'static str),
    #[error("WINDOWS_COMPONENT_DIGEST_MISMATCH: {component_id}@{revision}")]
    DigestMismatch { component_id: String, revision: u64 },
    #[error("WINDOWS_COMPONENT_REFERENCE_DIGEST_MISMATCH: {component_id}@{revision}")]
    ReferenceDigestMismatch { component_id: String, revision: u64 },
    #[error("WINDOWS_COMPONENT_IDENTITY_MISMATCH")]
    IdentityMismatch,
    #[error("WINDOWS_COMPONENT_REVISION_MISMATCH")]
    RevisionMismatch,
    #[error("WINDOWS_COMPONENT_REVISION_EXISTS")]
    RevisionExists,
    #[error("WINDOWS_COMPONENT_UNAVAILABLE: {component_id}@{revision}")]
    ComponentUnavailable { component_id: String, revision: u64 },
    #[error("WINDOWS_COMPONENT_ARCH_UNSUPPORTED: {component_id} does not support {arch}")]
    ArchitectureUnsupported { component_id: String, arch: String },
    #[error("WINDOWS_COMPONENT_DEPENDENCY_CYCLE: {component_id}@{revision}")]
    DependencyCycle { component_id: String, revision: u64 },
    #[error("WINDOWS_COMPONENT_DUPLICATE_REFERENCE: {0}")]
    DuplicateReference(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedWindowsComponentPlan {
    pub ordered: Vec<WindowsComponentManifest>,
}

pub fn seal_component(
    mut manifest: WindowsComponentManifest,
) -> Result<WindowsComponentManifest, WindowsComponentRegistryError> {
    validate_manifest_structure(&manifest, false)?;
    manifest.digest.clear();
    manifest.digest = component_digest(&manifest)?;
    Ok(manifest)
}

pub fn verify_component(
    manifest: &WindowsComponentManifest,
) -> Result<(), WindowsComponentRegistryError> {
    validate_manifest_structure(manifest, true)?;
    let expected = component_digest(manifest)?;
    if manifest.digest != expected {
        return Err(WindowsComponentRegistryError::DigestMismatch {
            component_id: manifest.component_id.clone(),
            revision: manifest.revision,
        });
    }
    Ok(())
}

pub fn store_component_revision(
    component_dir: &Path,
    manifest: &WindowsComponentManifest,
) -> Result<(), WindowsComponentRegistryError> {
    verify_component(manifest)?;
    let path = component_revision_path(component_dir, &manifest.component_id, manifest.revision);
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "component revision has no parent",
        )
    })?;
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    let temp = parent.join(format!(".component.{}.tmp", Uuid::now_v7()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    file.write_all(&serde_json::to_vec_pretty(manifest)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    match fs::hard_link(&temp, &path) {
        Ok(()) => {
            fs::remove_file(&temp)?;
            File::open(parent)?.sync_all()?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            fs::remove_file(&temp)?;
            Err(WindowsComponentRegistryError::RevisionExists)
        }
        Err(error) => {
            let _ = fs::remove_file(&temp);
            Err(error.into())
        }
    }
}

pub fn load_component_revision(
    component_dir: &Path,
    component_id: &str,
    revision: u64,
) -> Result<WindowsComponentManifest, WindowsComponentRegistryError> {
    let path = component_revision_path(component_dir, component_id, revision);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(WindowsComponentRegistryError::ComponentUnavailable {
                component_id: component_id.to_owned(),
                revision,
            });
        }
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(WindowsComponentRegistryError::Manifest(
            "component revision is not a regular non-symlink file",
        ));
    }
    let manifest: WindowsComponentManifest = serde_json::from_slice(&fs::read(&path)?)?;
    if manifest.component_id != component_id {
        return Err(WindowsComponentRegistryError::IdentityMismatch);
    }
    if manifest.revision != revision {
        return Err(WindowsComponentRegistryError::RevisionMismatch);
    }
    verify_component(&manifest)?;
    Ok(manifest)
}

pub fn resolve_component_plan(
    component_dir: &Path,
    dependencies: &[String],
    workload_arch: &str,
) -> Result<ResolvedWindowsComponentPlan, WindowsComponentRegistryError> {
    let mut roots = Vec::with_capacity(dependencies.len());
    let mut seen_roots = HashSet::new();
    for raw in dependencies {
        let reference = WindowsComponentReference::parse(raw)?;
        let canonical = reference.to_string();
        if !seen_roots.insert(canonical.clone()) {
            return Err(WindowsComponentRegistryError::DuplicateReference(canonical));
        }
        roots.push(reference);
    }

    let mut resolver = Resolver {
        component_dir,
        workload_arch,
        visit: HashMap::new(),
        ordered: Vec::new(),
    };
    for reference in &roots {
        resolver.visit(reference)?;
    }
    Ok(ResolvedWindowsComponentPlan {
        ordered: resolver.ordered,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Complete,
}

struct Resolver<'a> {
    component_dir: &'a Path,
    workload_arch: &'a str,
    visit: HashMap<(String, u64), VisitState>,
    ordered: Vec<WindowsComponentManifest>,
}

impl Resolver<'_> {
    fn visit(
        &mut self,
        reference: &WindowsComponentReference,
    ) -> Result<(), WindowsComponentRegistryError> {
        let key = (reference.component_id.clone(), reference.revision);
        match self.visit.get(&key) {
            Some(VisitState::Visiting) => {
                return Err(WindowsComponentRegistryError::DependencyCycle {
                    component_id: reference.component_id.clone(),
                    revision: reference.revision,
                });
            }
            Some(VisitState::Complete) => {
                let actual = self
                    .ordered
                    .iter()
                    .find(|item| {
                        item.component_id == reference.component_id
                            && item.revision == reference.revision
                    })
                    .expect("complete resolver state has an ordered manifest");
                if actual.digest != reference.digest {
                    return Err(WindowsComponentRegistryError::ReferenceDigestMismatch {
                        component_id: reference.component_id.clone(),
                        revision: reference.revision,
                    });
                }
                return Ok(());
            }
            None => {}
        }

        let manifest = load_component_revision(
            self.component_dir,
            &reference.component_id,
            reference.revision,
        )?;
        self.visit.insert(key.clone(), VisitState::Visiting);

        for dependency in &manifest.depends_on {
            let child = WindowsComponentReference::parse(dependency)?;
            self.visit(&child)?;
        }

        if manifest.digest != reference.digest {
            return Err(WindowsComponentRegistryError::ReferenceDigestMismatch {
                component_id: reference.component_id.clone(),
                revision: reference.revision,
            });
        }
        if !manifest
            .workload_arches
            .iter()
            .any(|arch| arch == "any" || arch == self.workload_arch)
        {
            return Err(WindowsComponentRegistryError::ArchitectureUnsupported {
                component_id: manifest.component_id.clone(),
                arch: self.workload_arch.to_owned(),
            });
        }

        self.visit.insert(key, VisitState::Complete);
        self.ordered.push(manifest);
        Ok(())
    }
}

fn validate_manifest_structure(
    manifest: &WindowsComponentManifest,
    require_digest: bool,
) -> Result<(), WindowsComponentRegistryError> {
    if manifest.schema != WINDOWS_COMPONENT_SCHEMA {
        return Err(WindowsComponentRegistryError::Schema(
            manifest.schema.clone(),
        ));
    }
    if manifest.revision == 0 {
        return Err(WindowsComponentRegistryError::Manifest(
            "component revision must be positive",
        ));
    }
    let dummy_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let identity_probe = format!(
        "windows-component:{}@{}#{}",
        manifest.component_id, manifest.revision, dummy_digest
    );
    WindowsComponentReference::parse(&identity_probe)?;
    if require_digest && !valid_sha256_label(&manifest.digest) {
        return Err(WindowsComponentRegistryError::Manifest(
            "component digest must be a canonical SHA-256 label",
        ));
    }
    if manifest.workload_arches.is_empty()
        || manifest
            .workload_arches
            .iter()
            .any(|arch| !matches!(arch.as_str(), "x86" | "x86_64" | "any"))
    {
        return Err(WindowsComponentRegistryError::Manifest(
            "component workload_arches are invalid",
        ));
    }
    let mut dependencies = HashSet::new();
    for raw in &manifest.depends_on {
        let parsed = WindowsComponentReference::parse(raw)?;
        if !dependencies.insert(parsed.to_string()) {
            return Err(WindowsComponentRegistryError::DuplicateReference(
                parsed.to_string(),
            ));
        }
    }
    if manifest.verification.is_empty() {
        return Err(WindowsComponentRegistryError::Manifest(
            "component requires at least one verification probe",
        ));
    }
    match manifest.installer_kind {
        WindowsInstallerKind::Builtin => {
            if manifest.artifact_identity.is_some()
                || manifest.artifact_path.is_some()
                || !manifest.installer_args.is_empty()
                || !manifest.accepted_exit_codes.is_empty()
            {
                return Err(WindowsComponentRegistryError::Manifest(
                    "BUILTIN component cannot declare installer artifact/arguments/exit codes",
                ));
            }
        }
        WindowsInstallerKind::Msi | WindowsInstallerKind::Exe => {
            let Some(identity) = &manifest.artifact_identity else {
                return Err(WindowsComponentRegistryError::Manifest(
                    "installer component requires artifact identity",
                ));
            };
            if !valid_sha256_label(identity) {
                return Err(WindowsComponentRegistryError::Manifest(
                    "installer artifact identity is invalid",
                ));
            }
            let Some(path) = &manifest.artifact_path else {
                return Err(WindowsComponentRegistryError::Manifest(
                    "installer component requires artifact path",
                ));
            };
            if !Path::new(path).is_absolute() {
                return Err(WindowsComponentRegistryError::Manifest(
                    "installer artifact path must be absolute",
                ));
            }
            if manifest.accepted_exit_codes.is_empty() {
                return Err(WindowsComponentRegistryError::Manifest(
                    "installer component requires accepted exit codes",
                ));
            }
        }
    }
    Ok(())
}

fn component_digest(
    manifest: &WindowsComponentManifest,
) -> Result<String, WindowsComponentRegistryError> {
    let mut canonical = manifest.clone();
    canonical.digest.clear();
    digest_json(&canonical)
}

fn digest_json<T: Serialize>(value: &T) -> Result<String, WindowsComponentRegistryError> {
    let bytes = serde_json::to_vec(value)?;
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(7 + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(encoded)
}

fn component_revision_path(component_dir: &Path, component_id: &str, revision: u64) -> PathBuf {
    component_dir
        .join(component_id)
        .join("revisions")
        .join(format!("{revision:020}.json"))
}
