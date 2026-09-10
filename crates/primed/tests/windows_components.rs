use prime_contracts::{
    WindowsComponentKind, WindowsComponentManifest, WindowsInstallerKind, WindowsVerificationProbe,
    WINDOWS_COMPONENT_SCHEMA,
};
use primed::windows_components::{
    resolve_component_plan, seal_component, store_component_revision, WindowsComponentRegistryError,
};
use tempfile::tempdir;

fn component(
    id: &str,
    revision: u64,
    arches: &[&str],
    depends_on: Vec<String>,
) -> WindowsComponentManifest {
    WindowsComponentManifest {
        schema: WINDOWS_COMPONENT_SCHEMA.to_owned(),
        component_id: id.to_owned(),
        revision,
        digest: String::new(),
        display_name: id.to_owned(),
        kind: WindowsComponentKind::Runtime,
        workload_arches: arches.iter().map(|value| (*value).to_owned()).collect(),
        depends_on,
        installer_kind: WindowsInstallerKind::Builtin,
        artifact_identity: None,
        artifact_path: None,
        installer_args: vec![],
        accepted_exit_codes: vec![],
        restart_compatibility_environment: false,
        verification: vec![WindowsVerificationProbe::FileExists {
            path: format!("drive_c/PrimeW2/{id}.txt"),
        }],
        limitations: vec![],
    }
}

fn install(root: &std::path::Path, manifest: WindowsComponentManifest) -> WindowsComponentManifest {
    let sealed = seal_component(manifest).expect("seal component");
    store_component_revision(root, &sealed).expect("store component");
    sealed
}

fn reference(manifest: &WindowsComponentManifest) -> String {
    format!(
        "windows-component:{}@{}#{}",
        manifest.component_id, manifest.revision, manifest.digest
    )
}

#[test]
fn sealed_component_round_trips_through_immutable_registry() {
    let dir = tempdir().unwrap();
    let sealed = install(
        dir.path(),
        component("runtime.base", 1, &["x86_64"], vec![]),
    );
    let plan = resolve_component_plan(dir.path(), &[reference(&sealed)], "x86_64").unwrap();
    assert_eq!(plan.ordered, vec![sealed]);
}

#[test]
fn resolver_rejects_reference_digest_mismatch() {
    let dir = tempdir().unwrap();
    let sealed = install(
        dir.path(),
        component("runtime.base", 1, &["x86_64"], vec![]),
    );
    let bad = format!(
        "windows-component:{}@{}#sha256:{}",
        sealed.component_id,
        sealed.revision,
        "b".repeat(64)
    );
    assert!(matches!(
        resolve_component_plan(dir.path(), &[bad], "x86_64"),
        Err(WindowsComponentRegistryError::ReferenceDigestMismatch { .. })
    ));
}

#[test]
fn resolver_orders_dependencies_before_dependents_and_rejects_cycles() {
    let dir = tempdir().unwrap();
    let leaf = install(
        dir.path(),
        component("runtime.leaf", 1, &["x86_64"], vec![]),
    );
    let middle = install(
        dir.path(),
        component("runtime.middle", 1, &["x86_64"], vec![reference(&leaf)]),
    );
    let top = install(
        dir.path(),
        component("runtime.top", 1, &["x86_64"], vec![reference(&middle)]),
    );
    let plan = resolve_component_plan(dir.path(), &[reference(&top)], "x86_64").unwrap();
    assert_eq!(
        plan.ordered
            .iter()
            .map(|item| item.component_id.as_str())
            .collect::<Vec<_>>(),
        vec!["runtime.leaf", "runtime.middle", "runtime.top"]
    );

    let cycle_dir = tempdir().unwrap();
    let placeholder = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let a_raw = component(
        "cycle.a",
        1,
        &["x86_64"],
        vec![format!("windows-component:cycle.b@1#{placeholder}")],
    );
    let b_raw = component(
        "cycle.b",
        1,
        &["x86_64"],
        vec![format!("windows-component:cycle.a@1#{placeholder}")],
    );
    let mut a = seal_component(a_raw).unwrap();
    let mut b = seal_component(b_raw).unwrap();
    // Bind each edge to the final peer digest, then reseal until the test registry is self-consistent.
    // The resolver must still reject the graph structurally rather than recurse forever.
    a.depends_on[0] = format!("windows-component:cycle.b@1#{}", b.digest);
    a = seal_component(a).unwrap();
    b.depends_on[0] = format!("windows-component:cycle.a@1#{}", a.digest);
    b = seal_component(b).unwrap();
    a.depends_on[0] = format!("windows-component:cycle.b@1#{}", b.digest);
    a = seal_component(a).unwrap();
    store_component_revision(cycle_dir.path(), &a).unwrap();
    store_component_revision(cycle_dir.path(), &b).unwrap();
    let result = resolve_component_plan(cycle_dir.path(), &[reference(&a)], "x86_64");
    assert!(matches!(
        result,
        Err(WindowsComponentRegistryError::DependencyCycle { .. })
    ));
}

#[test]
fn resolver_rejects_duplicate_roots_and_wrong_architecture() {
    let dir = tempdir().unwrap();
    let sealed = install(
        dir.path(),
        component("runtime.base", 1, &["x86_64"], vec![]),
    );
    let pin = reference(&sealed);
    assert!(matches!(
        resolve_component_plan(dir.path(), &[pin.clone(), pin.clone()], "x86_64"),
        Err(WindowsComponentRegistryError::DuplicateReference(_))
    ));
    assert!(matches!(
        resolve_component_plan(dir.path(), &[pin], "x86"),
        Err(WindowsComponentRegistryError::ArchitectureUnsupported { .. })
    ));
}

#[test]
fn shipped_wine_mono_component_resolves_from_trusted_registry() {
    let component_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../image/windows-components");
    let manifest =
        primed::windows_components::load_component_revision(&component_dir, "runtime.wine-mono", 1)
            .expect("load shipped Wine Mono component");
    assert_eq!(
        manifest.digest,
        "sha256:47526839b2fc8c981330d77d2c3b6a4b2528b5cff2ddd547f33aef0671782689"
    );
    assert_eq!(manifest.installer_kind, WindowsInstallerKind::Builtin);
    assert!(manifest
        .limitations
        .iter()
        .any(|item| item.contains("managed application execution is W3")));
    let plan = resolve_component_plan(&component_dir, &[reference(&manifest)], "x86_64")
        .expect("resolve shipped Wine Mono component");
    assert_eq!(plan.ordered, vec![manifest]);
}
