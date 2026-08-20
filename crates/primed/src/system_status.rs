use prime_contracts::{
    CapabilityAccepts, CapabilityAvailability, CapabilityDescriptor, CapabilityHealth,
    CapabilityPlacement, CapabilityProvider, CapabilityRollback, HardwareGraph, HealthStatus,
};
use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

pub const SYSTEM_STATUS_SCHEMA: &str = "prime.system-status.v1";
pub const SYSTEM_STATUS_CAPABILITY: &str = "prime.system.status";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct NetworkLinkStatus {
    interface: String,
    wireless: bool,
    oper_state: Option<String>,
    carrier: Option<bool>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct AudioCardStatus {
    kernel_name: String,
    id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PowerSupplyStatus {
    kernel_name: String,
    supply_type: Option<String>,
    status: Option<String>,
    capacity_percent: Option<u8>,
    online: Option<bool>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ThermalZoneStatus {
    kernel_name: String,
    zone_type: Option<String>,
    temperature_millicelsius: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ControlTruth {
    ready: bool,
    limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct SystemStatusSnapshot {
    schema: &'static str,
    observed_at: String,
    network_links: Vec<NetworkLinkStatus>,
    audio_cards: Vec<AudioCardStatus>,
    power_supplies: Vec<PowerSupplyStatus>,
    thermal_zones: Vec<ThermalZoneStatus>,
    network_control: ControlTruth,
    audio_control: ControlTruth,
    power_mutation: ControlTruth,
    limitations: Vec<String>,
}

pub fn capability_descriptor(
    root: &Path,
    hardware: &HardwareGraph,
    provider: CapabilityProvider,
    placement: CapabilityPlacement,
    observed_at: String,
) -> CapabilityDescriptor {
    let status = observe(root, hardware, observed_at.clone());
    let availability = if status.limitations.is_empty() {
        CapabilityAvailability::Available
    } else {
        CapabilityAvailability::Degraded
    };
    let health_status = if status.limitations.is_empty() {
        HealthStatus::Healthy
    } else {
        HealthStatus::Degraded
    };
    let limitations = status.limitations.clone();
    let resources = serde_json::to_value(status).unwrap_or_else(|_| {
        json!({
            "schema": SYSTEM_STATUS_SCHEMA,
            "observed_at": observed_at,
            "limitations": ["Prime could not serialize system status"]
        })
    });

    CapabilityDescriptor {
        capability_id: SYSTEM_STATUS_CAPABILITY.to_owned(),
        capability_version: "1.0.0".to_owned(),
        family: "system".to_owned(),
        provider,
        availability,
        effects: Vec::new(),
        accepts: CapabilityAccepts::default(),
        permissions: vec!["prime.system.read".to_owned()],
        resources,
        hardware_requirements: Vec::new(),
        limits: json!({
            "network_control": false,
            "audio_control": false,
            "power_mutation": false,
            "raw_mac_addresses": false,
            "ip_configuration": false,
        }),
        health: CapabilityHealth {
            status: health_status,
            observed_at,
            evidence_refs: vec![SYSTEM_STATUS_SCHEMA.to_owned()],
        },
        limitations,
        placement,
        expected_evidence: vec![SYSTEM_STATUS_SCHEMA.to_owned()],
        rollback: CapabilityRollback {
            supported: false,
            mode: None,
            limitations: vec!["System status is observation, not a rollback object".to_owned()],
        },
    }
}

fn observe(root: &Path, hardware: &HardwareGraph, observed_at: String) -> SystemStatusSnapshot {
    let mut limitations = Vec::new();
    let mut network_links = Vec::new();
    for interface in &hardware.inventory.network_interfaces {
        let base = rooted(root, &format!("/sys/class/net/{}", interface.kernel_name));
        let oper_state = read_trimmed(base.join("operstate"));
        let carrier = read_trimmed(base.join("carrier")).and_then(|value| match value.as_str() {
            "1" => Some(true),
            "0" => Some(false),
            _ => None,
        });
        if oper_state.is_none() {
            limitations.push(format!(
                "network operstate unavailable for {}",
                interface.kernel_name
            ));
        }
        network_links.push(NetworkLinkStatus {
            interface: interface.kernel_name.clone(),
            wireless: interface.wireless,
            oper_state,
            carrier,
        });
    }

    let audio_cards = hardware
        .inventory
        .sound_cards
        .iter()
        .map(|card| AudioCardStatus {
            kernel_name: card.kernel_name.clone(),
            id: card.id.clone(),
        })
        .collect();

    let power_root = rooted(root, "/sys/class/power_supply");
    let power_supplies = list_names(&power_root)
        .into_iter()
        .map(|kernel_name| {
            let base = power_root.join(&kernel_name);
            PowerSupplyStatus {
                kernel_name,
                supply_type: read_trimmed(base.join("type")),
                status: read_trimmed(base.join("status")),
                capacity_percent: read_trimmed(base.join("capacity"))
                    .and_then(|value| value.parse::<u8>().ok()),
                online: read_trimmed(base.join("online")).and_then(|value| match value.as_str() {
                    "1" => Some(true),
                    "0" => Some(false),
                    _ => None,
                }),
            }
        })
        .collect();

    let thermal_zones = hardware
        .inventory
        .thermal_zones
        .iter()
        .map(|zone| {
            let base = rooted(root, &format!("/sys/class/thermal/{}", zone.kernel_name));
            ThermalZoneStatus {
                kernel_name: zone.kernel_name.clone(),
                zone_type: zone.zone_type.clone(),
                temperature_millicelsius: read_trimmed(base.join("temp"))
                    .and_then(|value| value.parse::<i64>().ok()),
            }
        })
        .collect();

    limitations.sort();
    limitations.dedup();
    SystemStatusSnapshot {
        schema: SYSTEM_STATUS_SCHEMA,
        observed_at,
        network_links,
        audio_cards,
        power_supplies,
        thermal_zones,
        network_control: ControlTruth {
            ready: false,
            limitations: vec!["Prime P1 has not earned a network mutation backend yet".to_owned()],
        },
        audio_control: ControlTruth {
            ready: false,
            limitations: vec![
                "Prime P1 has not earned an audio mixer/control backend yet".to_owned()
            ],
        },
        power_mutation: ControlTruth {
            ready: true,
            limitations: Vec::new(),
        },
        limitations,
    }
}

fn rooted(root: &Path, absolute: &str) -> PathBuf {
    root.join(absolute.trim_start_matches('/'))
}

fn read_trimmed(path: PathBuf) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn list_names(path: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };
    let mut names = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use prime_contracts::{HardwareInventory, NetworkHardware, SoundHardware, ThermalHardware};
    use tempfile::TempDir;

    fn write(root: &Path, path: &str, value: &str) {
        let path = rooted(root, path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, value).expect("write fixture");
    }

    #[test]
    fn status_is_sanitized_and_fail_closed_for_controls() {
        let temp = TempDir::new().expect("tempdir");
        write(temp.path(), "/sys/class/net/enp1s0/operstate", "up\n");
        write(temp.path(), "/sys/class/net/enp1s0/carrier", "1\n");
        write(temp.path(), "/sys/class/power_supply/AC/type", "Mains\n");
        write(temp.path(), "/sys/class/power_supply/AC/online", "1\n");
        write(
            temp.path(),
            "/sys/class/thermal/thermal_zone0/temp",
            "47000\n",
        );

        let hardware = HardwareGraph {
            schema: "prime.hardware-graph.v1".to_owned(),
            revision: 1,
            topology_digest: "sha256:test".to_owned(),
            observed_at: "2026-08-20T00:00:00Z".to_owned(),
            inventory: HardwareInventory {
                network_interfaces: vec![NetworkHardware {
                    kernel_name: "enp1s0".to_owned(),
                    wireless: false,
                    ..NetworkHardware::default()
                }],
                sound_cards: vec![SoundHardware {
                    kernel_name: "card0".to_owned(),
                    id: Some("PCH".to_owned()),
                }],
                thermal_zones: vec![ThermalHardware {
                    kernel_name: "thermal_zone0".to_owned(),
                    zone_type: Some("x86_pkg_temp".to_owned()),
                }],
                ..HardwareInventory::default()
            },
            limitations: Vec::new(),
        };

        let snapshot = observe(temp.path(), &hardware, "2026-08-20T00:00:01Z".to_owned());
        assert_eq!(snapshot.network_links[0].oper_state.as_deref(), Some("up"));
        assert_eq!(snapshot.network_links[0].carrier, Some(true));
        assert_eq!(snapshot.power_supplies[0].online, Some(true));
        assert_eq!(
            snapshot.thermal_zones[0].temperature_millicelsius,
            Some(47000)
        );
        assert!(!snapshot.network_control.ready);
        assert!(!snapshot.audio_control.ready);
        assert!(snapshot.power_mutation.ready);
    }
}
