//! Read-only, unqualified ARM64 board observations for future Prime Mobile.
//!
//! This is a candidate source adapter, NOT `prime.hardware-graph.v1`, Host
//! Identity enrollment, device-port certification, or a platform driver.
//! Kernel-exposed Device Tree, CPU topology, and platform bus descriptors are
//! inventory clues. They do not prove OEM firmware, GPU, touch, or telephony.
//! No IMEI, eFuse, serial, MAC, vendor boot slot, or protected partition read.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

// Reuses the existing Mobile board-observation candidate schema from PR #223.
pub const SCHEMA: &str = "prime.mobile.board-observation.candidate.v0";
const MAX_SOURCE_BYTES: u64 = 4096;
const MAX_CPU_INDEX: u32 = 4095;
const MAX_PLATFORM_DEVICES: usize = 128;
const MAX_COMPATIBLES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformDriverObservation {
    /// A *kernel name*, never a durable board/device identity.
    pub kernel_name: String,
    /// `Some` means a currently bound kernel driver symlink was present.
    /// It does NOT mean the device or a userspace backend works.
    pub driver_bound: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arm64BoardObservation {
    pub schema: &'static str,
    pub claimed_host_arch: String,
    pub kernel_model: Option<String>,
    pub kernel_compatible: Option<Vec<String>>,
    pub kernel_release: Option<String>,
    pub cpu_present: Option<Vec<u32>>,
    pub cpu_online: Option<Vec<u32>>,
    pub platform_devices: Vec<PlatformDriverObservation>,
    pub model_source: Option<&'static str>,
    pub compatible_source: Option<&'static str>,
    pub kernel_release_source: Option<&'static str>,
    pub limitations: Vec<String>,
    pub source_is_hardware_attestation: bool,
    pub any_device_functionality_proven: bool,
    pub firmware_or_device_modified: bool,
}

fn confined_path(root: &Path, rel: &str) -> Option<PathBuf> {
    let base = fs::canonicalize(root).ok()?;
    let path = fs::canonicalize(base.join(rel.trim_start_matches('/'))).ok()?;
    path.starts_with(&base).then_some(path)
}

fn confined_bytes(root: &Path, rel: &str) -> Option<Vec<u8>> {
    let target = confined_path(root, rel)?;
    if !target.is_file() {
        return None;
    }
    let f = File::open(target).ok()?;
    let mut data = Vec::new();
    // Limit reads even when sysfs reports a zero or unreliable st_size.
    f.take(MAX_SOURCE_BYTES + 1).read_to_end(&mut data).ok()?;
    (data.len() as u64 <= MAX_SOURCE_BYTES).then_some(data)
}

fn first_valid<T>(
    root: &Path,
    candidates: &'static [&'static str],
    parse: impl Fn(&[u8]) -> Option<T>,
) -> (Option<T>, Option<&'static str>) {
    for path in candidates {
        if let Some(bytes) = confined_bytes(root, path) {
            if let Some(value) = parse(&bytes) {
                return (Some(value), Some(*path));
            }
        }
    }
    (None, None)
}

fn parse_model(raw: &[u8]) -> Option<String> {
    if !raw.ends_with(&[0]) || raw.len() < 2 || raw.len() > 257 {
        return None;
    }
    let text = std::str::from_utf8(&raw[..raw.len() - 1]).ok()?.trim();
    (!text.is_empty() && text.len() <= 256 && !text.chars().any(char::is_control))
        .then(|| text.to_owned())
}

fn parse_compatible(raw: &[u8]) -> Option<Vec<String>> {
    if !raw.ends_with(&[0]) || raw.len() < 2 {
        return None;
    }
    let list = raw[..raw.len() - 1]
        .split(|b| *b == 0)
        .map(|bytes| {
            if bytes.is_empty() || bytes.len() > 128 ||
                !bytes.iter().all(|b| (32..=126).contains(b)) {
                return None;
            }
            String::from_utf8(bytes.to_vec()).ok()
        })
        .collect::<Option<Vec<String>>>()?;
    (!list.is_empty() && list.len() <= MAX_COMPATIBLES).then_some(list)
}

