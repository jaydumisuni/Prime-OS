#![cfg(target_os = "linux")]

use prime_contracts::{
    StorageCapacity, StorageGenerationAccounting, StorageInventory, StorageMount, StoragePreflight,
    StoragePreflightReason, StoragePreflightRequest, StoragePressure, StoragePressureState,
    StorageReservePolicy, StorageReserveVisibility, StorageScope, StorageTotals,
    STORAGE_INVENTORY_SCHEMA, STORAGE_PREFLIGHT_SCHEMA, STORAGE_RESERVE_POLICY_SCHEMA,
};
use std::collections::{HashMap, HashSet};
use std::ffi::{CString, OsString};
use std::fs;
use std::io;
use std::mem::MaybeUninit;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("could not read Linux mount inventory: {0}")]
    MountInfo(#[source] io::Error),
    #[error("storage reserve policy could not be read: {0}")]
    PolicyIo(#[source] io::Error),
    #[error("storage reserve policy is invalid JSON: {0}")]
    PolicyJson(#[source] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityCounters {
    pub fragment_size_bytes: u64,
    pub blocks: u64,
    pub free_blocks: u64,
    pub available_blocks: u64,
}

pub trait CapacitySource {
    fn read_capacity(&self, mount_point: &Path) -> io::Result<CapacityCounters>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StatvfsCapacitySource;

impl CapacitySource for StatvfsCapacitySource {
    fn read_capacity(&self, mount_point: &Path) -> io::Result<CapacityCounters> {
        statvfs_counters(mount_point)
    }
}

#[derive(Debug, Clone)]
struct ParsedMount {
    mount_id: u64,
    parent_mount_id: u64,
    major_minor: String,
    root: Vec<u8>,
    mount_point: Vec<u8>,
    filesystem_type: String,
    mount_source: Option<Vec<u8>>,
    read_only: bool,
    scope: StorageScope,
    filesystem_key: String,
}

pub fn load_reserve_policy(path: &Path) -> Result<Option<StorageReservePolicy>, StorageError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(StorageError::PolicyIo(error)),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(StorageError::PolicyJson)
}

pub fn validate_reserve_policy(policy: &StorageReservePolicy) -> Result<(), String> {
    if policy.schema != STORAGE_RESERVE_POLICY_SCHEMA {
        return Err(format!(
            "unexpected reserve policy schema {}; expected {}",
            policy.schema, STORAGE_RESERVE_POLICY_SCHEMA
        ));
    }
    if policy.critical_space_bytes > policy.low_space_warning_bytes {
        return Err(
            "critical_space_bytes must be less than or equal to low_space_warning_bytes".to_owned(),
        );
    }
    Ok(())
}

pub fn probe_host(
    mountinfo_path: &Path,
    observed_at: String,
    current_generation_id: String,
    reserve_policy: Option<&StorageReservePolicy>,
) -> Result<StorageInventory, StorageError> {
    let mountinfo = fs::read(mountinfo_path).map_err(StorageError::MountInfo)?;
    Ok(probe_with_source(
        &mountinfo,
        observed_at,
        current_generation_id,
        reserve_policy,
        &StatvfsCapacitySource,
    ))
}

pub fn probe_with_source<S: CapacitySource>(
    mountinfo: &[u8],
    observed_at: String,
    current_generation_id: String,
    reserve_policy: Option<&StorageReservePolicy>,
    capacity_source: &S,
) -> StorageInventory {
    let mut limitations = Vec::new();
    let mut parsed = Vec::new();

    for (line_index, line) in mountinfo.split(|byte| *byte == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        match parse_mount_line(line) {
            Ok(mount) => parsed.push(mount),
            Err(reason) => limitations.push(format!(
                "mountinfo line {} was ignored: {}",
                line_index + 1,
                reason
            )),
        }
    }

    let mut root_candidates = parsed
        .iter()
        .filter(|mount| mount.mount_point == b"/")
        .map(|mount| mount.mount_id)
        .collect::<Vec<_>>();
    root_candidates.sort_unstable();
    let root_mount_id = root_candidates.last().copied();
    if root_candidates.len() > 1 {
        limitations.push(format!(
            "multiple visible root mount candidates were reported; using highest mount ID {}",
            root_mount_id.unwrap_or_default()
        ));
    }
    if root_mount_id.is_none() {
        limitations.push("no mountinfo entry for the process root was found".to_owned());
    }

    let mut mounts = Vec::with_capacity(parsed.len());
    for mount in parsed {
        let path = PathBuf::from(OsString::from_vec(mount.mount_point.clone()));
        let mut mount_limitations = Vec::new();
        let capacity = match capacity_source.read_capacity(&path) {
            Ok(counters) => match capacity_from_counters(counters) {
                Ok(capacity) => Some(capacity),
                Err(reason) => {
                    mount_limitations.push(reason);
                    None
                }
            },
            Err(error) => {
                mount_limitations.push(format!("statvfs capacity unavailable: {error}"));
                None
            }
        };
        mounts.push(StorageMount {
            mount_id: mount.mount_id,
            parent_mount_id: mount.parent_mount_id,
            major_minor: mount.major_minor,
            root: display_path_bytes(&mount.root),
            mount_point: display_path_bytes(&mount.mount_point),
            filesystem_type: mount.filesystem_type,
            mount_source: mount.mount_source.as_deref().map(display_path_bytes),
            read_only: mount.read_only,
            scope: mount.scope,
            filesystem_key: mount.filesystem_key,
            capacity,
            limitations: mount_limitations,
        });
    }
    mounts.sort_by_key(|mount| mount.mount_id);

    let local_physical_totals = aggregate_local_physical(&mounts, &mut limitations);
    let generation_accounting = StorageGenerationAccounting {
        current_generation_id,
        current_generation_bytes: None,
        previous_known_good_bytes: None,
        recovery_generation_bytes: None,
        staged_generation_bytes: None,
        limitations: vec![
            "Per-generation physical byte attribution remains unavailable until the generation storage layout can prove snapshot/reflink ownership without double counting"
                .to_owned(),
        ],
    };

    let (reserve, validated_policy) = reserve_visibility(reserve_policy);
    let pressure = evaluate_pressure(&mounts, root_mount_id, validated_policy);

    StorageInventory {
        schema: STORAGE_INVENTORY_SCHEMA.to_owned(),
        observed_at,
        mount_namespace_source: "/proc/self/mountinfo".to_owned(),
        mounts,
        local_physical_totals,
        root_mount_id,
        generation_accounting,
        reserve,
        pressure,
        limitations,
    }
}

pub fn preflight(
    inventory: &StorageInventory,
    request: &StoragePreflightRequest,
    observed_at: String,
) -> StoragePreflight {
    if request.schema != STORAGE_PREFLIGHT_SCHEMA {
        return preflight_result(
            request,
            None,
            None,
            None,
            false,
            StoragePreflightReason::InvalidRequestSchema,
            observed_at,
        );
    }

    let target_mount_id = request.target_mount_id.or(inventory.root_mount_id);
    let Some(target_mount_id) = target_mount_id else {
        return preflight_result(
            request,
            None,
            None,
            inventory.reserve.protected_rollback_recovery_bytes,
            false,
            StoragePreflightReason::TargetMountMissing,
            observed_at,
        );
    };

    let Some(mount) = inventory
        .mounts
        .iter()
        .find(|mount| mount.mount_id == target_mount_id)
    else {
        return preflight_result(
            request,
            Some(target_mount_id),
            None,
            inventory.reserve.protected_rollback_recovery_bytes,
            false,
            StoragePreflightReason::TargetMountMissing,
            observed_at,
        );
    };

    if mount.scope != StorageScope::LocalPhysical {
        return preflight_result(
            request,
            Some(target_mount_id),
            mount
                .capacity
                .as_ref()
                .map(|capacity| capacity.available_bytes),
            inventory.reserve.protected_rollback_recovery_bytes,
            false,
            StoragePreflightReason::TargetMountNotLocalPhysical,
            observed_at,
        );
    }

    let Some(capacity) = &mount.capacity else {
        return preflight_result(
            request,
            Some(target_mount_id),
            None,
            inventory.reserve.protected_rollback_recovery_bytes,
            false,
            StoragePreflightReason::TargetCapacityUnavailable,
            observed_at,
        );
    };

    let Some(reserve) = inventory.reserve.protected_rollback_recovery_bytes else {
        return preflight_result(
            request,
            Some(target_mount_id),
            Some(capacity.available_bytes),
            None,
            false,
            StoragePreflightReason::ProtectedReserveUnconfigured,
            observed_at,
        );
    };

    let Some(required_with_reserve) = request.required_staging_bytes.checked_add(reserve) else {
        return preflight_result(
            request,
            Some(target_mount_id),
            Some(capacity.available_bytes),
            Some(reserve),
            false,
            StoragePreflightReason::ArithmeticOverflow,
            observed_at,
        );
    };

    let remaining_after_stage = capacity
        .available_bytes
        .checked_sub(request.required_staging_bytes);
    let admitted = capacity.available_bytes >= required_with_reserve;
    let mut result = preflight_result(
        request,
        Some(target_mount_id),
        Some(capacity.available_bytes),
        Some(reserve),
        admitted,
        if admitted {
            StoragePreflightReason::SpaceAvailableWithProtectedReserve
        } else {
            StoragePreflightReason::ProtectedReserveWouldBeConsumed
        },
        observed_at,
    );
    result.remaining_after_stage_bytes = remaining_after_stage;
    result
}

fn preflight_result(
    request: &StoragePreflightRequest,
    target_mount_id: Option<u64>,
    available_bytes: Option<u64>,
    reserve: Option<u64>,
    admitted: bool,
    reason: StoragePreflightReason,
    observed_at: String,
) -> StoragePreflight {
    StoragePreflight {
        schema: STORAGE_PREFLIGHT_SCHEMA.to_owned(),
        target_mount_id,
        required_staging_bytes: request.required_staging_bytes,
        available_bytes,
        protected_rollback_recovery_bytes: reserve,
        remaining_after_stage_bytes: None,
        admitted,
        reason,
        observed_at,
    }
}

fn reserve_visibility(
    policy: Option<&StorageReservePolicy>,
) -> (StorageReserveVisibility, Option<&StorageReservePolicy>) {
    let Some(policy) = policy else {
        return (
            StorageReserveVisibility {
                policy_configured: false,
                protected_rollback_recovery_bytes: None,
                limitations: vec![
                    "Prime image has not configured rollback/recovery reserve bytes".to_owned(),
                ],
            },
            None,
        );
    };

    match validate_reserve_policy(policy) {
        Ok(()) => (
            StorageReserveVisibility {
                policy_configured: true,
                protected_rollback_recovery_bytes: Some(policy.protected_rollback_recovery_bytes),
                limitations: Vec::new(),
            },
            Some(policy),
        ),
        Err(reason) => (
            StorageReserveVisibility {
                policy_configured: false,
                protected_rollback_recovery_bytes: None,
                limitations: vec![format!("storage reserve policy invalid: {reason}")],
            },
            None,
        ),
    }
}

fn evaluate_pressure(
    mounts: &[StorageMount],
    root_mount_id: Option<u64>,
    policy: Option<&StorageReservePolicy>,
) -> StoragePressure {
    let Some(policy) = policy else {
        return StoragePressure {
            state: StoragePressureState::Unknown,
            available_bytes: root_mount_id.and_then(|id| {
                mounts
                    .iter()
                    .find(|mount| mount.mount_id == id)
                    .and_then(|mount| mount.capacity.as_ref())
                    .map(|capacity| capacity.available_bytes)
            }),
            low_threshold_bytes: None,
            critical_threshold_bytes: None,
            limitations: vec!["storage pressure thresholds are not configured".to_owned()],
        };
    };

    let Some(root_id) = root_mount_id else {
        return StoragePressure {
            state: StoragePressureState::Unknown,
            available_bytes: None,
            low_threshold_bytes: Some(policy.low_space_warning_bytes),
            critical_threshold_bytes: Some(policy.critical_space_bytes),
            limitations: vec!["root mount identity is unavailable".to_owned()],
        };
    };
    let Some(root) = mounts.iter().find(|mount| mount.mount_id == root_id) else {
        return StoragePressure {
            state: StoragePressureState::Unknown,
            available_bytes: None,
            low_threshold_bytes: Some(policy.low_space_warning_bytes),
            critical_threshold_bytes: Some(policy.critical_space_bytes),
            limitations: vec!["root mount record is unavailable".to_owned()],
        };
    };
    if root.scope != StorageScope::LocalPhysical {
        return StoragePressure {
            state: StoragePressureState::Unknown,
            available_bytes: root
                .capacity
                .as_ref()
                .map(|capacity| capacity.available_bytes),
            low_threshold_bytes: Some(policy.low_space_warning_bytes),
            critical_threshold_bytes: Some(policy.critical_space_bytes),
            limitations: vec!["root mount is not classified as LOCAL_PHYSICAL".to_owned()],
        };
    }
    let Some(capacity) = &root.capacity else {
        return StoragePressure {
            state: StoragePressureState::Unknown,
            available_bytes: None,
            low_threshold_bytes: Some(policy.low_space_warning_bytes),
            critical_threshold_bytes: Some(policy.critical_space_bytes),
            limitations: vec!["root filesystem capacity is unavailable".to_owned()],
        };
    };

    StoragePressure {
        state: if capacity.available_bytes <= policy.critical_space_bytes {
            StoragePressureState::Critical
        } else if capacity.available_bytes <= policy.low_space_warning_bytes {
            StoragePressureState::Low
        } else {
            StoragePressureState::Normal
        },
        available_bytes: Some(capacity.available_bytes),
        low_threshold_bytes: Some(policy.low_space_warning_bytes),
        critical_threshold_bytes: Some(policy.critical_space_bytes),
        limitations: Vec::new(),
    }
}

fn aggregate_local_physical(
    mounts: &[StorageMount],
    limitations: &mut Vec<String>,
) -> StorageTotals {
    let mut by_key: HashMap<&str, &StorageCapacity> = HashMap::new();
    let mut conflicting = HashSet::new();

    for mount in mounts
        .iter()
        .filter(|mount| mount.scope == StorageScope::LocalPhysical)
    {
        let Some(capacity) = mount.capacity.as_ref() else {
            continue;
        };
        match by_key.get(mount.filesystem_key.as_str()) {
            None => {
                by_key.insert(&mount.filesystem_key, capacity);
            }
            Some(existing) if *existing == capacity => {}
            Some(_) => {
                conflicting.insert(mount.filesystem_key.as_str());
            }
        }
    }

    let mut totals = StorageTotals::default();
    for (key, capacity) in by_key {
        if conflicting.contains(key) {
            limitations.push(format!(
                "local filesystem {key} reported inconsistent capacity across multiple mounts and was excluded from aggregate totals"
            ));
            continue;
        }
        if add_capacity(&mut totals, capacity).is_err() {
            limitations.push(format!(
                "local filesystem {key} overflowed aggregate storage counters and was excluded"
            ));
        }
    }
    totals
}

fn add_capacity(totals: &mut StorageTotals, capacity: &StorageCapacity) -> Result<(), ()> {
    let next = StorageTotals {
        filesystem_count: totals.filesystem_count.checked_add(1).ok_or(())?,
        total_bytes: totals
            .total_bytes
            .checked_add(capacity.total_bytes)
            .ok_or(())?,
        free_bytes: totals
            .free_bytes
            .checked_add(capacity.free_bytes)
            .ok_or(())?,
        available_bytes: totals
            .available_bytes
            .checked_add(capacity.available_bytes)
            .ok_or(())?,
        used_bytes: totals
            .used_bytes
            .checked_add(capacity.used_bytes)
            .ok_or(())?,
        reserved_bytes: totals
            .reserved_bytes
            .checked_add(capacity.reserved_bytes)
            .ok_or(())?,
    };
    *totals = next;
    Ok(())
}

pub fn capacity_from_counters(counters: CapacityCounters) -> Result<StorageCapacity, String> {
    if counters.fragment_size_bytes == 0 {
        return Err("statvfs fragment size is zero".to_owned());
    }
    if counters.free_blocks > counters.blocks {
        return Err("statvfs free blocks exceed total blocks".to_owned());
    }
    if counters.available_blocks > counters.free_blocks {
        return Err("statvfs available blocks exceed free blocks".to_owned());
    }

    let total_bytes = counters
        .blocks
        .checked_mul(counters.fragment_size_bytes)
        .ok_or_else(|| "statvfs total byte calculation overflowed".to_owned())?;
    let free_bytes = counters
        .free_blocks
        .checked_mul(counters.fragment_size_bytes)
        .ok_or_else(|| "statvfs free byte calculation overflowed".to_owned())?;
    let available_bytes = counters
        .available_blocks
        .checked_mul(counters.fragment_size_bytes)
        .ok_or_else(|| "statvfs available byte calculation overflowed".to_owned())?;
    let used_bytes = total_bytes
        .checked_sub(free_bytes)
        .ok_or_else(|| "statvfs used byte calculation underflowed".to_owned())?;
    let reserved_bytes = free_bytes
        .checked_sub(available_bytes)
        .ok_or_else(|| "statvfs reserved byte calculation underflowed".to_owned())?;

    Ok(StorageCapacity {
        source: "statvfs".to_owned(),
        fragment_size_bytes: counters.fragment_size_bytes,
        total_bytes,
        free_bytes,
        available_bytes,
        used_bytes,
        reserved_bytes,
    })
}

#[allow(clippy::unnecessary_cast)]
fn statvfs_counters(path: &Path) -> io::Result<CapacityCounters> {
    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "mount path unexpectedly contains an interior NUL byte",
        )
    })?;
    let mut buffer = MaybeUninit::<libc::statvfs>::uninit();
    let result = unsafe { libc::statvfs(c_path.as_ptr(), buffer.as_mut_ptr()) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    let stat = unsafe { buffer.assume_init() };
    Ok(CapacityCounters {
        fragment_size_bytes: stat.f_frsize as u64,
        blocks: stat.f_blocks as u64,
        free_blocks: stat.f_bfree as u64,
        available_blocks: stat.f_bavail as u64,
    })
}

