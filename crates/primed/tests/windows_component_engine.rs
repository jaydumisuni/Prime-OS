use prime_contracts::{
    WindowsComponentKind, WindowsComponentManifest, WindowsInstallerKind, WindowsVerificationProbe,
    WINDOWS_COMPONENT_SCHEMA,
};
use primed::windows_component_engine::{
    all_probes_pass, component_marker_matches, verify_component_state, write_component_marker,
    WindowsComponentEngineError,
};
use primed::windows_components::seal_component;
use std::fs;
use std::os::unix::fs::symlink;
use tempfile::tempdir;

const DONOR: &str = "proof-donor-v1";

fn manifest(probes: Vec<WindowsVerificationProbe>) -> WindowsComponentManifest {
    seal_component(WindowsComponentManifest {
        schema: WINDOWS_COMPONENT_SCHEMA.to_owned(),
        component_id: "runtime.probe".to_owned(),
        revision: 1,
        digest: String::new(),
        display_name: "Probe Runtime".to_owned(),
        kind: WindowsComponentKind::Runtime,
        workload_arches: vec!["x86_64".to_owned()],
        depends_on: vec![],
        installer_kind: WindowsInstallerKind::Builtin,
        artifact_identity: None,
        artifact_path: None,
        installer_args: vec![],
        accepted_exit_codes: vec![],
        restart_compatibility_environment: false,
        verification: probes,
        limitations: vec![],
    })
    .unwrap()
}

#[test]
fn file_exists_and_sha256_probes_observe_real_prefix_state() {
    let dir = tempdir().unwrap();
    let prefix = dir.path().join("prefix");
    fs::create_dir(&prefix).unwrap();
    fs::create_dir(prefix.join("drive_c")).unwrap();
    fs::write(prefix.join("drive_c/proof.txt"), b"prime-w2").unwrap();
    let expected = "sha256:95cd83c223ed24a06c827e13fcfe174b6cefc091a87a6796ff43474c3ed21434";
    let m = manifest(vec![
        WindowsVerificationProbe::FileExists {
            path: "drive_c/proof.txt".to_owned(),
        },
        WindowsVerificationProbe::FileSha256 {
            path: "drive_c/proof.txt".to_owned(),
            sha256: expected.to_owned(),
        },
    ]);
    let results = verify_component_state(&prefix, &m).unwrap();
    assert!(all_probes_pass(&results));
    assert_eq!(results.len(), 2);

    fs::write(prefix.join("drive_c/proof.txt"), b"changed").unwrap();
    let changed = verify_component_state(&prefix, &m).unwrap();
    assert!(!all_probes_pass(&changed));
    assert!(changed[0].passed);
    assert!(!changed[1].passed);
}

#[test]
fn probe_paths_reject_traversal_absolute_paths_and_symlink_escape() {
    let dir = tempdir().unwrap();
    let prefix = dir.path().join("prefix");
    fs::create_dir(&prefix).unwrap();
    let outside = dir.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("secret.txt"), b"secret").unwrap();
    symlink(&outside, prefix.join("escape")).unwrap();

    for path in ["../outside/secret.txt", "/etc/passwd", "escape/secret.txt"] {
        let m = manifest(vec![WindowsVerificationProbe::FileExists {
            path: path.to_owned(),
        }]);
        assert!(matches!(
            verify_component_state(&prefix, &m),
            Err(WindowsComponentEngineError::UnsafeProbePath(_))
        ));
    }
}

#[test]
fn registry_probe_fails_closed_until_donor_query_is_bound() {
    let dir = tempdir().unwrap();
    let prefix = dir.path().join("prefix");
    fs::create_dir(&prefix).unwrap();
    let m = manifest(vec![WindowsVerificationProbe::RegistryValueEquals {
        hive: "HKLM".to_owned(),
        key: r"Software\PrimeW2".to_owned(),
        name: "Installed".to_owned(),
        value: "1".to_owned(),
    }]);
    assert!(matches!(
        verify_component_state(&prefix, &m),
        Err(WindowsComponentEngineError::ProbeUnsupported(_))
    ));
}

#[test]
fn marker_is_exact_cache_but_probe_failure_still_wins() {
    let dir = tempdir().unwrap();
    let state = dir.path().join("state");
    let prefix = dir.path().join("prefix");
    fs::create_dir(&state).unwrap();
    fs::create_dir(&prefix).unwrap();
    fs::create_dir(prefix.join("drive_c")).unwrap();
    fs::write(prefix.join("drive_c/proof.txt"), b"prime-w2").unwrap();
    let m = manifest(vec![WindowsVerificationProbe::FileExists {
        path: "drive_c/proof.txt".to_owned(),
    }]);
    let results = verify_component_state(&prefix, &m).unwrap();
    let marker_digest =
        write_component_marker(&state, &m, DONOR, "2026-09-10T12:00:00Z", &results).unwrap();
    assert!(marker_digest.starts_with("sha256:"));
    assert!(component_marker_matches(&state, &m, DONOR).unwrap());
    assert!(!component_marker_matches(&state, &m, "different-donor").unwrap());

    fs::remove_file(prefix.join("drive_c/proof.txt")).unwrap();
    let observed = verify_component_state(&prefix, &m).unwrap();
    assert!(!all_probes_pass(&observed));
    assert!(component_marker_matches(&state, &m, DONOR).unwrap());
}
