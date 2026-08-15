use prime_contracts::{
    BlockHardware, CpuHardware, DisplayConnector, FingerprintConfidence, FirmwareHardware,
    HardwareFingerprint, HardwareInventory, InputHardware, MemoryHardware, NetworkHardware,
    PciHardware, SoundHardware, SystemHardware, ThermalHardware, TpmHardware, UsbHardware,
    VirtualizationHardware,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("hardware topology could not be serialized: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResult {
    pub inventory: HardwareInventory,
    pub limitations: Vec<String>,
    pub fingerprint: HardwareFingerprint,
    pub topology_digest: String,
}

pub fn probe(root: &Path, host_arch: &str) -> Result<ProbeResult, ProbeError> {
    let mut limitations = Vec::new();
    let (system, fingerprint) = probe_system(root, &mut limitations);
    let firmware = probe_firmware(root);
    let cpu = probe_cpu(root, &mut limitations);
    let memory = probe_memory(root, &mut limitations);
    let pci_devices = probe_pci(root, &mut limitations);
    let usb_devices = probe_usb(root, &mut limitations);
    let block_devices = probe_block(root, &mut limitations);
    let network_interfaces = probe_network(root, &mut limitations);
    let display_connectors = probe_display(root);
    let input_devices = probe_input(root);
    let sound_cards = probe_sound(root);
    let thermal_zones = probe_thermal(root);
    let tpm_devices = probe_tpm(root);
    let virtualization = VirtualizationHardware {
        cpu_vmx: cpu.supports_vmx,
        cpu_svm: cpu.supports_svm,
        kvm_device_present: rooted(root, "/dev/kvm").exists(),
    };

    limitations.sort();
    limitations.dedup();

    let inventory = HardwareInventory {
        host_arch: host_arch.to_owned(),
        system,
        firmware,
        cpu,
        memory,
        pci_devices,
        usb_devices,
        block_devices,
        network_interfaces,
        display_connectors,
        input_devices,
        sound_cards,
        thermal_zones,
        tpm_devices,
        virtualization,
    };

    let canonical = serde_json::to_vec(&(&inventory, &limitations))?;
    let topology_digest = sha256_labelled(&canonical);

    Ok(ProbeResult {
        inventory,
        limitations,
        fingerprint,
        topology_digest,
    })
}

fn probe_system(root: &Path, limitations: &mut Vec<String>) -> (SystemHardware, HardwareFingerprint) {
    let dmi = rooted(root, "/sys/class/dmi/id");
    if !dmi.exists() {
        limitations.push("DMI system identity is unavailable".to_owned());
    }

    let system = SystemHardware {
        vendor: read_text(dmi.join("sys_vendor")),
        product_name: read_text(dmi.join("product_name")),
        product_version: read_text(dmi.join("product_version")),
        board_vendor: read_text(dmi.join("board_vendor")),
        board_name: read_text(dmi.join("board_name")),
        board_version: read_text(dmi.join("board_version")),
    };

    let private_fields = [
        ("product_uuid", read_text(dmi.join("product_uuid"))),
        ("product_serial", read_text(dmi.join("product_serial"))),
        ("board_serial", read_text(dmi.join("board_serial"))),
        ("chassis_serial", read_text(dmi.join("chassis_serial"))),
    ];
    let descriptor_fields = [
        ("sys_vendor", system.vendor.clone()),
        ("product_name", system.product_name.clone()),
        ("board_vendor", system.board_vendor.clone()),
        ("board_name", system.board_name.clone()),
    ];

    let meaningful_private = private_fields
        .iter()
        .filter_map(|(name, value)| meaningful_identity(value.as_deref()).map(|value| (*name, value)))
        .collect::<Vec<_>>();
    let meaningful_descriptors = descriptor_fields
        .iter()
        .filter_map(|(name, value)| meaningful_identity(value.as_deref()).map(|value| (*name, value)))
        .collect::<Vec<_>>();

    let has_uuid = meaningful_private
        .iter()
        .any(|(name, _)| *name == "product_uuid");
    let confidence = if has_uuid || meaningful_private.len() >= 2 {
        FingerprintConfidence::High
    } else if meaningful_private.len() == 1 {
        FingerprintConfidence::Medium
    } else if meaningful_descriptors.len() >= 2 {
        FingerprintConfidence::Low
    } else {
        FingerprintConfidence::Unprobed
    };

    let mut material = String::from("prime-hardware-fingerprint-v1\n");
    for (name, value) in meaningful_private
        .iter()
        .chain(meaningful_descriptors.iter())
    {
        material.push_str(name);
        material.push('=');
        material.push_str(value);
        material.push('\n');
    }
    let digest = if matches!(&confidence, FingerprintConfidence::Unprobed) {
        None
    } else {
        Some(sha256_labelled(material.as_bytes()))
    };

    (
        system,
        HardwareFingerprint {
            algorithm: "sha256".to_owned(),
            digest,
            confidence,
            observed_at: None,
        },
    )
}

fn probe_firmware(root: &Path) -> FirmwareHardware {
    let dmi = rooted(root, "/sys/class/dmi/id");
    FirmwareHardware {
        uefi: rooted(root, "/sys/firmware/efi").exists(),
        bios_vendor: read_text(dmi.join("bios_vendor")),
        bios_version: read_text(dmi.join("bios_version")),
        bios_date: read_text(dmi.join("bios_date")),
    }
}

fn probe_cpu(root: &Path, limitations: &mut Vec<String>) -> CpuHardware {
    let Some(cpuinfo) = read_text(rooted(root, "/proc/cpuinfo")) else {
        limitations.push("CPU information is unavailable".to_owned());
        return CpuHardware::default();
    };

    let mut cpu = CpuHardware::default();
    for line in cpuinfo.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "processor" => cpu.logical_cpus = cpu.logical_cpus.saturating_add(1),
            "vendor_id" if cpu.vendor.is_none() => cpu.vendor = nonempty(value),
            "model name" if cpu.model.is_none() => cpu.model = nonempty(value),
            "flags" | "Features" => {
                for flag in value.split_whitespace() {
                    cpu.supports_vmx |= flag == "vmx";
                    cpu.supports_svm |= flag == "svm";
                }
            }
            _ => {}
        }
    }
    if cpu.logical_cpus == 0 {
        limitations.push("CPU logical processor count could not be determined".to_owned());
    }
    cpu
}