fn parse_mount_line(line: &[u8]) -> Result<ParsedMount, String> {
    let fields = line
        .split(|byte| *byte == b' ')
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    if fields.len() < 10 {
        return Err("too few mountinfo fields".to_owned());
    }
    let separator = fields
        .iter()
        .position(|field| *field == b"-")
        .ok_or_else(|| "missing mountinfo separator".to_owned())?;
    if separator < 6 || separator + 3 >= fields.len() {
        return Err("mountinfo separator is in an invalid position".to_owned());
    }

    let mount_id = parse_ascii_u64(fields[0], "mount ID")?;
    let parent_mount_id = parse_ascii_u64(fields[1], "parent mount ID")?;
    let major_minor = ascii_string(fields[2], "major:minor")?;
    if !major_minor.contains(':') {
        return Err("major:minor field has no colon".to_owned());
    }
    let root = decode_mount_field(fields[3])?;
    let mount_point = decode_mount_field(fields[4])?;
    if mount_point.first().copied() != Some(b'/') {
        return Err("mount point is not absolute".to_owned());
    }
    let mount_options = fields[5];
    let read_only = mount_options
        .split(|byte| *byte == b',')
        .any(|option| option == b"ro");
    let filesystem_type = ascii_string(fields[separator + 1], "filesystem type")?;
    let mount_source = if fields[separator + 2] == b"none" {
        None
    } else {
        Some(decode_mount_field(fields[separator + 2])?)
    };
    let scope = classify_scope(&filesystem_type, mount_source.as_deref());
    let filesystem_key = format!("{major_minor}|{filesystem_type}");

    Ok(ParsedMount {
        mount_id,
        parent_mount_id,
        major_minor,
        root,
        mount_point,
        filesystem_type,
        mount_source,
        read_only,
        scope,
        filesystem_key,
    })
}

