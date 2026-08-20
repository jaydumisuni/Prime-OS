from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exact source anchor once, found {count}")
    return text.replace(old, new, 1)


contracts = Path("crates/prime-contracts/src/lib.rs")
text = contracts.read_text()
text = replace_once(text, "pub mod policy;\npub mod storage;", "pub mod policy;\npub mod storage;\npub mod system;", "contract system module")
text = replace_once(text, "pub use policy::*;\npub use storage::*;", "pub use policy::*;\npub use storage::*;\npub use system::*;", "contract system export")
contracts.write_text(text)

primed = Path("crates/primed/src/lib.rs")
text = primed.read_text()
text = replace_once(text, "pub mod policy;\npub mod registry;", "pub mod policy;\npub mod power;\npub mod registry;", "primed power module")
primed.write_text(text)

server = Path("crates/primed/src/server.rs")
text = server.read_text()
text = replace_once(
    text,
    "use crate::{identity, launcher, shell_api, storage, CoreState};",
    "use crate::{identity, launcher, power, shell_api, storage, CoreState};",
    "server power import",
)
text = replace_once(
    text,
    "    HostProjectionBody, InterfaceError, NativeLaunchRequest, ShellLaunchRequest,\n    StoragePreflightRequest, StorageProjection, VersionsProjection, CAPABILITY_INTERFACE,\n    CAPABILITY_INTERFACE_VERSION, NATIVE_LAUNCH_REQUEST_SCHEMA, SHELL_LAUNCH_REQUEST_SCHEMA,\n    STORAGE_PREFLIGHT_SCHEMA,",
    "    HostProjectionBody, InterfaceError, NativeLaunchRequest, ShellLaunchRequest,\n    StoragePreflightRequest, StorageProjection, SystemPowerRequest, VersionsProjection,\n    CAPABILITY_INTERFACE, CAPABILITY_INTERFACE_VERSION, NATIVE_LAUNCH_REQUEST_SCHEMA,\n    SHELL_LAUNCH_REQUEST_SCHEMA, STORAGE_PREFLIGHT_SCHEMA, SYSTEM_POWER_REQUEST_SCHEMA,",
    "server power contracts",
)
anchor = '    if request.method() == Method::POST && request.uri().path() == "/v1/exec/native/launch" {'
block = '''    if request.method() == Method::POST && request.uri().path() == "/v1/system/power" {
        if !shell_peer_authorized(peer_uid, peer_gid, shell_gid) {
            return Ok(error_response(
                StatusCode::FORBIDDEN,
                "PRIME_SYSTEM_POWER_AUTHORIZATION_REQUIRED",
                "Prime power mutation requires root or the dedicated Core socket group",
                Vec::new(),
                Vec::new(),
            ));
        }
        let body = match collect_mutation_body(request.into_body()).await {
            Ok(body) => body,
            Err(response) => return Ok(response),
        };
        let power_request: SystemPowerRequest = match serde_json::from_slice(&body) {
            Ok(request) => request,
            Err(_) => {
                return Ok(error_response(
                    StatusCode::BAD_REQUEST,
                    "PRIME_SYSTEM_POWER_REQUEST_INVALID",
                    "Power request is not valid prime.system-power-request.v1 JSON",
                    Vec::new(),
                    Vec::new(),
                ));
            }
        };
        if power_request.schema != SYSTEM_POWER_REQUEST_SCHEMA {
            return Ok(error_response(
                StatusCode::BAD_REQUEST,
                "PRIME_SYSTEM_POWER_SCHEMA_INVALID",
                "Power request schema is not supported",
                Vec::new(),
                Vec::new(),
            ));
        }
        let result = tokio::task::spawn_blocking(move || power::execute(&power_request)).await;
        return Ok(match result {
            Ok(Ok(evidence)) => json_response(StatusCode::ACCEPTED, &evidence, true),
            Ok(Err(error)) => error_response(
                StatusCode::CONFLICT,
                "PRIME_SYSTEM_POWER_DENIED",
                &error.to_string(),
                Vec::new(),
                Vec::new(),
            ),
            Err(_) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "PRIME_SYSTEM_POWER_TASK_FAILED",
                "Power mutation worker terminated before returning evidence",
                Vec::new(),
                Vec::new(),
            ),
        });
    }

'''
text = replace_once(text, anchor, block + anchor, "server power route")
server.write_text(text)

status = Path("crates/primed/src/system_status.rs")
text = status.read_text()
text = replace_once(
    text,
    '''        power_mutation: ControlTruth {
            ready: false,
            limitations: vec![
                "Prime P1 Capability Interface exposes no restart/shutdown mutation route yet"
                    .to_owned(),
            ],
        },''',
    '''        power_mutation: ControlTruth {
            ready: true,
            limitations: Vec::new(),
        },''',
    "system power truth",
)
text = replace_once(
    text,
    "        assert!(!snapshot.power_mutation.ready);",
    "        assert!(snapshot.power_mutation.ready);",
    "system power truth test",
)
status.write_text(text)
