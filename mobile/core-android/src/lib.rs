//! Android-native, read-only transport adapter using *shared* Prime contract types.
 //! This is deliberately not the Linux systemd/bootc primed binary and cannot
 //! represent bootc generation admission, phone hardware acceptance or flash
 //! authority. No mutation route or listener over TCP is implemented.

use prime_contracts::{
    CapabilitiesProjection, CapabilityAccepts, CapabilityAvailability, CapabilityDescriptor,
    CapabilityHealth, CapabilityPlacement, CapabilityProvider, CapabilityRollback,
    HardwareGraph, HardwareProjection, HealthStatus, VersionsProjection, CAPABILITY_INTERFACE,
    CAPABILITY_INTERFACE_VERSION, HARDWARE_GRAPH_SCHEMA,
};
use prime_hardware::ProbeResult;
use serde_json::json;
use std::path::Path;
use uuid::Uuid;

const RESPONSE_LIMIT: usize = 128 * 1024;
const REQUEST_LIMIT: usize = 8192;
const UNQUALIFIED_GENERATION: &str = "ANDROID_UNBOUND_NOT_ATTESTED";

pub struct AndroidCoreSnapshot {
    hardware: HardwareGraph,
}

impl AndroidCoreSnapshot {
    pub fn probe() -> Result<Self, String> {
        let result = prime_hardware::probe(Path::new("/"), "aarch64")
            .map_err(|_| "ANDROID_READ_ONLY_PROBE_FAILED")?;
        Ok(Self::from_probe(result))
    }

    pub fn from_probe(result: ProbeResult) -> Self {
        let mut limitations = result.limitations;
        limitations.push(
            "Android host identity, bootchain, vendor HAL and recovery are not attested".into(),
        );
        limitations.push("No bootc generation binding or phone device admission".into());
        limitations.sort();
        limitations.dedup();
        Self {
            hardware: HardwareGraph {
                schema: HARDWARE_GRAPH_SCHEMA.into(),
                revision: 0,
                topology_digest: result.topology_digest,
                observed_at: "UNQUALIFIED_ANDROID_PROBE".into(),
                inventory: result.inventory,
                limitations,
            },
        }
    }

    fn capability(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            capability_id: "prime.hardware.read".into(),
            capability_version: "1.0.0".into(),
            family: "hardware".into(),
            provider: CapabilityProvider {
                id: "prime.mobile.android.adapter".into(),
                generation_id: UNQUALIFIED_GENERATION.into(),
            },
            availability: CapabilityAvailability::Degraded,
            effects: Vec::new(),
            accepts: CapabilityAccepts::default(),
            permissions: vec!["prime.hardware.read".into()],
            resources: json!({}),
            hardware_requirements: Vec::new(),
            limits: json!({"read_only": true}),
            health: CapabilityHealth {
                status: HealthStatus::Unknown,
                observed_at: self.hardware.observed_at.clone(),
                evidence_refs: Vec::new(),
            },
            limitations: self.hardware.limitations.clone(),
            placement: CapabilityPlacement {
                scope: "HOST_LOCAL_UNQUALIFIED".into(),
                host_id: Uuid::nil(),
            },
            expected_evidence: vec!["prime.hardware-graph.v1".into()],
            rollback: CapabilityRollback {
                supported: false,
                mode: None,
                limitations: vec!["This adapter has no installation or rollback authority".into()],
            },
        }
    }

    /// Interpret a bounded HTTP/1.1 request. Never dispatch arbitrary methods,
    /// filesystem actions, power, update, launch, or other Core mutations.
    pub fn reply(&self, request: &[u8]) -> Vec<u8> {
        if request.len() > REQUEST_LIMIT || request.len() < 18
            || !request.ends_with(b"\r\n\r\n")
        {
            return http(400, &json!({"error":"INVALID_REQUEST"}));
        }
        let Ok(message) = std::str::from_utf8(request) else {
            return http(400, &json!({"error":"INVALID_ENCODING"}));
        };
        let mut lines = message.split("\r\n");
        let first = lines.next().unwrap_or_default();
        let mut parts = first.split(' ');
        let (Some(method), Some(route), Some(version)) = (parts.next(), parts.next(), parts.next())
        else {
            return http(400, &json!({"error":"INVALID_REQUEST_LINE"}));
        };
        if parts.next().is_some() || version != "HTTP/1.1" || !route.starts_with('/') {
            return http(400, &json!({"error":"INVALID_REQUEST_LINE"}));
        }
        if method != "GET" {
            return http(405, &json!({"error":"READ_ONLY_ANDROID_ADAPTER"}));
        }
        if route == "/v1/versions" {
            return http(
                200,
                &json!(VersionsProjection {
                    interface: CAPABILITY_INTERFACE.into(),
                    supported_versions: vec![CAPABILITY_INTERFACE_VERSION.into()],
                }),
            );
        }

        let mut accept = None;
        for line in lines {
            if line.is_empty() {
                break;
            }
            let Some((header, value)) = line.split_once(':') else {
                return http(400, &json!({"error":"INVALID_HEADER"}));
            };
            if header.eq_ignore_ascii_case("Prime-Interface-Accept") {
                if accept.is_some() {
                    return http(400, &json!({"error":"DUPLICATE_ACCEPT_VERSION"}));
                }
                accept = Some(value.trim());
            }
        }
        match accept {
            None => return http(400, &json!({"error":"VERSION_REQUIRED"})),
            Some(v) if v != CAPABILITY_INTERFACE_VERSION => {
                return http(409, &json!({"error":"INCOMPATIBLE_VERSION",
                    "supported_versions":[CAPABILITY_INTERFACE_VERSION]}))
            }
            _ => {}
        }

        match route {
            "/v1/capabilities" => http(
                200,
                &json!(CapabilitiesProjection {
                    interface: CAPABILITY_INTERFACE.into(),
                    interface_version: CAPABILITY_INTERFACE_VERSION.into(),
                    host_id: Uuid::nil(),
                    generation_id: UNQUALIFIED_GENERATION.into(),
                    capabilities: vec![self.capability()],
                }),
            ),
            "/v1/hardware" => http(
                200,
                &json!(HardwareProjection {
                    interface: CAPABILITY_INTERFACE.into(),
                    interface_version: CAPABILITY_INTERFACE_VERSION.into(),
                    host_id: Uuid::nil(),
                    generation_id: UNQUALIFIED_GENERATION.into(),
                    graph: self.hardware.clone(),
                }),
            ),
            _ => http(404, &json!({"error":"UNSUPPORTED_READ_ONLY_ROUTE"})),
        }
    }
}

