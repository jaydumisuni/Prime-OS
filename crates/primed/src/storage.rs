use prime_contracts::{
    StorageGenerationAccounting, StorageInventory, StoragePressure, StoragePressureEvidence,
    StoragePressureState, StorageReservePolicy, StorageReserveVisibility, StorageTotals,
    STORAGE_INVENTORY_SCHEMA, STORAGE_PRESSURE_SCHEMA,
};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum StorageStateError {
    #[error("storage state I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("storage state serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersistReport {
    pub previous_cache_corrupt: bool,
    pub pressure_event_written: bool,
}

pub fn observe(
    mountinfo_path: &Path,
    reserve_policy_path: &Path,
    observed_at: String,
    generation_id: String,
) -> StorageInventory {
    let mut policy_limitations = Vec::new();
    let reserve_policy = match prime_storage::load_reserve_policy(reserve_policy_path) {
        Ok(policy) => policy,
        Err(error) => {
            policy_limitations.push(format!("storage reserve policy load failed: {error}"));
            None
        }
    };

    let mut inventory = match prime_storage::probe_host(
        mountinfo_path,
        observed_at.clone(),
        generation_id.clone(),
        reserve_policy.as_ref(),
    ) {
        Ok(inventory) => inventory,
        Err(error) => unavailable_inventory(
            observed_at,
            generation_id,
            reserve_policy.as_ref(),
            error.to_string(),
        ),
    };

    if !policy_limitations.is_empty() {
        inventory.limitations.extend(policy_limitations.clone());
        inventory.reserve.limitations.extend(policy_limitations.clone());
        inventory.pressure.limitations.extend(policy_limitations);
        inventory.limitations.sort();
        inventory.limitations.dedup();
        inventory.reserve.limitations.sort();
        inventory.reserve.limitations.dedup();
        inventory.pressure.limitations.sort();
        inventory.pressure.limitations.dedup();
    }

    inventory
}

