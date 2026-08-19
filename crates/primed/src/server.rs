use crate::{identity, launcher, storage, CoreState};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::header::{HeaderMap, HeaderValue, CACHE_CONTROL, CONTENT_TYPE};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use prime_contracts::{
    CapabilitiesProjection, HardwareProjection, HealthProjection, HealthStatus, HostProjection,
    HostProjectionBody, InterfaceError, NativeLaunchRequest, StoragePreflightRequest,
    StorageProjection, VersionsProjection, CAPABILITY_INTERFACE, CAPABILITY_INTERFACE_VERSION,
    NATIVE_LAUNCH_REQUEST_SCHEMA, STORAGE_PREFLIGHT_SCHEMA,
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
const MAX_MUTATION_BODY_BYTES: usize = 16 * 1024;

#[derive(Debug, PartialEq, Eq)]
enum NegotiationError {
    VersionRequired,
    Incompatible(Vec<String>),
}

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

    let mut state = state;
    state
        .begin_generation_health_proving("prime.core.socket.bound.v1")
        .map_err(|error| io::Error::other(format!("generation health proving failed: {error}")))?;

    loop {
        let (stream, _) = listener.accept().await?;
        let credential = match stream.peer_cred() {
            Ok(credential) => credential,
            Err(error) => {
                eprintln!("primed rejected connection without Unix peer credentials: {error}");
                continue;
            }
        };
        let peer_uid = credential.uid();
        let state = state.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let service = service_fn(move |request| route(request, state.clone(), peer_uid));
            if let Err(error) = http1::Builder::new().serve_connection(io, service).await {
                eprintln!("primed connection failed: {error}");
            }
        });
    }
}

