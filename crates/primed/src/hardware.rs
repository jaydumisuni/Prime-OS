use prime_contracts::{HardwareGraph, HARDWARE_GRAPH_SCHEMA};
use prime_hardware::ProbeResult;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

const P1_PROOF_VENDOR: &str = "HP";
const P1_PROOF_PRODUCT: &str = "HP 290 G4 Microtower PC";
const P1_MIN_MEMORY_BYTES: u64 = 8_000_000_000;
const P1_PRIMARY_DISK_MIN_BYTES: u64 = 900_000_000_000;
const P1_SECONDARY_DISK_MIN_BYTES: u64 = 450_000_000_000;

#[derive(Debug, Error)]
pub enum HardwareStateError {
    #[error("hardware state I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("hardware state JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("hardware graph schema is {found}, expected {expected}")]
    Schema {
        found: String,
        expected: &'static str,
    },
    #[error("hardware graph revision overflow")]
    RevisionOverflow,
}

pub fn load_or_update(
    path: &Path,
    probe: ProbeResult,
    observed_at: String,
) -> Result<HardwareGraph, HardwareStateError> {
    let previous = if path.exists() {
        Some(load(path)?)
    } else {
        None
    };
    let revision = match previous {
        Some(ref previous) if previous.topology_digest == probe.topology_digest => {
            previous.revision
        }
        Some(ref previous) => previous
            .revision
            .checked_add(1)
            .ok_or(HardwareStateError::RevisionOverflow)?,
        None => 1,
    };
    let graph = HardwareGraph {
        schema: HARDWARE_GRAPH_SCHEMA.to_owned(),
        revision,
        topology_digest: probe.topology_digest,
        observed_at,
        inventory: probe.inventory,
        limitations: probe.limitations,
    };
    write_atomic(path, &serde_json::to_vec_pretty(&graph)?, 0o600)?;
    Ok(graph)
}

pub fn p1_baseline_limitations(graph: &HardwareGraph) -> Vec<String> {
    let inventory = &graph.inventory;
    let mut limitations = graph
        .limitations
        .iter()
        .map(|item| format!("Hardware probe limitation: {item}"))
        .collect::<Vec<_>>();

    if inventory.host_arch != "x86_64" {
        limitations.push(format!(
            "P1 proof Host architecture is {}, expected x86_64",
            inventory.host_arch
        ));
    }
    if inventory.system.vendor.as_deref() != Some(P1_PROOF_VENDOR) {
        limitations.push(format!(
            "P1 proof Host vendor is {:?}, expected {P1_PROOF_VENDOR}",
            inventory.system.vendor
        ));
    }
    if inventory.system.product_name.as_deref() != Some(P1_PROOF_PRODUCT) {
        limitations.push(format!(
            "P1 proof Host product is {:?}, expected {P1_PROOF_PRODUCT}",
            inventory.system.product_name
        ));
    }
    if !inventory.firmware.uefi {
        limitations.push("P1 proof Host is not running through UEFI".to_owned());
    }
    if inventory.cpu.vendor.as_deref() != Some("GenuineIntel") {
        limitations.push(format!(
            "P1 proof CPU vendor is {:?}, expected GenuineIntel",
            inventory.cpu.vendor
        ));
    }
    if !inventory
        .cpu
        .model
        .as_deref()
        .is_some_and(|model| model.contains("i7-10700"))
    {
        limitations.push(format!(
            "P1 proof CPU model is {:?}, expected Intel Core i7-10700",
            inventory.cpu.model
        ));
    }
    if inventory.memory.total_bytes.unwrap_or(0) < P1_MIN_MEMORY_BYTES {
        limitations.push(format!(
            "P1 proof memory is {:?} bytes, expected at least {P1_MIN_MEMORY_BYTES}",
            inventory.memory.total_bytes
        ));
    }

    let intel_i915_display = inventory.pci_devices.iter().any(|device| {
        device.vendor_id.as_deref() == Some("0x8086")
            && device.class_family.as_deref() == Some("DISPLAY")
            && device.driver.as_deref() == Some("i915")
    });
    if !intel_i915_display {
        limitations.push(
            "P1 proof Host has no Intel DISPLAY-class PCI device bound to i915".to_owned(),
        );
    }

    let usb_controller = inventory.pci_devices.iter().any(|device| {
        device
            .class_code
            .as_deref()
            .is_some_and(|class_code| class_code.starts_with("0x0c03"))
            && device
                .driver
                .as_deref()
                .is_some_and(|driver| !driver.is_empty())
    });
    if !usb_controller {
        limitations.push("P1 proof Host has no USB controller with a bound kernel driver".to_owned());
    }

    let connected_output = inventory.display_connectors.iter().any(|connector| {
        connector.status.as_deref() == Some("connected") && !connector.modes.is_empty()
    });
    if !connected_output {
        limitations.push(
            "P1 proof Host has no connected display connector with an advertised mode".to_owned(),
        );
    }
    if inventory.input_devices.is_empty() {
        limitations.push("P1 proof Host has no discovered input device".to_owned());
    }
    if inventory.sound_cards.is_empty() {
        limitations.push("P1 proof Host has no discovered sound card".to_owned());
    }

    let ethernet = inventory.network_interfaces.iter().any(|interface| {
        interface.interface_type == Some(1)
            && !interface.wireless
            && interface
                .driver
                .as_deref()
                .is_some_and(|driver| !driver.is_empty())
    });
    if !ethernet {
        limitations.push(
            "P1 proof Host has no Ethernet interface with a bound kernel driver".to_owned(),
        );
    }

    let mut disk_sizes = inventory
        .block_devices
        .iter()
        .filter(|device| {
            device.kind == "DISK"
                && device.read_only != Some(true)
                && device.removable != Some(true)
        })
        .filter_map(|device| device.size_bytes)
        .collect::<Vec<_>>();
    disk_sizes.sort_unstable_by(|left, right| right.cmp(left));
    if disk_sizes.first().copied().unwrap_or(0) < P1_PRIMARY_DISK_MIN_BYTES {
        limitations.push(format!(
            "P1 proof Host has no writable non-removable disk of at least {P1_PRIMARY_DISK_MIN_BYTES} bytes"
        ));
    }
    if disk_sizes.get(1).copied().unwrap_or(0) < P1_SECONDARY_DISK_MIN_BYTES {
        limitations.push(format!(
            "P1 proof Host has no second writable non-removable disk of at least {P1_SECONDARY_DISK_MIN_BYTES} bytes"
        ));
    }

    limitations.sort();
    limitations.dedup();
    limitations
}