fn classify_scope(filesystem_type: &str, mount_source: Option<&[u8]>) -> StorageScope {
    let fs = filesystem_type.to_ascii_lowercase();
    if fs == "overlay" {
        return StorageScope::Overlay;
    }
    if matches!(fs.as_str(), "tmpfs" | "ramfs" | "hugetlbfs") {
        return StorageScope::Memory;
    }
    if is_virtual_filesystem(&fs) {
        return StorageScope::Virtual;
    }
    if is_remote_filesystem(&fs) {
        return StorageScope::Remote;
    }
    if is_local_filesystem(&fs) || mount_source.is_some_and(|source| source.starts_with(b"/dev/")) {
        return StorageScope::LocalPhysical;
    }
    StorageScope::Unknown
}

fn is_local_filesystem(fs: &str) -> bool {
    matches!(
        fs,
        "btrfs"
            | "ext2"
            | "ext3"
            | "ext4"
            | "xfs"
            | "f2fs"
            | "vfat"
            | "fat"
            | "msdos"
            | "exfat"
            | "ntfs"
            | "ntfs3"
            | "udf"
            | "iso9660"
    )
}

fn is_remote_filesystem(fs: &str) -> bool {
    matches!(
        fs,
        "nfs"
            | "nfs4"
            | "cifs"
            | "smb3"
            | "ceph"
            | "9p"
            | "afs"
            | "coda"
            | "glusterfs"
            | "fuse.sshfs"
    ) || fs.starts_with("fuse.sshfs")
}