fn parse_release(raw: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(raw).ok()?.trim();
    (!s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
        .then(|| s.to_owned())
}

fn parse_cpu_list(raw: &[u8]) -> Option<Vec<u32>> {
    let text = std::str::from_utf8(raw).ok()?.trim();
    if text.is_empty() || text.len() > 1024 {
        return None;
    }
    let mut cpus = BTreeSet::new();
    for segment in text.split(',') {
        if segment.is_empty() || segment.trim() != segment {
            return None;
        }
        let mut bounds = segment.split('-');
        let low = bounds.next()?.parse::<u32>().ok()?;
        let high = match bounds.next() {
            Some(value) => value.parse::<u32>().ok()?,
            None => low,
        };
        if bounds.next().is_some() || high < low || high > MAX_CPU_INDEX {
            return None;
        }
        for cpu in low..=high {
            if !cpus.insert(cpu) {
                return None; // duplicates or overlapping ranges are unqualified
            }
        }
    }
    Some(cpus.into_iter().collect())
}

fn safe_kernel_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 128 && name != "." && name != ".." &&
        name.chars().all(|c| !c.is_control() && c != '/' && c != '\\')
}

fn platform_devices(root: &Path, limitations: &mut Vec<String>) -> Vec<PlatformDriverObservation> {
    let Some(base) = confined_path(root, "/sys/bus/platform/devices") else {
        limitations.push("PLATFORM_BUS_UNAVAILABLE".to_owned());
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&base) else {
        limitations.push("PLATFORM_BUS_UNREADABLE".to_owned());
        return Vec::new();
    };
    let root_base = match fs::canonicalize(root) {
        Ok(path) => path,
        Err(_) => return Vec::new(),
    };
    let mut names = entries.filter_map(Result::ok)
        .filter_map(|x| x.file_name().into_string().ok())
        .filter(|name| safe_kernel_name(name))
        .collect::<Vec<_>>();
    names.sort();
    if names.len() > MAX_PLATFORM_DEVICES {
        limitations.push("PLATFORM_DEVICE_LIMIT_REACHED".to_owned());
        names.truncate(MAX_PLATFORM_DEVICES);
    }
    let mut devices = Vec::new();
    for name in names {
        // sysfs bus entries often are symlinks to /sys/devices. Follow only
        // targets that resolve within the explicitly selected observation root.
        let Ok(real_device) = fs::canonicalize(base.join(&name)) else {
            limitations.push("PLATFORM_DEVICE_UNRESOLVED".to_owned());
            continue;
        };
        if !real_device.starts_with(&root_base) || !real_device.is_dir() {
            limitations.push("PLATFORM_DEVICE_OUTSIDE_ROOT".to_owned());
            continue;
        }
        let link = real_device.join("driver");
        let driver = if fs::symlink_metadata(&link).is_ok() {
            match fs::canonicalize(&link) {
                Ok(target) if target.starts_with(&root_base) && target.is_dir() =>
                    target.file_name().and_then(|s| s.to_str()).filter(|s| safe_kernel_name(s))
                        .map(str::to_owned),
                _ => {
                    limitations.push("PLATFORM_DRIVER_LINK_UNQUALIFIED".to_owned());
                    None
                }
            }
        } else {
            None
        };
        devices.push(PlatformDriverObservation { kernel_name: name, driver_bound: driver });
    }
    devices
}