pub fn load(path: &Path) -> Result<HardwareGraph, HardwareStateError> {
    let graph: HardwareGraph = serde_json::from_slice(&fs::read(path)?)?;
    if graph.schema != HARDWARE_GRAPH_SCHEMA {
        return Err(HardwareStateError::Schema {
            found: graph.schema,
            expected: HARDWARE_GRAPH_SCHEMA,
        });
    }
    Ok(graph)
}

fn write_atomic(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "hardware state path has no parent",
        )
    })?;
    fs::create_dir_all(parent)?;
    let temp_path = parent.join(format!(".hardware.{}.tmp", Uuid::now_v7()));
    let mut temp = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&temp_path)?;
    temp.write_all(bytes)?;
    temp.write_all(b"\n")?;
    temp.sync_all()?;
    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use prime_contracts::{
        BlockHardware, CpuHardware, DisplayConnector, FingerprintConfidence, FirmwareHardware,
        HardwareFingerprint, HardwareInventory, InputHardware, MemoryHardware, NetworkHardware,
        PciHardware, SoundHardware, SystemHardware, VirtualizationHardware,
    };

    fn probe(digest: &str) -> ProbeResult {
        ProbeResult {
            inventory: HardwareInventory {
                host_arch: "x86_64".to_owned(),
                ..HardwareInventory::default()
            },
            limitations: Vec::new(),
            fingerprint: HardwareFingerprint {
                algorithm: "sha256".to_owned(),
                digest: None,
                confidence: FingerprintConfidence::Unprobed,
                observed_at: None,
            },
            topology_digest: digest.to_owned(),
        }
    }

    fn p1_graph() -> HardwareGraph {
        HardwareGraph {
            schema: HARDWARE_GRAPH_SCHEMA.to_owned(),
            revision: 1,
            topology_digest: "sha256:p1".to_owned(),
            observed_at: "2026-08-18T21:00:00Z".to_owned(),
            inventory: HardwareInventory {
                host_arch: "x86_64".to_owned(),
                system: SystemHardware {
                    vendor: Some("HP".to_owned()),
                    product_name: Some("HP 290 G4 Microtower PC".to_owned()),
                    ..SystemHardware::default()
                },
                firmware: FirmwareHardware {
                    uefi: true,
                    ..FirmwareHardware::default()
                },
                cpu: CpuHardware {
                    logical_cpus: 16,
                    vendor: Some("GenuineIntel".to_owned()),
                    model: Some("Intel(R) Core(TM) i7-10700 CPU @ 2.90GHz".to_owned()),
                    supports_vmx: true,
                    supports_svm: false,
                },
                memory: MemoryHardware {
                    total_bytes: Some(8_388_608_000),
                },
                pci_devices: vec![
                    PciHardware {
                        address: "0000:00:02.0".to_owned(),
                        vendor_id: Some("0x8086".to_owned()),
                        device_id: Some("0x9bc5".to_owned()),
                        class_code: Some("0x030000".to_owned()),
                        class_family: Some("DISPLAY".to_owned()),
                        subsystem_vendor_id: None,
                        subsystem_device_id: None,
                        driver: Some("i915".to_owned()),
                    },
                    PciHardware {
                        address: "0000:00:14.0".to_owned(),
                        vendor_id: Some("0x8086".to_owned()),
                        device_id: Some("0x43ed".to_owned()),
                        class_code: Some("0x0c0330".to_owned()),
                        class_family: Some("SERIAL_BUS".to_owned()),
                        subsystem_vendor_id: None,
                        subsystem_device_id: None,
                        driver: Some("xhci_hcd".to_owned()),
                    },
                ],
                block_devices: vec![
                    BlockHardware {
                        kernel_name: "nvme0n1".to_owned(),
                        kind: "DISK".to_owned(),
                        size_bytes: Some(1_000_204_886_016),
                        read_only: Some(false),
                        removable: Some(false),
                        ..BlockHardware::default()
                    },
                    BlockHardware {
                        kernel_name: "sda".to_owned(),
                        kind: "DISK".to_owned(),
                        size_bytes: Some(500_107_862_016),
                        read_only: Some(false),
                        removable: Some(false),
                        ..BlockHardware::default()
                    },
                ],
                network_interfaces: vec![NetworkHardware {
                    kernel_name: "enp0s31f6".to_owned(),
                    interface_type: Some(1),
                    driver: Some("e1000e".to_owned()),
                    wireless: false,
                }],
                display_connectors: vec![DisplayConnector {
                    kernel_name: "card0-HDMI-A-1".to_owned(),
                    status: Some("connected".to_owned()),
                    modes: vec!["1920x1080".to_owned()],
                }],
                input_devices: vec![InputHardware {
                    kernel_name: "input0".to_owned(),
                    name: Some("AT Translated Set 2 keyboard".to_owned()),
                }],
                sound_cards: vec![SoundHardware {
                    kernel_name: "card0".to_owned(),
                    id: Some("PCH".to_owned()),
                }],
                virtualization: VirtualizationHardware {
                    cpu_vmx: true,
                    cpu_svm: false,
                    kvm_device_present: true,
                },
                ..HardwareInventory::default()
            },
            limitations: Vec::new(),
        }
    }

    #[test]
    fn revision_changes_only_when_topology_digest_changes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("hardware/current.json");
        let first = load_or_update(&path, probe("sha256:a"), "t1".to_owned()).expect("first");
        let same = load_or_update(&path, probe("sha256:a"), "t2".to_owned()).expect("same");
        let changed = load_or_update(&path, probe("sha256:b"), "t3".to_owned()).expect("changed");
        assert_eq!(first.revision, 1);
        assert_eq!(same.revision, 1);
        assert_eq!(changed.revision, 2);
    }

    #[test]
    fn corrupt_existing_graph_fails_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("hardware/current.json");
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(&path, "broken").expect("write corrupt graph");
        assert!(load_or_update(&path, probe("sha256:a"), "t1".to_owned()).is_err());
    }

    #[test]
    fn p1_proof_host_baseline_accepts_the_frozen_hp_target() {
        assert!(p1_baseline_limitations(&p1_graph()).is_empty());
    }

    #[test]
    fn p1_proof_host_baseline_rejects_missing_graphics_usb_and_secondary_storage() {
        let mut graph = p1_graph();
        graph.inventory.pci_devices[0].driver = None;
        graph.inventory.pci_devices[1].driver = None;
        graph.inventory.block_devices.pop();
        let limitations = p1_baseline_limitations(&graph);
        assert!(limitations
            .iter()
            .any(|item| item.contains("bound to i915")));
        assert!(limitations
            .iter()
            .any(|item| item.contains("USB controller")));
        assert!(limitations
            .iter()
            .any(|item| item.contains("second writable non-removable disk")));
    }
}
