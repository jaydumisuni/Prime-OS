use prime_contracts::{
    ApplicationsProjection, ShellLaunchRequest, CAPABILITY_INTERFACE_VERSION,
    SHELL_LAUNCH_REQUEST_SCHEMA,
};
use serde_json::Value;
use std::{
    env,
    fmt,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    time::Duration,
};

const DEFAULT_CORE_SOCKET: &str = "/run/prime/core.sock";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub(crate) enum CoreClientError {
    Io(String),
    Protocol(String),
    Http(u16, String),
    Json(String),
}

impl fmt::Display for CoreClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(formatter, "I/O: {message}"),
            Self::Protocol(message) => write!(formatter, "protocol: {message}"),
            Self::Http(status, message) => write!(formatter, "HTTP {status}: {message}"),
            Self::Json(message) => write!(formatter, "JSON: {message}"),
        }
    }
}

pub(crate) struct CoreClient {
    socket_path: PathBuf,
}

impl CoreClient {
    pub(crate) fn from_env() -> Self {
        let socket_path = env::var_os("PRIME_CORE_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CORE_SOCKET));
        Self { socket_path }
    }

    pub(crate) fn applications(&self) -> Result<ApplicationsProjection, CoreClientError> {
        let body = self.request("GET", "/v1/applications", None)?;
        serde_json::from_slice(&body).map_err(|error| CoreClientError::Json(error.to_string()))
    }

    pub(crate) fn launch(&self, application_id: uuid::Uuid) -> Result<(), CoreClientError> {
        let request = ShellLaunchRequest {
            schema: SHELL_LAUNCH_REQUEST_SCHEMA.to_owned(),
            application_id,
        };
        let body = serde_json::to_vec(&request)
            .map_err(|error| CoreClientError::Json(error.to_string()))?;
        self.request("POST", "/v1/shell/launch", Some(&body))?;
        Ok(())
    }

    pub(crate) fn system_status_lines(&self) -> Result<Vec<String>, CoreClientError> {
        let body = self.request("GET", "/v1/capabilities/prime.system.status", None)?;
        let value: Value = serde_json::from_slice(&body)
            .map_err(|error| CoreClientError::Json(error.to_string()))?;
        let resources = value
            .get("resources")
            .ok_or_else(|| CoreClientError::Protocol("system status resources missing".to_owned()))?;

        let mut lines = Vec::new();
        if let Some(networks) = resources.get("network_links").and_then(Value::as_array) {
            for link in networks.iter().take(3) {
                let interface = string_field(link, "interface", "NETWORK");
                let state = string_field(link, "oper_state", "UNKNOWN").to_uppercase();
                let carrier = link.get("carrier").and_then(Value::as_bool);
                let suffix = match carrier {
                    Some(true) => " CARRIER",
                    Some(false) => " NO-CARRIER",
                    None => "",
                };
                lines.push(format!("NET {interface}: {state}{suffix}"));
            }
        }
        if let Some(cards) = resources.get("audio_cards").and_then(Value::as_array) {
            for card in cards.iter().take(2) {
                let name = string_field(card, "kernel_name", "AUDIO");
                let id = string_field(card, "id", "DEVICE");
                lines.push(format!("AUDIO {name}: {id}"));
            }
        }
        if let Some(supplies) = resources.get("power_supplies").and_then(Value::as_array) {
            for supply in supplies.iter().take(2) {
                let name = string_field(supply, "kernel_name", "POWER");
                let status = string_field(supply, "status", "UNKNOWN").to_uppercase();
                let capacity = supply
                    .get("capacity_percent")
                    .and_then(Value::as_u64)
                    .map(|value| format!(" {value}%"))
                    .unwrap_or_default();
                let online = match supply.get("online").and_then(Value::as_bool) {
                    Some(true) => " ONLINE",
                    Some(false) => " OFFLINE",
                    None => "",
                };
                lines.push(format!("PWR {name}: {status}{capacity}{online}"));
            }
        }
        if let Some(zones) = resources.get("thermal_zones").and_then(Value::as_array) {
            for zone in zones.iter().take(2) {
                let zone_type = string_field(zone, "zone_type", "THERMAL");
                let temperature = zone
                    .get("temperature_millicelsius")
                    .and_then(Value::as_i64)
                    .map(|value| format!("{}C", value / 1000))
                    .unwrap_or_else(|| "UNKNOWN".to_owned());
                lines.push(format!("TEMP {zone_type}: {temperature}"));
            }
        }

        append_control_truth(resources, "network_control", "NETWORK CTRL", &mut lines);
        append_control_truth(resources, "audio_control", "AUDIO CTRL", &mut lines);
        append_control_truth(resources, "power_mutation", "POWER CTRL", &mut lines);

        if lines.is_empty() {
            lines.push("NO SYSTEM STATUS EVIDENCE".to_owned());
        }
        Ok(lines)
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<Vec<u8>, CoreClientError> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|error| CoreClientError::Io(error.to_string()))?;
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .map_err(|error| CoreClientError::Io(error.to_string()))?;
        stream
            .set_write_timeout(Some(IO_TIMEOUT))
            .map_err(|error| CoreClientError::Io(error.to_string()))?;

        let body = body.unwrap_or_default();
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: prime\r\nPrime-Interface-Accept: {CAPABILITY_INTERFACE_VERSION}\r\nConnection: close\r\n"
        );
        if !body.is_empty() {
            request.push_str("Content-Type: application/json\r\n");
            request.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }
        request.push_str("\r\n");
        stream
            .write_all(request.as_bytes())
            .and_then(|_| stream.write_all(body))
            .map_err(|error| CoreClientError::Io(error.to_string()))?;
        stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(|error| CoreClientError::Io(error.to_string()))?;