async fn route(
    request: Request<Incoming>,
    state: CoreState,
    peer_uid: u32,
) -> Result<Response<Full<Bytes>>, Infallible> {
    if request.method() == Method::GET && request.uri().path() == "/v1/versions" {
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
        Err(error) => return Ok(negotiation_error_response(error)),
    };

    if request.method() == Method::POST && request.uri().path() == "/v1/exec/native/launch" {
        if peer_uid != 0 {
            return Ok(error_response(
                StatusCode::FORBIDDEN,
                "PRIME_EXEC_AUTHORIZATION_REQUIRED",
                "P1 native launch admission requires Unix peer UID 0",
                Vec::new(),
                Vec::new(),
            ));
        }
        let body = match collect_mutation_body(request.into_body()).await {
            Ok(body) => body,
            Err(response) => return Ok(response),
        };
        let launch_request: NativeLaunchRequest = match serde_json::from_slice(&body) {
            Ok(request) => request,
            Err(_) => {
                return Ok(error_response(
                    StatusCode::BAD_REQUEST,
                    "PRIME_NATIVE_LAUNCH_REQUEST_INVALID",
                    "Native launch request is not valid prime.native-launch-request.v1 JSON",
                    Vec::new(),
                    Vec::new(),
                ));
            }
        };
        if launch_request.schema != NATIVE_LAUNCH_REQUEST_SCHEMA {
            return Ok(error_response(
                StatusCode::BAD_REQUEST,
                "PRIME_NATIVE_LAUNCH_SCHEMA_INVALID",
                "Native launch request schema is not supported",
                Vec::new(),
                Vec::new(),
            ));
        }

        let launch_state = state.clone();
        let result = tokio::task::spawn_blocking(move || {
            launcher::launch_native(
                &launch_state.state_dir,
                &launch_state.systemd_run,
                &launch_state.host,
                &launch_state.generation,
                &launch_request,
            )
        })
        .await;

        return Ok(match result {
            Ok(Ok(evidence)) => json_response(StatusCode::OK, &evidence, true),
            Ok(Err(error)) => error_response(
                StatusCode::CONFLICT,
                "PRIME_NATIVE_LAUNCH_DENIED",
                &error.to_string(),
                Vec::new(),
                Vec::new(),
            ),
            Err(_) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "PRIME_NATIVE_LAUNCH_TASK_FAILED",
                "Native launch worker terminated before returning evidence",
                Vec::new(),
                Vec::new(),
            ),
        });
    }

    if request.method() == Method::POST && request.uri().path() == "/v1/storage/preflight" {
        if peer_uid != 0 {
            return Ok(error_response(
                StatusCode::FORBIDDEN,
                "PRIME_STORAGE_AUTHORIZATION_REQUIRED",
                "P1 storage preflight requires Unix peer UID 0",
                Vec::new(),
                Vec::new(),
            ));
        }
        let body = match collect_mutation_body(request.into_body()).await {
            Ok(body) => body,
            Err(response) => return Ok(response),
        };
        let preflight_request: StoragePreflightRequest = match serde_json::from_slice(&body) {
            Ok(request) => request,
            Err(_) => {
                return Ok(error_response(
                    StatusCode::BAD_REQUEST,
                    "PRIME_STORAGE_PREFLIGHT_REQUEST_INVALID",
                    "Storage preflight request is not valid prime.storage-preflight.v1 JSON",
                    Vec::new(),
                    Vec::new(),
                ));
            }
        };

        if preflight_request.schema != STORAGE_PREFLIGHT_SCHEMA {
            let observed_at = match identity::now_rfc3339() {
                Ok(observed_at) => observed_at,
                Err(error) => {
                    return Ok(error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "PRIME_STORAGE_CLOCK_FAILED",
                        &error.to_string(),
                        Vec::new(),
                        Vec::new(),
                    ));
                }
            };
            let snapshot = match state.storage.read() {
                Ok(storage) => storage.clone(),
                Err(_) => {
                    return Ok(error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "PRIME_STORAGE_STATE_UNAVAILABLE",
                        "Storage state lock is poisoned",
                        Vec::new(),
                        Vec::new(),
                    ));
                }
            };
            let result = prime_storage::preflight(&snapshot, &preflight_request, observed_at);
            return Ok(json_response(StatusCode::BAD_REQUEST, &result, true));
        }

        let preflight_state = state.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let observed_at = identity::now_rfc3339().map_err(|error| error.to_string())?;
            let mut refreshed = storage::observe(
                &preflight_state.storage_mountinfo,
                &preflight_state.storage_policy_file,
                observed_at.clone(),
                preflight_state.generation.generation_id.clone(),
            );
            if let Err(error) = storage::persist_snapshot(
                &preflight_state.state_dir,
                preflight_state.host.host_id,
                &preflight_state.generation.generation_id,
                &refreshed,
            ) {
                refreshed
                    .limitations
                    .push(format!("storage state persistence failed: {error}"));
            }
            let preflight = prime_storage::preflight(&refreshed, &preflight_request, observed_at);
            let mut current = preflight_state
                .storage
                .write()
                .map_err(|_| "storage state lock is poisoned".to_owned())?;
            *current = refreshed;
            Ok(preflight)
        })
        .await;

        return Ok(match result {
            Ok(Ok(preflight)) => json_response(StatusCode::OK, &preflight, true),
            Ok(Err(error)) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "PRIME_STORAGE_PREFLIGHT_FAILED",
                &error,
                Vec::new(),
                Vec::new(),
            ),
            Err(_) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "PRIME_STORAGE_PREFLIGHT_TASK_FAILED",
                "Storage preflight worker terminated before returning a result",
                Vec::new(),
                Vec::new(),
            ),
        });
    }

    if request.method() != Method::GET {
        return Ok(error_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "PRIME_METHOD_NOT_ALLOWED",
            "This Prime route does not accept that HTTP method",
            Vec::new(),
            Vec::new(),
        ));
    }

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
                    hardware_graph_revision: state.hardware.revision,
                },
            },
            true,
        ),
        "/v1/hardware" => json_response(
            StatusCode::OK,
            &HardwareProjection {
                interface: CAPABILITY_INTERFACE.to_owned(),
                interface_version: negotiated.to_owned(),
                host_id: state.host.host_id,
                generation_id: state.generation.generation_id.clone(),
                graph: (*state.hardware).clone(),
            },
            true,
        ),
        "/v1/storage" => match state.storage.read() {
            Ok(storage) => json_response(
                StatusCode::OK,
                &StorageProjection {
                    interface: CAPABILITY_INTERFACE.to_owned(),
                    interface_version: negotiated.to_owned(),
                    host_id: state.host.host_id,
                    generation_id: state.generation.generation_id.clone(),
                    inventory: storage.clone(),
                },
                true,
            ),
            Err(_) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "PRIME_STORAGE_STATE_UNAVAILABLE",
                "Storage state lock is poisoned",
                Vec::new(),
                Vec::new(),
            ),
        },
        "/v1/health" => json_response(
            StatusCode::OK,
            &HealthProjection {
                interface: CAPABILITY_INTERFACE.to_owned(),
                interface_version: negotiated.to_owned(),
                host_id: state.host.host_id,
                generation_id: state.generation.generation_id.clone(),
                status: if state.health_limitations.is_empty() {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Degraded
                },
                observed_at: state.started_at.clone(),
                limitations: (*state.health_limitations).clone(),
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
                capabilities: state.capabilities_snapshot(),
            },
            true,
        ),
        path if path.starts_with("/v1/capabilities/") => {
            let id = path.trim_start_matches("/v1/capabilities/");
            let capabilities = state.capabilities_snapshot();
            match capabilities.iter().find(|item| item.capability_id == id) {
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

async fn collect_mutation_body(body: Incoming) -> Result<Bytes, Response<Full<Bytes>>> {
    let collected = body.collect().await.map_err(|_| {
        error_response(
            StatusCode::BAD_REQUEST,
            "PRIME_REQUEST_BODY_INVALID",
            "Prime could not read the request body",
            Vec::new(),
            Vec::new(),
        )
    })?;
    let bytes = collected.to_bytes();
    if bytes.len() > MAX_MUTATION_BODY_BYTES {
        return Err(error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PRIME_REQUEST_BODY_TOO_LARGE",
            "Request exceeds the P1 mutation body limit",
            Vec::new(),
            Vec::new(),
        ));
    }
    Ok(bytes)
}

fn negotiate(headers: &HeaderMap) -> Result<&'static str, NegotiationError> {
    let Some(value) = headers.get(ACCEPT_HEADER) else {
        return Err(NegotiationError::VersionRequired);
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
        Err(NegotiationError::Incompatible(requested))
    }
}

