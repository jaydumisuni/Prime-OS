use crate::windows_state;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const WINDOWS_WINE_BINARY: &str = "/usr/bin/wine";
pub const WINDOWS_WINESERVER_BINARY: &str = "/usr/sbin/wineserver";
pub const WINDOWS_WINE_DONOR_FINGERPRINT: &str =
    "wine-core-11.0-3.fc44.x86_64+wine-core-11.0-3.fc44.i686+wine-common-11.0-3.fc44+wine-mono-10.4.1-2.fc44";
pub const PRIME_WINDOWS_INIT_MARKER: &str = ".prime-w1-initialized";
pub const PRIME_COMPOSITOR_RUNTIME: &str = "/run/prime-compositor";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WineCommandSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

pub fn donor_environment(
    application_id: Uuid,
    runtime_dir: &Path,
    wayland_socket: Option<&str>,
) -> BTreeMap<String, String> {
    let mut environment = BTreeMap::from([
        (
            "HOME".to_owned(),
            runtime_dir.join("home").display().to_string(),
        ),
        ("LANG".to_owned(), "C.UTF-8".to_owned()),
        ("PATH".to_owned(), "/usr/bin:/usr/sbin".to_owned()),
        ("WINEDEBUG".to_owned(), "-all".to_owned()),
        (
            "WINEPREFIX".to_owned(),
            windows_state::application_prefix(application_id)
                .display()
                .to_string(),
        ),
        (
            "XDG_CACHE_HOME".to_owned(),
            runtime_dir.join("cache").display().to_string(),
        ),
        (
            "XDG_CONFIG_HOME".to_owned(),
            runtime_dir.join("config").display().to_string(),
        ),
    ]);
    if let Some(socket) = wayland_socket {
        environment.insert(
            "XDG_RUNTIME_DIR".to_owned(),
            PRIME_COMPOSITOR_RUNTIME.to_owned(),
        );
        environment.insert("WAYLAND_DISPLAY".to_owned(), socket.to_owned());
    }
    environment
}

pub fn prefix_initialization_commands(
    application_id: Uuid,
    runtime_dir: &Path,
    wayland_socket: Option<&str>,
) -> Vec<WineCommandSpec> {
    let environment = donor_environment(application_id, runtime_dir, wayland_socket);
    vec![
        WineCommandSpec {
            program: PathBuf::from(WINDOWS_WINE_BINARY),
            args: vec!["wineboot".to_owned(), "--init".to_owned()],
            env: environment.clone(),
        },
        WineCommandSpec {
            program: PathBuf::from(WINDOWS_WINESERVER_BINARY),
            args: vec!["-w".to_owned()],
            env: environment,
        },
    ]
}

pub fn prefix_marker_matches(prefix: &Path) -> Result<bool, io::Error> {
    let marker = prefix.join(PRIME_WINDOWS_INIT_MARKER);
    match fs::read_to_string(marker) {
        Ok(value) => Ok(value == format!("{WINDOWS_WINE_DONOR_FINGERPRINT}\n")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub fn write_prefix_marker(prefix: &Path) -> Result<(), io::Error> {
    let marker = prefix.join(PRIME_WINDOWS_INIT_MARKER);
    if let Ok(metadata) = fs::symlink_metadata(&marker) {
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Prime Windows initialization marker is not a regular non-symlink file",
            ));
        }
    }
    let temp = prefix.join(format!(
        "{}.{}.tmp",
        PRIME_WINDOWS_INIT_MARKER,
        Uuid::now_v7()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    file.write_all(format!("{WINDOWS_WINE_DONOR_FINGERPRINT}\n").as_bytes())?;
    file.sync_all()?;
    fs::rename(&temp, &marker)?;
    fs::set_permissions(&marker, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn donor_fingerprint_binds_complete_fedora_runtime_closure() {
        assert_eq!(
            WINDOWS_WINE_DONOR_FINGERPRINT,
            "wine-core-11.0-3.fc44.x86_64+wine-core-11.0-3.fc44.i686+wine-common-11.0-3.fc44+wine-mono-10.4.1-2.fc44"
        );
    }

    #[test]
    fn donor_environment_uses_application_prefix_and_ephemeral_runtime_state() {
        let app = Uuid::nil();
        let runtime = PathBuf::from("/run/prime-win-component-001122");
        let env = donor_environment(app, &runtime, Some("wayland-4"));
        assert_eq!(
            env.get("WINEPREFIX").map(String::as_str),
            Some("/var/lib/prime-win-app-00000000000000000000000000000000/wine-prefix")
        );
        assert_eq!(
            env.get("HOME"),
            Some(&runtime.join("home").display().to_string())
        );
        assert_eq!(
            env.get("WAYLAND_DISPLAY").map(String::as_str),
            Some("wayland-4")
        );
        assert_eq!(
            env.get("XDG_RUNTIME_DIR").map(String::as_str),
            Some("/run/prime-compositor")
        );
        assert!(!env.contains_key("DISPLAY"));
    }

    #[test]
    fn initialization_marker_requires_exact_donor_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!prefix_marker_matches(dir.path()).unwrap());
        std::fs::write(dir.path().join(PRIME_WINDOWS_INIT_MARKER), b"stale\n").unwrap();
        assert!(!prefix_marker_matches(dir.path()).unwrap());
        write_prefix_marker(dir.path()).unwrap();
        assert!(prefix_marker_matches(dir.path()).unwrap());
        assert_eq!(
            std::fs::read_to_string(dir.path().join(PRIME_WINDOWS_INIT_MARKER)).unwrap(),
            format!("{WINDOWS_WINE_DONOR_FINGERPRINT}\n")
        );
    }

    #[test]
    fn cold_prefix_initialization_is_fixed_wineboot_then_wineserver_wait() {
        let commands = prefix_initialization_commands(
            Uuid::nil(),
            &PathBuf::from("/run/prime-win-component-001122"),
            None,
        );
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].program, PathBuf::from("/usr/bin/wine"));
        assert_eq!(commands[0].args, vec!["wineboot", "--init"]);
        assert_eq!(commands[1].program, PathBuf::from("/usr/sbin/wineserver"));
        assert_eq!(commands[1].args, vec!["-w"]);
    }
}