fn probe_memory(root: &Path, limitations: &mut Vec<String>) -> MemoryHardware {
    let Some(meminfo) = read_text(rooted(root, "/proc/meminfo")) else {
        limitations.push("Memory information is unavailable".to_owned());
        return MemoryHardware::default();
    };
    let total_bytes = meminfo.lines().find_map(|line| {
        let value = line.strip_prefix("MemTotal:")?.split_whitespace().next()?;
        value.parse::<u64>().ok()?.checked_mul(1024)
    });
    if total_bytes.is_none() {
        limitations.push("Physical memory total could not be determined".to_owned());
    }
    MemoryHardware { total_bytes }
}

fn probe_pci(root: &Path, limitations: &mut Vec<String>) -> Vec<PciHardware> {
    let base = rooted(root, "/sys/bus/pci/devices");
    let names = list_names_required(&base, "PCI inventory", limitations);
    names
        .into_iter()
        .map(|address| {
            let path = base.join(&address);
            let class_code = read_text(path.join("class")).map(normalize_hex);
            PciHardware {
                address,
                vendor_id: read_text(path.join("vendor")).map(normalize_hex),
                device_id: read_text(path.join("device")).map(normalize_hex),
                class_family: class_code.as_deref().and_then(pci_family),
                class_code,
                subsystem_vendor_id: read_text(path.join("subsystem_vendor")).map(normalize_hex),
                subsystem_device_id: read_text(path.join("subsystem_device")).map(normalize_hex),
                driver: symlink_basename(path.join("driver")),
            }
        })
        .collect()
}

fn probe_usb(root: &Path, limitations: &mut Vec<String>) -> Vec<UsbHardware> {
    let base = rooted(root, "/sys/bus/usb/devices");
    let names = list_names_required(&base, "USB inventory", limitations);
    names
        .into_iter()
        .filter_map(|kernel_name| {
            let path = base.join(&kernel_name);
            let vendor_id = read_text(path.join("idVendor"))?;
            let product_id = read_text(path.join("idProduct"))?;
            Some(UsbHardware {
                kernel_name,
                bus_number: read_u32(path.join("busnum")),
                device_number: read_u32(path.join("devnum")),
                vendor_id: normalize_hex(vendor_id),
                product_id: normalize_hex(product_id),
                device_class: read_text(path.join("bDeviceClass")).map(normalize_hex),
                device_subclass: read_text(path.join("bDeviceSubClass")).map(normalize_hex),
                device_protocol: read_text(path.join("bDeviceProtocol")).map(normalize_hex),
                manufacturer: read_text(path.join("manufacturer")),
                product: read_text(path.join("product")),
                speed_mbps: read_text(path.join("speed")),
            })
        })
        .collect()
}

