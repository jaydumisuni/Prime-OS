use prime_contracts::{
    valid_component_identifier, valid_component_version, validate_component_manifest,
    ComponentManifestError, PersistentDataPolicy, PrimeComponentManifest,
    APPLICATIONS_PROJECTION_SCHEMA, CAPABILITY_INTERFACE_VERSION,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
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
                // Durable failure evidence must not admit unbounded payloads.
                // Bound UTF-8 bytes, not Unicode scalar count, before logging.
                !reason.trim().is_empty()
                    && reason.len() <= 4096
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
                                0x00AD | 0x034F | 0x115F | 0x1160 | 0x17B4..=0x17B5 | 0x180B..=0x180F | 0x200B
                                    | 0x200C | 0x200D | 0x2060..=0x206F | 0x2800 | 0x3164
                                    | 0xFE00..=0xFE0F | 0xFEFF | 0xFFA0 | 0xFFF0..=0xFFF8
                                    | 0xE0000..=0xE0FFF | 0x0600..=0x0605 | 0x06DD | 0x070F | 0x08E2
                                    | 0x0890..=0x0891 | 0x110BD | 0x110CD | 0x13430..=0x1343F
                                    | 0xFFF9..=0xFFFB | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A
                                    | 0x1D185..=0x1D18B | 0x1D1AA..=0x1D1AD
                            )
                            // Noncharacters must not enter durable transaction
                            // failure/recovery evidence on any Unicode plane.
                            || {
                                let point = c as u32;
                                (0xFDD0..=0xFDEF).contains(&point)
                                    || (point & 0xFFFE) == 0xFFFE
                            }
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
    #[error("component {0} requires Prime generation handoff, not component installation")]
    BaseSystemGenerationHandoffRequired(String),
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
    #[error("installed component {0} has no pinned manifest for dependency-safe removal")]
    MissingInstalledManifest(String),
    #[error("installed component {0} manifest disagrees with its inventory identity")]
    InstalledManifestMismatch(String),
    #[error("candidate component {0} manifest disagrees with the verified package identity")]
    CandidateManifestMismatch(String),
    #[error("component {component_id} cannot silently transfer publisher from {installed_publisher} to {candidate_publisher}")]
    PublisherIdentityChanged {
        component_id: String,
        installed_publisher: String,
        candidate_publisher: String,
    },
    #[error("installed component {0} has duplicate manifest evidence")]
    DuplicateInstalledManifest(String),
    #[error("component {0} appears more than once in the registration selection")]
    DuplicateRegistrationTarget(String),
    #[error("explicit registration scope differs from prerequisite closure at {0}")]
    RegistrationScopeMismatch(String),
    #[error("component {0} appears more than once in the unregistration selection")]
    DuplicateUnregistrationTarget(String),
    #[error("explicit unregistration scope differs from dependent closure at {0}")]
    UnregistrationScopeMismatch(String),
    #[error("registration {registration} is claimed by both {first_component_id} and {second_component_id}")]
    RegistrationIdentityConflict {
        registration: String,
        first_component_id: String,
        second_component_id: String,
    },
    #[error("installed component dependency graph contains a cycle through {0}")]
    InstalledDependencyCycle(String),
    #[error("removing {component_id} would strand installed dependent {dependent_id}")]
    DependentComponentInstalled {
        component_id: String,
        dependent_id: String,
    },
    #[error("updating or rolling back {component_id} would invalidate the exact package digest pinned by {dependent_id}")]
    DependentDigestPinned {
        component_id: String,
        dependent_id: String,
    },
    #[error(
        "rolling back {component_id} below revision {minimum_revision} would strand {dependent_id}"
    )]
    DependentRevisionFloor {
        component_id: String,
        dependent_id: String,
        minimum_revision: u64,
    },
    #[error("component transaction for {0} does not advance the installed revision")]
    RevisionNotAdvanced(String),
    #[error("update for {0} reuses the installed package digest across distinct revisions")]
    UpdateDigestUnchanged(String),
    #[error("rollback for {0} reuses the current package digest across distinct revisions")]
    RollbackDigestUnchanged(String),
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
        Some(current) if package.revision > current.revision => {
            if package.package_digest == current.package_digest {
                return Err(ComponentAdmissionError::UpdateDigestUnchanged(
                    package.component_id.clone(),
                ));
            }
            ComponentTransactionKind::Update {
                previous_revision: current.revision,
                previous_package_digest: current.package_digest.clone(),
            }
        }
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

/// Dependency-aware P2 install/update preflight over a complete installed inventory.
///
/// Installed manifests must be independently authenticated and pinned by the
/// caller. This only checks structural reverse dependencies; it does not
/// authorize package installation, update, activation, or promotion.
pub fn plan_component_transaction_checked(
    package: &VerifiedComponentPackage,
    installed: &[InstalledComponent],
    installed_manifests: &[PrimeComponentManifest],
    candidate_manifest: &PrimeComponentManifest,
) -> Result<ComponentTransactionPlan, ComponentAdmissionError> {
    let plan = plan_component_transaction(package, installed)?;
    // The caller separately authenticates the candidate manifest. A package
    // digest alone does not prove its post-install/update dependencies.
    validate_component_manifest(candidate_manifest)?;
    if candidate_manifest.component_id != package.component_id
        || candidate_manifest.revision != package.revision
        || candidate_manifest.package_digest != package.package_digest
        || candidate_manifest.version != package.version
        || candidate_manifest.publisher_id != package.publisher_id
    {
        return Err(ComponentAdmissionError::CandidateManifestMismatch(
            package.component_id.clone(),
        ));
    }
    // A VerifiedComponentPackage is publicly constructible. Checked planning
    // must independently preserve the immutable-base boundary even when a
    // caller skips the offline verifier's GenerationHandoff outcome.
    if !candidate_manifest.base_system_requirements.is_empty() {
        return Err(
            ComponentAdmissionError::BaseSystemGenerationHandoffRequired(
                candidate_manifest.component_id.clone(),
            ),
        );
    }
    let manifests = index_installed_manifests(installed, installed_manifests)?;
    if matches!(&plan.kind, ComponentTransactionKind::Update { .. }) {
        // A new digest and a separately admitted signature do not authorize
        // changing the owner of an already installed component identity.
        // Publisher transfer needs its own explicit authenticated protocol.
        let original = manifests
            .get(package.component_id.as_str())
            .expect("checked update has a complete installed manifest");
        if original.publisher_id != candidate_manifest.publisher_id {
            return Err(ComponentAdmissionError::PublisherIdentityChanged {
                component_id: package.component_id.clone(),
                installed_publisher: original.publisher_id.clone(),
                candidate_publisher: candidate_manifest.publisher_id.clone(),
            });
        }
        for manifest in manifests.values() {
            if manifest.component_id != package.component_id
                && manifest.dependencies.iter().any(|dep| {
                    dep.component_id == package.component_id && dep.exact_package_digest.is_some()
                })
            {
                return Err(ComponentAdmissionError::DependentDigestPinned {
                    component_id: package.component_id.clone(),
                    dependent_id: manifest.component_id.clone(),
                });
            }
        }
    }
    // A previously valid inventory may become cyclic after an update.
    // Recheck the complete proposed graph before returning a checked plan.
    let mut resulting_installed = installed.to_vec();
    let mut resulting_manifests = installed_manifests.to_vec();
    let replacement = InstalledComponent {
        component_id: package.component_id.clone(),
        revision: package.revision,
        package_digest: package.package_digest.clone(),
    };
    if matches!(&plan.kind, ComponentTransactionKind::Install) {
        resulting_installed.push(replacement);
        resulting_manifests.push(candidate_manifest.clone());
    } else {
        let current = resulting_installed
            .iter_mut()
            .find(|item| item.component_id == package.component_id)
            .expect("checked update has an installed target");
        *current = replacement;
        let current_manifest = resulting_manifests
            .iter_mut()
            .find(|item| item.component_id == package.component_id)
            .expect("complete installed manifest inventory was checked");
        *current_manifest = candidate_manifest.clone();
    }
    index_installed_manifests(&resulting_installed, &resulting_manifests)?;
    Ok(plan)
}

/// Structural removal plan only. Does not inspect reverse dependencies or
/// authorize execution; use the checked preflight with trusted manifests.
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

/// Validate a complete exact inventory before planning dependent-sensitive work.
fn index_installed_manifests<'a>(
    installed: &[InstalledComponent],
    installed_manifests: &'a [PrimeComponentManifest],
) -> Result<BTreeMap<&'a str, &'a PrimeComponentManifest>, ComponentAdmissionError> {
    let inventory = index_installed(installed)?;
    let mut manifests = BTreeMap::new();
    for manifest in installed_manifests {
        validate_component_manifest(manifest)?;
        let Some(current) = inventory.get(manifest.component_id.as_str()) else {
            return Err(ComponentAdmissionError::InstalledManifestMismatch(
                manifest.component_id.clone(),
            ));
        };
        if manifest.revision != current.revision
            || manifest.package_digest != current.package_digest
        {
            return Err(ComponentAdmissionError::InstalledManifestMismatch(
                manifest.component_id.clone(),
            ));
        }
        if manifests
            .insert(manifest.component_id.as_str(), manifest)
            .is_some()
        {
            return Err(ComponentAdmissionError::DuplicateInstalledManifest(
                manifest.component_id.clone(),
            ));
        }
    }
    for current in installed {
        if !manifests.contains_key(current.component_id.as_str()) {
            return Err(ComponentAdmissionError::MissingInstalledManifest(
                current.component_id.clone(),
            ));
        }
    }
    // Registration names are global Prime authority identities, not scoped
    // to each package. Two independently valid manifests cannot both own
    // the same service, Application Profile or exposed capability.
    let mut registration_owners = BTreeMap::<&str, &str>::new();
    for manifest in manifests.values() {
        for registration in manifest
            .application_profiles
            .iter()
            .chain(&manifest.services)
            .chain(&manifest.capabilities)
        {
            if let Some(previous_owner) =
                registration_owners.insert(registration.as_str(), manifest.component_id.as_str())
            {
                return Err(ComponentAdmissionError::RegistrationIdentityConflict {
                    registration: registration.clone(),
                    first_component_id: previous_owner.to_owned(),
                    second_component_id: manifest.component_id.clone(),
                });
            }
        }
    }
    // Exact manifest identity is insufficient if the installed dependency
    // graph is already broken. Reject that inventory before issuing any
    // dependency-sensitive update, removal, or rollback plan.
    for manifest in manifests.values() {
        validate_dependencies(manifest, installed)?;
    }
    // A complete inventory can still be impossible to service when mutually
    // dependent manifests form a cycle. Check this without recursive traversal
    // (untrusted inventories must not exhaust the stack).
    let mut outstanding = BTreeMap::<&str, usize>::new();
    let mut dependents = BTreeMap::<&str, Vec<&str>>::new();
    for (&id, manifest) in &manifests {
        outstanding.insert(id, manifest.dependencies.len());
        for dependency in &manifest.dependencies {
            dependents
                .entry(dependency.component_id.as_str())
                .or_default()
                .push(id);
        }
    }
    let mut ready: Vec<&str> = outstanding
        .iter()
        .filter_map(|(&id, &count)| (count == 0).then_some(id))
        .collect();
    let mut visited = 0;
    while let Some(id) = ready.pop() {
        visited += 1;
        if let Some(users) = dependents.get(id) {
            for user in users {
                let count = outstanding
                    .get_mut(user)
                    .expect("dependency inventory was validated");
                *count -= 1;
                if *count == 0 {
                    ready.push(user);
                }
            }
        }
    }
    if visited != manifests.len() {
        let id = outstanding
            .iter()
            .find_map(|(&id, &count)| (count > 0).then_some(id))
            .expect("unvisited dependency cycle has a nonzero incoming degree");
        return Err(ComponentAdmissionError::InstalledDependencyCycle(
            id.to_owned(),
        ));
    }
    Ok(manifests)
}

/// Return a deterministic dependency-first registration preview for a complete,
/// separately authenticated installed manifest inventory.
///
/// This is source-only planning evidence. It neither registers services nor
/// authorizes component activation, removal, installation, or promotion.
/// Unrelated ready components are ordered by canonical component ID.
pub fn plan_component_registration_order_checked(
    installed: &[InstalledComponent],
    installed_manifests: &[PrimeComponentManifest],
) -> Result<Vec<String>, ComponentAdmissionError> {
    let manifests = index_installed_manifests(installed, installed_manifests)?;
    let mut outstanding = BTreeMap::<&str, usize>::new();
    let mut dependents = BTreeMap::<&str, Vec<&str>>::new();
    for (&id, manifest) in &manifests {
        outstanding.insert(id, manifest.dependencies.len());
        for dependency in &manifest.dependencies {
            dependents
                .entry(dependency.component_id.as_str())
                .or_default()
                .push(id);
        }
    }
    let mut ready: BTreeSet<&str> = outstanding
        .iter()
        .filter_map(|(&id, &count)| (count == 0).then_some(id))
        .collect();
    let mut order = Vec::with_capacity(manifests.len());
    while let Some(id) = ready.pop_first() {
        order.push(id.to_owned());
        if let Some(users) = dependents.get(id) {
            for user in users {
                let count = outstanding
                    .get_mut(user)
                    .expect("complete inventory dependencies were validated");
                *count -= 1;
                if *count == 0 {
                    ready.insert(user);
                }
            }
        }
    }
    // The inventory validator already rejects cycles. Retain this guard
    // independently so a future refactor cannot emit a partial preview.
    if order.len() != manifests.len() {
        let id = outstanding
            .iter()
            .find_map(|(&id, &count)| (count > 0).then_some(id))
            .expect("incomplete registration order has an unresolved dependency");
        return Err(ComponentAdmissionError::InstalledDependencyCycle(
            id.to_owned(),
        ));
    }
    Ok(order)
}