/// Read kernel-projected ARM64 board information; never enroll a Host ID.
///
/// `host_arch` is a caller assertion, not an independently attested machine
/// identity. A synthetic root remains synthetic, even when it contains a
/// plausible board model or driver symlink.
///
/// This is *not wired* into `prime.hardware-graph.v1` or `primed` readiness.
pub fn observe(root: &Path, host_arch: &str) -> Arm64BoardObservation {
    let mut result = Arm64BoardObservation {
        schema: SCHEMA,
        claimed_host_arch: host_arch.to_owned(),
        kernel_model: None,
        kernel_compatible: None,
        kernel_release: None,
        cpu_present: None,
        cpu_online: None,
        platform_devices: Vec::new(),
        model_source: None,
        compatible_source: None,
        kernel_release_source: None,
        limitations: Vec::new(),
        source_is_hardware_attestation: false,
        any_device_functionality_proven: false,
        firmware_or_device_modified: false,
    };
    if !matches!(host_arch, "aarch64" | "arm64") {
        result.limitations.push("HOST_NOT_IDENTIFIED_AS_ARM64".to_owned());
        return result;
    }
    let (model, model_source) = first_valid(root,
        &["/proc/device-tree/model", "/sys/firmware/devicetree/base/model"], parse_model);
    result.kernel_model = model;
    result.model_source = model_source;
    if result.kernel_model.is_none() {
        result.limitations.push("MODEL_NOT_OBSERVED".to_owned());
    }
    let (compatible, compatible_source) = first_valid(root,
        &["/proc/device-tree/compatible", "/sys/firmware/devicetree/base/compatible"],
        parse_compatible);
    result.kernel_compatible = compatible;
    result.compatible_source = compatible_source;
    if result.kernel_compatible.is_none() {
        result.limitations.push("COMPATIBLE_NOT_OBSERVED".to_owned());
    }
    let (release, release_source) = first_valid(root,
        &["/proc/sys/kernel/osrelease"], parse_release);
    result.kernel_release = release;
    result.kernel_release_source = release_source;
    if result.kernel_release.is_none() {
        result.limitations.push("KERNEL_RELEASE_NOT_OBSERVED".to_owned());
    }
    result.cpu_present = confined_bytes(root, "/sys/devices/system/cpu/present")
        .as_deref().and_then(parse_cpu_list);
    if result.cpu_present.is_none() {
        result.limitations.push("CPU_PRESENT_NOT_OBSERVED".to_owned());
    }
    result.cpu_online = confined_bytes(root, "/sys/devices/system/cpu/online")
        .as_deref().and_then(parse_cpu_list);
    if result.cpu_online.is_none() {
        result.limitations.push("CPU_ONLINE_NOT_OBSERVED".to_owned());
    }
    if let (Some(present), Some(online)) = (&result.cpu_present, &result.cpu_online) {
        if online.iter().any(|cpu| present.binary_search(cpu).is_err()) {
            result.limitations.push("CPU_ONLINE_NOT_SUBSET_OF_PRESENT".to_owned());
        }
    }
    result.platform_devices = platform_devices(root, &mut result.limitations);
    result.limitations.sort();
    result.limitations.dedup();
    result
}