fn probe_block(root: &Path, limitations: &mut Vec<String>) -> Vec<BlockHardware> {
    let base = rooted(root, "/sys/class/block");
    let names = list_names_required(&base, "block-device inventory", limitations);
    names
        .into_iter()
        .map(|kernel_name| {
            let path = base.join(&kernel_name);
            let kind = if path.join("partition").exists() {
                "PARTITION"
            } else if kernel_name.starts_with("loop")
                || kernel_name.starts_with("dm-")
                || kernel_name.starts_with("zram")
                || kernel_name.starts_with("ram")
            {
                "VIRTUAL"
            } else {
                "DISK"
            };
            BlockHardware {
                kernel_name,
                kind: kind.to_owned(),
                size_bytes: read_u64(path.join("size")).and_then(|sectors| sectors.checked_mul(512)),
                read_only: read_bool01(path.join("ro")),
                removable: read_bool01(path.join("removable")),
                rotational: read_bool01(path.join("queue/rotational")),
                logical_block_size: read_u64(path.join("queue/logical_block_size")),
                physical_block_size: read_u64(path.join("queue/physical_block_size")),
                vendor: read_text(path.join("device/vendor")),
                model: read_text(path.join("device/model")),
            }
        })
        .collect()
}

fn probe_network(root: &Path, limitations: &mut Vec<String>) -> Vec<NetworkHardware> {
    let base = rooted(root, "/sys/class/net");
    let names = list_names_required(&base, "network-interface inventory", limitations);
    names
        .into_iter()
        .map(|kernel_name| {
            let path = base.join(&kernel_name);
            NetworkHardware {
                kernel_name,
                interface_type: read_u32(path.join("type")),
                driver: symlink_basename(path.join("device/driver")),
                wireless: path.join("wireless").exists(),
            }
        })
        .collect()
}

fn probe_display(root: &Path) -> Vec<DisplayConnector> {
    let base = rooted(root, "/sys/class/drm");
    list_names_optional(&base)
        .into_iter()
        .filter_map(|kernel_name| {
            let path = base.join(&kernel_name);
            if !path.join("status").exists() {
                return None;
            }
            let mut modes = read_text(path.join("modes"))
                .map(|value| value.lines().filter_map(nonempty).collect::<Vec<_>>())
                .unwrap_or_default();
            modes.sort();
            modes.dedup();
            Some(DisplayConnector {
                kernel_name,
                status: read_text(path.join("status")),
                modes,
            })
        })
        .collect()
}

fn probe_input(root: &Path) -> Vec<InputHardware> {
    let base = rooted(root, "/sys/class/input");
    list_names_optional(&base)
        .into_iter()
        .filter(|name| name.starts_with("input"))
        .map(|kernel_name| InputHardware {
            name: read_text(base.join(&kernel_name).join("name")),
            kernel_name,
        })
        .collect()
}

fn probe_sound(root: &Path) -> Vec<SoundHardware> {
    let base = rooted(root, "/sys/class/sound");
    list_names_optional(&base)
        .into_iter()
        .filter(|name| name.starts_with("card"))
        .map(|kernel_name| SoundHardware {
            id: read_text(base.join(&kernel_name).join("id")),
            kernel_name,
        })
        .collect()
}

fn probe_thermal(root: &Path) -> Vec<ThermalHardware> {
    let base = rooted(root, "/sys/class/thermal");
    list_names_optional(&base)
        .into_iter()
        .filter(|name| name.starts_with("thermal_zone"))
        .map(|kernel_name| ThermalHardware {
            zone_type: read_text(base.join(&kernel_name).join("type")),
            kernel_name,
        })
        .collect()
}

fn probe_tpm(root: &Path) -> Vec<TpmHardware> {
    let base = rooted(root, "/sys/class/tpm");
    list_names_optional(&base)
        .into_iter()
        .filter(|name| name.starts_with("tpm"))
        .map(|kernel_name| TpmHardware { kernel_name })
        .collect()
}

fn list_names_required(base: &Path, family: &str, limitations: &mut Vec<String>) -> Vec<String> {
    let Ok(entries) = fs::read_dir(base) else {
        limitations.push(format!("{family} is unavailable"));
        return Vec::new();
    };
    sorted_names(entries)
}

