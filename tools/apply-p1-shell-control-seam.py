from pathlib import Path
import re


def replace_once(path: str, old: str, new: str) -> None:
    p = Path(path)
    text = p.read_text()
    if text.count(old) != 1:
        raise SystemExit(f"{path}: expected exactly one replacement target, got {text.count(old)}")
    p.write_text(text.replace(old, new, 1))


# Application Profile: additive source path that preserves old v1 digests when absent.
replace_once(
    "crates/prime-contracts/src/application.rs",
    "    pub workload_arch: Option<String>,\n",
    "    pub workload_arch: Option<String>,\n    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n    pub source_path: Option<String>,\n",
)

# Every existing typed fixture gets the backwards-compatible absent value.
for path in Path("crates").rglob("*.rs"):
    text = path.read_text()
    updated = re.sub(
        r"(?m)^(?P<i>\s*)workload_arch:\s*[^\n]+,\n(?!\s*source_path:)",
        lambda m: m.group(0) + f"{m.group('i')}source_path: None,\n",
        text,
    )
    if updated != text:
        path.write_text(updated)

# Capability Interface projection for selected application profiles.
replace_once(
    "crates/prime-contracts/src/lib.rs",
    "#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]\npub struct CapabilitiesProjection {\n    pub interface: String,\n    pub interface_version: String,\n    pub host_id: Uuid,\n    pub generation_id: String,\n    pub capabilities: Vec<CapabilityDescriptor>,\n}\n",
    "#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]\npub struct CapabilitiesProjection {\n    pub interface: String,\n    pub interface_version: String,\n    pub host_id: Uuid,\n    pub generation_id: String,\n    pub capabilities: Vec<CapabilityDescriptor>,\n}\n\n#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]\npub struct ApplicationsProjection {\n    pub interface: String,\n    pub interface_version: String,\n    pub host_id: Uuid,\n    pub generation_id: String,\n    pub applications: Vec<ApplicationProfile>,\n}\n",
)

# Registry: enumerate only exact selected, digest-valid, non-revoked profiles.
replace_once(
    "crates/primed/src/registry.rs",
    "pub fn load_selected_profile(\n    root: &Path,\n    application_id: Uuid,\n) -> Result<ApplicationProfile, RegistryError> {\n    let revision = read_selected(&profile_root(root, application_id).join(\"selected\"))?;\n    let profile = load_profile_revision(root, application_id, revision)?;\n    if profile.revoked {\n        return Err(RegistryError::Revoked);\n    }\n    Ok(profile)\n}\n",
    "pub fn load_selected_profile(\n    root: &Path,\n    application_id: Uuid,\n) -> Result<ApplicationProfile, RegistryError> {\n    let revision = read_selected(&profile_root(root, application_id).join(\"selected\"))?;\n    let profile = load_profile_revision(root, application_id, revision)?;\n    if profile.revoked {\n        return Err(RegistryError::Revoked);\n    }\n    Ok(profile)\n}\n\npub fn list_selected_profiles(root: &Path) -> Result<Vec<ApplicationProfile>, RegistryError> {\n    let applications_root = root.join(\"applications\");\n    let entries = match fs::read_dir(&applications_root) {\n        Ok(entries) => entries,\n        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),\n        Err(error) => return Err(error.into()),\n    };\n\n    let mut profiles = Vec::new();\n    for entry in entries {\n        let entry = entry?;\n        if !entry.file_type()?.is_dir() {\n            return Err(RegistryError::InvalidSelected);\n        }\n        let name = entry\n            .file_name()\n            .into_string()\n            .map_err(|_| RegistryError::InvalidSelected)?;\n        let application_id = Uuid::parse_str(&name).map_err(|_| RegistryError::InvalidSelected)?;\n        let selected = entry.path().join(\"selected\");\n        match fs::symlink_metadata(&selected) {\n            Ok(metadata) if metadata.file_type().is_file() => {}\n            Ok(_) => return Err(RegistryError::InvalidSelected),\n            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,\n            Err(error) => return Err(error.into()),\n        }\n        profiles.push(load_selected_profile(root, application_id)?);\n    }\n    profiles.sort_by_key(|profile| profile.application_id.to_string());\n    Ok(profiles)\n}\n",
)