fn is_virtual_filesystem(fs: &str) -> bool {
    matches!(
        fs,
        "proc"
            | "sysfs"
            | "devtmpfs"
            | "devpts"
            | "cgroup"
            | "cgroup2"
            | "securityfs"
            | "debugfs"
            | "tracefs"
            | "pstore"
            | "configfs"
            | "mqueue"
            | "fusectl"
            | "autofs"
            | "efivarfs"
            | "binfmt_misc"
            | "bpf"
            | "nsfs"
    )
}

fn parse_ascii_u64(field: &[u8], name: &str) -> Result<u64, String> {
    let value = ascii_string(field, name)?;
    value
        .parse::<u64>()
        .map_err(|_| format!("{name} is not an unsigned integer"))
}

fn ascii_string(field: &[u8], name: &str) -> Result<String, String> {
    if !field.is_ascii() {
        return Err(format!("{name} is not ASCII"));
    }
    String::from_utf8(field.to_vec()).map_err(|_| format!("{name} is not valid ASCII"))
}

fn decode_mount_field(field: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoded = Vec::with_capacity(field.len());
    let mut index = 0;
    while index < field.len() {
        if field[index] == b'\\' && index + 3 < field.len() {
            let digits = &field[index + 1..index + 4];
            if digits.iter().all(|byte| (b'0'..=b'7').contains(byte)) {
                let value = u16::from(digits[0] - b'0') * 64
                    + u16::from(digits[1] - b'0') * 8
                    + u16::from(digits[2] - b'0');
                let byte = u8::try_from(value)
                    .map_err(|_| "mountinfo escape exceeds one byte".to_owned())?;
                decoded.push(byte);
                index += 4;
                continue;
            }
        }
        decoded.push(field[index]);
        index += 1;
    }
    if decoded.contains(&0) {
        return Err("decoded mount field contains a NUL byte".to_owned());
    }
    Ok(decoded)
}