fn http(code: u16, value: &serde_json::Value) -> Vec<u8> {
    let payload = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
    if payload.len() > RESPONSE_LIMIT - 256 {
        return http(500, &json!({"error":"RESPONSE_TOO_LARGE"}));
    }
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        _ => "Internal Server Error",
    };
    let header = format!(
        "HTTP/1.1 {code} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        payload.len()
    );
    let mut response = header.into_bytes();
    response.extend(payload);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use prime_contracts::{FingerprintConfidence, HardwareFingerprint, HardwareInventory};
    use std::path::Path;

    fn snapshot() -> AndroidCoreSnapshot {
        // A genuine library probe on this host is source-only test context.
        let p = prime_hardware::probe(Path::new("/"), "aarch64").unwrap();
        AndroidCoreSnapshot::from_probe(p)
    }

    fn code(s: &[u8]) -> u16 {
        std::str::from_utf8(&s[9..12]).unwrap().parse().unwrap()
    }

    #[test]
    fn versions_and_capabilities_are_real_contract_types_not_boot_admission() {
        let s = snapshot();
        let v = s.reply(b"GET /v1/versions HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert_eq!(code(&v), 200);
        assert!(String::from_utf8_lossy(&v).contains("prime.capability.v1"));
        let c = s.reply(b"GET /v1/capabilities HTTP/1.1\r\nPrime-Interface-Accept: 1.0\r\n\r\n");
        assert_eq!(code(&c), 200);
        let text = String::from_utf8_lossy(&c);
        assert!(text.contains("prime.hardware.read"));
        assert!(text.contains("ANDROID_UNBOUND_NOT_ATTESTED"));
        assert!(text.contains("prime.mobile.android.adapter"));
        assert!(!text.contains("KNOWN_GOOD"));
    }

    #[test]
    fn hardware_projection_is_from_shared_real_probe_but_unqualified() {
        let s = snapshot();
        let r = s.reply(b"GET /v1/hardware HTTP/1.1\r\nPrime-Interface-Accept: 1.0\r\n\r\n");
        assert_eq!(code(&r), 200);
        let text = String::from_utf8_lossy(&r);
        assert!(text.contains(HARDWARE_GRAPH_SCHEMA));
        assert!(text.contains("UNQUALIFIED_ANDROID_PROBE"));
        assert!(text.contains("No bootc generation binding"));
    }

    #[test]
    fn mutation_routes_never_execute() {
        let s = snapshot();
        for p in ["/v1/shell/launch", "/v1/system/power", "/v1/storage/preflight"] {
            let r = format!("POST {p} HTTP/1.1\r\nPrime-Interface-Accept: 1.0\r\n\r\n");
            assert_eq!(code(&s.reply(r.as_bytes())), 405);
        }
    }

    #[test]
    fn negotiation_requires_version_and_rejects_incompatible() {
        let s = snapshot();
        assert_eq!(code(&s.reply(b"GET /v1/capabilities HTTP/1.1\r\n\r\n")), 400);
        assert_eq!(code(&s.reply(b"GET /v1/capabilities HTTP/1.1\r\nPrime-Interface-Accept: 9.0\r\n\r\n")), 409);
        assert_eq!(code(&s.reply(b"GET /v1/capabilities HTTP/1.1\r\nPrime-Interface-Accept: 1.0\r\nPrime-Interface-Accept: 1.0\r\n\r\n")), 400);
    }

    #[test]
    fn malformed_and_large_requests_are_rejected() {
        let s = snapshot();
        assert_eq!(code(&s.reply(b"hello")), 400);
        assert_eq!(code(&s.reply(&vec![b'a'; REQUEST_LIMIT + 1])), 400);
        assert_eq!(code(&s.reply(b"GET /v1/versions HTTP/1.1\r\n")), 400);
    }
}