# Registry proof for deterministic selected-profile enumeration and corruption failure.
replace_once(
    "crates/primed/src/registry.rs",
    "    #[test]\n    fn selected_profile_revalidates_digest() {\n",
    "    #[test]\n    fn selected_profiles_are_listed_deterministically() {\n        let dir = tempfile::tempdir().expect(\"tempdir\");\n        let policy = policy(Uuid::now_v7());\n        let mut profiles = vec![\n            profile(Uuid::parse_str(\"00000000-0000-7000-8000-000000000002\").expect(\"uuid\"), &policy),\n            profile(Uuid::parse_str(\"00000000-0000-7000-8000-000000000001\").expect(\"uuid\"), &policy),\n        ];\n        for item in &profiles {\n            store_profile_revision(dir.path(), item).expect(\"store\");\n            select_profile_revision(dir.path(), item.application_id, item.profile_revision).expect(\"select\");\n        }\n        profiles.sort_by_key(|item| item.application_id.to_string());\n        assert_eq!(list_selected_profiles(dir.path()).expect(\"list\"), profiles);\n    }\n\n    #[test]\n    fn selected_profile_revalidates_digest() {\n",
)

# Prime Core: bind the socket to the dedicated shell group and grant only native-launch mutation to that principal.
replace_once(
    "crates/primed/src/server.rs",
    "use crate::{identity, launcher, storage, CoreState};\n",
    "use crate::{identity, launcher, registry, storage, CoreState};\n",
)
replace_once(
    "crates/primed/src/server.rs",
    "    CapabilitiesProjection, HardwareProjection, HealthProjection, HealthStatus, HostProjection,\n",
    "    ApplicationsProjection, CapabilitiesProjection, HardwareProjection, HealthProjection, HealthStatus, HostProjection,\n",
)
replace_once(
    "crates/primed/src/server.rs",
    "use std::os::unix::fs::{FileTypeExt, PermissionsExt};\n",
    "use std::os::unix::fs::{chown, FileTypeExt, PermissionsExt};\n",
)
replace_once(
    "crates/primed/src/server.rs",
    "const MAX_MUTATION_BODY_BYTES: usize = 16 * 1024;\n",
    "const MAX_MUTATION_BODY_BYTES: usize = 16 * 1024;\nconst PRIME_SHELL_GROUP: &str = \"prime-shell\";\nconst SYSTEM_GROUP_FILE: &str = \"/etc/group\";\n",
)
replace_once(
    "crates/primed/src/server.rs",
    "pub async fn run(socket_path: &Path, state: CoreState) -> io::Result<()> {\n",
    "pub async fn run(socket_path: &Path, state: CoreState) -> io::Result<()> {\n    let shell_gid = resolve_group_gid(Path::new(SYSTEM_GROUP_FILE), PRIME_SHELL_GROUP)?;\n",
)
replace_once(
    "crates/primed/src/server.rs",
    "    let listener = UnixListener::bind(socket_path)?;\n    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o660))?;\n",
    "    let listener = UnixListener::bind(socket_path)?;\n    chown(socket_path, None, Some(shell_gid))?;\n    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o660))?;\n",
)
replace_once(
    "crates/primed/src/server.rs",
    "        let peer_uid = credential.uid();\n        let state = state.clone();\n\n        tokio::spawn(async move {\n            let io = TokioIo::new(stream);\n            let service = service_fn(move |request| route(request, state.clone(), peer_uid));\n",
    "        let peer_uid = credential.uid();\n        let peer_gid = credential.gid();\n        let state = state.clone();\n\n        tokio::spawn(async move {\n            let io = TokioIo::new(stream);\n            let service = service_fn(move |request| {\n                route(request, state.clone(), peer_uid, peer_gid, shell_gid)\n            });\n",
)
replace_once(
    "crates/primed/src/server.rs",
    "    state: CoreState,\n    peer_uid: u32,\n) -> Result<Response<Full<Bytes>>, Infallible> {\n",
    "    state: CoreState,\n    peer_uid: u32,\n    peer_gid: u32,\n    shell_gid: u32,\n) -> Result<Response<Full<Bytes>>, Infallible> {\n",
)
replace_once(
    "crates/primed/src/server.rs",
    "        if peer_uid != 0 {\n            return Ok(error_response(\n                StatusCode::FORBIDDEN,\n                \"PRIME_EXEC_AUTHORIZATION_REQUIRED\",\n                \"P1 native launch admission requires Unix peer UID 0\",\n",
    "        if !native_launch_authorized(peer_uid, peer_gid, shell_gid) {\n            return Ok(error_response(\n                StatusCode::FORBIDDEN,\n                \"PRIME_EXEC_AUTHORIZATION_REQUIRED\",\n                \"P1 native launch admission requires root or the dedicated Prime Shell service principal\",\n",
)
replace_once(
    "crates/primed/src/server.rs",
    "        \"/v1/capabilities\" => json_response(\n",
    "        \"/v1/applications\" => match registry::list_selected_profiles(&state.state_dir) {\n            Ok(applications) => json_response(\n                StatusCode::OK,\n                &ApplicationsProjection {\n                    interface: CAPABILITY_INTERFACE.to_owned(),\n                    interface_version: negotiated.to_owned(),\n                    host_id: state.host.host_id,\n                    generation_id: state.generation.generation_id.clone(),\n                    applications,\n                },\n                true,\n            ),\n            Err(error) => error_response(\n                StatusCode::INTERNAL_SERVER_ERROR,\n                \"PRIME_APPLICATION_REGISTRY_UNAVAILABLE\",\n                &error.to_string(),\n                Vec::new(),\n                Vec::new(),\n            ),\n        },\n        \"/v1/capabilities\" => json_response(\n",
)
replace_once(
    "crates/primed/src/server.rs",
    "async fn collect_mutation_body(body: Incoming) -> Result<Bytes, Response<Full<Bytes>>> {\n",
    "fn native_launch_authorized(peer_uid: u32, peer_gid: u32, shell_gid: u32) -> bool {\n    peer_uid == 0 || peer_gid == shell_gid\n}\n\nfn resolve_group_gid(path: &Path, group_name: &str) -> io::Result<u32> {\n    let content = fs::read_to_string(path)?;\n    let mut matched = None;\n    for line in content.lines() {\n        let line = line.trim();\n        if line.is_empty() || line.starts_with('#') {\n            continue;\n        }\n        let fields = line.split(':').collect::<Vec<_>>();\n        if fields.first().copied() != Some(group_name) {\n            continue;\n        }\n        if fields.len() < 3 || matched.is_some() {\n            return Err(io::Error::new(io::ErrorKind::InvalidData, \"Prime Shell group record is invalid or duplicated\"));\n        }\n        let gid = fields[2]\n            .parse::<u32>()\n            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, \"Prime Shell group GID is invalid\"))?;\n        matched = Some(gid);\n    }\n    matched.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, \"Prime Shell group is missing\"))\n}\n\nasync fn collect_mutation_body(body: Incoming) -> Result<Bytes, Response<Full<Bytes>>> {\n",
)
replace_once(
    "crates/primed/src/server.rs",
    "    #[test]\n    fn negotiation_accepts_v1() {\n",
    "    #[test]\n    fn native_launch_authorization_is_bounded_to_root_or_shell_principal() {\n        assert!(native_launch_authorized(0, 0, 991));\n        assert!(native_launch_authorized(991, 991, 991));\n        assert!(!native_launch_authorized(1000, 1000, 991));\n    }\n\n    #[test]\n    fn prime_shell_group_resolution_is_exact() {\n        let dir = tempfile::tempdir().expect(\"tempdir\");\n        let group_file = dir.path().join(\"group\");\n        fs::write(&group_file, \"root:x:0:\\nprime-shell:x:991:\\nusers:x:100:\\n\").expect(\"write group fixture\");\n        assert_eq!(resolve_group_gid(&group_file, PRIME_SHELL_GROUP).expect(\"gid\"), 991);\n    }\n\n    #[test]\n    fn negotiation_accepts_v1() {\n",
)