/// Preview dependent-first unregistration of a complete, separately authenticated
/// installed component inventory.
///
/// This is the exact reverse of the deterministic dependency-first registration
/// preview: consumers are considered before providers, even across multiple
/// dependency levels. The shared inventory validator rejects missing manifests,
/// identity conflicts and dependency cycles before returning any plan.
///
/// The result is source-only planning evidence. It performs no unregistration,
/// shutdown, removal, rollback, activation or protected authority change.
pub fn plan_component_unregistration_order_checked(
    installed: &[InstalledComponent],
    installed_manifests: &[PrimeComponentManifest],
) -> Result<Vec<String>, ComponentAdmissionError> {
    let mut order = plan_component_registration_order_checked(installed, installed_manifests)?;
    order.reverse();
    Ok(order)
}

/// Preview registration of selected optional components together with every
/// transitively required installed prerequisite, in deterministic dependency-first
/// order. All installed components and pinned manifests must pass full inventory
/// validation even when a selection is empty.
///
/// The preview is deliberately conservative: it includes prerequisites whether
/// or not a separate runtime has already registered them. It does not infer
/// current registration state, install packages, register services or capabilities,
/// or grant activation/promotion authority.
pub fn plan_component_registration_subset_checked(
    selected_component_ids: &[&str],
    installed: &[InstalledComponent],
    installed_manifests: &[PrimeComponentManifest],
) -> Result<Vec<String>, ComponentAdmissionError> {
    let full_order = plan_component_registration_order_checked(installed, installed_manifests)?;
    let manifests = index_installed_manifests(installed, installed_manifests)?;
    let mut selected = BTreeSet::new();
    for &id in selected_component_ids {
        if !valid_component_identifier(id) {
            return Err(ComponentAdmissionError::InvalidTargetComponentId(
                id.to_owned(),
            ));
        }
        if !manifests.contains_key(id) {
            return Err(ComponentAdmissionError::DependencyMissing(id.to_owned()));
        }
        if !selected.insert(id) {
            return Err(ComponentAdmissionError::DuplicateRegistrationTarget(
                id.to_owned(),
            ));
        }
    }
    let mut required = BTreeSet::new();
    let mut pending: Vec<&str> = selected.into_iter().collect();
    while let Some(id) = pending.pop() {
        if required.insert(id) {
            let manifest = manifests
                .get(id)
                .expect("selection and dependency inventory were validated");
            pending.extend(
                manifest
                    .dependencies
                    .iter()
                    .map(|dependency| dependency.component_id.as_str()),
            );
        }
    }
    Ok(full_order
        .into_iter()
        .filter(|id| required.contains(id.as_str()))
        .collect())
}

/// Preview a registration only when the caller declares the complete
/// prerequisite-closed scope. The subset planner may expand prerequisites;
/// this stricter form requires every expanded component to be named explicitly
/// and rejects unrelated entries. The declaration is not approval, signature
/// verification, installation, activation, or runtime registration authority.
pub fn plan_component_registration_explicit_scope_checked(
    selected_component_ids: &[&str],
    explicit_scope_component_ids: &[&str],
    installed: &[InstalledComponent],
    installed_manifests: &[PrimeComponentManifest],
) -> Result<Vec<String>, ComponentAdmissionError> {
    let closure = plan_component_registration_subset_checked(
        selected_component_ids,
        installed,
        installed_manifests,
    )?;
    let inventory = index_installed(installed)?;
    let mut scope = BTreeSet::new();
    for &id in explicit_scope_component_ids {
        if !valid_component_identifier(id) {
            return Err(ComponentAdmissionError::InvalidTargetComponentId(
                id.to_owned(),
            ));
        }
        if !inventory.contains_key(id) {
            return Err(ComponentAdmissionError::DependencyMissing(id.to_owned()));
        }
        if !scope.insert(id) {
            return Err(ComponentAdmissionError::DuplicateRegistrationTarget(
                id.to_owned(),
            ));
        }
    }
    let required: BTreeSet<&str> = closure.iter().map(String::as_str).collect();
    if let Some(id) = required.symmetric_difference(&scope).next() {
        return Err(ComponentAdmissionError::RegistrationScopeMismatch(
            (*id).to_owned(),
        ));
    }
    Ok(closure)
}

/// Preview unregistration of selected installed components and every transitive
/// installed dependent that would otherwise be stranded. The returned closure
/// is explicit and dependent-first, not permission to silently remove consumers.
///
/// Callers must independently authenticate the complete installed manifest
/// inventory and obtain approval for every member of the expanded selection.
/// This is planning-only evidence: no services, packages, data, or authorities
/// are changed. Unknown, duplicate, and noncanonical targets fail closed.
pub fn plan_component_unregistration_dependent_closure_checked(
    selected_component_ids: &[&str],
    installed: &[InstalledComponent],
    installed_manifests: &[PrimeComponentManifest],
) -> Result<Vec<String>, ComponentAdmissionError> {
    let full_order = plan_component_unregistration_order_checked(installed, installed_manifests)?;
    let manifests = index_installed_manifests(installed, installed_manifests)?;
    let mut selected = BTreeSet::new();
    for &id in selected_component_ids {
        if !valid_component_identifier(id) {
            return Err(ComponentAdmissionError::InvalidTargetComponentId(
                id.to_owned(),
            ));
        }
        if !manifests.contains_key(id) {
            return Err(ComponentAdmissionError::DependencyMissing(id.to_owned()));
        }
        if !selected.insert(id) {
            return Err(ComponentAdmissionError::DuplicateUnregistrationTarget(
                id.to_owned(),
            ));
        }
    }
    let mut dependents = BTreeMap::<&str, Vec<&str>>::new();
    for manifest in manifests.values() {
        for dependency in &manifest.dependencies {
            dependents
                .entry(dependency.component_id.as_str())
                .or_default()
                .push(manifest.component_id.as_str());
        }
    }
    let mut required = BTreeSet::new();
    let mut pending: Vec<&str> = selected.into_iter().collect();
    while let Some(id) = pending.pop() {
        if required.insert(id) {
            if let Some(users) = dependents.get(id) {
                pending.extend(users.iter().copied());
            }
        }
    }
    Ok(full_order
        .into_iter()
        .filter(|id| required.contains(id.as_str()))
        .collect())
}

/// Preview a dependent-closed unregistration only when the caller supplies
/// the complete explicit scope. An expanded dependent closure is never silently
/// accepted as the requested scope; unrelated components cannot be smuggled in.
///
/// The scope is an untrusted declaration, NOT approval or authentication.
/// Callers must separately authenticate installed manifests and obtain
/// authorization for every returned component before any mutation.
pub fn plan_component_unregistration_explicit_scope_checked(
    selected_component_ids: &[&str],
    explicit_scope_component_ids: &[&str],
    installed: &[InstalledComponent],
    installed_manifests: &[PrimeComponentManifest],
) -> Result<Vec<String>, ComponentAdmissionError> {
    let closure = plan_component_unregistration_dependent_closure_checked(
        selected_component_ids,
        installed,
        installed_manifests,
    )?;
    let inventory = index_installed(installed)?;
    let mut scope = BTreeSet::new();
    for &id in explicit_scope_component_ids {
        if !valid_component_identifier(id) {
            return Err(ComponentAdmissionError::InvalidTargetComponentId(
                id.to_owned(),
            ));
        }
        if !inventory.contains_key(id) {
            return Err(ComponentAdmissionError::DependencyMissing(id.to_owned()));
        }
        if !scope.insert(id) {
            return Err(ComponentAdmissionError::DuplicateUnregistrationTarget(
                id.to_owned(),
            ));
        }
    }
    let required: BTreeSet<&str> = closure.iter().map(String::as_str).collect();
    if let Some(id) = required.symmetric_difference(&scope).next() {
        return Err(ComponentAdmissionError::UnregistrationScopeMismatch(
            (*id).to_owned(),
        ));
    }
    Ok(closure)
}

/// Preview a selected subset of component unregistrations without stranding
/// installed consumers that are not part of the selection.
///
/// The complete installed inventory and its pinned manifests are validated
/// before planning. A selected provider may be unregistered only when every
/// installed dependent is also selected; dependency-first registration is
/// reversed to yield dependent-first unregistration. Unknown, noncanonical
/// and duplicate selections fail closed. No state or authority is mutated.
pub fn plan_component_unregistration_subset_checked(
    selected_component_ids: &[&str],
    installed: &[InstalledComponent],
    installed_manifests: &[PrimeComponentManifest],
) -> Result<Vec<String>, ComponentAdmissionError> {
    let full_order = plan_component_unregistration_order_checked(installed, installed_manifests)?;
    let manifests = index_installed_manifests(installed, installed_manifests)?;
    let mut selected = BTreeSet::new();
    for &id in selected_component_ids {
        if !valid_component_identifier(id) {
            return Err(ComponentAdmissionError::InvalidTargetComponentId(
                id.to_owned(),
            ));
        }
        if !manifests.contains_key(id) {
            return Err(ComponentAdmissionError::DependencyMissing(id.to_owned()));
        }
        if !selected.insert(id) {
            return Err(ComponentAdmissionError::DuplicateUnregistrationTarget(
                id.to_owned(),
            ));
        }
    }
    for manifest in manifests.values() {
        if selected.contains(manifest.component_id.as_str()) {
            continue;
        }
        for dependency in &manifest.dependencies {
            if selected.contains(dependency.component_id.as_str()) {
                return Err(ComponentAdmissionError::DependentComponentInstalled {
                    component_id: dependency.component_id.clone(),
                    dependent_id: manifest.component_id.clone(),
                });
            }
        }
    }
    Ok(full_order
        .into_iter()
        .filter(|id| selected.contains(id.as_str()))
        .collect())
}

