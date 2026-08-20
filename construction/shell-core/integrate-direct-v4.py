from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exact source anchor once, found {count}")
    return text.replace(old, new, 1)


lib = Path("crates/primed/src/lib.rs")
text = lib.read_text()
text = replace_once(
    text,
    "pub mod server;\npub mod storage;",
    "pub mod server;\npub mod shell_api;\npub mod storage;",
    "primed lib shell module",
)
lib.write_text(text)

server = Path("crates/primed/src/server.rs")
text = server.read_text()
text = replace_once(
    text,
    "use crate::{identity, launcher, storage, CoreState};",
    "use crate::{identity, launcher, shell_api, storage, CoreState};",
    "server crate imports",
)
text = replace_once(
    text,
    "    HostProjectionBody, InterfaceError, NativeLaunchRequest, StoragePreflightRequest,\n    StorageProjection, VersionsProjection, CAPABILITY_INTERFACE, CAPABILITY_INTERFACE_VERSION,\n    NATIVE_LAUNCH_REQUEST_SCHEMA, STORAGE_PREFLIGHT_SCHEMA,",
    "    HostProjectionBody, InterfaceError, NativeLaunchRequest, ShellLaunchRequest,\n    StoragePreflightRequest, StorageProjection, VersionsProjection, CAPABILITY_INTERFACE,\n    CAPABILITY_INTERFACE_VERSION, NATIVE_LAUNCH_REQUEST_SCHEMA, SHELL_LAUNCH_REQUEST_SCHEMA,\n    STORAGE_PREFLIGHT_SCHEMA,",
    "server contract imports",
)
text = replace_once(
    text,
    "use std::os::unix::fs::{FileTypeExt, PermissionsExt};",
    "use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};",
    "server metadata import",
)
text = replace_once(
    text,
    "enum NegotiationError {\n    VersionRequired,\n    Incompatible(Vec<String>),\n}\n\npub async fn run",
    "enum NegotiationError {\n    VersionRequired,\n    Incompatible(Vec<String>),\n}\n\nfn shell_peer_authorized(peer_uid: u32, peer_gid: u32, socket_gid: u32) -> bool {\n    peer_uid == 0 || (socket_gid != 0 && peer_gid == socket_gid)\n}\n\npub async fn run",
    "server shell authorization helper",
)
text = replace_once(
    text,
    "    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o660))?;\n\n    let mut state = state;",
    "    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o660))?;\n    let shell_gid = fs::metadata(socket_path)?.gid();\n\n    let mut state = state;",
    "server socket gid",
)
text = replace_once(
    text,
    "        let peer_uid = credential.uid();\n        let state = state.clone();",
    "        let peer_uid = credential.uid();\n        let peer_gid = credential.gid();\n        let state = state.clone();",
    "server peer gid",
)
text = replace_once(
    text,
    "            let service = service_fn(move |request| route(request, state.clone(), peer_uid));",
    "            let service = service_fn(move |request| {\n                route(request, state.clone(), peer_uid, peer_gid, shell_gid)\n            });",
    "server route closure",
)
text = replace_once(
    text,
    "async fn route(\n    request: Request<Incoming>,\n    state: CoreState,\n    peer_uid: u32,\n) -> Result<Response<Full<Bytes>>, Infallible> {",
    "async fn route(\n    request: Request<Incoming>,\n    state: CoreState,\n    peer_uid: u32,\n    peer_gid: u32,\n    shell_gid: u32,\n) -> Result<Response<Full<Bytes>>, Infallible> {",
    "server route signature",
)
native_anchor = '    if request.method() == Method::POST && request.uri().path() == "/v1/exec/native/launch" {'
shell_block = '''    if request.method() == Method::POST && request.uri().path() == "/v1/shell/launch" {
        if !shell_peer_authorized(peer_uid, peer_gid, shell_gid) {
            return Ok(error_response(
                StatusCode::FORBIDDEN,
                "PRIME_SHELL_AUTHORIZATION_REQUIRED",
                "Prime Shell launch requires root or the dedicated Core socket group",
                Vec::new(),
                Vec::new(),
            ));
        }
        let body = match collect_mutation_body(request.into_body()).await {
            Ok(body) => body,
            Err(response) => return Ok(response),
        };
        let shell_request: ShellLaunchRequest = match serde_json::from_slice(&body) {
            Ok(request) => request,
            Err(_) => {
                return Ok(error_response(
                    StatusCode::BAD_REQUEST,
                    "PRIME_SHELL_LAUNCH_REQUEST_INVALID",
                    "Shell launch request is not valid prime.shell-launch-request.v1 JSON",
                    Vec::new(),
                    Vec::new(),
                ));
            }
        };
        if shell_request.schema != SHELL_LAUNCH_REQUEST_SCHEMA {
            return Ok(error_response(
                StatusCode::BAD_REQUEST,
                "PRIME_SHELL_LAUNCH_SCHEMA_INVALID",
                "Shell launch request schema is not supported",
                Vec::new(),
                Vec::new(),
            ));
        }

        let launch_state = state.clone();
        let result = tokio::task::spawn_blocking(move || {
            shell_api::launch_selected(&launch_state, &shell_request)
        })
        .await;

        return Ok(match result {
            Ok(Ok(evidence)) => json_response(StatusCode::OK, &evidence, true),
            Ok(Err(error)) => error_response(
                StatusCode::CONFLICT,
                "PRIME_SHELL_LAUNCH_DENIED",
                &error.to_string(),
                Vec::new(),
                Vec::new(),
            ),
            Err(_) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "PRIME_SHELL_LAUNCH_TASK_FAILED",
                "Shell launch worker terminated before returning evidence",
                Vec::new(),
                Vec::new(),
            ),
        });
    }

'''
text = replace_once(text, native_anchor, shell_block + native_anchor, "server shell launch route")
text = replace_once(
    text,
    '        "/v1/capabilities" => json_response(',
    '''        "/v1/applications" => match shell_api::applications_projection(&state, negotiated) {
            Ok(applications) => json_response(StatusCode::OK, &applications, true),
            Err(error) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "PRIME_APPLICATION_REGISTRY_UNAVAILABLE",
                &error.to_string(),
                Vec::new(),
                Vec::new(),
            ),
        },
        "/v1/capabilities" => json_response(''',
    "server applications projection route",
)
text = replace_once(
    text,
    "mod tests {\n    use super::*;\n\n    #[test]\n    fn negotiation_accepts_v1()",
    "mod tests {\n    use super::*;\n\n    #[test]\n    fn shell_authorization_accepts_root() {\n        assert!(shell_peer_authorized(0, 0, 0));\n    }\n\n    #[test]\n    fn shell_authorization_accepts_dedicated_nonzero_socket_group() {\n        assert!(shell_peer_authorized(1000, 991, 991));\n    }\n\n    #[test]\n    fn shell_authorization_rejects_wrong_group() {\n        assert!(!shell_peer_authorized(1000, 992, 991));\n    }\n\n    #[test]\n    fn shell_authorization_does_not_delegate_root_group() {\n        assert!(!shell_peer_authorized(1000, 0, 0));\n    }\n\n    #[test]\n    fn negotiation_accepts_v1()",
    "server shell authorization tests",
)
server.write_text(text)