# systemd-sysusers owns a non-login shell identity; no fixed numeric UID/GID is assumed.
sysusers = Path("image/sysusers/prime-shell.conf")
sysusers.parent.mkdir(parents=True, exist_ok=True)
sysusers.write_text('u! prime-shell - "Prime Shell service principal" /nonexistent /usr/sbin/nologin\n')

replace_once(
    "image/systemd/primed.service",
    "After=local-fs.target\n",
    "Requires=systemd-sysusers.service\nAfter=systemd-sysusers.service local-fs.target\n",
)

replace_once(
    "image/Containerfile",
    "COPY image/systemd/prime-recovery.target /usr/lib/systemd/system/prime-recovery.target\n",
    "COPY image/systemd/prime-recovery.target /usr/lib/systemd/system/prime-recovery.target\nCOPY image/sysusers/prime-shell.conf /usr/lib/sysusers.d/prime-shell.conf\n",
)
replace_once(
    "image/Containerfile",
    "    chmod 0444 /usr/lib/bootc/install/10-prime.toml; \\\n",
    "    chmod 0444 /usr/lib/bootc/install/10-prime.toml /usr/lib/sysusers.d/prime-shell.conf; \\\n",
)
replace_once(
    "image/Containerfile",
    "    test -f /usr/lib/systemd/system/prime-recovery.target; \\\n",
    "    test -f /usr/lib/systemd/system/prime-recovery.target; \\\n    grep -Fx 'u! prime-shell - \"Prime Shell service principal\" /nonexistent /usr/sbin/nologin' /usr/lib/sysusers.d/prime-shell.conf; \\\n",
)