fn list_names_optional(base: &Path) -> Vec<String> {
    fs::read_dir(base).map(sorted_names).unwrap_or_default()
}

fn sorted_names(entries: fs::ReadDir) -> Vec<String> {
    let mut names = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn rooted(root: &Path, absolute: &str) -> PathBuf {
    root.join(absolute.trim_start_matches('/'))
}

fn read_text(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path).ok().and_then(|value| nonempty(&value))
}

fn read_u64(path: impl AsRef<Path>) -> Option<u64> {
    read_text(path)?.parse().ok()
}

fn read_u32(path: impl AsRef<Path>) -> Option<u32> {
    read_text(path)?.parse().ok()
}

fn read_bool01(path: impl AsRef<Path>) -> Option<bool> {
    match read_text(path)?.as_str() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn nonempty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn normalize_hex(value: String) -> String {
    value.trim().to_ascii_lowercase()
}

fn pci_family(class_code: &str) -> Option<String> {
    let code = u32::from_str_radix(class_code.trim_start_matches("0x"), 16).ok()?;
    let family = match (code >> 16) & 0xff {
        0x01 => "STORAGE",
        0x02 => "NETWORK",
        0x03 => "DISPLAY",
        0x04 => "MULTIMEDIA",
        0x06 => "BRIDGE",
        0x0c => "SERIAL_BUS",
        _ => "OTHER",
    };
    Some(family.to_owned())
}

fn symlink_basename(path: impl AsRef<Path>) -> Option<String> {
    fs::read_link(path)
        .ok()?
        .file_name()?
        .to_str()
        .map(str::to_owned)
}

fn meaningful_identity(value: Option<&str>) -> Option<String> {
    let value = value?.trim().to_ascii_lowercase();
    let compact = value.replace('-', "");
    let generic = value.is_empty()
        || matches!(
            value.as_str(),
            "none"
                | "unknown"
                | "not specified"
                | "default string"
                | "to be filled by o.e.m."
                | "to be filled by oem"
        )
        || (!compact.is_empty() && compact.chars().all(|character| character == '0'))
        || (!compact.is_empty() && compact.chars().all(|character| character == 'f'));
    (!generic).then_some(value)
}

fn sha256_labelled(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(7 + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(root: &Path, path: &str, value: &str) {
        let path = rooted(root, path);
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture dir");
        fs::write(path, value).expect("write fixture");
    }

    fn fixture() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        write(
            root,
            "/proc/cpuinfo",
            "processor : 0\nvendor_id : GenuineIntel\nmodel name : Intel(R) Core(TM) i7-10700 CPU @ 2.90GHz\nflags : fpu vmx sse\nprocessor : 1\nvendor_id : GenuineIntel\nmodel name : Intel(R) Core(TM) i7-10700 CPU @ 2.90GHz\nflags : fpu vmx sse\n",
        );
        write(root, "/proc/meminfo", "MemTotal:       8192000 kB\n");
        write(root, "/sys/class/dmi/id/sys_vendor", "HP\n");
        write(root, "/sys/class/dmi/id/product_name", "HP 290 G4 Microtower PC\n");
        write(root, "/sys/class/dmi/id/board_vendor", "HP\n");
        write(root, "/sys/class/dmi/id/board_name", "8767\n");
        write(root, "/sys/class/dmi/id/product_uuid", "00112233-4455-6677-8899-aabbccddeeff\n");
        write(root, "/sys/class/dmi/id/product_serial", "PRIVATE-SERIAL\n");
        write(root, "/sys/class/dmi/id/bios_vendor", "AMI\n");
        write(root, "/sys/class/dmi/id/bios_version", "P1\n");
        fs::create_dir_all(rooted(root, "/sys/firmware/efi")).expect("efi fixture");
        write(root, "/sys/bus/pci/devices/0000:00:02.0/vendor", "0x8086\n");
        write(root, "/sys/bus/pci/devices/0000:00:02.0/device", "0x9bc5\n");
        write(root, "/sys/bus/pci/devices/0000:00:02.0/class", "0x030000\n");
        write(root, "/sys/bus/pci/devices/0000:00:1f.6/vendor", "0x8086\n");
        write(root, "/sys/bus/pci/devices/0000:00:1f.6/device", "0x0d4c\n");
        write(root, "/sys/bus/pci/devices/0000:00:1f.6/class", "0x020000\n");
        write(root, "/sys/bus/usb/devices/1-2/idVendor", "046d\n");
        write(root, "/sys/bus/usb/devices/1-2/idProduct", "c077\n");
        write(root, "/sys/bus/usb/devices/1-2/product", "USB Optical Mouse\n");
        write(root, "/sys/bus/usb/devices/1-2/serial", "DO-NOT-EXPORT\n");
        write(root, "/sys/class/block/nvme0n1/size", "1953525168\n");
        write(root, "/sys/class/block/nvme0n1/ro", "0\n");
        write(root, "/sys/class/block/nvme0n1/removable", "0\n");
        write(root, "/sys/class/block/nvme0n1/queue/rotational", "0\n");
        write(root, "/sys/class/block/nvme0n1/queue/logical_block_size", "512\n");
        write(root, "/sys/class/block/nvme0n1/queue/physical_block_size", "512\n");
        write(root, "/sys/class/block/nvme0n1/device/model", "Samsung NVMe\n");
        write(root, "/sys/class/block/nvme0n1/device/serial", "DO-NOT-EXPORT-DISK\n");
        write(root, "/sys/class/net/enp0s31f6/type", "1\n");
        write(root, "/sys/class/net/enp0s31f6/address", "00:11:22:33:44:55\n");
        write(root, "/sys/class/drm/card0-HDMI-A-1/status", "connected\n");
        write(root, "/sys/class/drm/card0-HDMI-A-1/modes", "1920x1080\n1280x720\n");
        write(root, "/sys/class/input/input0/name", "AT Translated Set 2 keyboard\n");
        write(root, "/sys/class/sound/card0/id", "PCH\n");
        write(root, "/sys/class/thermal/thermal_zone0/type", "x86_pkg_temp\n");
        fs::create_dir_all(rooted(root, "/sys/class/tpm/tpm0")).expect("tpm fixture");
        write(root, "/dev/kvm", "fixture\n");
        temp
    }

    #[test]
    fn probes_generic_linux_hardware_without_exporting_private_ids() {
        let temp = fixture();
        let result = probe(temp.path(), "x86_64").expect("probe");

        assert_eq!(result.inventory.cpu.logical_cpus, 2);
        assert!(result.inventory.cpu.supports_vmx);
        assert_eq!(result.inventory.memory.total_bytes, Some(8_388_608_000));
        assert_eq!(result.inventory.pci_devices.len(), 2);
        assert_eq!(
            result.inventory.pci_devices[0].class_family.as_deref(),
            Some("DISPLAY")
        );
        assert_eq!(result.inventory.usb_devices.len(), 1);
        assert_eq!(result.inventory.block_devices.len(), 1);
        assert_eq!(result.inventory.display_connectors.len(), 1);
        assert!(result.inventory.virtualization.kvm_device_present);
        assert!(matches!(
            result.fingerprint.confidence,
            FingerprintConfidence::High
        ));

        let serialized = serde_json::to_string(&result.inventory).expect("serialize inventory");
        assert!(!serialized.contains("00112233-4455-6677-8899-aabbccddeeff"));
        assert!(!serialized.contains("PRIVATE-SERIAL"));
        assert!(!serialized.contains("DO-NOT-EXPORT"));
        assert!(!serialized.contains("00:11:22:33:44:55"));
    }

    #[test]
    fn topology_digest_is_deterministic_and_changes_with_topology() {
        let temp = fixture();
        let first = probe(temp.path(), "x86_64").expect("first probe");
        let second = probe(temp.path(), "x86_64").expect("second probe");
        assert_eq!(first.topology_digest, second.topology_digest);

        write(
            temp.path(),
            "/sys/class/block/nvme0n1/device/model",
            "Replacement NVMe\n",
        );
        let changed = probe(temp.path(), "x86_64").expect("changed probe");
        assert_ne!(first.topology_digest, changed.topology_digest);
    }

    #[test]
    fn missing_kernel_families_are_degraded_not_invented() {
        let temp = tempfile::tempdir().expect("tempdir");
        let result = probe(temp.path(), "x86_64").expect("partial probe");
        assert!(!result.limitations.is_empty());
        assert_eq!(result.inventory.cpu.logical_cpus, 0);
        assert!(matches!(
            result.fingerprint.confidence,
            FingerprintConfidence::Unprobed
        ));
    }
}