fn display_path_bytes(bytes: &[u8]) -> String {
    if let Ok(value) = std::str::from_utf8(bytes) {
        return value.to_owned();
    }
    let mut rendered = String::new();
    for byte in bytes {
        if (0x20..=0x7e).contains(byte) && *byte != b'\\' {
            rendered.push(char::from(*byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut rendered, "\\x{byte:02x}").expect("writing to String cannot fail");
        }
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FixtureCapacity {
        values: HashMap<Vec<u8>, CapacityCounters>,
    }

    impl FixtureCapacity {
        fn with(mut self, path: &[u8], counters: CapacityCounters) -> Self {
            self.values.insert(path.to_vec(), counters);
            self
        }
    }

    impl CapacitySource for FixtureCapacity {
        fn read_capacity(&self, mount_point: &Path) -> io::Result<CapacityCounters> {
            self.values
                .get(mount_point.as_os_str().as_bytes())
                .copied()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "fixture missing"))
        }
    }

    fn counters(total: u64, free: u64, available: u64) -> CapacityCounters {
        CapacityCounters {
            fragment_size_bytes: 4096,
            blocks: total,
            free_blocks: free,
            available_blocks: available,
        }
    }

    fn policy(reserve: u64, low: u64, critical: u64) -> StorageReservePolicy {
        StorageReservePolicy {
            schema: STORAGE_RESERVE_POLICY_SCHEMA.to_owned(),
            protected_rollback_recovery_bytes: reserve,
            low_space_warning_bytes: low,
            critical_space_bytes: critical,
        }
    }

    #[test]
    fn capacity_math_distinguishes_free_available_used_and_reserved() {
        let capacity = capacity_from_counters(CapacityCounters {
            fragment_size_bytes: 4096,
            blocks: 100,
            free_blocks: 30,
            available_blocks: 25,
        })
        .expect("capacity");
        assert_eq!(capacity.total_bytes, 409_600);
        assert_eq!(capacity.free_bytes, 122_880);
        assert_eq!(capacity.available_bytes, 102_400);
        assert_eq!(capacity.used_bytes, 286_720);
        assert_eq!(capacity.reserved_bytes, 20_480);
    }

    #[test]
    fn parser_decodes_mountinfo_escapes_and_ignores_optional_fields() {
        let mountinfo = b"36 35 8:1 / / rw,relatime shared:1 master:2 - ext4 /dev/sda1 rw\n37 36 8:1 /data\\040root /data\\040space rw - ext4 /dev/sda1 rw\n";
        let source = FixtureCapacity::default()
            .with(b"/", counters(100, 30, 25))
            .with(b"/data space", counters(100, 30, 25));
        let inventory =
            probe_with_source(mountinfo, "t1".to_owned(), "g1".to_owned(), None, &source);
        let data = inventory
            .mounts
            .iter()
            .find(|mount| mount.mount_id == 37)
            .expect("data mount");
        assert_eq!(data.root, "/data root");
        assert_eq!(data.mount_point, "/data space");
        assert_eq!(inventory.local_physical_totals.filesystem_count, 1);
    }

    #[test]
    fn invalid_octal_escape_is_limited_not_panicked() {
        let inventory = probe_with_source(
            b"36 35 8:1 / /bad\\777path rw - ext4 /dev/sda1 rw\n",
            "t1".to_owned(),
            "g1".to_owned(),
            None,
            &FixtureCapacity::default(),
        );
        assert!(inventory.mounts.is_empty());
        assert!(inventory
            .limitations
            .iter()
            .any(|limitation| limitation.contains("escape exceeds one byte")));
    }

    #[test]
    fn local_totals_do_not_count_overlay_remote_memory_or_duplicate_bind_mounts() {
        let mountinfo = b"36 35 8:1 / / rw - ext4 /dev/sda1 rw\n37 36 8:1 / /bind rw - ext4 /dev/sda1 rw\n38 36 0:40 / /run rw - tmpfs tmpfs rw\n39 36 0:41 / /overlay rw - overlay overlay rw\n40 36 0:42 / /remote rw - nfs4 server:/share rw\n";
        let source = FixtureCapacity::default()
            .with(b"/", counters(100, 30, 25))
            .with(b"/bind", counters(100, 30, 25))
            .with(b"/run", counters(10, 8, 8))
            .with(b"/overlay", counters(100, 30, 25))
            .with(b"/remote", counters(1000, 900, 900));
        let inventory =
            probe_with_source(mountinfo, "t1".to_owned(), "g1".to_owned(), None, &source);
        assert_eq!(inventory.local_physical_totals.filesystem_count, 1);
        assert_eq!(inventory.local_physical_totals.total_bytes, 409_600);
        assert_eq!(inventory.mounts[2].scope, StorageScope::Memory);
        assert_eq!(inventory.mounts[3].scope, StorageScope::Overlay);
        assert_eq!(inventory.mounts[4].scope, StorageScope::Remote);
    }

    #[test]
    fn malformed_mount_line_is_limited_not_invented() {
        let inventory = probe_with_source(
            b"not a mount line\n36 35 8:1 / / rw - ext4 /dev/sda1 rw\n",
            "t1".to_owned(),
            "g1".to_owned(),
            None,
            &FixtureCapacity::default().with(b"/", counters(10, 5, 4)),
        );
        assert_eq!(inventory.mounts.len(), 1);
        assert!(inventory
            .limitations
            .iter()
            .any(|limitation| limitation.contains("line 1")));
    }

    #[test]
    fn missing_policy_yields_unknown_pressure_and_denied_preflight() {
        let inventory = probe_with_source(
            b"36 35 8:1 / / rw - ext4 /dev/sda1 rw\n",
            "t1".to_owned(),
            "g1".to_owned(),
            None,
            &FixtureCapacity::default().with(b"/", counters(100, 30, 25)),
        );
        assert_eq!(inventory.pressure.state, StoragePressureState::Unknown);
        let result = preflight(
            &inventory,
            &StoragePreflightRequest {
                schema: STORAGE_PREFLIGHT_SCHEMA.to_owned(),
                required_staging_bytes: 4096,
                target_mount_id: None,
            },
            "t2".to_owned(),
        );
        assert!(!result.admitted);
        assert_eq!(
            result.reason,
            StoragePreflightReason::ProtectedReserveUnconfigured
        );
    }

    #[test]
    fn preflight_preserves_explicit_reserve() {
        let reserve = 10 * 4096;
        let inventory = probe_with_source(
            b"36 35 8:1 / / rw - ext4 /dev/sda1 rw\n",
            "t1".to_owned(),
            "g1".to_owned(),
            Some(&policy(reserve, 20 * 4096, 10 * 4096)),
            &FixtureCapacity::default().with(b"/", counters(100, 30, 25)),
        );
        let admitted = preflight(
            &inventory,
            &StoragePreflightRequest {
                schema: STORAGE_PREFLIGHT_SCHEMA.to_owned(),
                required_staging_bytes: 10 * 4096,
                target_mount_id: None,
            },
            "t2".to_owned(),
        );
        assert!(admitted.admitted);
        assert_eq!(
            admitted.reason,
            StoragePreflightReason::SpaceAvailableWithProtectedReserve
        );

        let denied = preflight(
            &inventory,
            &StoragePreflightRequest {
                schema: STORAGE_PREFLIGHT_SCHEMA.to_owned(),
                required_staging_bytes: 16 * 4096,
                target_mount_id: None,
            },
            "t3".to_owned(),
        );
        assert!(!denied.admitted);
        assert_eq!(
            denied.reason,
            StoragePreflightReason::ProtectedReserveWouldBeConsumed
        );
    }

    #[test]
    fn pressure_boundaries_are_mechanical() {
        let low = 20 * 4096;
        let critical = 10 * 4096;
        let low_inventory = probe_with_source(
            b"36 35 8:1 / / rw - ext4 /dev/sda1 rw\n",
            "t1".to_owned(),
            "g1".to_owned(),
            Some(&policy(5 * 4096, low, critical)),
            &FixtureCapacity::default().with(b"/", counters(100, 30, 20)),
        );
        assert_eq!(low_inventory.pressure.state, StoragePressureState::Low);

        let critical_inventory = probe_with_source(
            b"36 35 8:1 / / rw - ext4 /dev/sda1 rw\n",
            "t1".to_owned(),
            "g1".to_owned(),
            Some(&policy(5 * 4096, low, critical)),
            &FixtureCapacity::default().with(b"/", counters(100, 30, 10)),
        );
        assert_eq!(
            critical_inventory.pressure.state,
            StoragePressureState::Critical
        );
    }

    #[test]
    fn invalid_reserve_policy_is_not_treated_as_configured() {
        let invalid = policy(1, 10, 11);
        assert!(validate_reserve_policy(&invalid).is_err());
        let inventory = probe_with_source(
            b"36 35 8:1 / / rw - ext4 /dev/sda1 rw\n",
            "t1".to_owned(),
            "g1".to_owned(),
            Some(&invalid),
            &FixtureCapacity::default().with(b"/", counters(100, 30, 25)),
        );
        assert!(!inventory.reserve.policy_configured);
        assert_eq!(inventory.pressure.state, StoragePressureState::Unknown);
    }
}