# Frozen contract correction: Shell can discover selected profiles and use one bounded launch grant.
replace_once(
    "docs/contracts/PRIME_CAPABILITY_INTERFACE_V1.md",
    "GET /v1/health\nGET /v1/capabilities\nGET /v1/capabilities/{capability_id}\n",
    "GET /v1/health\nGET /v1/applications\nGET /v1/capabilities\nGET /v1/capabilities/{capability_id}\n",
)
replace_once(
    "docs/contracts/PRIME_CAPABILITY_INTERFACE_V1.md",
    "Both P1 routes are Host-local and require Unix peer UID `0`. Socket access alone does not imply execution or update authorization.\n",
    "Both P1 routes are Host-local. Storage preflight remains Unix peer UID `0` only. Native launch accepts either UID `0` or the dedicated non-login `prime-shell` service principal, whose primary group owns the Core socket. That grant is route-specific: it does not authorize storage mutation, generic command execution or Prime state access. Socket access alone does not imply any mutation beyond the route-specific grant.\n",
)
replace_once(
    "docs/contracts/PRIME_CAPABILITY_INTERFACE_V1.md",
    "`GET /v1/storage` returns the latest Prime `prime.storage-inventory.v1` observation. The storage preflight path refreshes mount/capacity truth before calculating admission rather than relying on a stale cached observation.\n",
    "`GET /v1/storage` returns the latest Prime `prime.storage-inventory.v1` observation. The storage preflight path refreshes mount/capacity truth before calculating admission rather than relying on a stale cached observation.\n\n`GET /v1/applications` returns only exact selected, digest-valid, non-revoked Application Profile revisions. The registry is enumerated by Prime Core; Shell does not read `/var/lib/prime` directly. An empty registry is an empty list. A corrupt selected record fails the projection rather than being silently hidden.\n",
)