pub fn persist_snapshot(
    state_dir: &Path,
    host_id: Uuid,
    generation_id: &str,
    inventory: &StorageInventory,
) -> Result<PersistReport, StorageStateError> {
    let storage_dir = state_dir.join("storage");
    fs::create_dir_all(&storage_dir)?;
    fs::set_permissions(&storage_dir, fs::Permissions::from_mode(0o700))?;
    let current_path = storage_dir.join("current.json");

    let (previous_state, previous_cache_corrupt) = match fs::read(&current_path) {
        Ok(bytes) => match serde_json::from_slice::<StorageInventory>(&bytes) {
            Ok(previous) => (Some(previous.pressure.state), false),
            Err(_) => (None, true),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => (None, false),
        Err(error) => return Err(error.into()),
    };

    let pressure_event_written = if previous_state.as_ref() != Some(&inventory.pressure.state) {
        write_pressure_evidence(
            state_dir,
            host_id,
            generation_id,
            previous_state,
            inventory,
        )?;
        true
    } else {
        false
    };

    write_atomic_json(&current_path, inventory, 0o600)?;
    Ok(PersistReport {
        previous_cache_corrupt,
        pressure_event_written,
    })
}

fn write_pressure_evidence(
    state_dir: &Path,
    host_id: Uuid,
    generation_id: &str,
    previous_state: Option<StoragePressureState>,
    inventory: &StorageInventory,
) -> Result<(), StorageStateError> {
    let evidence_dir = state_dir.join("evidence/storage-pressure");
    fs::create_dir_all(&evidence_dir)?;
    fs::set_permissions(&evidence_dir, fs::Permissions::from_mode(0o700))?;

    let evidence = StoragePressureEvidence {
        schema: STORAGE_PRESSURE_SCHEMA.to_owned(),
        evidence_id: Uuid::now_v7(),
        host_id,
        generation_id: generation_id.to_owned(),
        previous_state,
        current_state: inventory.pressure.state.clone(),
        root_mount_id: inventory.root_mount_id,
        available_bytes: inventory.pressure.available_bytes,
        observed_at: inventory.observed_at.clone(),
    };
    let path = evidence_dir.join(format!("{}.json", evidence.evidence_id));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    file.write_all(&serde_json::to_vec_pretty(&evidence)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    File::open(&evidence_dir)?.sync_all()?;
    Ok(())
}

fn write_atomic_json<T: serde::Serialize>(
    path: &Path,
    value: &T,
    mode: u32,
) -> Result<(), StorageStateError> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage state path has no parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;
    let temp_path = parent.join(format!(".storage.{}.tmp", Uuid::now_v7()));
    let mut temp = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&temp_path)?;
    temp.write_all(&serde_json::to_vec_pretty(value)?)?;
    temp.write_all(b"\n")?;
    temp.sync_all()?;
    fs::set_permissions(&temp_path, fs::Permissions::from_mode(mode))?;
    fs::rename(&temp_path, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn unavailable_inventory(
    observed_at: String,
    generation_id: String,
    reserve_policy: Option<&StorageReservePolicy>,
    reason: String,
) -> StorageInventory {
    let (reserve, pressure) = match reserve_policy {
        Some(policy) if prime_storage::validate_reserve_policy(policy).is_ok() => (
            StorageReserveVisibility {
                policy_configured: true,
                protected_rollback_recovery_bytes: Some(
                    policy.protected_rollback_recovery_bytes,
                ),
                limitations: vec!["storage inventory is unavailable".to_owned()],
            },
            StoragePressure {
                state: StoragePressureState::Unknown,
                available_bytes: None,
                low_threshold_bytes: Some(policy.low_space_warning_bytes),
                critical_threshold_bytes: Some(policy.critical_space_bytes),
                limitations: vec!["storage inventory is unavailable".to_owned()],
            },
        ),
        Some(policy) => {
            let limitation = prime_storage::validate_reserve_policy(policy)
                .err()
                .map(|reason| format!("storage reserve policy invalid: {reason}"))
                .unwrap_or_else(|| "storage reserve policy is invalid".to_owned());
            (
                StorageReserveVisibility {
                    policy_configured: false,
                    protected_rollback_recovery_bytes: None,
                    limitations: vec![limitation.clone()],
                },
                StoragePressure {
                    state: StoragePressureState::Unknown,
                    available_bytes: None,
                    low_threshold_bytes: None,
                    critical_threshold_bytes: None,
                    limitations: vec![limitation],
                },
            )
        }
        None => (
            StorageReserveVisibility {
                policy_configured: false,
                protected_rollback_recovery_bytes: None,
                limitations: vec![
                    "Prime image has not configured rollback/recovery reserve bytes".to_owned(),
                ],
            },
            StoragePressure {
                state: StoragePressureState::Unknown,
                available_bytes: None,
                low_threshold_bytes: None,
                critical_threshold_bytes: None,
                limitations: vec!["storage pressure thresholds are not configured".to_owned()],
            },
        ),
    };

    StorageInventory {
        schema: STORAGE_INVENTORY_SCHEMA.to_owned(),
        observed_at,
        mount_namespace_source: "/proc/self/mountinfo".to_owned(),
        mounts: Vec::new(),
        local_physical_totals: StorageTotals::default(),
        root_mount_id: None,
        generation_accounting: StorageGenerationAccounting {
            current_generation_id: generation_id,
            current_generation_bytes: None,
            previous_known_good_bytes: None,
            recovery_generation_bytes: None,
            staged_generation_bytes: None,
            limitations: vec!["generation storage accounting is unavailable".to_owned()],
        },
        reserve,
        pressure,
        limitations: vec![format!("storage inventory unavailable: {reason}")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prime_contracts::{StoragePressureState, STORAGE_RESERVE_POLICY_SCHEMA};

    #[test]
    fn missing_mountinfo_degrades_without_panicking() {
        let dir = tempfile::tempdir().expect("tempdir");
        let inventory = observe(
            &dir.path().join("missing-mountinfo"),
            &dir.path().join("missing-policy"),
            "t1".to_owned(),
            "g1".to_owned(),
        );
        assert!(inventory.mounts.is_empty());
        assert_eq!(inventory.pressure.state, StoragePressureState::Unknown);
        assert!(inventory
            .limitations
            .iter()
            .any(|limitation| limitation.contains("inventory unavailable")));
    }

    #[test]
    fn corrupt_policy_is_explicitly_unconfigured() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mountinfo = dir.path().join("mountinfo");
        let policy = dir.path().join("policy.json");
        fs::write(&mountinfo, b"36 35 8:1 / / rw - ext4 /dev/sda1 rw\n")
            .expect("mountinfo");
        fs::write(&policy, b"not-json").expect("policy");
        let inventory = observe(&mountinfo, &policy, "t1".to_owned(), "g1".to_owned());
        assert!(!inventory.reserve.policy_configured);
        assert!(inventory
            .reserve
            .limitations
            .iter()
            .any(|limitation| limitation.contains("policy load failed")));
    }

    #[test]
    fn pressure_transition_evidence_is_append_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let host_id = Uuid::now_v7();
        let inventory = unavailable_inventory(
            "t1".to_owned(),
            "g1".to_owned(),
            Some(&StorageReservePolicy {
                schema: STORAGE_RESERVE_POLICY_SCHEMA.to_owned(),
                protected_rollback_recovery_bytes: 1,
                low_space_warning_bytes: 2,
                critical_space_bytes: 1,
            }),
            "fixture".to_owned(),
        );
        let first = persist_snapshot(dir.path(), host_id, "g1", &inventory).expect("first");
        assert!(first.pressure_event_written);
        let second = persist_snapshot(dir.path(), host_id, "g1", &inventory).expect("second");
        assert!(!second.pressure_event_written);
        let count = fs::read_dir(dir.path().join("evidence/storage-pressure"))
            .expect("evidence dir")
            .count();
        assert_eq!(count, 1);
    }
}