        let mut response = Vec::new();
        stream
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut response)
            .map_err(|error| CoreClientError::Io(error.to_string()))?;
        if response.len() > MAX_RESPONSE_BYTES {
            return Err(CoreClientError::Protocol(
                "Core response exceeded Shell limit".to_owned(),
            ));
        }
        parse_http_response(&response)
    }
}

fn parse_http_response(response: &[u8]) -> Result<Vec<u8>, CoreClientError> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| CoreClientError::Protocol("HTTP header terminator missing".to_owned()))?;
    let header = std::str::from_utf8(&response[..header_end])
        .map_err(|error| CoreClientError::Protocol(error.to_string()))?;
    let mut lines = header.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| CoreClientError::Protocol("HTTP status line missing".to_owned()))?;
    let mut status_parts = status_line.split_whitespace();
    let version = status_parts.next().unwrap_or_default();
    if !version.starts_with("HTTP/1.") {
        return Err(CoreClientError::Protocol(
            "unexpected Core HTTP version".to_owned(),
        ));
    }
    let status = status_parts
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| CoreClientError::Protocol("HTTP status invalid".to_owned()))?;

    let body = response[(header_end + 4)..].to_vec();
    if !(200..300).contains(&status) {
        let message = String::from_utf8_lossy(&body);
        return Err(CoreClientError::Http(
            status,
            message.chars().take(240).collect(),
        ));
    }
    Ok(body)
}

fn string_field(value: &Value, field: &str, fallback: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

fn append_control_truth(resources: &Value, field: &str, label: &str, lines: &mut Vec<String>) {
    let Some(control) = resources.get(field) else {
        return;
    };
    let ready = control.get("ready").and_then(Value::as_bool).unwrap_or(false);
    lines.push(format!(
        "{label}: {}",
        if ready { "READY" } else { "UNAVAILABLE" }
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_success_response() {
        let body = br#"{\"ok\":true}"#;
        let mut response = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n".to_vec();
        response.extend_from_slice(body);
        assert_eq!(parse_http_response(&response).expect("response"), body);
    }

    #[test]
    fn rejects_non_success_response() {
        let response = b"HTTP/1.1 403 Forbidden\r\n\r\nblocked";
        assert!(matches!(
            parse_http_response(response),
            Err(CoreClientError::Http(403, _))
        ));
    }
}
