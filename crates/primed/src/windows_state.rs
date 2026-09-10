use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub const COMPATIBILITY_LOCK_NAME: &str = "compatibility.lock";

#[derive(Debug, Error)]
pub enum WindowsStateError {
    #[error("Windows compatibility-state I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("Windows compatibility-state path is unsafe: {0}")]
    UnsafePath(&'static str),
}

pub fn application_state_directory_name(application_id: Uuid) -> String {
    format!(
        "prime-win-app-{}",
        application_id.to_string().replace('-', "")
    )
}

pub fn application_state_root(application_id: Uuid) -> PathBuf {
    PathBuf::from("/var/lib").join(application_state_directory_name(application_id))
}

pub fn application_prefix(application_id: Uuid) -> PathBuf {
    application_state_root(application_id).join("wine-prefix")
}

pub fn ensure_private_directory(path: &Path) -> Result<(), WindowsStateError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
                return Err(WindowsStateError::UnsafePath(
                    "path is not a regular non-symlink directory",
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => match fs::create_dir(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(path)?;
                if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
                    return Err(WindowsStateError::UnsafePath(
                        "path raced to a non-directory or symlink",
                    ));
                }
            }
            Err(error) => return Err(error.into()),
        },
        Err(error) => return Err(error.into()),
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

pub fn acquire_compatibility_lock(state_root: &Path) -> Result<File, WindowsStateError> {
    ensure_private_directory(state_root)?;
    let locks = state_root.join("locks");
    ensure_private_directory(&locks)?;
    let path = locks.join(COMPATIBILITY_LOCK_NAME);
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(WindowsStateError::UnsafePath(
                "compatibility lock is not a regular non-symlink file",
            ));
        }
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)?;
    file.lock()?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, OpenOptions};
    use std::os::unix::fs::{symlink, PermissionsExt};
    use tempfile::tempdir;
    use uuid::Uuid;

    #[test]
    fn application_state_paths_are_stable_and_application_scoped() {
        let id = Uuid::parse_str("12345678-1234-4234-8234-123456789abc").unwrap();
        assert_eq!(
            application_state_directory_name(id),
            "prime-win-app-12345678123442348234123456789abc"
        );
        assert_eq!(
            application_state_root(id),
            std::path::PathBuf::from("/var/lib/prime-win-app-12345678123442348234123456789abc")
        );
        assert_eq!(
            application_prefix(id),
            std::path::PathBuf::from(
                "/var/lib/prime-win-app-12345678123442348234123456789abc/wine-prefix"
            )
        );
    }

    #[test]
    fn private_directory_is_reusable_and_symlink_is_rejected() {
        let dir = tempdir().unwrap();
        let state = dir.path().join("state");
        fs::create_dir(&state).unwrap();
        fs::set_permissions(&state, fs::Permissions::from_mode(0o755)).unwrap();
        ensure_private_directory(&state).unwrap();
        assert_eq!(
            fs::metadata(&state).unwrap().permissions().mode() & 0o777,
            0o700
        );

        let target = dir.path().join("target");
        fs::create_dir(&target).unwrap();
        let link = dir.path().join("link");
        symlink(&target, &link).unwrap();
        assert!(matches!(
            ensure_private_directory(&link),
            Err(WindowsStateError::UnsafePath(_))
        ));
    }

    #[test]
    fn compatibility_lock_is_exclusive_and_released_with_file_lifetime() {
        let dir = tempdir().unwrap();
        ensure_private_directory(dir.path()).unwrap();
        let first = acquire_compatibility_lock(dir.path()).unwrap();
        let lock_path = dir.path().join("locks/compatibility.lock");
        let second = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        assert!(second.try_lock().is_err());
        drop(first);
        second
            .try_lock()
            .expect("lock released with first file handle");
        second.unlock().unwrap();
    }
}