/// Candidate-only JSON projection compatible with the existing Python board
/// probe's schema, with additive CPU/platform fields. Does not imply that
/// observations came from a physical or currently connected handset.
pub fn candidate_json(result: &Arm64BoardObservation, observed_at: Option<&str>) -> serde_json::Value {
    let devices = result.platform_devices.iter().map(|dev| serde_json::json!({
        "kernel_name": dev.kernel_name,
        "driver_bound": dev.driver_bound,
        "device_functionality_proven": false,
    })).collect::<Vec<_>>();
    serde_json::json!({
        "schema": SCHEMA,
        "status": "KERNEL_READ_ONLY_NOT_DEVICE_CERTIFICATION",
        "observed_at": observed_at,
        "host_arch": result.claimed_host_arch,
        "host_arch_attested": false,
        "board_model": result.kernel_model,
        "board_compatible": result.kernel_compatible,
        "kernel_release": result.kernel_release,
        "cpu_present": result.cpu_present,
        "cpu_online": result.cpu_online,
        "platform_devices": devices,
        "sources": {
            "model": result.model_source,
            "compatible": result.compatible_source,
            "kernel_release": result.kernel_release_source,
        },
        "limitations": result.limitations,
        "source_is_hardware_attestation": false,
        "any_device_functionality_proven": false,
        "firmware_or_device_modified": false,
        "proof_boundary": "Kernel-projected board/CPU/driver information does not attest device identity, bootloader state, actual driver functionality, telephony, recovery, or safe flashing.",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel.trim_start_matches('/'));
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, bytes).expect("write");
    }

    #[test]
    fn arm64_dt_cpu_and_platform_driver_are_only_unattested_evidence() {
        let temp = tempfile::tempdir().expect("fixture");
        let root = temp.path();
        write(root, "/sys/firmware/devicetree/base/model", b"Test Phone\0");
        write(root, "/sys/firmware/devicetree/base/compatible", b"vendor,board\0vendor,soc\0");
        write(root, "/proc/sys/kernel/osrelease", b"6.12-test\n");
        write(root, "/sys/devices/system/cpu/present", b"0-3,6\n");
        write(root, "/sys/devices/system/cpu/online", b"0-2,6\n");
        let device = root.join("sys/devices/platform/test-device");
        let driver = root.join("sys/bus/platform/drivers/test-driver");
        fs::create_dir_all(&device).expect("device");
        fs::create_dir_all(&driver).expect("driver");
        fs::create_dir_all(root.join("sys/bus/platform/devices")).expect("bus");
        #[cfg(unix)] {
            std::os::unix::fs::symlink(&device,
                root.join("sys/bus/platform/devices/test-device")).expect("device link");
            std::os::unix::fs::symlink(&driver, device.join("driver")).expect("driver link");
        }
        let r = observe(root, "aarch64");
        assert_eq!(r.kernel_model.as_deref(), Some("Test Phone"));
        assert_eq!(r.kernel_compatible.as_ref().map(Vec::len), Some(2));
        assert_eq!(r.cpu_present, Some(vec![0,1,2,3,6]));
        assert_eq!(r.cpu_online, Some(vec![0,1,2,6]));
        #[cfg(unix)] assert_eq!(r.platform_devices[0].driver_bound.as_deref(), Some("test-driver"));
        assert!(!r.source_is_hardware_attestation);
        assert!(!r.any_device_functionality_proven);
        assert!(!r.firmware_or_device_modified);
    }

    #[test]
    fn missing_dt_and_bus_are_unknown_not_fake_success() {
        let root = tempfile::tempdir().expect("fixture");
        let r = observe(root.path(), "aarch64");
        assert!(r.kernel_model.is_none());
        assert!(r.kernel_compatible.is_none());
        assert!(r.platform_devices.is_empty());
        assert!(r.limitations.contains(&"MODEL_NOT_OBSERVED".to_owned()));
        assert!(r.limitations.contains(&"PLATFORM_BUS_UNAVAILABLE".to_owned()));
    }

    #[test]
    fn no_x86_desktop_invasion() {
        let root = tempfile::tempdir().expect("fixture");
        let r = observe(root.path(), "x86_64");
        assert_eq!(r.limitations, vec!["HOST_NOT_IDENTIFIED_AS_ARM64".to_owned()]);
        assert!(r.cpu_present.is_none());
    }

    #[test]
    fn rejects_unterminated_or_invalid_compatible() {
        assert!(parse_compatible(b"vendor,board").is_none());
        assert!(parse_compatible(b"\0").is_none());
        assert!(parse_compatible(b"\xff\0").is_none());
        assert_eq!(parse_compatible(b"vendor,board\0vendor,soc\0"),
            Some(vec!["vendor,board".to_owned(),"vendor,soc".to_owned()]));
    }

    #[test]
    fn rejects_unbounded_overlapping_cpu_lists() {
        assert!(parse_cpu_list(b"0-4294967295").is_none());
        assert!(parse_cpu_list(b"0-3,3-5").is_none());
        assert!(parse_cpu_list(b"0-,3").is_none());
        assert!(parse_cpu_list(b"1-0").is_none());
        assert_eq!(parse_cpu_list(b"0-3,6\n"), Some(vec![0,1,2,3,6]));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_outside_fixture_root_is_rejected() {
        let temp = tempfile::tempdir().expect("fixture");
        let root = temp.path();
        fs::create_dir_all(root.join("proc/device-tree")).expect("directory");
        std::os::unix::fs::symlink("/etc/passwd", root.join("proc/device-tree/model"))
            .expect("symlink");
        let r = observe(root, "aarch64");
        assert!(r.kernel_model.is_none());
    }

    #[test]
    fn inconsistent_online_cpu_is_explicit_limitation() {
        let root = tempfile::tempdir().expect("fixture");
        write(root.path(), "/sys/devices/system/cpu/present", b"0-1\n");
        write(root.path(), "/sys/devices/system/cpu/online", b"0,2\n");
        let r = observe(root.path(), "aarch64");
        assert!(r.limitations.contains(&"CPU_ONLINE_NOT_SUBSET_OF_PRESENT".to_owned()));
    }


    #[test]
    fn json_projection_reuses_candidate_board_schema_without_claiming_proof() {
        let root = tempfile::tempdir().expect("fixture");
        write(root.path(), "/sys/firmware/devicetree/base/model", b"Test Model\0");
        let r = observe(root.path(), "aarch64");
        let json = candidate_json(&r, Some("2026-11-15T14:30:00Z"));
        assert_eq!(json["schema"], "prime.mobile.board-observation.candidate.v0");
        assert_eq!(json["board_model"], "Test Model");
        assert_eq!(json["observed_at"], "2026-11-15T14:30:00Z");
        assert_eq!(json["host_arch_attested"], false);
        assert_eq!(json["any_device_functionality_proven"], false);
        assert_eq!(json["firmware_or_device_modified"], false);
    }


    #[cfg(unix)]
    #[test]
    fn escaping_platform_or_driver_symlinks_never_promote_a_device() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().expect("fixture");
        let root = temp.path();
        fs::create_dir_all(root.join("sys/bus/platform/devices")).expect("bus");
        fs::create_dir_all(root.join("sys/devices/platform/owned")).expect("device");
        symlink("/etc", root.join("sys/bus/platform/devices/escaping"))
            .expect("outside-root device link");
        symlink("/etc", root.join("sys/devices/platform/owned/driver"))
            .expect("outside-root driver link");
        symlink(root.join("sys/devices/platform/owned"),
            root.join("sys/bus/platform/devices/owned")).expect("inside-root link");
        let observation = observe(root, "aarch64");
        assert_eq!(observation.platform_devices.len(), 1);
        assert_eq!(observation.platform_devices[0].kernel_name, "owned");
        assert!(observation.platform_devices[0].driver_bound.is_none());
        assert!(observation.limitations.contains(&"PLATFORM_DEVICE_OUTSIDE_ROOT".to_owned()));
        assert!(observation.limitations.contains(&"PLATFORM_DRIVER_LINK_UNQUALIFIED".to_owned()));
        assert!(!observation.any_device_functionality_proven);
    }

    #[test]
    fn bounded_sources_and_private_device_fields_never_escape() {
        let temp = tempfile::tempdir().expect("fixture");
        let root = temp.path();
        write(root, "/sys/firmware/devicetree/base/model", &vec![b'X'; 4097]);
        write(root, "/sys/firmware/devicetree/base/serial-number", b"DO-NOT-EXPORT-BOARD-SERIAL\0");
        write(root, "/proc/sys/kernel/osrelease", b"6.12.0-test\n");
        let result = observe(root, "aarch64");
        assert!(result.kernel_model.is_none());
        assert_eq!(result.kernel_release.as_deref(), Some("6.12.0-test"));
        let projected = candidate_json(&result, None).to_string();
        assert!(!projected.contains("DO-NOT-EXPORT"));
        assert!(!result.source_is_hardware_attestation);
    }

    #[test]
    fn partial_cpu_topology_keeps_unknown_and_never_promotes_readiness() {
        let temp = tempfile::tempdir().expect("fixture");
        write(temp.path(), "/sys/devices/system/cpu/online", b"0-3\n");
        let r = observe(temp.path(), "aarch64");
        assert_eq!(r.cpu_online, Some(vec![0, 1, 2, 3]));
        assert!(r.cpu_present.is_none());
        assert!(r.limitations.contains(&"CPU_PRESENT_NOT_OBSERVED".to_owned()));
        let projection = candidate_json(&r, None);
        assert_eq!(projection["any_device_functionality_proven"], false);
        assert_eq!(projection["firmware_or_device_modified"], false);
    }

    #[test]
    fn repeated_observation_on_same_fixture_is_deterministic() {
        let root = tempfile::tempdir().expect("fixture");
        write(root.path(), "/sys/firmware/devicetree/base/model", b"Device X\0");
        assert_eq!(observe(root.path(), "aarch64"), observe(root.path(), "aarch64"));
    }
}