fn negotiation_error_response(error: NegotiationError) -> Response<Full<Bytes>> {
    match error {
        NegotiationError::VersionRequired => error_response(
            StatusCode::BAD_REQUEST,
            "PRIME_INTERFACE_VERSION_REQUIRED",
            "Send Prime-Interface-Accept with supported interface versions",
            vec![CAPABILITY_INTERFACE_VERSION.to_owned()],
            Vec::new(),
        ),
        NegotiationError::Incompatible(requested) => error_response(
            StatusCode::CONFLICT,
            "PRIME_INTERFACE_INCOMPATIBLE",
            "No mutually supported Prime Capability Interface version exists",
            vec![CAPABILITY_INTERFACE_VERSION.to_owned()],
            requested,
        ),
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
        b"{\"error\":\"PRIME_SERIALIZATION_FAILURE\",\"message\":\"Prime could not serialize its own response\"}".to_vec()
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
        assert_eq!(
            negotiate(&headers).expect_err("zero overlap must fail"),
            NegotiationError::Incompatible(vec!["2.0".to_owned()])
        );
    }

    #[test]
    fn negotiation_requires_explicit_versions() {
        assert_eq!(
            negotiate(&HeaderMap::new()).expect_err("missing versions must fail"),
            NegotiationError::VersionRequired
        );
    }
}
