use prime_contracts::{HardwareGraph, HARDWARE_GRAPH_SCHEMA};
use prime_hardware::ProbeResult;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum HardwareStateError {
    #[error("hardware state I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("hardware state JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("hardware graph schema is {found}, expected {expected}")]
    Schema {
        found: String,
        expected: &'static str,
    },
    #[error("hardware graph revision overflow")]
    RevisionOverflow,
}

pub fn load_or_update(
    path: &Path,
    probe: ProbeResult,
    observed_at: String,
) -> Result<HardwareGraph, HardwareStateError> {
    let previous = if path.exists() {
        Some(load(path)?)
    } else {
        None
    };
    let revision = match previous {
        Some(ref previous) if previous.topology_digest == probe.topology_digest => {
            previous.revision
        }
        Some(ref previous) => previous
            .revision
            .checked_add(1)
            .ok_or(HardwareStateError::RevisionOverflow)?,
        None => 1,
    };
    let graph = HardwareGraph {
        schema: HARDWARE_GRAPH_SCHEMA.to_owned(),
        revision,
        topology_digest: probe.topology_digest,
        observed_at,
        inventory: probe.inventory,
        limitations: probe.limitations,
    };
    write_atomic(path, &serde_json::to_vec_pretty(&graph)?, 0o600)?;
    Ok(graph)
}

pub fn load(path: &Path) -> Result<HardwareGraph, HardwareStateError> {
    let graph: HardwareGraph = serde_json::from_slice(&fs::read(path)?)?;
    if graph.schema != HARDWARE_GRAPH_SCHEMA {
        return Err(HardwareStateError::Schema {
            found: graph.schema,
            expected: HARDWARE_GRAPH_SCHEMA,
        });
    }
    Ok(graph)
}

fn write_atomic(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "hardware state path has no parent",
        )
    })?;
    fs::create_dir_all(parent)?;
    let temp_path = parent.join(format!(".hardware.{}.tmp", Uuid::now_v7()));
    let mut temp = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&temp_path)?;
    temp.write_all(bytes)?;
    temp.write_all(b"\n")?;
    temp.sync_all()?;
    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use prime_contracts::{FingerprintConfidence, HardwareFingerprint, HardwareInventory};

    fn probe(digest: &str) -> ProbeResult {
        ProbeResult {
            inventory: HardwareInventory {
                host_arch: "x86_64".to_owned(),
                ..HardwareInventory::default()
            },
            limitations: Vec::new(),
            fingerprint: HardwareFingerprint {
                algorithm: "sha256".to_owned(),
                digest: None,
                confidence: FingerprintConfidence::Unprobed,
                observed_at: None,
            },
            topology_digest: digest.to_owned(),
        }
    }

    #[test]
    fn revision_changes_only_when_topology_digest_changes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("hardware/current.json");
        let first = load_or_update(&path, probe("sha256:a"), "t1".to_owned()).expect("first");
        let same = load_or_update(&path, probe("sha256:a"), "t2".to_owned()).expect("same");
        let changed = load_or_update(&path, probe("sha256:b"), "t3".to_owned()).expect("changed");
        assert_eq!(first.revision, 1);
        assert_eq!(same.revision, 1);
        assert_eq!(changed.revision, 2);
    }

    #[test]
    fn corrupt_existing_graph_fails_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("hardware/current.json");
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(&path, "broken").expect("write corrupt graph");
        assert!(load_or_update(&path, probe("sha256:a"), "t1".to_owned()).is_err());
    }
}