/// Dependency-aware P2 removal preflight over a complete installed inventory.
///
/// Callers must separately authenticate and pin installed manifest evidence.
/// Matching identity/digest here is not signature verification. This is
/// construction-only planning, not removal, deletion, or activation authority.
/// The primitive removal planner cannot establish reverse-dependency safety.
pub fn plan_component_removal_checked(
    component_id: &str,
    installed: &[InstalledComponent],
    installed_manifests: &[PrimeComponentManifest],
) -> Result<ComponentTransactionPlan, ComponentAdmissionError> {
    let plan = plan_component_removal(component_id, installed)?;
    let manifests = index_installed_manifests(installed, installed_manifests)?;
    for manifest in manifests.values() {
        if manifest.component_id != component_id
            && manifest
                .dependencies
                .iter()
                .any(|dep| dep.component_id == component_id)
        {
            return Err(ComponentAdmissionError::DependentComponentInstalled {
                component_id: component_id.to_owned(),
                dependent_id: manifest.component_id.clone(),
            });
        }
    }
    Ok(plan)
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
    if current.package_digest == retained.package_digest {
        return Err(ComponentAdmissionError::RollbackDigestUnchanged(
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

/// Dependency-aware rollback preflight over a complete, authenticated installed
/// manifest inventory. The retained package and manifest require separate
/// signature/digest admission; this only establishes structural compatibility.
/// No rollback, filesystem mutation, or promotion is performed here.
pub fn plan_component_rollback_checked(
    current: &InstalledComponent,
    retained: &InstalledComponent,
    retained_manifest: &PrimeComponentManifest,
    installed: &[InstalledComponent],
    installed_manifests: &[PrimeComponentManifest],
) -> Result<ComponentTransactionPlan, ComponentAdmissionError> {
    let plan = plan_component_rollback(current, retained)?;
    // The retained manifest is separately authenticated by the caller. A
    // digest-only rollback target cannot prove the post-rollback dependency
    // graph, so bind its exact identity before issuing a checked plan.
    validate_component_manifest(retained_manifest)?;
    if retained_manifest.component_id != retained.component_id
        || retained_manifest.revision != retained.revision
        || retained_manifest.package_digest != retained.package_digest
    {
        return Err(ComponentAdmissionError::InstalledManifestMismatch(
            retained.component_id.clone(),
        ));
    }
    // Retained packages requiring image-owned changes belong to the Prime
    // generation/update authority, never to an optional-component rollback.
    // The public checked planner must enforce this independently of whether
    // the caller ran the offline verifier (which returns GenerationHandoff).
    if !retained_manifest.base_system_requirements.is_empty() {
        return Err(
            ComponentAdmissionError::BaseSystemGenerationHandoffRequired(
                retained_manifest.component_id.clone(),
            ),
        );
    }
    let manifests = index_installed_manifests(installed, installed_manifests)?;
    let observed = manifests
        .get(current.component_id.as_str())
        .ok_or_else(|| {
            ComponentAdmissionError::InstalledManifestMismatch(current.component_id.clone())
        })?;
    if observed.revision != current.revision || observed.package_digest != current.package_digest {
        return Err(ComponentAdmissionError::InstalledManifestMismatch(
            current.component_id.clone(),
        ));
    }
    // Retained package authentication alone is not publisher-transfer authority.
    if observed.publisher_id != retained_manifest.publisher_id {
        return Err(ComponentAdmissionError::PublisherIdentityChanged {
            component_id: current.component_id.clone(),
            installed_publisher: observed.publisher_id.clone(),
            candidate_publisher: retained_manifest.publisher_id.clone(),
        });
    }

    for manifest in manifests.values() {
        if manifest.component_id == current.component_id {
            continue;
        }
        for dependency in &manifest.dependencies {
            if dependency.component_id != current.component_id {
                continue;
            }
            if retained.revision < dependency.minimum_revision {
                return Err(ComponentAdmissionError::DependentRevisionFloor {
                    component_id: current.component_id.clone(),
                    dependent_id: manifest.component_id.clone(),
                    minimum_revision: dependency.minimum_revision,
                });
            }
            if dependency
                .exact_package_digest
                .as_deref()
                .is_some_and(|digest| digest != retained.package_digest)
            {
                return Err(ComponentAdmissionError::DependentDigestPinned {
                    component_id: current.component_id.clone(),
                    dependent_id: manifest.component_id.clone(),
                });
            }
        }
    }

    // Check the resulting inventory, not just the currently installed one.
    // An authenticated retained package can introduce a new dependency on an
    // installed dependent, creating a cycle that the old inventory lacks.
    let mut resulting_installed = installed.to_vec();
    for component in &mut resulting_installed {
        if component.component_id == current.component_id {
            *component = retained.clone();
        }
    }
    let resulting_manifests: Vec<_> = installed_manifests
        .iter()
        .map(|manifest| {
            if manifest.component_id == current.component_id {
                retained_manifest.clone()
            } else {
                manifest.clone()
            }
        })
        .collect();
    index_installed_manifests(&resulting_installed, &resulting_manifests)?;
    Ok(plan)
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
    #[cfg(unix)]
    let original_ancestors = private_package_ancestors(package_path)?
        .ok_or(ComponentAdmissionError::UnsafePackagePath)?;

    let observed_digest = sha256_file(package_path)?;
    #[cfg(unix)]
    {
        // Do not invoke either external admission callback if the original
        // package or its private ancestor custody changed during first hash.
        // The final post-callback check remains necessary independently.
        if !package_custody_matches(package_path, &original_metadata, &original_ancestors)? {
            return Err(ComponentAdmissionError::UnsafePackagePath);
        }
    }
    if observed_digest != manifest.package_digest {
        return Err(ComponentAdmissionError::PackageDigestMismatch);
    }

    if !signature_admitted(manifest) {
        return Err(ComponentAdmissionError::SignatureRejected);
    }
    #[cfg(unix)]
    if !package_custody_matches(package_path, &original_metadata, &original_ancestors)? {
        // A signature callback may mutate the package or its custody chain.
        // Do not invoke the separate generation authority on stale evidence.
        return Err(ComponentAdmissionError::UnsafePackagePath);
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
    // Callbacks can also rewrite identical bytes on the same inode. Bind
    // final readback to the initial file metadata, not merely to its digest
    // and inode number. Recheck the pathname after the final digest read.
    let final_digest = sha256_file(package_path)?;
    #[cfg(unix)]
    {
        // Each hash pins its own ancestors, but callbacks can mutate a
        // private parent between the two hashes without changing file bytes.
        // Bind the entire custody chain to the initial admission snapshot.
        if private_package_ancestors(package_path)?.as_ref() != Some(&original_ancestors) {
            return Err(ComponentAdmissionError::UnsafePackagePath);
        }
        let final_metadata = fs::symlink_metadata(package_path)?;
        if !stable_package_metadata(&original_metadata, &current_metadata)
            || !stable_package_metadata(&original_metadata, &final_metadata)
        {
            return Err(ComponentAdmissionError::UnsafePackagePath);
        }
    }
    if current_metadata.len() != original_metadata.len() || final_digest != observed_digest {
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

#[cfg(unix)]
fn private_package_inode(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    // A path can be canonical and singly linked yet writable by a different
    // account. Never admit an inode that a peer can alter during verification.
    metadata.is_file()
        && metadata.nlink() == 1
        && (metadata.uid() == 0 || metadata.uid() == unsafe { libc::geteuid() })
        && metadata.mode() & 0o022 == 0
}

#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageAncestorIdentity {
    dev: u64,
    ino: u64,
    uid: u32,
    mode: u32,
    mtime: Option<i64>,
    mtime_nsec: Option<i64>,
    ctime: Option<i64>,
    ctime_nsec: Option<i64>,
}

#[cfg(unix)]
fn private_package_ancestors(
    path: &Path,
) -> Result<Option<Vec<PackageAncestorIdentity>>, io::Error> {
    use std::os::unix::fs::MetadataExt;
    // A private package inode is insufficient when a peer can replace one
    // of its ancestor directories. Root/current-owner sticky directories
    // (such as /tmp) retain the owner-protected unlink boundary.
    let effective_uid = unsafe { libc::geteuid() };
    let mut ancestor = path.parent();
    let mut identities = Vec::new();
    while let Some(directory) = ancestor {
        let metadata = fs::symlink_metadata(directory)?;
        let trusted_owner = metadata.uid() == 0 || metadata.uid() == effective_uid;
        let peer_writable = metadata.mode() & 0o022 != 0;
        let sticky = metadata.mode() & 0o1000 != 0;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || !trusted_owner
            || (peer_writable && !sticky)
        {
            return Ok(None);
        }
        // In a private directory, mutation timestamps catch in-place
        // entry changes even when its inode and permissions remain stable.
        // A trusted sticky, peer-writable directory (such as /tmp) may
        // change for unrelated peer-owned entries. Its sticky bit protects
        // our owned next component against peer removal, so keep binding
        // its inode/owner/mode without treating unrelated timestamp churn
        // as a mutation of this package's custody chain.
        let private_entries = !peer_writable;
        identities.push(PackageAncestorIdentity {
            dev: metadata.dev(),
            ino: metadata.ino(),
            uid: metadata.uid(),
            mode: metadata.mode(),
            mtime: private_entries.then_some(metadata.mtime()),
            mtime_nsec: private_entries.then_some(metadata.mtime_nsec()),
            ctime: private_entries.then_some(metadata.ctime()),
            ctime_nsec: private_entries.then_some(metadata.ctime_nsec()),
        });
        ancestor = directory.parent();
    }
    Ok(Some(identities))
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
    if !private_package_inode(&metadata) || private_package_ancestors(path)?.is_none() {
        return Err(ComponentAdmissionError::UnsafePackagePath);
    }
    // A regular final file can still be reached through a symlinked parent.
    // Reject aliases and traversal so the admitted package path is canonical.
    if fs::canonicalize(path)? != path {
        return Err(ComponentAdmissionError::UnsafePackagePath);
    }
    Ok(())
}

#[cfg(unix)]
fn stable_package_metadata(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    private_package_inode(before)
        && private_package_inode(after)
        && before.uid() == after.uid()
        && before.mode() == after.mode()
        && before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec()
}

#[cfg(unix)]
fn package_custody_matches(
    path: &Path,
    original: &fs::Metadata,
    ancestors: &[PackageAncestorIdentity],
) -> Result<bool, ComponentAdmissionError> {
    let named = fs::symlink_metadata(path)?;
    Ok(stable_package_metadata(original, &named)
        && private_package_ancestors(path)?.as_deref() == Some(ancestors))
}

fn sha256_file(path: &Path) -> Result<String, ComponentAdmissionError> {
    // Pin a regular, unaliased inode for hashing. A pathname checked by
    // validate_package_path can be swapped before File::open; O_NOFOLLOW
    // and O_NONBLOCK prevent a symlink escape or blocking FIFO substitution.
    #[cfg(unix)]
    let (mut file, opened_metadata, opened_ancestors) = {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|error| {
                if error.raw_os_error() == Some(libc::ELOOP) {
                    ComponentAdmissionError::UnsafePackagePath
                } else {
                    ComponentAdmissionError::Io(error)
                }
            })?;
        let opened = file.metadata()?;
        let named = fs::symlink_metadata(path)?;
        let opened_ancestors =
            private_package_ancestors(path)?.ok_or(ComponentAdmissionError::UnsafePackagePath)?;
        if !private_package_inode(&opened)
            || !private_package_inode(&named)
            || opened.dev() != named.dev()
            || opened.ino() != named.ino()
            || fs::canonicalize(path)? != path
        {
            return Err(ComponentAdmissionError::UnsafePackagePath);
        }
        (file, opened, opened_ancestors)
    };
    #[cfg(not(unix))]
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // A pinned descriptor prevents path redirection, but does not stop
        // another writer mutating the same inode while it is being hashed.
        // Reject such reads rather than admitting a mixed-time digest.
        let after = file.metadata()?;
        let named = fs::symlink_metadata(path)?;
        if !stable_package_metadata(&opened_metadata, &after)
            || !private_package_inode(&named)
            || private_package_ancestors(path)?.as_ref() != Some(&opened_ancestors)
            || named.dev() != opened_metadata.dev()
            || named.ino() != opened_metadata.ino()
            || fs::canonicalize(path)? != path
        {
            return Err(ComponentAdmissionError::UnsafePackagePath);
        }
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
    fn checked_component_plan_never_bypasses_base_generation_handoff() {
        // An offline verifier returns GenerationHandoff for this manifest.
        // A caller can nevertheless construct the public VerifiedComponentPackage
        // directly: checked planning must not turn it into a component install.
        let mut candidate = manifest(b"base-driver");
        candidate.base_system_requirements = vec!["kernel.module.example".to_owned()];
        let package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/unadmitted-base-driver.primepkg"),
            package_digest: candidate.package_digest.clone(),
            component_id: candidate.component_id.clone(),
            revision: candidate.revision,
            version: candidate.version.clone(),
            publisher_id: candidate.publisher_id.clone(),
        };
        assert!(matches!(
            plan_component_transaction_checked(&package, &[], &[], &candidate),
            Err(ComponentAdmissionError::BaseSystemGenerationHandoffRequired(id))
                if id == candidate.component_id
        ));

        let mut optional = candidate.clone();
        optional.base_system_requirements.clear();
        assert!(matches!(
            plan_component_transaction_checked(&package, &[], &[], &optional),
            Ok(ComponentTransactionPlan {
                kind: ComponentTransactionKind::Install,
                ..
            })
        ));

        let current = InstalledComponent {
            component_id: candidate.component_id.clone(),
            revision: 1,
            package_digest: DIGEST_A.to_owned(),
        };
        let mut original = candidate.clone();
        original.base_system_requirements.clear();
        original.revision = 1;
        original.package_digest = current.package_digest.clone();
        assert!(matches!(
            plan_component_transaction_checked(
                &package,
                std::slice::from_ref(&current),
                &[original],
                &candidate,
            ),
            Err(ComponentAdmissionError::BaseSystemGenerationHandoffRequired(id))
                if id == candidate.component_id
        ));
    }

    #[test]
    fn checked_rollback_never_bypasses_base_generation_handoff() {
        // A retained manifest may require image-owned changes even when its
        // digest and publisher are independently authenticated. Rollback must
        // not silently turn that generation handoff into a component plan.
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
        let mut current_manifest = manifest(b"current");
        current_manifest.revision = current.revision;
        current_manifest.version = "3.0.0".to_owned();
        current_manifest.package_digest = current.package_digest.clone();
        let mut retained_manifest = current_manifest.clone();
        retained_manifest.revision = retained.revision;
        retained_manifest.version = "2.0.0".to_owned();
        retained_manifest.package_digest = retained.package_digest.clone();
        retained_manifest.base_system_requirements = vec!["kernel.module.example".to_owned()];

        assert!(matches!(
            plan_component_rollback_checked(
                &current,
                &retained,
                &retained_manifest,
                std::slice::from_ref(&current),
                std::slice::from_ref(&current_manifest),
            ),
            Err(ComponentAdmissionError::BaseSystemGenerationHandoffRequired(id))
                if id == retained.component_id
        ));

        retained_manifest.base_system_requirements.clear();
        assert!(matches!(
            plan_component_rollback_checked(
                &current,
                &retained,
                &retained_manifest,
                std::slice::from_ref(&current),
                &[current_manifest],
            ),
            Ok(ComponentTransactionPlan {
                kind: ComponentTransactionKind::Rollback { .. },
                ..
            })
        ));
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
    fn update_rejects_same_package_digest_across_distinct_revisions() {
        let package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/component.primepkg"),
            package_digest: DIGEST_A.to_owned(),
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
        assert!(matches!(
            plan_component_transaction(&package, &current),
            Err(ComponentAdmissionError::UpdateDigestUnchanged(id)) if id == package.component_id
        ));
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
    fn rollback_rejects_same_digest_across_distinct_revisions() {
        let current = InstalledComponent {
            component_id: "origins.runtime".to_owned(),
            revision: 3,
            package_digest: DIGEST_A.to_owned(),
        };
        let retained = InstalledComponent {
            component_id: current.component_id.clone(),
            revision: 2,
            package_digest: current.package_digest.clone(),
        };
        assert!(
            plan_component_rollback(&current, &retained).is_err(),
            "distinct package revisions must not share an identical content digest"
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
    fn transaction_failure_reasons_are_bounded_in_utf8_bytes() {
        use ComponentTransactionState as State;
        const LIMIT: usize = 4096;

        for reason in ["a".repeat(LIMIT), "é".repeat(LIMIT / 2)] {
            for next in [
                State::Failed {
                    reason: reason.clone(),
                },
                State::RecoveryRequired {
                    reason: reason.clone(),
                },
            ] {
                assert!(State::Applying.advance(next).is_ok());
            }
        }

        for reason in ["a".repeat(LIMIT + 1), "é".repeat(LIMIT / 2 + 1)] {
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
                    "oversized transaction reason was admitted"
                );
            }
            assert_eq!(
                (State::Failed {
                    reason: reason.clone()
                })
                .advance(State::RecoveryRequired {
                    reason: "restore prior component".into()
                }),
                Err(ComponentTransactionTransitionError),
                "oversized prior failure reason was admitted"
            );
        }
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
    fn transaction_reasons_reject_invisible_math_operators_and_braille_blank() {
        use ComponentTransactionState as State;
        for mark in ['\u{2061}', '\u{2062}', '\u{2063}', '\u{2064}', '\u{2800}'] {
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
                        "invisible failure/recovery reason accepted: {reason:?}"
                    );
                }
                assert_eq!(
                    (State::Failed {
                        reason: reason.clone()
                    })
                    .advance(State::RecoveryRequired {
                        reason: "restore previous package".into(),
                    }),
                    Err(ComponentTransactionTransitionError),
                    "invisible prior failure reason accepted: {reason:?}"
                );
            }
        }
    }

    #[test]
    fn transaction_reasons_reject_shorthand_and_musical_format_controls() {
        use ComponentTransactionState as State;
        for point in (0x1BCA0..=0x1BCA3).chain(0x1D173..=0x1D17A) {
            let hidden = char::from_u32(point).unwrap();
            let reason = format!("disk{hidden}failure");
            assert_eq!(
                State::Applying.advance(State::Failed {
                    reason: reason.clone(),
                }),
                Err(ComponentTransactionTransitionError),
                "format control accepted in failure reason: {point:x}"
            );
            assert_eq!(
                (State::Failed { reason }).advance(State::RecoveryRequired {
                    reason: "restore previous package".into(),
                }),
                Err(ComponentTransactionTransitionError),
                "format control accepted in prior failure reason: {point:x}"
            );
        }
    }

    #[test]
    fn transaction_reasons_reject_musical_combining_marks() {
        use ComponentTransactionState as State;
        for point in (0x1D185..=0x1D18B).chain(0x1D1AA..=0x1D1AD) {
            let hidden = char::from_u32(point).unwrap();
            let reason = format!("disk{hidden}failure");
            assert_eq!(
                State::Applying.advance(State::Failed {
                    reason: reason.clone(),
                }),
                Err(ComponentTransactionTransitionError),
                "musical combining mark accepted in failure reason: {point:x}"
            );
            assert_eq!(
                (State::Failed { reason }).advance(State::RecoveryRequired {
                    reason: "restore previous package".into(),
                }),
                Err(ComponentTransactionTransitionError),
                "musical combining mark accepted in prior failure reason: {point:x}"
            );
        }
    }

    #[test]
    fn transaction_reasons_reject_khmer_inherent_vowels() {
        use ComponentTransactionState as State;
        for hidden in ['\u{17B4}', '\u{17B5}'] {
            let reason = format!("disk{hidden}failure");
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
                    "Khmer inherent vowel accepted in transaction reason: {reason:?}"
                );
            }
            assert_eq!(
                (State::Failed { reason }).advance(State::RecoveryRequired {
                    reason: "restore previous package".into(),
                }),
                Err(ComponentTransactionTransitionError),
                "Khmer inherent vowel accepted in prior failure reason"
            );
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
    fn transaction_reasons_reject_reserved_invisible_ranges() {
        use ComponentTransactionState as State;

        // Reserved format and tag code points are not whitespace or control
        // characters, but cannot supply trustworthy human-readable evidence.
        for point in [0x2065, 0xFFF0, 0xFFF8, 0xE0080, 0xE0FFF] {
            let mark = char::from_u32(point).unwrap();
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
                        "reserved invisible mark accepted: U+{point:06X}"
                    );
                }
                assert_eq!(
                    (State::Failed {
                        reason: reason.clone(),
                    })
                    .advance(State::RecoveryRequired {
                        reason: "restore previous package".into(),
                    }),
                    Err(ComponentTransactionTransitionError),
                    "reserved prior failure reason accepted: U+{point:06X}"
                );
                assert_eq!(
                    (State::RecoveryRequired { reason }).advance(State::RolledBack),
                    Err(ComponentTransactionTransitionError),
                    "reserved recovery reason allowed rollback: U+{point:06X}"
                );
            }
        }
        assert!(State::Applying
            .advance(State::Failed {
                reason: "disk failure".into(),
            })
            .is_ok());
    }

    #[test]
    fn transaction_reasons_reject_remaining_unicode_format_controls() {
        use ComponentTransactionState as State;

        // Match the manifest text boundary: these Unicode Cf controls are not
        // necessarily whitespace or Rust control chars, yet can spoof durable
        // failure/recovery evidence.
        for point in (0x0600..=0x0605)
            .chain([0x06DD, 0x070F, 0x08E2, 0x0890, 0x0891, 0x110BD, 0x110CD])
            .chain(0x13430..=0x1343F)
            .chain(0xFFF9..=0xFFFB)
        {
            let mark = char::from_u32(point).unwrap();
            let reason = format!("disk{mark}failure");
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
                    "format control admitted into new state: U+{point:04X}"
                );
            }
            assert_eq!(
                (State::Failed {
                    reason: reason.clone(),
                })
                .advance(State::RecoveryRequired {
                    reason: "restore previous package".into(),
                }),
                Err(ComponentTransactionTransitionError),
                "format control admitted from prior failure: U+{point:04X}"
            );
            assert_eq!(
                (State::RecoveryRequired { reason }).advance(State::RolledBack),
                Err(ComponentTransactionTransitionError),
                "format control allowed rollback from recovery: U+{point:04X}"
            );
        }
        assert!(State::Applying
            .advance(State::Failed {
                reason: "disk failure".into(),
            })
            .is_ok());
    }

    #[test]
    fn transaction_reasons_reject_supplementary_variation_selectors() {
        use ComponentTransactionState as State;

        // Supplementary variation selectors are invisible, including when they
        // are the entire reason. They cannot establish durable failure evidence.
        for point in 0xE0100..=0xE01EF {
            let mark = char::from_u32(point).unwrap();
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
                        "supplementary variation selector accepted: U+{point:06X}"
                    );
                }
                assert_eq!(
                    (State::Failed {
                        reason: reason.clone(),
                    })
                    .advance(State::RecoveryRequired {
                        reason: "restore previous package".into(),
                    }),
                    Err(ComponentTransactionTransitionError),
                    "invalid prior failure reason accepted: U+{point:06X}"
                );
                assert_eq!(
                    (State::RecoveryRequired { reason }).advance(State::RolledBack),
                    Err(ComponentTransactionTransitionError),
                    "invalid recovery reason allowed rollback: U+{point:06X}"
                );
            }
        }
    }

    #[test]
    fn transaction_reasons_reject_unicode_noncharacters() {
        use ComponentTransactionState as State;
        let points = (0xFDD0..=0xFDEF)
            .chain((0..=16).flat_map(|plane| [plane * 0x10000 + 0xFFFE, plane * 0x10000 + 0xFFFF]));
        for point in points {
            let mark = char::from_u32(point).unwrap();
            let reason = format!("disk{mark}failure");
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
                    "noncharacter admitted into transaction: U+{point:06X}"
                );
            }
            assert_eq!(
                (State::Failed {
                    reason: reason.clone()
                })
                .advance(State::RecoveryRequired {
                    reason: "restore".into()
                }),
                Err(ComponentTransactionTransitionError),
                "noncharacter admitted from prior failure: U+{point:06X}"
            );
            assert_eq!(
                (State::RecoveryRequired { reason }).advance(State::RolledBack),
                Err(ComponentTransactionTransitionError),
                "noncharacter allowed rollback: U+{point:06X}"
            );
        }
        assert!(State::Applying
            .advance(State::Failed {
                reason: "ordinary disk failure".into(),
            })
            .is_ok());
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
    fn offline_package_rejects_peer_writable_inode() {
        use std::os::unix::fs::PermissionsExt;
        let bytes = b"p2-private-package-custody";
        let (_dir, path) = write_package(bytes);
        let manifest = manifest(bytes);
        for mode in [0o620, 0o602] {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
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
            assert!(matches!(
                sha256_file(&path),
                Err(ComponentAdmissionError::UnsafePackagePath)
            ));
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(
            verify_offline_component_package(
                &path,
                &manifest,
                &context(&[]),
                |_| true,
                |_, _| true,
            ),
            Ok(ComponentAdmissionOutcome::Ready(_))
        ));
        assert_eq!(sha256_file(&path).unwrap(), package_digest(bytes));
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_rejects_peer_writable_ancestor_directory() {
        use std::os::unix::fs::PermissionsExt;
        let bytes = b"p2-ancestor-directory-custody";
        let (dir, _) = write_package(bytes);
        let parent = dir.path().join("package-parent");
        fs::create_dir(&parent).unwrap();
        let path = parent.join("component.primepkg");
        fs::write(&path, bytes).unwrap();
        let manifest = manifest(bytes);

        for mode in [0o770, 0o707] {
            fs::set_permissions(&parent, fs::Permissions::from_mode(mode)).unwrap();
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
            assert!(matches!(
                sha256_file(&path),
                Err(ComponentAdmissionError::UnsafePackagePath)
            ));
        }

        // A private directory and an owner-controlled sticky directory both
        // preserve the existing source-only offline admission behavior.
        for mode in [0o700, 0o1707] {
            fs::set_permissions(&parent, fs::Permissions::from_mode(mode)).unwrap();
            assert!(matches!(
                verify_offline_component_package(
                    &path,
                    &manifest,
                    &context(&[]),
                    |_| true,
                    |_, _| true,
                ),
                Ok(ComponentAdmissionOutcome::Ready(_))
            ));
            assert_eq!(sha256_file(&path).unwrap(), package_digest(bytes));
        }
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_ancestor_snapshot_detects_directory_replacement() {
        let bytes = b"p2-ancestor-identity-snapshot";
        let (dir, _) = write_package(bytes);
        let parent = dir.path().join("package-parent");
        fs::create_dir(&parent).unwrap();
        let path = parent.join("component.primepkg");
        fs::write(&path, bytes).unwrap();

        let before = private_package_ancestors(&path).unwrap().unwrap();
        assert_eq!(before, private_package_ancestors(&path).unwrap().unwrap());

        // Keep the old directory alive so a replacement cannot reuse its inode.
        let moved = dir.path().join("moved-package-parent");
        fs::rename(&parent, &moved).unwrap();
        fs::create_dir(&parent).unwrap();
        fs::write(&path, bytes).unwrap();
        let after = private_package_ancestors(&path).unwrap().unwrap();
        assert_ne!(before, after);
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_ancestor_snapshot_detects_same_inode_metadata_change() {
        let bytes = b"p2-ancestor-directory-metadata-custody";
        let (dir, _) = write_package(bytes);
        let parent = dir.path().join("package-parent");
        fs::create_dir(&parent).unwrap();
        let path = parent.join("component.primepkg");
        fs::write(&path, bytes).unwrap();

        let before = private_package_ancestors(&path).unwrap().unwrap();
        // Change directory metadata without replacing the directory or
        // package inode. A dev/ino/uid/mode-only snapshot would miss this.
        let parent_file = fs::File::open(&parent).unwrap();
        parent_file
            .set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
            .unwrap();
        let after = private_package_ancestors(&path).unwrap().unwrap();
        assert_eq!(before[0].dev, after[0].dev);
        assert_eq!(before[0].ino, after[0].ino);
        assert_eq!(before[0].uid, after[0].uid);
        assert_eq!(before[0].mode, after[0].mode);
        assert_ne!(before, after);
        assert_eq!(sha256_file(&path).unwrap(), package_digest(bytes));
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_allows_unrelated_sticky_ancestor_entry_churn() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let bytes = b"p2-sticky-ancestor-unrelated-entry";
        let (dir, _) = write_package(bytes);
        let shared = dir.path().join("shared-sticky");
        fs::create_dir(&shared).unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o1777)).unwrap();
        let path = shared.join("component.primepkg");
        fs::write(&path, bytes).unwrap();

        let before = private_package_ancestors(&path).unwrap().unwrap();
        assert!(before[0].mtime.is_none());
        let sticky_inode = fs::symlink_metadata(&shared).unwrap().ino();
        fs::write(shared.join("unrelated-peer-entry"), b"unrelated").unwrap();
        let after = private_package_ancestors(&path).unwrap().unwrap();
        assert_eq!(
            before, after,
            "unrelated sticky-directory churn is not a package replacement"
        );
        assert_eq!(after[0].ino, sticky_inode);
        assert_eq!(sha256_file(&path).unwrap(), package_digest(bytes));
        assert!(matches!(
            verify_offline_component_package(
                &path,
                &manifest(bytes),
                &context(&[]),
                |_| true,
                |_, _| true,
            ),
            Ok(ComponentAdmissionOutcome::Ready(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_detects_sticky_ancestor_inode_replacement() {
        use std::os::unix::fs::PermissionsExt;
        let bytes = b"p2-sticky-ancestor-replacement";
        let (dir, _) = write_package(bytes);
        let shared = dir.path().join("shared-sticky");
        fs::create_dir(&shared).unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o1777)).unwrap();
        let path = shared.join("component.primepkg");
        fs::write(&path, bytes).unwrap();
        let before = private_package_ancestors(&path).unwrap().unwrap();

        let moved = dir.path().join("old-shared-sticky");
        fs::rename(&shared, &moved).unwrap();
        fs::create_dir(&shared).unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o1777)).unwrap();
        fs::write(&path, bytes).unwrap();
        let after = private_package_ancestors(&path).unwrap().unwrap();
        assert_ne!(before[0].ino, after[0].ino);
        assert_ne!(before, after);
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
    fn offline_package_pre_signature_custody_detects_parent_mutation() {
        let bytes = b"p2-pre-signature-parent";
        let (dir, _) = write_package(bytes);
        let parent = dir.path().join("private-parent");
        fs::create_dir(&parent).unwrap();
        let path = parent.join("component.primepkg");
        fs::write(&path, bytes).unwrap();
        let original = fs::symlink_metadata(&path).unwrap();
        let ancestors = private_package_ancestors(&path).unwrap().unwrap();
        assert!(package_custody_matches(&path, &original, &ancestors).unwrap());

        fs::File::open(&parent)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
            .unwrap();
        assert!(!package_custody_matches(&path, &original, &ancestors).unwrap());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_pre_signature_custody_detects_same_byte_rewrite() {
        let bytes = b"p2-pre-signature-same-byte";
        let (_dir, path) = write_package(bytes);
        let original = fs::symlink_metadata(&path).unwrap();
        let ancestors = private_package_ancestors(&path).unwrap().unwrap();
        assert!(package_custody_matches(&path, &original, &ancestors).unwrap());

        fs::write(&path, bytes).unwrap();
        fs::File::open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
            .unwrap();
        assert!(!package_custody_matches(&path, &original, &ancestors).unwrap());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_pre_signature_custody_allows_sticky_entry_churn() {
        use std::os::unix::fs::PermissionsExt;
        let bytes = b"p2-pre-signature-sticky";
        let (dir, _) = write_package(bytes);
        let parent = dir.path().join("shared-sticky");
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o1777)).unwrap();
        let path = parent.join("component.primepkg");
        fs::write(&path, bytes).unwrap();
        let original = fs::symlink_metadata(&path).unwrap();
        let ancestors = private_package_ancestors(&path).unwrap().unwrap();
        fs::write(parent.join("unrelated-peer-entry"), b"unrelated").unwrap();
        assert!(package_custody_matches(&path, &original, &ancestors).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_skips_generation_callback_after_signature_custody_change() {
        let bytes = b"p2-signature-generation-boundary";
        let (_dir, path) = write_package(bytes);
        let manifest = manifest(bytes);
        let generation_called = std::cell::Cell::new(false);
        let result = verify_offline_component_package(
            &path,
            &manifest,
            &context(&[]),
            |_| {
                // The digest is unchanged, but the original inode custody is not.
                fs::write(&path, bytes).unwrap();
                fs::File::options()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
                    .unwrap();
                true
            },
            |_, _| {
                generation_called.set(true);
                true
            },
        );
        assert!(matches!(
            result,
            Err(ComponentAdmissionError::UnsafePackagePath)
        ));
        assert!(
            !generation_called.get(),
            "generation callback must not run on a mutated package"
        );
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_rejects_private_parent_mutation_during_signature() {
        use std::os::unix::fs::MetadataExt;
        let bytes = b"p2-callback-parent-custody";
        let (dir, _) = write_package(bytes);
        let parent = dir.path().join("private-parent");
        fs::create_dir(&parent).unwrap();
        let path = parent.join("component.primepkg");
        fs::write(&path, bytes).unwrap();
        let before_file = fs::symlink_metadata(&path).unwrap();
        let before_ancestors = private_package_ancestors(&path).unwrap().unwrap();

        let result = verify_offline_component_package(
            &path,
            &manifest(bytes),
            &context(&[]),
            |_| {
                // Changing only a private parent must invalidate the original
                // custody snapshot even if both hashes see identical bytes.
                fs::File::open(&parent)
                    .unwrap()
                    .set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
                    .unwrap();
                true
            },
            |_, _| true,
        );
        assert_ne!(
            before_ancestors,
            private_package_ancestors(&path).unwrap().unwrap()
        );
        assert_eq!(
            before_file.ino(),
            fs::symlink_metadata(&path).unwrap().ino()
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(matches!(
            result,
            Err(ComponentAdmissionError::UnsafePackagePath)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_allows_sticky_parent_churn_during_signature() {
        use std::os::unix::fs::PermissionsExt;
        let bytes = b"p2-sticky-parent-callback";
        let (dir, _) = write_package(bytes);
        let shared = dir.path().join("shared-sticky");
        fs::create_dir(&shared).unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o1777)).unwrap();
        let path = shared.join("component.primepkg");
        fs::write(&path, bytes).unwrap();

        let result = verify_offline_component_package(
            &path,
            &manifest(bytes),
            &context(&[]),
            |_| {
                fs::write(shared.join("unrelated-entry"), b"peer-data").unwrap();
                true
            },
            |_, _| true,
        );
        assert!(matches!(result, Ok(ComponentAdmissionOutcome::Ready(_))));
    }

    #[cfg(unix)]
    #[test]
    fn offline_package_rejects_same_byte_in_place_rewrite_during_signature() {
        use std::os::unix::fs::MetadataExt;
        let bytes = b"origins-package-v2";
        let (_dir, path) = write_package(bytes);
        let manifest = manifest(bytes);
        let before = fs::symlink_metadata(&path).unwrap();
        let result = verify_offline_component_package(
            &path,
            &manifest,
            &context(&[]),
            |_| {
                // Same bytes and inode must not launder a new package state.
                fs::write(&path, bytes).unwrap();
                fs::File::options()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
                    .unwrap();
                true
            },
            |_, _| true,
        );
        let after = fs::symlink_metadata(&path).unwrap();
        assert_eq!(before.ino(), after.ino());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_ne!(before.mtime(), after.mtime());
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

    #[cfg(unix)]
    #[test]
    fn package_hasher_rejects_symlink_and_directory_aliases() {
        use std::os::unix::fs::symlink;
        let (dir, package) = write_package(b"p2-package-hasher");
        let alias = dir.path().join("alias.primepkg");
        symlink(&package, &alias).unwrap();
        assert!(matches!(
            sha256_file(&alias),
            Err(ComponentAdmissionError::UnsafePackagePath)
        ));
        assert!(matches!(
            sha256_file(dir.path()),
            Err(ComponentAdmissionError::UnsafePackagePath)
        ));
        // A substituted FIFO must be rejected without blocking on a writer.
        use std::os::unix::ffi::OsStrExt;
        let fifo = dir.path().join("blocking-alias.primepkg");
        let fifo_name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo_name.as_ptr(), 0o600) }, 0);
        assert!(matches!(
            sha256_file(&fifo),
            Err(ComponentAdmissionError::UnsafePackagePath)
        ));
        assert_eq!(
            sha256_file(&package).unwrap(),
            package_digest(b"p2-package-hasher")
        );
    }

    #[cfg(unix)]
    #[test]
    fn package_hash_metadata_guard_detects_in_place_mutation() {
        let (_dir, package) = write_package(b"p2-stable-inode");
        let before = fs::metadata(&package).unwrap();
        assert!(stable_package_metadata(
            &before,
            &fs::metadata(&package).unwrap()
        ));
        // Same inode and canonical path do not imply stable bytes.
        fs::write(&package, b"p2-mutated-in-place-with-a-different-length").unwrap();
        let after = fs::metadata(&package).unwrap();
        use std::os::unix::fs::MetadataExt;
        assert_eq!(before.ino(), after.ino());
        assert!(!stable_package_metadata(&before, &after));
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

    #[test]
    fn checked_removal_rejects_stranded_installed_dependents() {
        let target = InstalledComponent {
            component_id: "origins.runtime".to_owned(),
            revision: 2,
            package_digest: DIGEST_A.to_owned(),
        };
        let consumer = InstalledComponent {
            component_id: "consumer.runtime".to_owned(),
            revision: 1,
            package_digest: DIGEST_B.to_owned(),
        };
        let mut target_manifest = manifest(b"origins");
        target_manifest.package_digest = DIGEST_A.to_owned();
        let mut consumer_manifest = manifest(b"consumer");
        consumer_manifest.component_id = consumer.component_id.clone();
        consumer_manifest.services = vec!["consumerd".to_owned()];
        consumer_manifest.capabilities = vec!["consumer.factory".to_owned()];
        consumer_manifest.revision = consumer.revision;
        consumer_manifest.package_digest = consumer.package_digest.clone();
        consumer_manifest.dependencies = vec![ComponentDependency {
            component_id: target.component_id.clone(),
            minimum_revision: 1,
            exact_package_digest: None,
        }];
        let installed = [target.clone(), consumer.clone()];
        assert!(matches!(
            plan_component_removal_checked(&target.component_id, &installed, &[target_manifest, consumer_manifest]),
            Err(ComponentAdmissionError::DependentComponentInstalled { dependent_id, component_id })
                if dependent_id == consumer.component_id && component_id == target.component_id
        ));
    }

    #[test]
    fn checked_removal_requires_complete_exact_manifest_inventory() {
        let target = InstalledComponent {
            component_id: "origins.runtime".to_owned(),
            revision: 2,
            package_digest: DIGEST_A.to_owned(),
        };
        let consumer = InstalledComponent {
            component_id: "consumer.runtime".to_owned(),
            revision: 1,
            package_digest: DIGEST_B.to_owned(),
        };
        let installed = [target.clone(), consumer.clone()];
        let mut target_manifest = manifest(b"origins");
        target_manifest.package_digest = target.package_digest.clone();
        assert!(matches!(
            plan_component_removal_checked(&target.component_id, &installed, &[target_manifest.clone()]),
            Err(ComponentAdmissionError::MissingInstalledManifest(id)) if id == consumer.component_id
        ));
        let mut consumer_manifest = manifest(b"consumer");
        consumer_manifest.component_id = consumer.component_id.clone();
        consumer_manifest.services = vec!["consumerd".to_owned()];
        consumer_manifest.capabilities = vec!["consumer.factory".to_owned()];
        consumer_manifest.revision = consumer.revision;
        consumer_manifest.package_digest = DIGEST_A.to_owned();
        assert!(matches!(
            plan_component_removal_checked(&target.component_id, &installed, &[target_manifest.clone(), consumer_manifest.clone()]),
            Err(ComponentAdmissionError::InstalledManifestMismatch(id)) if id == consumer.component_id
        ));
        consumer_manifest.package_digest = consumer.package_digest.clone();
        let plan = plan_component_removal_checked(
            &target.component_id,
            &installed,
            &[target_manifest.clone(), consumer_manifest.clone()],
        )
        .unwrap();
        assert!(matches!(plan.kind, ComponentTransactionKind::Remove { .. }));
        assert!(matches!(
            plan_component_removal_checked(&target.component_id, &installed, &[target_manifest.clone(), target_manifest, consumer_manifest]),
            Err(ComponentAdmissionError::DuplicateInstalledManifest(id)) if id == target.component_id
        ));
    }

    #[test]
    fn checked_update_refuses_silent_component_publisher_transfer() {
        let installed = InstalledComponent {
            component_id: "origins.runtime".to_owned(),
            revision: 2,
            package_digest: DIGEST_A.to_owned(),
        };
        let mut original = manifest(b"original");
        original.package_digest = installed.package_digest.clone();
        let package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/publisher-transfer.primepkg"),
            package_digest: DIGEST_B.to_owned(),
            component_id: installed.component_id.clone(),
            revision: 3,
            version: "3.0.0".to_owned(),
            publisher_id: "other.publisher".to_owned(),
        };
        let mut candidate = original.clone();
        candidate.package_digest = package.package_digest.clone();
        candidate.revision = package.revision;
        candidate.version = package.version.clone();
        candidate.publisher_id = package.publisher_id.clone();
        assert!(matches!(plan_component_transaction_checked(
            &package, std::slice::from_ref(&installed), std::slice::from_ref(&original), &candidate
        ), Err(ComponentAdmissionError::PublisherIdentityChanged {
            component_id, installed_publisher, candidate_publisher
        }) if component_id == installed.component_id
            && installed_publisher == original.publisher_id
            && candidate_publisher == package.publisher_id));

        candidate.publisher_id = original.publisher_id.clone();
        let mut original_publisher_package = package;
        original_publisher_package.publisher_id = original.publisher_id.clone();
        assert!(
            plan_component_transaction_checked(
                &original_publisher_package,
                &[installed],
                &[original],
                &candidate
            )
            .is_ok(),
            "same-publisher updates remain structurally eligible"
        );
    }

    #[test]
    fn checked_rollback_refuses_silent_component_publisher_transfer() {
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
        let mut current_manifest = manifest(b"current");
        current_manifest.revision = current.revision;
        current_manifest.package_digest = current.package_digest.clone();
        let mut retained_manifest = current_manifest.clone();
        retained_manifest.revision = retained.revision;
        retained_manifest.package_digest = retained.package_digest.clone();
        retained_manifest.publisher_id = "other.publisher".to_owned();
        assert!(matches!(plan_component_rollback_checked(
            &current, &retained, &retained_manifest, std::slice::from_ref(&current), std::slice::from_ref(&current_manifest)
        ), Err(ComponentAdmissionError::PublisherIdentityChanged {
            component_id, installed_publisher, candidate_publisher
        }) if component_id == current.component_id
            && installed_publisher == current_manifest.publisher_id
            && candidate_publisher == retained_manifest.publisher_id));

        retained_manifest.publisher_id = current_manifest.publisher_id.clone();
        assert!(
            plan_component_rollback_checked(
                &current,
                &retained,
                &retained_manifest,
                std::slice::from_ref(&current),
                &[current_manifest]
            )
            .is_ok(),
            "same-publisher rollback remains structurally eligible"
        );
    }

    #[test]
    fn checked_update_rejects_pinned_installed_dependents() {
        let target = InstalledComponent {
            component_id: "origins.runtime".to_owned(),
            revision: 2,
            package_digest: DIGEST_A.to_owned(),
        };
        let consumer = InstalledComponent {
            component_id: "consumer.runtime".to_owned(),
            revision: 1,
            package_digest: DIGEST_B.to_owned(),
        };
        let mut target_manifest = manifest(b"target");
        target_manifest.package_digest = target.package_digest.clone();
        let mut consumer_manifest = manifest(b"consumer");
        consumer_manifest.component_id = consumer.component_id.clone();
        consumer_manifest.services = vec!["consumerd".to_owned()];
        consumer_manifest.capabilities = vec!["consumer.factory".to_owned()];
        consumer_manifest.revision = consumer.revision;
        consumer_manifest.package_digest = consumer.package_digest.clone();
        consumer_manifest.dependencies = vec![ComponentDependency {
            component_id: target.component_id.clone(),
            minimum_revision: 1,
            exact_package_digest: Some(target.package_digest.clone()),
        }];
        let package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/new-component.primepkg"),
            package_digest: DIGEST_B.to_owned(),
            component_id: target.component_id.clone(),
            revision: 3,
            version: "3.0.0".to_owned(),
            publisher_id: "thetechguy.origins".to_owned(),
        };
        let mut candidate_manifest = target_manifest.clone();
        candidate_manifest.revision = package.revision;
        candidate_manifest.version = package.version.clone();
        candidate_manifest.package_digest = package.package_digest.clone();
        let installed = [target.clone(), consumer.clone()];
        assert!(matches!(
            plan_component_transaction(&package, &installed)
                .unwrap()
                .kind,
            ComponentTransactionKind::Update { .. }
        ));
        assert!(matches!(
            plan_component_transaction_checked(
                &package, &installed, &[target_manifest.clone(), consumer_manifest.clone()],
                &candidate_manifest,
            ),
            Err(ComponentAdmissionError::DependentDigestPinned { component_id, dependent_id })
                if component_id == target.component_id && dependent_id == consumer.component_id
        ));
        consumer_manifest.dependencies[0].exact_package_digest = None;
        assert!(matches!(
            plan_component_transaction_checked(
                &package,
                &installed,
                &[target_manifest.clone(), consumer_manifest.clone()],
                &candidate_manifest,
            )
            .unwrap()
            .kind,
            ComponentTransactionKind::Update { .. }
        ));
        assert!(matches!(
            plan_component_transaction_checked(&package, &installed, &[target_manifest.clone()], &candidate_manifest),
            Err(ComponentAdmissionError::MissingInstalledManifest(id)) if id == consumer.component_id
        ));
        consumer_manifest.package_digest = DIGEST_A.to_owned();
        assert!(matches!(
            plan_component_transaction_checked(
                &package, &installed, &[target_manifest, consumer_manifest],
                &candidate_manifest,
            ),
            Err(ComponentAdmissionError::InstalledManifestMismatch(id)) if id == consumer.component_id
        ));
    }

    #[test]
    fn checked_plans_reject_cross_component_registration_ownership_aliases() {
        let first = InstalledComponent {
            component_id: "origins.runtime".to_owned(),
            revision: 2,
            package_digest: DIGEST_A.to_owned(),
        };
        let second = InstalledComponent {
            component_id: "consumer.runtime".to_owned(),
            revision: 1,
            package_digest: DIGEST_B.to_owned(),
        };
        let mut first_manifest = manifest(b"origins");
        first_manifest.package_digest = first.package_digest.clone();
        let mut second_manifest = manifest(b"consumer");
        second_manifest.component_id = second.component_id.clone();
        second_manifest.revision = second.revision;
        second_manifest.package_digest = second.package_digest.clone();
        second_manifest.services = vec!["consumerd".to_owned()];
        second_manifest.capabilities = vec!["consumer.factory".to_owned()];
        let installed = [first.clone(), second.clone()];
        assert!(index_installed_manifests(
            &installed,
            &[first_manifest.clone(), second_manifest.clone()],
        )
        .is_ok());

        for family in 0..3 {
            let mut conflicting = second_manifest.clone();
            match family {
                0 => conflicting.services = first_manifest.services.clone(),
                1 => conflicting.capabilities = first_manifest.capabilities.clone(),
                _ => conflicting.application_profiles = first_manifest.services.clone(),
            }
            assert!(
                index_installed_manifests(&installed, &[first_manifest.clone(), conflicting])
                    .is_err(),
                "distinct components must not claim the same registration (family {family})"
            );
        }

        let package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/new-component.primepkg"),
            package_digest: DIGEST_B.to_owned(),
            component_id: "new.runtime".to_owned(),
            revision: 1,
            version: "1.0.0".to_owned(),
            publisher_id: "thetechguy.origins".to_owned(),
        };
        let mut candidate = first_manifest.clone();
        candidate.component_id = package.component_id.clone();
        candidate.revision = package.revision;
        candidate.version = package.version.clone();
        candidate.package_digest = package.package_digest.clone();
        assert!(
            plan_component_transaction_checked(
                &package,
                &installed,
                &[first_manifest.clone(), second_manifest.clone()],
                &candidate
            )
            .is_err(),
            "checked install must reject an already-owned service and capability"
        );

        candidate.services = vec!["newd".to_owned()];
        candidate.capabilities = vec!["new.factory".to_owned()];
        assert!(
            plan_component_transaction_checked(
                &package,
                &installed,
                &[first_manifest.clone(), second_manifest.clone()],
                &candidate,
            )
            .is_ok(),
            "distinct registrations remain admissible"
        );

        let update_package = VerifiedComponentPackage {
            component_id: first.component_id.clone(),
            revision: 3,
            version: "3.0.0".to_owned(),
            ..package.clone()
        };
        let mut update_manifest = first_manifest.clone();
        update_manifest.revision = update_package.revision;
        update_manifest.version = update_package.version.clone();
        update_manifest.package_digest = update_package.package_digest.clone();
        update_manifest.services = second_manifest.services.clone();
        assert!(
            plan_component_transaction_checked(
                &update_package,
                &installed,
                &[first_manifest.clone(), second_manifest.clone()],
                &update_manifest,
            )
            .is_err(),
            "checked update cannot claim a service owned by another installed component"
        );

        let retained = InstalledComponent {
            component_id: first.component_id.clone(),
            revision: 1,
            package_digest: DIGEST_B.to_owned(),
        };
        let mut retained_manifest = first_manifest.clone();
        retained_manifest.revision = retained.revision;
        retained_manifest.package_digest = retained.package_digest.clone();
        retained_manifest.services = second_manifest.services.clone();
        retained_manifest.capabilities = vec!["origins.factory".to_owned()];
        assert!(
            plan_component_rollback_checked(
                &first,
                &retained,
                &retained_manifest,
                &installed,
                &[first_manifest, second_manifest],
            )
            .is_err(),
            "checked rollback must not transfer another component's service identity"
        );
    }

    #[test]
    fn checked_update_rejects_cycle_from_authenticated_candidate() {
        let current = InstalledComponent {
            component_id: "origins.runtime".to_owned(),
            revision: 2,
            package_digest: DIGEST_A.to_owned(),
        };
        let consumer = InstalledComponent {
            component_id: "consumer.runtime".to_owned(),
            revision: 1,
            package_digest: DIGEST_B.to_owned(),
        };
        let mut current_manifest = manifest(b"current");
        current_manifest.package_digest = current.package_digest.clone();
        let mut consumer_manifest = manifest(b"consumer");
        consumer_manifest.component_id = consumer.component_id.clone();
        consumer_manifest.services = vec!["consumerd".to_owned()];
        consumer_manifest.capabilities = vec!["consumer.factory".to_owned()];
        consumer_manifest.revision = consumer.revision;
        consumer_manifest.package_digest = consumer.package_digest.clone();
        consumer_manifest.dependencies = vec![ComponentDependency {
            component_id: current.component_id.clone(),
            minimum_revision: 1,
            exact_package_digest: None,
        }];
        let installed = [current.clone(), consumer.clone()];
        let manifests = [current_manifest.clone(), consumer_manifest];
        assert!(index_installed_manifests(&installed, &manifests).is_ok());
        let package = VerifiedComponentPackage {
            package_path: PathBuf::from("/tmp/new-component.primepkg"),
            package_digest: DIGEST_B.to_owned(),
            component_id: current.component_id.clone(),
            revision: 3,
            version: "3.0.0".to_owned(),
            publisher_id: "thetechguy.origins".to_owned(),
        };
        let mut candidate = current_manifest;
        candidate.revision = package.revision;
        candidate.version = package.version.clone();
        candidate.package_digest = package.package_digest.clone();
        candidate.dependencies = vec![ComponentDependency {
            component_id: consumer.component_id.clone(),
            minimum_revision: 1,
            exact_package_digest: None,
        }];
        // A <- B is valid, but updating A to depend on B creates A <-> B.
        assert!(plan_component_transaction(&package, &installed).is_ok());
        assert!(matches!(
            plan_component_transaction_checked(&package, &installed, &manifests, &candidate),
            Err(ComponentAdmissionError::InstalledDependencyCycle(_))
        ));
        candidate.dependencies.clear();
        assert!(matches!(
            plan_component_transaction_checked(&package, &installed, &manifests, &candidate)
                .unwrap()
                .kind,
            ComponentTransactionKind::Update { .. }
        ));
        for field in 0..4 {
            let mut forged = candidate.clone();
            match field {
                0 => forged.revision += 1,
                1 => forged.package_digest = DIGEST_A.to_owned(),
                2 => forged.version = "4.0.0".to_owned(),
                _ => forged.publisher_id = "thetechguy.other".to_owned(),
            }
            assert!(matches!(
                plan_component_transaction_checked(&package, &installed, &manifests, &forged),
                Err(ComponentAdmissionError::CandidateManifestMismatch(_))
            ));
        }
        candidate.dependencies = vec![ComponentDependency {
            component_id: "missing.runtime".to_owned(),
            minimum_revision: 1,
            exact_package_digest: None,
        }];
        assert!(matches!(
            plan_component_transaction_checked(&package, &installed, &manifests, &candidate),
            Err(ComponentAdmissionError::DependencyMissing(_))
        ));
    }

    #[test]
    fn checked_rollback_rejects_cycle_introduced_by_retained_manifest() {
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
        let consumer = InstalledComponent {
            component_id: "consumer.runtime".to_owned(),
            revision: 1,
            package_digest: DIGEST_A.to_owned(),
        };
        let mut current_manifest = manifest(b"current");
        current_manifest.revision = current.revision;
        current_manifest.package_digest = current.package_digest.clone();
        let mut retained_manifest = current_manifest.clone();
        retained_manifest.revision = retained.revision;
        retained_manifest.package_digest = retained.package_digest.clone();
        retained_manifest.dependencies = vec![ComponentDependency {
            component_id: consumer.component_id.clone(),
            minimum_revision: 1,
            exact_package_digest: None,
        }];
        let mut consumer_manifest = manifest(b"consumer");
        consumer_manifest.component_id = consumer.component_id.clone();
        consumer_manifest.services = vec!["consumerd".to_owned()];
        consumer_manifest.capabilities = vec!["consumer.factory".to_owned()];
        consumer_manifest.revision = consumer.revision;
        consumer_manifest.package_digest = consumer.package_digest.clone();
        consumer_manifest.dependencies = vec![ComponentDependency {
            component_id: current.component_id.clone(),
            minimum_revision: 1,
            exact_package_digest: None,
        }];
        let installed = [current.clone(), consumer.clone()];
        let manifests = [current_manifest, consumer_manifest];
        assert!(index_installed_manifests(&installed, &manifests).is_ok());
        assert!(matches!(
            plan_component_rollback_checked(
                &current,
                &retained,
                &retained_manifest,
                &installed,
                &manifests
            ),
            Err(ComponentAdmissionError::InstalledDependencyCycle(_))
        ));
        retained_manifest.dependencies.clear();
        assert!(plan_component_rollback_checked(
            &current,
            &retained,
            &retained_manifest,
            &installed,
            &manifests
        )
        .is_ok());
        retained_manifest.package_digest = current.package_digest.clone();
        assert!(matches!(
            plan_component_rollback_checked(
                &current,
                &retained,
                &retained_manifest,
                &installed,
                &manifests
            ),
            Err(ComponentAdmissionError::InstalledManifestMismatch(_))
        ));
    }

    #[test]
    fn checked_rollback_rejects_cyclic_installed_dependency_inventory() {
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
        let consumer = InstalledComponent {
            component_id: "consumer.runtime".to_owned(),
            revision: 1,
            package_digest: DIGEST_A.to_owned(),
        };
        let mut target_manifest = manifest(b"target");
        target_manifest.revision = current.revision;
        target_manifest.package_digest = current.package_digest.clone();
        target_manifest.dependencies = vec![ComponentDependency {
            component_id: consumer.component_id.clone(),
            minimum_revision: 1,
            exact_package_digest: None,
        }];
        let mut retained_manifest = target_manifest.clone();
        retained_manifest.revision = retained.revision;
        retained_manifest.package_digest = retained.package_digest.clone();
        let mut consumer_manifest = manifest(b"consumer");
        consumer_manifest.component_id = consumer.component_id.clone();
        consumer_manifest.services = vec!["consumerd".to_owned()];
        consumer_manifest.capabilities = vec!["consumer.factory".to_owned()];
        consumer_manifest.revision = consumer.revision;
        consumer_manifest.package_digest = consumer.package_digest.clone();
        consumer_manifest.dependencies = vec![ComponentDependency {
            component_id: current.component_id.clone(),
            minimum_revision: 1,
            exact_package_digest: None,
        }];
        assert!(matches!(
            plan_component_rollback_checked(
                &current,
                &retained,
                &retained_manifest,
                &[current.clone(), consumer],
                &[target_manifest, consumer_manifest],
            ),
            Err(ComponentAdmissionError::InstalledDependencyCycle(_))
        ));
    }

    #[test]
    fn checked_rollback_rejects_inconsistent_installed_dependency_closure() {
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
        let consumer = InstalledComponent {
            component_id: "consumer.runtime".to_owned(),
            revision: 1,
            package_digest: DIGEST_A.to_owned(),
        };
        let installed = [current.clone(), consumer.clone()];
        let mut target_manifest = manifest(b"target");
        target_manifest.revision = current.revision;
        target_manifest.package_digest = current.package_digest.clone();
        let mut retained_manifest = target_manifest.clone();
        retained_manifest.revision = retained.revision;
        retained_manifest.package_digest = retained.package_digest.clone();
        let mut consumer_manifest = manifest(b"consumer");
        consumer_manifest.component_id = consumer.component_id.clone();
        consumer_manifest.services = vec!["consumerd".to_owned()];
        consumer_manifest.capabilities = vec!["consumer.factory".to_owned()];
        consumer_manifest.revision = consumer.revision;
        consumer_manifest.package_digest = consumer.package_digest.clone();
        consumer_manifest.dependencies = vec![ComponentDependency {
            component_id: "missing.runtime".to_owned(),
            minimum_revision: 1,
            exact_package_digest: None,
        }];
        let check = |consumer_manifest: &PrimeComponentManifest| {
            plan_component_rollback_checked(
                &current,
                &retained,
                &retained_manifest,
                &installed,
                &[target_manifest.clone(), consumer_manifest.clone()],
            )
        };
        assert!(matches!(
            check(&consumer_manifest),
            Err(ComponentAdmissionError::DependencyMissing(id)) if id == "missing.runtime"
        ));
        consumer_manifest.dependencies[0].component_id = current.component_id.clone();
        consumer_manifest.dependencies[0].minimum_revision = 4;
        assert!(matches!(
            check(&consumer_manifest),
            Err(ComponentAdmissionError::DependencyRevision { component_id, minimum_revision: 4 })
                if component_id == current.component_id
        ));
        consumer_manifest.dependencies[0].minimum_revision = 3;
        consumer_manifest.dependencies[0].exact_package_digest = Some(DIGEST_A.to_owned());
        assert!(matches!(
            check(&consumer_manifest),
            Err(ComponentAdmissionError::DependencyDigest(id)) if id == current.component_id
        ));
        consumer_manifest.dependencies[0].exact_package_digest = None;
        assert!(matches!(
            check(&consumer_manifest),
            Err(ComponentAdmissionError::DependentRevisionFloor { .. })
        ));
    }

    #[test]
    fn checked_rollback_respects_installed_dependents_and_exact_inventory() {
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
        let consumer = InstalledComponent {
            component_id: "consumer.runtime".to_owned(),
            revision: 1,
            package_digest: DIGEST_A.to_owned(),
        };
        let installed = [current.clone(), consumer.clone()];
        let mut target_manifest = manifest(b"target");
        target_manifest.revision = current.revision;
        target_manifest.package_digest = current.package_digest.clone();
        let mut retained_manifest = target_manifest.clone();
        retained_manifest.revision = retained.revision;
        retained_manifest.package_digest = retained.package_digest.clone();
        let mut consumer_manifest = manifest(b"consumer");
        consumer_manifest.component_id = consumer.component_id.clone();
        consumer_manifest.services = vec!["consumerd".to_owned()];
        consumer_manifest.capabilities = vec!["consumer.factory".to_owned()];
        consumer_manifest.revision = consumer.revision;
        consumer_manifest.package_digest = consumer.package_digest.clone();
        consumer_manifest.dependencies = vec![ComponentDependency {
            component_id: current.component_id.clone(),
            minimum_revision: 3,
            exact_package_digest: None,
        }];
        let manifests = [target_manifest.clone(), consumer_manifest.clone()];
        assert!(matches!(
            plan_component_rollback_checked(&current, &retained, &retained_manifest, &installed, &manifests),
            Err(ComponentAdmissionError::DependentRevisionFloor {
                component_id, dependent_id, minimum_revision: 3
            }) if component_id == current.component_id && dependent_id == consumer.component_id
        ));

        consumer_manifest.dependencies[0].minimum_revision = 2;
        consumer_manifest.dependencies[0].exact_package_digest =
            Some(current.package_digest.clone());
        assert!(matches!(
            plan_component_rollback_checked(
                &current, &retained, &retained_manifest, &installed, &[target_manifest.clone(), consumer_manifest.clone()]
            ),
            Err(ComponentAdmissionError::DependentDigestPinned { component_id, dependent_id })
                if component_id == current.component_id && dependent_id == consumer.component_id
        ));

        consumer_manifest.dependencies[0].exact_package_digest = None;
        let plan = plan_component_rollback_checked(
            &current,
            &retained,
            &retained_manifest,
            &installed,
            &[target_manifest.clone(), consumer_manifest.clone()],
        )
        .unwrap();
        assert!(matches!(
            plan.kind,
            ComponentTransactionKind::Rollback { .. }
        ));
        assert_eq!(plan.target_revision, 2);
        assert_eq!(plan.target_package_digest, DIGEST_A);

        assert!(matches!(
            plan_component_rollback_checked(&current, &retained, &retained_manifest, &installed, &[target_manifest.clone()]),
            Err(ComponentAdmissionError::MissingInstalledManifest(id)) if id == consumer.component_id
        ));
        let mut forged_current = current.clone();
        forged_current.package_digest = format!("sha256:{}", "c".repeat(64));
        assert!(matches!(
            plan_component_rollback_checked(
                &forged_current, &retained, &retained_manifest, &installed, &[target_manifest, consumer_manifest]
            ),
            Err(ComponentAdmissionError::InstalledManifestMismatch(id)) if id == current.component_id
        ));
    }

    #[test]
    fn checked_registration_order_is_dependency_first_and_stable() {
        let make = |id: &str, dependencies: &[&str]| {
            let mut item = manifest(id.as_bytes());
            item.component_id = id.to_owned();
            item.services = vec![format!("{id}.service")];
            item.capabilities = vec![format!("{id}.capability")];
            item.dependencies = dependencies
                .iter()
                .map(|dependency| ComponentDependency {
                    component_id: (*dependency).to_owned(),
                    minimum_revision: 1,
                    exact_package_digest: None,
                })
                .collect();
            item
        };
        let origins = make("origins.runtime", &[]);
        let provider = make("provider.runtime", &["origins.runtime"]);
        let consumer = make("consumer.runtime", &["provider.runtime"]);
        let alpha = make("alpha.runtime", &[]);
        let manifests = vec![consumer, alpha, provider, origins];
        let installed: Vec<_> = manifests
            .iter()
            .map(|item| InstalledComponent {
                component_id: item.component_id.clone(),
                revision: item.revision,
                package_digest: item.package_digest.clone(),
            })
            .collect();
        let expected = vec![
            "alpha.runtime".to_owned(),
            "origins.runtime".to_owned(),
            "provider.runtime".to_owned(),
            "consumer.runtime".to_owned(),
        ];
        assert_eq!(
            plan_component_registration_order_checked(&installed, &manifests).unwrap(),
            expected
        );
        assert_eq!(
            plan_component_unregistration_order_checked(&installed, &manifests).unwrap(),
            vec![
                "consumer.runtime",
                "provider.runtime",
                "origins.runtime",
                "alpha.runtime"
            ],
            "dependents must unregister before their dependencies"
        );
        let mut reversed_installed = installed.clone();
        reversed_installed.reverse();
        let mut reversed_manifests = manifests.clone();
        reversed_manifests.reverse();
        assert_eq!(
            plan_component_registration_order_checked(&reversed_installed, &reversed_manifests,)
                .unwrap(),
            expected,
            "source inventory order must not alter dependency registration order"
        );
        assert_eq!(
            plan_component_unregistration_order_checked(&reversed_installed, &reversed_manifests)
                .unwrap(),
            vec![
                "consumer.runtime",
                "provider.runtime",
                "origins.runtime",
                "alpha.runtime"
            ],
            "source inventory order must not alter dependency unregistration order"
        );
    }

    #[test]
    fn checked_subset_unregistration_preserves_unselected_dependents() {
        let make = |id: &str, dependencies: &[&str]| {
            let mut item = manifest(id.as_bytes());
            item.component_id = id.to_owned();
            item.services = vec![format!("{id}.service")];
            item.capabilities = vec![format!("{id}.capability")];
            item.dependencies = dependencies
                .iter()
                .map(|dependency| ComponentDependency {
                    component_id: (*dependency).to_owned(),
                    minimum_revision: 1,
                    exact_package_digest: None,
                })
                .collect();
            item
        };
        let manifests = vec![
            make("consumer.runtime", &["provider.runtime"]),
            make("alpha.runtime", &[]),
            make("provider.runtime", &["origins.runtime"]),
            make("origins.runtime", &[]),
        ];
        let installed: Vec<_> = manifests
            .iter()
            .map(|item| InstalledComponent {
                component_id: item.component_id.clone(),
                revision: item.revision,
                package_digest: item.package_digest.clone(),
            })
            .collect();

        assert_eq!(
            plan_component_unregistration_subset_checked(
                &["provider.runtime", "consumer.runtime"],
                &installed,
                &manifests
            )
            .unwrap(),
            vec!["consumer.runtime", "provider.runtime"]
        );
        let mut shuffled = manifests.clone();
        shuffled.reverse();
        assert_eq!(
            plan_component_unregistration_subset_checked(
                &["consumer.runtime", "provider.runtime"],
                &installed,
                &shuffled
            )
            .unwrap(),
            vec!["consumer.runtime", "provider.runtime"]
        );
        assert!(matches!(
            plan_component_unregistration_subset_checked(
                &["provider.runtime"], &installed, &manifests
            ),
            Err(ComponentAdmissionError::DependentComponentInstalled { component_id, dependent_id })
                if component_id == "provider.runtime" && dependent_id == "consumer.runtime"
        ));
        assert!(matches!(
            plan_component_unregistration_subset_checked(
                &["origins.runtime", "consumer.runtime"], &installed, &manifests
            ),
            Err(ComponentAdmissionError::DependentComponentInstalled { component_id, dependent_id })
                if component_id == "origins.runtime" && dependent_id == "provider.runtime"
        ));
        assert!(matches!(
            plan_component_unregistration_subset_checked(
                &["provider.runtime", "provider.runtime"], &installed, &manifests
            ),
            Err(ComponentAdmissionError::DuplicateUnregistrationTarget(id))
                if id == "provider.runtime"
        ));
        assert!(matches!(
            plan_component_unregistration_subset_checked(
                &["missing.runtime"], &installed, &manifests
            ),
            Err(ComponentAdmissionError::DependencyMissing(id))
                if id == "missing.runtime"
        ));
        assert_eq!(
            plan_component_unregistration_subset_checked(&[], &installed, &manifests).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn checked_subset_registration_includes_transitive_prerequisites() {
        let make = |id: &str, dependencies: &[&str]| {
            let mut item = manifest(id.as_bytes());
            item.component_id = id.to_owned();
            item.services = vec![format!("{id}.service")];
            item.capabilities = vec![format!("{id}.capability")];
            item.dependencies = dependencies
                .iter()
                .map(|dependency| ComponentDependency {
                    component_id: (*dependency).to_owned(),
                    minimum_revision: 1,
                    exact_package_digest: None,
                })
                .collect();
            item
        };
        let manifests = vec![
            make("consumer.runtime", &["provider.runtime"]),
            make("unrelated.runtime", &[]),
            make("provider.runtime", &["origins.runtime"]),
            make("origins.runtime", &[]),
        ];
        let installed: Vec<_> = manifests
            .iter()
            .map(|item| InstalledComponent {
                component_id: item.component_id.clone(),
                revision: item.revision,
                package_digest: item.package_digest.clone(),
            })
            .collect();
        let expected = vec!["origins.runtime", "provider.runtime", "consumer.runtime"];
        assert_eq!(
            plan_component_registration_subset_checked(
                &["consumer.runtime"],
                &installed,
                &manifests
            )
            .unwrap(),
            expected
        );
        let mut shuffled = manifests.clone();
        shuffled.reverse();
        assert_eq!(
            plan_component_registration_subset_checked(
                &["provider.runtime", "consumer.runtime"],
                &installed,
                &shuffled
            )
            .unwrap(),
            expected
        );
        assert_eq!(
            plan_component_registration_subset_checked(&[], &installed, &manifests).unwrap(),
            Vec::<String>::new()
        );
        assert!(matches!(
            plan_component_registration_subset_checked(
                &["provider.runtime", "provider.runtime"], &installed, &manifests
            ),
            Err(ComponentAdmissionError::DuplicateRegistrationTarget(id))
                if id == "provider.runtime"
        ));
        assert!(matches!(
            plan_component_registration_subset_checked(
                &["missing.runtime"], &installed, &manifests
            ),
            Err(ComponentAdmissionError::DependencyMissing(id))
                if id == "missing.runtime"
        ));
        assert!(matches!(
            plan_component_registration_subset_checked(
                &["Not Canonical"], &installed, &manifests
            ),
            Err(ComponentAdmissionError::InvalidTargetComponentId(id))
                if id == "Not Canonical"
        ));
        assert!(matches!(
            plan_component_registration_subset_checked(
                &[], &installed, &manifests[..3]
            ),
            Err(ComponentAdmissionError::MissingInstalledManifest(id))
                if id == "origins.runtime"
        ));
    }

    #[test]
    fn explicit_registration_scope_requires_exact_prerequisite_closure() {
        let make = |id: &str, dependencies: &[&str]| {
            let mut item = manifest(id.as_bytes());
            item.component_id = id.to_owned();
            item.services = vec![format!("{id}.service")];
            item.capabilities = vec![format!("{id}.capability")];
            item.dependencies = dependencies
                .iter()
                .map(|dependency| ComponentDependency {
                    component_id: (*dependency).to_owned(),
                    minimum_revision: 1,
                    exact_package_digest: None,
                })
                .collect();
            item
        };
        let manifests = vec![
            make("leaf.runtime", &["left.runtime", "right.runtime"]),
            make("unrelated.runtime", &[]),
            make("right.runtime", &["origins.runtime"]),
            make("origins.runtime", &[]),
            make("left.runtime", &["origins.runtime"]),
        ];
        let installed: Vec<_> = manifests
            .iter()
            .map(|item| InstalledComponent {
                component_id: item.component_id.clone(),
                revision: item.revision,
                package_digest: item.package_digest.clone(),
            })
            .collect();
        let exact = [
            "leaf.runtime",
            "left.runtime",
            "right.runtime",
            "origins.runtime",
        ];
        let expected = vec![
            "origins.runtime",
            "left.runtime",
            "right.runtime",
            "leaf.runtime",
        ];
        assert_eq!(
            plan_component_registration_explicit_scope_checked(
                &["leaf.runtime"],
                &exact,
                &installed,
                &manifests
            )
            .unwrap(),
            expected
        );
        let mut shuffled = manifests.clone();
        shuffled.reverse();
        assert_eq!(
            plan_component_registration_explicit_scope_checked(
                &["leaf.runtime"],
                &[
                    "origins.runtime",
                    "right.runtime",
                    "left.runtime",
                    "leaf.runtime"
                ],
                &installed,
                &shuffled
            )
            .unwrap(),
            expected
        );
        for scope in [
            &exact[..3],
            &[
                "leaf.runtime",
                "left.runtime",
                "right.runtime",
                "origins.runtime",
                "unrelated.runtime",
            ][..],
        ] {
            assert!(matches!(
                plan_component_registration_explicit_scope_checked(
                    &["leaf.runtime"],
                    scope,
                    &installed,
                    &manifests
                ),
                Err(ComponentAdmissionError::RegistrationScopeMismatch(_))
            ));
        }
        assert!(matches!(
            plan_component_registration_explicit_scope_checked(
                &["leaf.runtime"],
                &["leaf.runtime", "leaf.runtime"],
                &installed,
                &manifests
            ),
            Err(ComponentAdmissionError::DuplicateRegistrationTarget(_))
        ));
        assert!(matches!(
            plan_component_registration_explicit_scope_checked(
                &["leaf.runtime"],
                &["leaf.runtime", "missing.runtime"],
                &installed,
                &manifests
            ),
            Err(ComponentAdmissionError::DependencyMissing(_))
        ));
        assert!(matches!(
            plan_component_registration_explicit_scope_checked(
                &["leaf.runtime"],
                &["Not Canonical"],
                &installed,
                &manifests
            ),
            Err(ComponentAdmissionError::InvalidTargetComponentId(_))
        ));
        assert_eq!(
            plan_component_registration_explicit_scope_checked(&[], &[], &installed, &manifests)
                .unwrap(),
            Vec::<String>::new()
        );
        assert!(matches!(
            plan_component_registration_explicit_scope_checked(
                &[],
                &["unrelated.runtime"],
                &installed,
                &manifests
            ),
            Err(ComponentAdmissionError::RegistrationScopeMismatch(_))
        ));
        assert!(matches!(
            plan_component_registration_explicit_scope_checked(
                &[],
                &[],
                &installed,
                &manifests[..4]
            ),
            Err(ComponentAdmissionError::MissingInstalledManifest(_))
        ));
    }

    #[test]
    fn checked_dependent_closure_unregistration_expands_consumers_only() {
        let make = |id: &str, dependencies: &[&str]| {
            let mut item = manifest(id.as_bytes());
            item.component_id = id.to_owned();
            item.services = vec![format!("{id}.service")];
            item.capabilities = vec![format!("{id}.capability")];
            item.dependencies = dependencies
                .iter()
                .map(|dependency| ComponentDependency {
                    component_id: (*dependency).to_owned(),
                    minimum_revision: 1,
                    exact_package_digest: None,
                })
                .collect();
            item
        };
        let manifests = vec![
            make("consumer.runtime", &["provider.runtime"]),
            make("unrelated.runtime", &[]),
            make("provider.runtime", &["origins.runtime"]),
            make("other.consumer", &["origins.runtime"]),
            make("origins.runtime", &[]),
        ];
        let installed: Vec<_> = manifests
            .iter()
            .map(|item| InstalledComponent {
                component_id: item.component_id.clone(),
                revision: item.revision,
                package_digest: item.package_digest.clone(),
            })
            .collect();
        assert_eq!(
            plan_component_unregistration_dependent_closure_checked(
                &["origins.runtime"],
                &installed,
                &manifests
            )
            .unwrap(),
            vec![
                "consumer.runtime",
                "provider.runtime",
                "other.consumer",
                "origins.runtime"
            ]
        );
        let mut shuffled = manifests.clone();
        shuffled.reverse();
        assert_eq!(
            plan_component_unregistration_dependent_closure_checked(
                &["provider.runtime"],
                &installed,
                &shuffled
            )
            .unwrap(),
            vec!["consumer.runtime", "provider.runtime"]
        );
        assert_eq!(
            plan_component_unregistration_dependent_closure_checked(&[], &installed, &manifests)
                .unwrap(),
            Vec::<String>::new()
        );
        assert!(matches!(
            plan_component_unregistration_dependent_closure_checked(
                &["provider.runtime", "provider.runtime"], &installed, &manifests
            ),
            Err(ComponentAdmissionError::DuplicateUnregistrationTarget(id))
                if id == "provider.runtime"
        ));
        assert!(matches!(
            plan_component_unregistration_dependent_closure_checked(
                &["missing.runtime"], &installed, &manifests
            ),
            Err(ComponentAdmissionError::DependencyMissing(id))
                if id == "missing.runtime"
        ));
        assert!(matches!(
            plan_component_unregistration_dependent_closure_checked(
                &["Not Canonical"], &installed, &manifests
            ),
            Err(ComponentAdmissionError::InvalidTargetComponentId(id))
                if id == "Not Canonical"
        ));
        assert!(matches!(
            plan_component_unregistration_dependent_closure_checked(
                &[], &installed, &manifests[..4]
            ),
            Err(ComponentAdmissionError::MissingInstalledManifest(id))
                if id == "origins.runtime"
        ));
    }

    #[test]
    fn explicit_unregistration_scope_requires_exact_dependent_closure() {
        let make = |id: &str, dependencies: &[&str]| {
            let mut item = manifest(id.as_bytes());
            item.component_id = id.to_owned();
            item.services = vec![format!("{id}.service")];
            item.capabilities = vec![format!("{id}.capability")];
            item.dependencies = dependencies
                .iter()
                .map(|dependency| ComponentDependency {
                    component_id: (*dependency).to_owned(),
                    minimum_revision: 1,
                    exact_package_digest: None,
                })
                .collect();
            item
        };
        let manifests = vec![
            make("leaf.runtime", &["left.runtime", "right.runtime"]),
            make("unrelated.runtime", &[]),
            make("right.runtime", &["origins.runtime"]),
            make("origins.runtime", &[]),
            make("left.runtime", &["origins.runtime"]),
        ];
        let installed: Vec<_> = manifests
            .iter()
            .map(|item| InstalledComponent {
                component_id: item.component_id.clone(),
                revision: item.revision,
                package_digest: item.package_digest.clone(),
            })
            .collect();
        let exact = [
            "origins.runtime",
            "left.runtime",
            "right.runtime",
            "leaf.runtime",
        ];
        let expected = vec![
            "leaf.runtime",
            "right.runtime",
            "left.runtime",
            "origins.runtime",
        ];
        assert_eq!(
            plan_component_unregistration_explicit_scope_checked(
                &["origins.runtime"],
                &exact,
                &installed,
                &manifests
            )
            .unwrap(),
            expected
        );
        let mut shuffled = manifests.clone();
        shuffled.reverse();
        assert_eq!(
            plan_component_unregistration_explicit_scope_checked(
                &["origins.runtime"],
                &[
                    "leaf.runtime",
                    "right.runtime",
                    "left.runtime",
                    "origins.runtime"
                ],
                &installed,
                &shuffled
            )
            .unwrap(),
            expected
        );
        for scope in [
            &exact[..3],
            &[
                "origins.runtime",
                "left.runtime",
                "right.runtime",
                "leaf.runtime",
                "unrelated.runtime",
            ][..],
        ] {
            assert!(matches!(
                plan_component_unregistration_explicit_scope_checked(
                    &["origins.runtime"],
                    scope,
                    &installed,
                    &manifests
                ),
                Err(ComponentAdmissionError::UnregistrationScopeMismatch(_))
            ));
        }
        assert!(matches!(
            plan_component_unregistration_explicit_scope_checked(
                &["origins.runtime"],
                &["origins.runtime", "origins.runtime"],
                &installed,
                &manifests
            ),
            Err(ComponentAdmissionError::DuplicateUnregistrationTarget(_))
        ));
        assert!(matches!(
            plan_component_unregistration_explicit_scope_checked(
                &["origins.runtime"],
                &["origins.runtime", "missing.runtime"],
                &installed,
                &manifests
            ),
            Err(ComponentAdmissionError::DependencyMissing(_))
        ));
        assert_eq!(
            plan_component_unregistration_explicit_scope_checked(&[], &[], &installed, &manifests)
                .unwrap(),
            Vec::<String>::new()
        );
        assert!(matches!(
            plan_component_unregistration_explicit_scope_checked(
                &[],
                &["unrelated.runtime"],
                &installed,
                &manifests
            ),
            Err(ComponentAdmissionError::UnregistrationScopeMismatch(_))
        ));
    }

    #[test]
    fn explicit_scopes_reject_unrelated_inventory_corruption_even_when_empty() {
        // Explicit scopes are not a shortcut around complete installed-inventory
        // validation. A damaged unrelated package must invalidate both plans,
        // including an empty declaration that would otherwise produce no steps.
        let mut anchor = manifest(b"scope-anchor");
        anchor.component_id = "anchor.runtime".to_owned();
        anchor.services = vec!["anchor.service".to_owned()];
        anchor.capabilities = vec!["anchor.capability".to_owned()];
        let mut unrelated = manifest(b"scope-unrelated");
        unrelated.component_id = "unrelated.runtime".to_owned();
        unrelated.services = vec!["unrelated.service".to_owned()];
        unrelated.capabilities = vec!["unrelated.capability".to_owned()];
        let manifests = vec![anchor, unrelated];
        let installed: Vec<_> = manifests
            .iter()
            .map(|item| InstalledComponent {
                component_id: item.component_id.clone(),
                revision: item.revision,
                package_digest: item.package_digest.clone(),
            })
            .collect();

        let mut rebounded = installed.clone();
        rebounded[1].package_digest = manifest(b"other-package").package_digest;
        for selected in [&[][..], &["anchor.runtime"][..]] {
            let scope = selected;
            assert!(matches!(
                plan_component_registration_explicit_scope_checked(
                    selected, scope, &rebounded, &manifests
                ),
                Err(ComponentAdmissionError::InstalledManifestMismatch(id))
                    if id == "unrelated.runtime"
            ));
            assert!(matches!(
                plan_component_unregistration_explicit_scope_checked(
                    selected, scope, &rebounded, &manifests
                ),
                Err(ComponentAdmissionError::InstalledManifestMismatch(id))
                    if id == "unrelated.runtime"
            ));
        }

        let mut conflicting = manifests.clone();
        conflicting[1].services = conflicting[0].services.clone();
        for selected in [&[][..], &["anchor.runtime"][..]] {
            assert!(matches!(
                plan_component_registration_explicit_scope_checked(
                    selected,
                    selected,
                    &installed,
                    &conflicting
                ),
                Err(ComponentAdmissionError::RegistrationIdentityConflict { .. })
            ));
            assert!(matches!(
                plan_component_unregistration_explicit_scope_checked(
                    selected,
                    selected,
                    &installed,
                    &conflicting
                ),
                Err(ComponentAdmissionError::RegistrationIdentityConflict { .. })
            ));
        }
    }

    #[test]
    fn deep_component_scopes_are_dependency_closed_without_recursive_traversal() {
        // Long installed dependency chains must not be truncated, reversed,
        // or treated as authority to register/remove unrelated components.
        // The preview remains source-only; no component state is mutated.
        const DEPTH: usize = 160;
        let mut manifests = Vec::new();
        for index in 0..DEPTH {
            let id = format!("chain.{index:03}");
            let mut item = manifest(id.as_bytes());
            item.component_id = id.clone();
            item.services = vec![format!("{id}.service")];
            item.capabilities = vec![format!("{id}.capability")];
            if index > 0 {
                item.dependencies = vec![ComponentDependency {
                    component_id: format!("chain.{:03}", index - 1),
                    minimum_revision: 1,
                    exact_package_digest: None,
                }];
            }
            manifests.push(item);
        }
        let mut unrelated = manifest(b"unrelated");
        unrelated.component_id = "unrelated.runtime".to_owned();
        unrelated.services = vec!["unrelated.service".to_owned()];
        unrelated.capabilities = vec!["unrelated.capability".to_owned()];
        manifests.push(unrelated);
        let installed: Vec<_> = manifests
            .iter()
            .map(|item| InstalledComponent {
                component_id: item.component_id.clone(),
                revision: item.revision,
                package_digest: item.package_digest.clone(),
            })
            .collect();

        let expected_registration: Vec<_> = (0..DEPTH)
            .map(|index| format!("chain.{index:03}"))
            .collect();
        let registration =
            plan_component_registration_subset_checked(&["chain.159"], &installed, &manifests)
                .unwrap();
        assert_eq!(registration, expected_registration);
        let registration_scope: Vec<_> = registration.iter().map(String::as_str).collect();
        assert_eq!(
            plan_component_registration_explicit_scope_checked(
                &["chain.159"],
                &registration_scope,
                &installed,
                &manifests,
            )
            .unwrap(),
            expected_registration
        );

        let expected_unregistration: Vec<_> = (80..DEPTH)
            .rev()
            .map(|index| format!("chain.{index:03}"))
            .collect();
        let unregistration = plan_component_unregistration_dependent_closure_checked(
            &["chain.080"],
            &installed,
            &manifests,
        )
        .unwrap();
        assert_eq!(unregistration, expected_unregistration);
        let unregistration_scope: Vec<_> = unregistration.iter().map(String::as_str).collect();
        assert_eq!(
            plan_component_unregistration_explicit_scope_checked(
                &["chain.080"],
                &unregistration_scope,
                &installed,
                &manifests,
            )
            .unwrap(),
            expected_unregistration
        );

        // An incomplete declaration must not silently expand into an
        // authorized scope, even when the missing member is far away.
        assert!(matches!(
            plan_component_registration_explicit_scope_checked(
                &["chain.159"],
                &registration_scope[..registration_scope.len() - 1],
                &installed,
                &manifests,
            ),
            Err(ComponentAdmissionError::RegistrationScopeMismatch(_))
        ));
        assert!(matches!(
            plan_component_unregistration_explicit_scope_checked(
                &["chain.080"],
                &unregistration_scope[..unregistration_scope.len() - 1],
                &installed,
                &manifests,
            ),
            Err(ComponentAdmissionError::UnregistrationScopeMismatch(_))
        ));

        manifests.reverse();
        assert_eq!(
            plan_component_registration_subset_checked(&["chain.159"], &installed, &manifests,)
                .unwrap(),
            expected_registration,
            "manifest enumeration order must not change deep prerequisite closure"
        );
    }

    #[test]
    fn checked_registration_order_rejects_incomplete_or_cyclic_inventory() {
        let mut origins = manifest(b"origins");
        origins.dependencies = vec![ComponentDependency {
            component_id: "consumer.runtime".to_owned(),
            minimum_revision: 1,
            exact_package_digest: None,
        }];
        let mut consumer = manifest(b"consumer");
        consumer.component_id = "consumer.runtime".to_owned();
        consumer.services = vec!["consumerd".to_owned()];
        consumer.capabilities = vec!["consumer.factory".to_owned()];
        consumer.dependencies = vec![ComponentDependency {
            component_id: origins.component_id.clone(),
            minimum_revision: 1,
            exact_package_digest: None,
        }];
        let installed: Vec<_> = [&origins, &consumer]
            .iter()
            .map(|item| InstalledComponent {
                component_id: item.component_id.clone(),
                revision: item.revision,
                package_digest: item.package_digest.clone(),
            })
            .collect();
        assert!(matches!(
            plan_component_registration_order_checked(&installed, &[origins.clone()]),
            Err(ComponentAdmissionError::MissingInstalledManifest(id)) if id == consumer.component_id
        ));
        assert!(matches!(
            plan_component_unregistration_order_checked(&installed, &[origins.clone()]),
            Err(ComponentAdmissionError::MissingInstalledManifest(id)) if id == consumer.component_id
        ));
        assert!(matches!(
            plan_component_registration_order_checked(
                &installed,
                &[origins.clone(), consumer.clone()]
            ),
            Err(ComponentAdmissionError::InstalledDependencyCycle(_))
        ));
        assert!(matches!(
            plan_component_unregistration_order_checked(&installed, &[origins, consumer]),
            Err(ComponentAdmissionError::InstalledDependencyCycle(_))
        ));
    }
}
