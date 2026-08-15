use crate::CoreState;
use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::header::{HeaderMap, HeaderValue, CACHE_CONTROL, CONTENT_TYPE};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use prime_contracts::{
    CapabilitiesProjection, HealthProjection, HealthStatus, HostProjection, HostProjectionBody,
    InterfaceError, VersionsProjection, CAPABILITY_INTERFACE, CAPABILITY_INTERFACE_VERSION,
};
use serde::Serialize;
use std::convert::Infallible;
use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::Path;
use tokio::net::UnixListener;

const ACCEPT_HEADER: &str = "prime-interface-accept";
const VERSION_HEADER: &str = "prime-interface-version";

pub async fn run(socket_path: &Path, state: CoreState) -> io::Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(socket_path) {
        if !metadata.file_type().is_socket() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} exists and is not a socket", socket_path.display()),
            ));
        }
        fs::remove_file(socket_path)?;
    }

    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let listener = UnixListener::bind(socket_path)?;
    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o660))?;

    loop {
        let (stream, _) = listener.accept().await?;
        if let Err(error) = stream.peer_cred() {
            eprintln!("primed rejected connection without Unix peer credentials: {error}");
            continue;
        }
        let state = state.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let service = service_fn(move |request| route(request, state.clone()));
            if let Err(error) = http1::Builder::new().serve_connection(io, service).await {
                eprintln!("primed connection failed: {error}");
            }
        });
    }
}

async fn route(
    request: Request<Incoming>,
    state: CoreState,
) -> Result<Response<Full<Bytes>>, Infallible> {
    if request.method() != Method::GET {
        return Ok(error_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "PRIME_METHOD_NOT_ALLOWED",
            "P1 core interface currently exposes read-only GET projections",
            Vec::new(),
            Vec::new(),
        ));
    }

    if request.uri().path() == "/v1/versions" {
        return Ok(json_response(
            StatusCode::OK,
            &VersionsProjection {
                interface: CAPABILITY_INTERFACE.to_owned(),
                supported_versions: vec![CAPABILITY_INTERFACE_VERSION.to_owned()],
            },
            false,
        ));
    }

    let negotiated = match negotiate(request.headers()) {
        Ok(version) => version,
        Err(response) => return Ok(response),
    };

    let response = match request.uri().path() {
        "/v1/host" => json_response(
            StatusCode::OK,
            &HostProjection {
                interface: CAPABILITY_INTERFACE.to_owned(),
                interface_version: negotiated.to_owned(),
                host: HostProjectionBody {
                    host_id: state.host.host_id,
                    host_arch: state.host.host_arch.clone(),
                    generation_id: state.generation.generation_id.clone(),
                    hardware_graph_revision: 0,
                },
            },
            true,
        ),
        "/v1/health" => json_response(
            StatusCode::OK,
            &HealthProjection {
                interface: CAPABILITY_INTERFACE.to_owned(),
                interface_version: negotiated.to_owned(),
                host_id: state.host.host_id,
                generation_id: state.generation.generation_id.clone(),
                status: HealthStatus::Degraded,
                observed_at: state.started_at.clone(),
                limitations: vec![
                    "Hardware graph has not yet advanced beyond revision 0".to_owned(),
                ],
            },
            true,
        ),
        "/v1/capabilities" => json_response(
            StatusCode::OK,
            &CapabilitiesProjection {
                interface: CAPABILITY_INTERFACE.to_owned(),
                interface_version: negotiated.to_owned(),
                host_id: state.host.host_id,
                generation_id: state.generation.generation_id.clone(),
                capabilities: (*state.capabilities).clone(),
            },
            true,
        ),
        path if path.starts_with("/v1/capabilities/") => {
            let id = path.trim_start_matches("/v1/capabilities/");
            match state.capabilities.iter().find(|item| item.capability_id == id) {
                Some(capability) => json_response(StatusCode::OK, capability, true),
                None => error_response(
                    StatusCode::NOT_FOUND,
                    "PRIME_CAPABILITY_NOT_FOUND",
                    "No capability with that exact identifier is registered on this Host",
                    Vec::new(),
                    Vec::new(),
                ),
            }
        }
        _ => error_response(
            StatusCode::NOT_FOUND,
            "PRIME_ROUTE_NOT_FOUND",
            "Unknown Prime Capability Interface route",
            Vec::new(),
            Vec::new(),
        ),
    };

    Ok(response)
}

fn negotiate(headers: &HeaderMap) -> Result<&'static str, Response<Full<Bytes>>> {
    let Some(value) = headers.get(ACCEPT_HEADER) else {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "PRIME_INTERFACE_VERSION_REQUIRED",
            "Send Prime-Interface-Accept with supported interface versions",
            vec![CAPABILITY_INTERFACE_VERSION.to_owned()],
            Vec::new(),
        ));
    };

    let requested = value
        .to_str()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();

    if requested
        .iter()
        .any(|version| version == CAPABILITY_INTERFACE_VERSION)
    {
        Ok(CAPABILITY_INTERFACE_VERSION)
    } else {
        Err(error_response(
            StatusCode::CONFLICT,
            "PRIME_INTERFACE_INCOMPATIBLE",
            "No mutually supported Prime Capability Interface version exists",
            vec![CAPABILITY_INTERFACE_VERSION.to_owned()],
            requested,
        ))
    }
}

fn error_response(
    status: StatusCode,
    code: &str,
    message: &str,
    supported_versions: Vec<String>,
    requested_versions: Vec<String>,
) -> Response<Full<Bytes>> {
    json_response(
        status,
        &InterfaceError {
            error: code.to_owned(),
            message: message.to_owned(),
            supported_versions,
            requested_versions,
        },
        false,
    )
}

fn json_response<T: Serialize>(
    status: StatusCode,
    body: &T,
    include_version: bool,
) -> Response<Full<Bytes>> {
    let encoded = serde_json::to_vec(body).unwrap_or_else(|_| {
        br#"{"error":"PRIME_SERIALIZATION_FAILURE","message":"Prime could not serialize its own response"}"#.to_vec()
    });
    let mut response = Response::new(Full::new(Bytes::from(encoded)));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if include_version {
        response.headers_mut().insert(
            VERSION_HEADER,
            HeaderValue::from_static(CAPABILITY_INTERFACE_VERSION),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiation_accepts_v1() {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT_HEADER, HeaderValue::from_static("2.0, 1.0"));
        assert_eq!(negotiate(&headers).expect("v1 overlap"), "1.0");
    }

    #[test]
    fn negotiation_fails_without_overlap() {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT_HEADER, HeaderValue::from_static("2.0"));
        let response = negotiate(&headers).expect_err("zero overlap must fail");
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
}
