use prime_contracts::{ArtifactFormat, WindowsProviderManifest, WINDOWS_PROVIDER_MANIFEST_SCHEMA};
use primed::windows_personality::{load_provider, WindowsPersonalityError};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn write_adapter(path: &Path, mode: u32) {
    fs::write(path, b"#!/bin/false\n").expect("write adapter");
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("chmod adapter");
}

fn write_manifest(
    dir: &Path,
    file_name: &str,
    provider_id: &str,
    revision: u64,
    adapter_path: &Path,
    arches: &[&str],
) {
    let manifest = WindowsProviderManifest {
        schema: WINDOWS_PROVIDER_MANIFEST_SCHEMA.to_owned(),
        provider_id: provider_id.to_owned(),
        provider_revision: revision,
        adapter_path: adapter_path.display().to_string(),
        formats: vec![ArtifactFormat::Pe32, ArtifactFormat::Pe32Plus],
        workload_arches: arches.iter().map(|value| (*value).to_owned()).collect(),
        limitations: vec!["W1_PORTABLE_ONLY".to_owned()],
    };
    fs::write(
        dir.join(file_name),
        serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");
}

#[test]
fn missing_provider_directory_is_explicitly_unavailable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("missing");
    assert!(matches!(
        load_provider(&missing, &ArtifactFormat::Pe32Plus, "x86_64"),
        Err(WindowsPersonalityError::ProviderUnavailable(_))
    ));
}

#[test]
fn relative_adapter_path_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest = WindowsProviderManifest {
        schema: WINDOWS_PROVIDER_MANIFEST_SCHEMA.to_owned(),
        provider_id: "prime.windows.relative".to_owned(),
        provider_revision: 1,
        adapter_path: "relative/provider".to_owned(),
        formats: vec![ArtifactFormat::Pe32Plus],
        workload_arches: vec!["x86_64".to_owned()],
        limitations: vec![],
    };
    fs::write(
        dir.path().join("relative.json"),
        serde_json::to_vec(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");

    assert!(matches!(
        load_provider(dir.path(), &ArtifactFormat::Pe32Plus, "x86_64"),
        Err(WindowsPersonalityError::InvalidProvider(_))
    ));
}

#[test]
fn non_executable_adapter_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let adapter = dir.path().join("adapter");
    write_adapter(&adapter, 0o644);
    write_manifest(
        dir.path(),
        "provider.json",
        "prime.windows.nonexec",
        1,
        &adapter,
        &["x86_64"],
    );

    assert!(matches!(
        load_provider(dir.path(), &ArtifactFormat::Pe32Plus, "x86_64"),
        Err(WindowsPersonalityError::InvalidProvider(_))
    ));
}

#[test]
fn unsupported_architecture_has_no_compatible_provider() {
    let dir = tempfile::tempdir().expect("tempdir");
    let adapter = dir.path().join("adapter");
    write_adapter(&adapter, 0o755);
    write_manifest(
        dir.path(),
        "provider.json",
        "prime.windows.x64-only",
        1,
        &adapter,
        &["x86_64"],
    );

    assert!(matches!(
        load_provider(dir.path(), &ArtifactFormat::Pe32, "arm"),
        Err(WindowsPersonalityError::NoCompatibleProvider { .. })
    ));
}

#[test]
fn selection_is_deterministic_and_prefers_latest_revision_within_provider() {
    let dir = tempfile::tempdir().expect("tempdir");
    let adapter_a1 = dir.path().join("adapter-a1");
    let adapter_a2 = dir.path().join("adapter-a2");
    let adapter_b = dir.path().join("adapter-b");
    for path in [&adapter_a1, &adapter_a2, &adapter_b] {
        write_adapter(path, 0o755);
    }
    write_manifest(
        dir.path(),
        "b.json",
        "prime.windows.b",
        9,
        &adapter_b,
        &["x86_64"],
    );
    write_manifest(
        dir.path(),
        "a1.json",
        "prime.windows.a",
        1,
        &adapter_a1,
        &["x86_64"],
    );
    write_manifest(
        dir.path(),
        "a2.json",
        "prime.windows.a",
        2,
        &adapter_a2,
        &["x86_64"],
    );

    let selected = load_provider(dir.path(), &ArtifactFormat::Pe32Plus, "x86_64")
        .expect("compatible provider");
    assert_eq!(selected.manifest.provider_id, "prime.windows.a");
    assert_eq!(selected.manifest.provider_revision, 2);
    assert_eq!(selected.adapter_path, fs::canonicalize(adapter_a2).expect("canonical adapter"));
}