replace_once(
    "docs/contracts/PRIME_NATIVE_LAUNCH_V1.md",
    "## P1 authorization\n\nThe P1 route is Host-local and accepts only a Unix peer credential with UID `0`.\n\nThis is deliberately restrictive until Prime user/session authorization is implemented. Socket possession alone is execution authorization.\n" if False else "## P1 authorization\n\nThe P1 route is Host-local and accepts only a Unix peer credential with UID `0`.\n\nThis is deliberately restrictive until Prime user/session authorization is implemented. Socket possession alone is execution authorization.\n",
    "## P1 authorization\n\nThe P1 route is Host-local and accepts either Unix peer UID `0` or the dedicated non-login `prime-shell` service principal. `primed` resolves the `prime-shell` group at startup, assigns that group to `/run/prime/core.sock`, and fails closed if the group cannot be resolved.\n\nThe Prime Shell grant is deliberately route-specific. It authorizes only this already-bounded Application Profile launch route; it does not authorize storage preflight, generic commands, arbitrary environment/arguments, Prime state-directory reads, or broader user/session authority. Other non-root peers are rejected.\n",
)

# The exact source text above has historically used the correct "does not imply" wording; repair if required.
p = Path("docs/contracts/PRIME_NATIVE_LAUNCH_V1.md")
t = p.read_text()
if "accepts only a Unix peer credential with UID `0`" in t:
    old = "The P1 route is Host-local and accepts only a Unix peer credential with UID `0`.\n\nThis is deliberately restrictive until Prime user/session authorization is implemented. Socket possession alone is execution authorization."
    old2 = "The P1 route is Host-local and accepts only a Unix peer credential with UID `0`.\n\nThis is deliberately restrictive until Prime user/session authorization is implemented. Socket possession alone is execution authorization."
    raise SystemExit("native launch authorization block was not replaced exactly")

replace_once(
    "docs/contracts/PRIME_APPLICATION_PROFILE_V1.md",
    '    "workload_arch": "string-or-null"\n',
    '    "workload_arch": "string-or-null",\n    "source_path": "absolute-path-or-null"\n',
)
replace_once(
    "docs/contracts/PRIME_APPLICATION_PROFILE_V1.md",
    "## Digest rule\n",
    "`artifact.source_path` is an optional P1 launch-source locator. It is not artifact identity. When present it must be absolute and Prime Exec still copies, hashes, stages and re-inspects the bytes before execution. When absent it is omitted from canonical JSON serialization so pre-existing v1 profile digests remain valid.\n\n## Digest rule\n",
)

replace_once(
    "docs/contracts/PRIME_P1_SHELL_COMPOSITOR_V1.md",
    "`prime-shell` does not become Prime Core. It consumes versioned Prime capabilities and state through the existing Prime interface rather than reaching into `/var/lib/prime` or privileged kernel/device paths directly.\n",
    "`prime-shell` does not become Prime Core. It consumes versioned Prime capabilities and state through the existing Prime interface rather than reaching into `/var/lib/prime` or privileged kernel/device paths directly. P1 runs it as the dedicated non-login `prime-shell` service principal. Prime Core grants that principal read access to the Core socket plus the already-bounded native-launch route only; root-only storage mutation remains unavailable to Shell.\n",
)
replace_once(
    "docs/contracts/PRIME_P1_SHELL_COMPOSITOR_V1.md",
    "- list available Prime application/profile entries without inventing compatibility;\n- launch an admitted application through Prime Exec rather than direct arbitrary process execution;\n",
    "- list exact selected Prime application/profile entries through `GET /v1/applications` without reading Prime state directly or inventing compatibility;\n- launch an admitted application through Prime Exec rather than direct arbitrary process execution, using only a profile-declared `artifact.source_path` when one exists;\n",
)

print("P1_SHELL_CONTROL_SEAM_PATCHED")
