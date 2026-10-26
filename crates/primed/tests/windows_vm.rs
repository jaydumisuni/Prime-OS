use prime_contracts::{
    ApplicationArtifact, ApplicationProfile, ArtifactFormat, CompatibilityRecord, ExecutionBackend,
    MechanicalCompatibilityState, PolicyReference, RuntimeFamily, APPLICATION_PROFILE_SCHEMA,
};
use primed::windows_vm::{
    build_qemu_plan, validate_guest_definition, validate_vm_profile, VmGuestDefinition,
    VmPlanRequest,
};
use sha2::{Digest, Sha256};
use std::fs;
use tempfile::tempdir;
use uuid::Uuid;

fn private_tempdir() -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}

#[cfg(unix)]
#[test]
fn private_tempdir_is_private_under_worker_umask() {
    use std::os::unix::fs::PermissionsExt;
    let dir = private_tempdir();
    assert_eq!(
        fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

// Positive VM fixtures must be private regardless of the invoking worker's umask.
fn private_runtime_dir(path: &std::path::Path) {
    fs::create_dir(path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
}

fn private_overlay(path: &std::path::Path) {
    // Header-only fixture for plan admission; not a bootable guest disk.
    let mut header = vec![0u8; 104];
    header[..8].copy_from_slice(b"QFI\xfb\x00\x00\x00\x03");
    header[20..24].copy_from_slice(&16u32.to_be_bytes());
    header[24..32].copy_from_slice(&(64u64 * 1024 * 1024).to_be_bytes());
    header[100..104].copy_from_slice(&104u32.to_be_bytes());
    fs::write(path, header).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}

// Guest-image fixtures must satisfy the same private-file admission contract
// under both restrictive and permissive worker umasks.
fn private_guest_image(path: &std::path::Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}

fn definition(image: &std::path::Path, bytes: &[u8]) -> VmGuestDefinition {
    VmGuestDefinition {
        schema: "prime.windows-vm-guest.v1".to_owned(),
        guest_id: "windows-fallback-test".to_owned(),
        revision: 1,
        base_image: image.display().to_string(),
        base_sha256: digest(bytes),
        memory_mib: 4096,
        vcpus: 2,
    }
}

#[test]
fn guest_definition_rejects_unrecognized_json_fields() {
    let mut value = serde_json::to_value(VmGuestDefinition {
        schema: "prime.windows-vm-guest.v1".to_owned(),
        guest_id: "windows-fallback-test".to_owned(),
        revision: 1,
        base_image: "/tmp/guest.qcow2".to_owned(),
        base_sha256: "0".repeat(64),
        memory_mib: 2048,
        vcpus: 2,
    })
    .unwrap();
    value["future_extension"] = serde_json::json!(true);
    assert!(serde_json::from_value::<VmGuestDefinition>(value).is_err());
}

#[test]
fn guest_definition_requires_exact_digest_bound_regular_image() {
    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-deterministic-guest";
    private_guest_image(&image, bytes);

    let validated = validate_guest_definition(&definition(&image, bytes)).unwrap();
    assert_eq!(validated.base_image, image);

    fs::write(&image, b"tampered").unwrap();
    let error = validate_guest_definition(&definition(&image, bytes)).unwrap_err();
    assert!(error.to_string().contains("digest"));
}

#[test]
fn guest_definition_rejects_empty_digest_matching_image() {
    let dir = private_tempdir();
    let image = dir.path().join("empty-windows.qcow2");
    private_guest_image(&image, b"");

    // The SHA-256 of an empty file is valid, but it is not a bootable guest.
    let error = validate_guest_definition(&definition(&image, b"")).unwrap_err();
    assert!(error.to_string().contains("empty"));
}

#[cfg(unix)]
#[test]
fn guest_definition_rejects_hardlinked_base_image_alias() {
    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let outside = dir.path().join("unrelated-host-file");
    let bytes = b"prime-w8-guest-hardlink-alias";
    private_guest_image(&image, bytes);
    fs::hard_link(&image, &outside).unwrap();

    assert!(validate_guest_definition(&definition(&image, bytes)).is_err());
    assert_eq!(fs::read(&outside).unwrap(), bytes);
}

#[cfg(unix)]
#[test]
fn guest_definition_rejects_shared_writable_image_directory() {
    use std::os::unix::fs::PermissionsExt;

    let dir = private_tempdir();
    let shared = dir.path().join("shared");
    fs::create_dir(&shared).unwrap();
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o777)).unwrap();
    let image = shared.join("windows.qcow2");
    let bytes = b"prime-w8-shared-parent";
    private_guest_image(&image, bytes);

    let error = validate_guest_definition(&definition(&image, bytes)).unwrap_err();
    assert!(error.to_string().contains("directory"), "{error}");
}

#[test]
fn guest_definition_rejects_noncanonical_digest_encoding() {
    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-guest-canonical-digest";
    private_guest_image(&image, bytes);

    let canonical = definition(&image, bytes);
    assert!(validate_guest_definition(&canonical).is_ok());

    let mut uppercase = canonical.clone();
    uppercase.base_sha256 = uppercase.base_sha256.to_ascii_uppercase();
    assert_ne!(uppercase.base_sha256, canonical.base_sha256);
    assert!(validate_guest_definition(&uppercase).is_err());

    let mut nonhex = canonical;
    nonhex.base_sha256 = "g".repeat(64);
    assert!(validate_guest_definition(&nonhex).is_err());
}

#[test]
fn guest_definition_rejects_noncanonical_guest_identity() {
    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-guest-identity";
    private_guest_image(&image, bytes);

    for guest_id in [
        " leading",
        "trailing ",
        "line\nbreak",
        "guest/alias",
        "guest\\alias",
        "guest\u{202e}spoof",
        "guest.name",
    ] {
        let mut guest = definition(&image, bytes);
        guest.guest_id = guest_id.to_owned();
        assert!(validate_guest_definition(&guest).is_err());
    }

    let mut oversized = definition(&image, bytes);
    oversized.guest_id = "g".repeat(129);
    assert!(validate_guest_definition(&oversized).is_err());
}

#[test]
fn qemu_plan_rejects_invalid_qcow2_header_before_launch() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");

    let mut invalid_magic = vec![0u8; 104];
    invalid_magic[..8].copy_from_slice(b"BAD!\x00\x00\x00\x03");
    let mut invalid_version = vec![0u8; 104];
    invalid_version[..8].copy_from_slice(b"QFI\xfb\x00\x00\x00\x01");
    let truncated_v3 = {
        let mut bytes = vec![0u8; 72];
        bytes[..8].copy_from_slice(b"QFI\xfb\x00\x00\x00\x03");
        bytes
    };

    // Reject both malformed full-length headers and truncated v3 headers.
    for bytes in [invalid_magic, invalid_version, truncated_v3] {
        private_overlay(&overlay);
        fs::write(&overlay, bytes).unwrap();
        let error = build_qemu_plan(&VmPlanRequest {
            qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
            overlay_image: overlay.clone(),
            runtime_dir: runtime.clone(),
            memory_mib: 2048,
            vcpus: 2,
            usb_nodes: vec![],
        })
        .unwrap_err();
        assert!(error.to_string().contains("qcow2"), "{error}");
    }
}

#[test]
fn qemu_plan_rejects_inconsistent_qcow2_v3_header_length() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    // A valid magic/version does not make an impossible declared header safe.
    for (declared_length, actual_length) in [(0u32, 104usize), (103, 104), (112, 104)] {
        private_overlay(&overlay);
        let mut bytes = vec![0u8; actual_length];
        bytes[..8].copy_from_slice(b"QFI\xfb\x00\x00\x00\x03");
        bytes[100..104].copy_from_slice(&declared_length.to_be_bytes());
        fs::write(&overlay, bytes).unwrap();
        let error = build_qemu_plan(&request).unwrap_err();
        assert!(error.to_string().contains("qcow2"), "{error}");
    }

    // An extended header with the matching on-disk length is admissible to
    // this bounded header preflight, not a full qcow2 integrity qualification.
    private_overlay(&overlay);
    let mut extended = vec![0u8; 112];
    extended[..8].copy_from_slice(b"QFI\xfb\x00\x00\x00\x03");
    extended[20..24].copy_from_slice(&16u32.to_be_bytes());
    extended[24..32].copy_from_slice(&(64u64 * 1024 * 1024).to_be_bytes());
    extended[100..104].copy_from_slice(&112u32.to_be_bytes());
    fs::write(&overlay, extended).unwrap();
    assert!(build_qemu_plan(&request).is_ok());
}

#[test]
fn qemu_plan_rejects_unaligned_qcow2_v3_header_length() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    // qcow2 v3 header_length must be 8-byte aligned. These declarations
    // are otherwise within the file and first cluster, so only alignment
    // should reject them during bounded host preflight.
    for declared_length in [105u32, 111u32] {
        private_overlay(&overlay);
        let mut bytes = fs::read(&overlay).unwrap();
        bytes.resize(declared_length as usize, 0);
        bytes[100..104].copy_from_slice(&declared_length.to_be_bytes());
        fs::write(&overlay, bytes).unwrap();
        let error = build_qemu_plan(&request).unwrap_err();
        assert!(error.to_string().contains("qcow2"), "{error}");
    }
}

#[test]
fn qemu_plan_rejects_qcow2_v3_header_spilling_past_first_cluster() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    // A v3 header must fit inside its first qcow2 cluster, even when the
    // declared length fits the physical file. Header-only admission must
    // not hand a cross-cluster metadata declaration to QEMU.
    for (cluster_bits, declared_length) in [(9u32, 520u32), (16, 65544)] {
        let mut bytes = vec![0u8; declared_length as usize];
        bytes[..8].copy_from_slice(b"QFI\xfb\x00\x00\x00\x03");
        bytes[20..24].copy_from_slice(&cluster_bits.to_be_bytes());
        bytes[24..32].copy_from_slice(&(64u64 * 1024 * 1024).to_be_bytes());
        bytes[100..104].copy_from_slice(&declared_length.to_be_bytes());
        fs::write(&overlay, bytes).unwrap();
        let error = build_qemu_plan(&request).unwrap_err();
        assert!(
            error.to_string().contains("qcow2"),
            "{cluster_bits}: {error}"
        );
    }

    // A header ending exactly at the cluster boundary is not rejected
    // by this narrow geometry rule; bootability remains a separate gate.
    let mut boundary = vec![0u8; 512];
    boundary[..8].copy_from_slice(b"QFI\xfb\x00\x00\x00\x03");
    boundary[20..24].copy_from_slice(&9u32.to_be_bytes());
    boundary[24..32].copy_from_slice(&(64u64 * 1024 * 1024).to_be_bytes());
    boundary[100..104].copy_from_slice(&512u32.to_be_bytes());
    fs::write(&overlay, boundary).unwrap();
    assert!(build_qemu_plan(&request).is_ok());
}

#[test]
fn qemu_plan_rejects_nonzero_unadmitted_qcow2_v3_extended_header() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    // The fixed v3 header is 104 bytes. Unknown nonzero fields in a longer
    // declared header are not part of the admitted W8 VM state contract.
    // All cases fit inside the first cluster and the on-disk file.
    for (header_len, changed_byte) in [(112u32, 104usize), (112, 111), (520, 519)] {
        private_overlay(&overlay);
        let mut bytes = fs::read(&overlay).unwrap();
        bytes.resize(header_len as usize, 0);
        bytes[100..104].copy_from_slice(&header_len.to_be_bytes());
        bytes[changed_byte] = 0x80;
        fs::write(&overlay, bytes).unwrap();
        let error = build_qemu_plan(&request).unwrap_err();
        assert!(
            error.to_string().contains("extended header"),
            "length {header_len}, byte {changed_byte}: {error}"
        );
    }

    // Zero-filled extended header space remains admissible to this narrow
    // preflight. No test here establishes guest boot or qcow2 integrity.
    for header_len in [104u32, 112, 520] {
        private_overlay(&overlay);
        let mut bytes = fs::read(&overlay).unwrap();
        bytes.resize(header_len as usize, 0);
        bytes[100..104].copy_from_slice(&header_len.to_be_bytes());
        fs::write(&overlay, bytes).unwrap();
        assert!(build_qemu_plan(&request).is_ok(), "length {header_len}");
    }
}

#[test]
fn qemu_plan_rejects_unadmitted_qcow2_extensions_after_header() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    // Header extensions are stored after the v2 fixed header (72 bytes)
    // or the v3 declared header, not within the declared header itself.
    // The previous preflight checked only declared v3 header bytes.
    for (version, header_length, extension_offset) in [
        (2u32, 0u32, 72usize),
        (3, 104, 104),
        (3, 112, 112),
    ] {
        private_overlay(&overlay);
        let mut bytes = fs::read(&overlay).unwrap();
        bytes[4..8].copy_from_slice(&version.to_be_bytes());
        if version == 2 {
            bytes[72..104].fill(0);
        } else {
            bytes[100..104].copy_from_slice(&header_length.to_be_bytes());
        }
        bytes.resize(extension_offset + 8, 0);
        // A nonzero extension type would be interpreted by QEMU even though
        // the declared header itself contains no unadmitted fields.
        bytes[extension_offset..extension_offset + 4]
            .copy_from_slice(&0x6803_f857u32.to_be_bytes());
        fs::write(&overlay, &bytes).unwrap();
        let error = build_qemu_plan(&request).unwrap_err();
        assert!(
            error.to_string().contains("header extensions"),
            "version {version}, header {header_length}: {error}"
        );
    }

    // Zero-filled present first-cluster tail remains acceptable to this
    // bounded header-only preflight, without asserting guest bootability.
    for (version, header_length, extension_offset) in [
        (2u32, 0u32, 72usize),
        (3, 104, 104),
        (3, 112, 112),
    ] {
        private_overlay(&overlay);
        let mut bytes = fs::read(&overlay).unwrap();
        bytes[4..8].copy_from_slice(&version.to_be_bytes());
        if version == 2 {
            bytes[72..104].fill(0);
        } else {
            bytes[100..104].copy_from_slice(&header_length.to_be_bytes());
        }
        bytes.resize(extension_offset + 8, 0);
        fs::write(&overlay, bytes).unwrap();
        assert!(
            build_qemu_plan(&request).is_ok(),
            "version {version}, header {header_length}"
        );
    }
}

#[test]
fn qemu_plan_rejects_oversized_qcow2_v3_refcount_order() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    // QEMU's qcow2 format contract: refcount_bits = 1 << refcount_order;
    // a v3 order greater than 6 cannot describe a supported refcount.
    // Header-only preflight must reject it before producing a QEMU plan.
    for invalid_order in [7u32, u32::MAX] {
        private_overlay(&overlay);
        let mut bytes = fs::read(&overlay).unwrap();
        bytes[96..100].copy_from_slice(&invalid_order.to_be_bytes());
        fs::write(&overlay, bytes).unwrap();
        let error = build_qemu_plan(&request).unwrap_err();
        assert!(
            error.to_string().contains("qcow2"),
            "{invalid_order}: {error}"
        );
    }

    // Both a zero order and the documented maximum remain admitted at
    // this narrow header boundary, without claiming guest bootability.
    for valid_order in [0u32, 4, 6] {
        private_overlay(&overlay);
        let mut bytes = fs::read(&overlay).unwrap();
        bytes[96..100].copy_from_slice(&valid_order.to_be_bytes());
        fs::write(&overlay, bytes).unwrap();
        assert!(build_qemu_plan(&request).is_ok(), "{valid_order}");
    }
}

#[test]
fn qemu_plan_rejects_unaligned_qcow2_metadata_table_pointers() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    // Both fixed-header table offsets are cluster-based in qcow2 v2/v3.
    // Nonzero byte offsets within a cluster are not valid table pointers.
    for version in [2u32, 3] {
        for field_start in [40usize, 48] {
            for bad_offset in [1u64, 512, 65535, 65537, u64::MAX] {
                private_overlay(&overlay);
                let mut bytes = fs::read(&overlay).unwrap();
                bytes[4..8].copy_from_slice(&version.to_be_bytes());
                if version == 2 {
                    bytes.truncate(72);
                }
                bytes[field_start..field_start + 8]
                    .copy_from_slice(&bad_offset.to_be_bytes());
                fs::write(&overlay, bytes).unwrap();
                let error = build_qemu_plan(&request).unwrap_err();
                assert!(
                    error.to_string().contains("metadata table offset"),
                    "version {version}, field {field_start}, offset {bad_offset}: {error}"
                );
            }
        }
    }

    // A zero pointer or an aligned pointer is not rejected by this narrow
    // rule; table integrity and guest boot remain independent proof gates.
    for version in [2u32, 3] {
        for field_start in [40usize, 48] {
            for admitted_offset in [0u64, 65536, 131072] {
                private_overlay(&overlay);
                let mut bytes = fs::read(&overlay).unwrap();
                bytes[4..8].copy_from_slice(&version.to_be_bytes());
                if version == 2 {
                    bytes.truncate(72);
                }
                bytes[field_start..field_start + 8]
                    .copy_from_slice(&admitted_offset.to_be_bytes());
                fs::write(&overlay, bytes).unwrap();
                assert!(
                    build_qemu_plan(&request).is_ok(),
                    "version {version}, field {field_start}, offset {admitted_offset}"
                );
            }
        }
    }
}

#[test]
fn qemu_plan_rejects_qcow2_table_counts_without_offsets() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    // The qcow2 v2/v3 fixed header binds L1 size at bytes 36..40 to
    // its table pointer at 40..48, and refcount table cluster count at
    // 56..60 to its pointer at 48..56.
    for version in [2u32, 3] {
        for (count_start, offset_start) in [(36usize, 40usize), (56, 48)] {
            for count in [1u32, u32::MAX] {
                private_overlay(&overlay);
                let mut bytes = fs::read(&overlay).unwrap();
                bytes[4..8].copy_from_slice(&version.to_be_bytes());
                if version == 2 {
                    bytes.truncate(72);
                }
                bytes[count_start..count_start + 4].copy_from_slice(&count.to_be_bytes());
                bytes[offset_start..offset_start + 8].fill(0);
                fs::write(&overlay, bytes).unwrap();
                let error = build_qemu_plan(&request).unwrap_err();
                assert!(
                    error.to_string().contains("metadata table count"),
                    "version {version}, count field {count_start}, count {count}: {error}"
                );
            }
            // A zero count/offset or a nonzero count with an aligned
            // pointer is not rejected by this narrow coherence rule.
            for (count, offset) in [(0u32, 0u64), (1, 65536)] {
                private_overlay(&overlay);
                let mut bytes = fs::read(&overlay).unwrap();
                bytes[4..8].copy_from_slice(&version.to_be_bytes());
                if version == 2 {
                    bytes.truncate(72);
                }
                bytes[count_start..count_start + 4].copy_from_slice(&count.to_be_bytes());
                bytes[offset_start..offset_start + 8].copy_from_slice(&offset.to_be_bytes());
                fs::write(&overlay, bytes).unwrap();
                assert!(
                    build_qemu_plan(&request).is_ok(),
                    "version {version}, count field {count_start}, count {count}, offset {offset}"
                );
            }
        }
    }
}

#[test]
fn qemu_plan_rejects_unbound_internal_qcow2_snapshots() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    // W8's session overlay has no internal snapshot selection/restore authority.
    // The qcow2 v2/v3 header encodes snapshot count and table offset at
    // bytes 60..72; any such metadata must be rejected before QEMU planning.
    for version in [2u32, 3] {
        for (count, offset) in [(1u32, 65536u64), (1, 0), (0, 65536)] {
            private_overlay(&overlay);
            let mut bytes = fs::read(&overlay).unwrap();
            bytes[4..8].copy_from_slice(&version.to_be_bytes());
            if version == 2 {
                bytes.truncate(72);
            }
            bytes[60..64].copy_from_slice(&count.to_be_bytes());
            bytes[64..72].copy_from_slice(&offset.to_be_bytes());
            fs::write(&overlay, bytes).unwrap();
            let error = build_qemu_plan(&request).unwrap_err();
            assert!(
                error.to_string().contains("snapshot"),
                "version {version}, count {count}, offset {offset}: {error}"
            );
        }

        // Snapshot-free overlays remain admissible to this header-only gate.
        private_overlay(&overlay);
        let mut bytes = fs::read(&overlay).unwrap();
        bytes[4..8].copy_from_slice(&version.to_be_bytes());
        if version == 2 {
            bytes.truncate(72);
        }
        fs::write(&overlay, bytes).unwrap();
        assert!(build_qemu_plan(&request).is_ok(), "version {version}");
    }
}

#[test]
fn qemu_plan_rejects_invalid_qcow2_cluster_geometry() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    // These headers have valid magic/version but cannot describe a usable
    // qcow2 geometry. Header-only admission must not hand them to QEMU.
    for (cluster_bits, virtual_size) in [
        (0u32, 64u64 * 1024 * 1024),
        (8, 64 * 1024 * 1024),
        (22, 64 * 1024 * 1024),
        (16, 0),
        (16, u64::MAX),
    ] {
        private_overlay(&overlay);
        let mut header = fs::read(&overlay).unwrap();
        header[20..24].copy_from_slice(&cluster_bits.to_be_bytes());
        header[24..32].copy_from_slice(&virtual_size.to_be_bytes());
        fs::write(&overlay, header).unwrap();
        let error = build_qemu_plan(&request).unwrap_err();
        assert!(
            error.to_string().contains("qcow2"),
            "{cluster_bits}/{virtual_size}: {error}"
        );
    }

    // Both supported cluster-size boundaries remain valid at header-only
    // preflight; this does not certify a complete or bootable qcow2 image.
    for cluster_bits in [9u32, 16, 21] {
        private_overlay(&overlay);
        let mut header = fs::read(&overlay).unwrap();
        header[20..24].copy_from_slice(&cluster_bits.to_be_bytes());
        fs::write(&overlay, header).unwrap();
        assert!(build_qemu_plan(&request).is_ok(), "{cluster_bits}");
    }
}

#[test]
fn qemu_plan_rejects_unbound_qcow2_backing_and_external_data_files() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    // Version 2 and 3 can both reference a host backing file through the
    // fixed header. A nonzero size without an offset must fail closed too.
    for (version, backing_offset, backing_size, external_data) in [
        (2u32, 104u64, 12u32, false),
        (2, 0, 12, false),
        (3, 104, 12, false),
        (3, 0, 0, true),
    ] {
        private_overlay(&overlay);
        let mut header = fs::read(&overlay).unwrap();
        header[4..8].copy_from_slice(&version.to_be_bytes());
        header[8..16].copy_from_slice(&backing_offset.to_be_bytes());
        header[16..20].copy_from_slice(&backing_size.to_be_bytes());
        if external_data {
            header[72..80].copy_from_slice(&(1u64 << 2).to_be_bytes());
        }
        fs::write(&overlay, header).unwrap();
        let error = build_qemu_plan(&request).unwrap_err();
        assert!(
            error.to_string().contains("unbound external"),
            "version {version}: {error}"
        );
    }

    // An ordinary self-contained v2 header remains admissible to preflight.
    private_overlay(&overlay);
    let mut header = fs::read(&overlay).unwrap();
    header[4..8].copy_from_slice(&2u32.to_be_bytes());
    fs::write(&overlay, header).unwrap();
    assert!(build_qemu_plan(&request).is_ok());
}

#[test]
fn qemu_plan_rejects_unsupported_qcow2_crypto_and_incompatible_features() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    // An encrypted overlay requires unbound key authority. Incompatible
    // features can require QEMU behavior outside this minimal W8 contract.
    for (version, crypt_method, incompatible_features) in [
        (2u32, 1u32, 0u64),
        (3, 2, 0),
        (3, 0, 1u64 << 1),  // corrupt image
        (3, 0, 1u64 << 63), // unknown incompatible feature
    ] {
        private_overlay(&overlay);
        let mut header = fs::read(&overlay).unwrap();
        header[4..8].copy_from_slice(&version.to_be_bytes());
        header[32..36].copy_from_slice(&crypt_method.to_be_bytes());
        header[72..80].copy_from_slice(&incompatible_features.to_be_bytes());
        fs::write(&overlay, header).unwrap();
        let error = build_qemu_plan(&request).unwrap_err();
        assert!(error.to_string().contains("qcow2"), "{error}");
    }
}

#[test]
fn qemu_plan_rejects_unadmitted_qcow2_v3_compatible_and_autoclear_features() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    // The W8 header-only contract has no admission for lazy refcounts,
    // persistent bitmaps, or unknown compatible/autoclear metadata. Each
    // field must fail closed independently, including unknown high bits.
    for (compatible, autoclear) in [
        (1u64, 0u64),
        (0, 1),
        (1u64 << 63, 0),
        (0, 1u64 << 63),
        (1, 1),
    ] {
        private_overlay(&overlay);
        let mut header = fs::read(&overlay).unwrap();
        header[80..88].copy_from_slice(&compatible.to_be_bytes());
        header[88..96].copy_from_slice(&autoclear.to_be_bytes());
        fs::write(&overlay, header).unwrap();
        let error = build_qemu_plan(&request).unwrap_err();
        assert!(
            error.to_string().contains("qcow2"),
            "compatible {compatible:#x}, autoclear {autoclear:#x}: {error}"
        );
    }

    // A zero-feature header remains admissible to this bounded preflight;
    // this is not a bootable guest or physical Windows qualification.
    private_overlay(&overlay);
    assert!(build_qemu_plan(&request).is_ok());
}

#[test]
fn qemu_plan_rejects_empty_overlay_image() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    fs::write(&overlay, b"").unwrap();

    let error = build_qemu_plan(&VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay,
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    })
    .unwrap_err();
    assert!(error.to_string().contains("empty"));
}

#[cfg(unix)]
#[test]
fn qemu_plan_rejects_shared_writable_runtime_state() {
    use std::os::unix::fs::PermissionsExt;

    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);

    for (path, unsafe_mode) in [
        (&overlay, 0o664),
        (&overlay, 0o666),
        (&runtime, 0o775),
        (&runtime, 0o777),
    ] {
        let original = fs::metadata(path).unwrap().permissions();
        let mut permissions = original.clone();
        permissions.set_mode(unsafe_mode);
        fs::set_permissions(path, permissions).unwrap();

        let error = build_qemu_plan(&VmPlanRequest {
            qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
            overlay_image: overlay.clone(),
            runtime_dir: runtime.clone(),
            memory_mib: 2048,
            vcpus: 2,
            usb_nodes: vec![],
        })
        .unwrap_err();
        assert!(error.to_string().contains("writable"), "{error}");

        fs::set_permissions(path, original).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn qemu_plan_rejects_shared_writable_runtime_ancestor() {
    use std::os::unix::fs::PermissionsExt;

    let dir = private_tempdir();
    let shared = dir.path().join("shared-parent");
    fs::create_dir(&shared).unwrap();
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o777)).unwrap();
    let runtime = shared.join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);

    let error = build_qemu_plan(&VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay,
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    })
    .unwrap_err();
    assert!(error.to_string().contains("directory"), "{error}");
}

#[cfg(unix)]
#[test]
fn qemu_plan_rejects_fifo_executable_without_waiting_for_writer() {
    use std::ffi::CString;

    let dir = private_tempdir();
    let qemu = dir.path().join("qemu-fifo");
    let c_path = CString::new(qemu.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);

    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);

    // No FIFO writer exists. Executable admission must fail immediately,
    // rather than block inside an ordinary read-only open.
    let error = build_qemu_plan(&VmPlanRequest {
        qemu_binary: qemu,
        overlay_image: overlay,
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    })
    .unwrap_err();
    assert!(error.to_string().contains("not executable"), "{error}");
}

#[cfg(unix)]
#[test]
fn qemu_plan_rejects_shared_writable_executable() {
    use std::os::unix::fs::PermissionsExt;

    let dir = private_tempdir();
    let qemu = dir.path().join("qemu-test-binary");
    fs::write(&qemu, b"#!/bin/sh\n").unwrap();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);

    for unsafe_mode in [0o775, 0o777] {
        fs::set_permissions(&qemu, fs::Permissions::from_mode(unsafe_mode)).unwrap();
        let error = build_qemu_plan(&VmPlanRequest {
            qemu_binary: qemu.clone(),
            overlay_image: overlay.clone(),
            runtime_dir: runtime.clone(),
            memory_mib: 2048,
            vcpus: 2,
            usb_nodes: vec![],
        })
        .unwrap_err();
        assert!(error.to_string().contains("writable"), "{error}");
    }
}

#[cfg(unix)]
#[test]
fn qemu_plan_rejects_shared_writable_executable_directory() {
    use std::os::unix::fs::PermissionsExt;

    let dir = private_tempdir();
    let shared = dir.path().join("shared");
    fs::create_dir(&shared).unwrap();
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o777)).unwrap();
    let qemu = shared.join("qemu-test-binary");
    fs::write(&qemu, b"#!/bin/sh\n").unwrap();
    fs::set_permissions(&qemu, fs::Permissions::from_mode(0o700)).unwrap();

    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);
    let error = build_qemu_plan(&VmPlanRequest {
        qemu_binary: qemu,
        overlay_image: overlay,
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    })
    .unwrap_err();
    assert!(error.to_string().contains("directory"), "{error}");
}

#[cfg(unix)]
#[test]
fn qemu_plan_rejects_hardlinked_executable_alias() {
    use std::os::unix::fs::PermissionsExt;

    let dir = private_tempdir();
    let qemu = dir.path().join("qemu-test-binary");
    let alias = dir.path().join("unrelated-host-executable");
    fs::write(&qemu, b"#!/bin/sh\n").unwrap();
    fs::set_permissions(&qemu, fs::Permissions::from_mode(0o700)).unwrap();
    fs::hard_link(&qemu, &alias).unwrap();

    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);

    let error = build_qemu_plan(&VmPlanRequest {
        qemu_binary: qemu,
        overlay_image: overlay,
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    })
    .unwrap_err();
    assert!(error.to_string().contains("hardlink"), "{error}");
}

#[test]
fn qemu_plan_is_kvm_only_network_closed_and_agent_scoped() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);

    let plan = build_qemu_plan(&VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime.clone(),
        memory_mib: 4096,
        vcpus: 2,
        usb_nodes: vec![],
    })
    .unwrap();

    assert_eq!(
        plan.program,
        std::path::PathBuf::from("/usr/bin/qemu-system-x86_64")
    );
    let joined = plan.args.join(" ");
    assert!(joined.contains("-accel kvm"));
    assert!(joined.contains("-net none"));
    assert!(joined.contains("-nodefaults"));
    assert!(joined.contains("-no-user-config"));
    assert!(joined.contains("name=org.thetechguy.prime.agent"));
    assert!(joined.contains(overlay.to_str().unwrap()));
    assert!(!joined.contains("-nic user"));
    assert!(!joined.contains("hostfwd"));
}

#[cfg(unix)]
#[test]
fn qemu_plan_pins_executable_before_alias_retarget() {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let dir = private_tempdir();
    let trusted = dir.path().join("trusted-qemu");
    let replacement = dir.path().join("replacement-qemu");
    for binary in [&trusted, &replacement] {
        fs::write(binary, b"#!/bin/sh\n").unwrap();
        let mut permissions = fs::metadata(binary).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(binary, permissions).unwrap();
    }
    let alias = dir.path().join("qemu-alias");
    symlink(&trusted, &alias).unwrap();

    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);

    let plan = build_qemu_plan(&VmPlanRequest {
        qemu_binary: alias.clone(),
        overlay_image: overlay,
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    })
    .unwrap();

    fs::remove_file(&alias).unwrap();
    symlink(&replacement, &alias).unwrap();
    assert_eq!(plan.program, fs::canonicalize(&trusted).unwrap());
    assert_ne!(plan.program, fs::canonicalize(&alias).unwrap());
}

#[test]
fn qemu_plan_rejects_option_injection_through_runtime_paths() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);

    for unsafe_overlay in [
        runtime.join("overlay,readonly=off.qcow2"),
        runtime.join("overlay\nspoof.qcow2"),
    ] {
        fs::write(&unsafe_overlay, b"overlay").unwrap();
        assert!(build_qemu_plan(&VmPlanRequest {
            qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
            overlay_image: unsafe_overlay,
            runtime_dir: runtime.clone(),
            memory_mib: 2048,
            vcpus: 2,
            usb_nodes: vec![],
        })
        .is_err());
    }

    let unsafe_runtime = dir.path().join("runtime,server=on");
    fs::create_dir(&unsafe_runtime).unwrap();
    assert!(build_qemu_plan(&VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay,
        runtime_dir: unsafe_runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    })
    .is_err());
}

#[test]
fn qemu_plan_rejects_overlong_guest_agent_socket() {
    let dir = private_tempdir();
    let runtime = dir.path().join("s".repeat(100));
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);
    let error = build_qemu_plan(&VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay,
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    })
    .unwrap_err();
    assert!(error.to_string().contains("AF_UNIX"));
}

#[cfg(unix)]
#[test]
fn qemu_plan_rejects_preexisting_agent_socket_nodes() {
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;

    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);
    let agent = runtime.join("agent.sock");
    let outside = dir.path().join("unrelated-host-file");
    fs::write(&outside, b"must-remain-intact").unwrap();

    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay,
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };

    symlink(&outside, &agent).unwrap();
    assert!(build_qemu_plan(&request).is_err());
    fs::remove_file(&agent).unwrap();

    fs::write(&agent, b"occupied").unwrap();
    assert!(build_qemu_plan(&request).is_err());
    fs::remove_file(&agent).unwrap();

    let listener = UnixListener::bind(&agent).unwrap();
    assert!(build_qemu_plan(&request).is_err());
    drop(listener);
    fs::remove_file(&agent).unwrap();

    assert_eq!(fs::read(outside).unwrap(), b"must-remain-intact");
    assert!(build_qemu_plan(&request).is_ok());
}

#[test]
fn qemu_plan_rejects_cross_session_overlay_and_traversal() {
    let dir = private_tempdir();
    let runtime = dir.path().join("session-a");
    let other_runtime = dir.path().join("session-b");
    private_runtime_dir(&runtime);
    fs::create_dir(&other_runtime).unwrap();
    let other_overlay = other_runtime.join("overlay.qcow2");
    fs::write(&other_overlay, b"other-session").unwrap();

    for unsafe_overlay in [
        other_overlay.clone(),
        runtime.join("..").join("session-b").join("overlay.qcow2"),
    ] {
        assert!(build_qemu_plan(&VmPlanRequest {
            qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
            overlay_image: unsafe_overlay,
            runtime_dir: runtime.clone(),
            memory_mib: 2048,
            vcpus: 2,
            usb_nodes: vec![],
        })
        .is_err());
    }
}

#[cfg(unix)]
#[test]
fn qemu_plan_rejects_hardlinked_overlay_aliasing_host_file() {
    let dir = private_tempdir();
    let external = dir.path().join("unrelated-host-file");
    fs::write(&external, b"host-data").unwrap();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    fs::hard_link(&external, &overlay).unwrap();

    let error = build_qemu_plan(&VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay,
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    })
    .unwrap_err();
    assert!(error.to_string().contains("hardlink"));
    assert_eq!(fs::read(&external).unwrap(), b"host-data");
}

#[test]
fn qemu_plan_accepts_only_strict_w7_usb_nodes() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);

    let allowed = build_qemu_plan(&VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime.clone(),
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec!["/dev/bus/usb/001/018".into()],
    })
    .unwrap();
    assert!(allowed
        .args
        .join(" ")
        .contains("usb-host,hostbus=1,hostaddr=18"));

    let denied = build_qemu_plan(&VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay,
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec!["/dev/sda".into()],
    })
    .unwrap_err();
    assert!(denied.to_string().contains("USB"));
}

#[test]
fn qemu_plan_rejects_out_of_range_usb_device_addresses() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);

    // USB device addresses are seven-bit values (1..=127). A syntactically
    // valid three-digit path is not proof that QEMU can address that device.
    for device in [128, 255, 999] {
        let error = build_qemu_plan(&VmPlanRequest {
            qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
            overlay_image: overlay.clone(),
            runtime_dir: runtime.clone(),
            memory_mib: 2048,
            vcpus: 2,
            usb_nodes: vec![format!("/dev/bus/usb/001/{device:03}").into()],
        })
        .unwrap_err();
        assert!(error.to_string().contains("USB"), "{error}");
    }
}

#[test]
fn qemu_plan_rejects_oversized_usb_allowlist_before_qemu_launch() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);

    let request = |count: usize, duplicate: bool| VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime.clone(),
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: (1..=count)
            .map(|index| {
                let address = if duplicate { 1 } else { index };
                format!("/dev/bus/usb/001/{address:03}").into()
            })
            .collect(),
    };

    // A bounded W7 allowlist remains usable. Excess entries, including
    // duplicates, must not produce an unbounded QEMU device plan.
    assert!(build_qemu_plan(&request(15, false)).is_ok());
    for duplicate in [false, true] {
        let error = build_qemu_plan(&request(16, duplicate)).unwrap_err();
        assert!(error.to_string().contains("USB"), "{error}");
    }
}

fn vm_profile(backend: ExecutionBackend) -> ApplicationProfile {
    ApplicationProfile {
        schema: APPLICATION_PROFILE_SCHEMA.to_owned(),
        application_id: Uuid::nil(),
        profile_revision: 1,
        profile_digest: "sha256:test".to_owned(),
        display_name: "W8 VM fixture".to_owned(),
        artifact: ApplicationArtifact {
            identity: "sha256:fixture".to_owned(),
            format: ArtifactFormat::Pe32Plus,
            runtime_family: RuntimeFamily::Windows,
            workload_arch: Some("x86_64".to_owned()),
        },
        execution_backend: backend,
        dependencies: vec![],
        workload_policy: PolicyReference {
            policy_id: Uuid::nil(),
            policy_revision: 1,
            policy_digest: "sha256:policy".to_owned(),
        },
        permissions: vec![],
        compatibility: CompatibilityRecord {
            state: MechanicalCompatibilityState::Recognized,
            evidence_refs: vec![],
        },
        revoked: false,
        revocation_reason: None,
        created_at: "2026-09-13T00:00:00Z".to_owned(),
    }
}

#[test]
fn vm_fallback_is_explicit_backend_authority_not_personality_retry() {
    validate_vm_profile(&vm_profile(ExecutionBackend::Vm)).unwrap();
    let error = validate_vm_profile(&vm_profile(ExecutionBackend::Personality)).unwrap_err();
    assert!(error.to_string().contains("VM"));
}

#[test]
fn guest_agent_handshake_is_versioned_arch_bound_and_digest_bound() {
    use primed::windows_vm::{
        validate_guest_agent_hello, GuestAgentHello, WINDOWS_VM_AGENT_SCHEMA,
    };

    let hello = GuestAgentHello {
        schema: WINDOWS_VM_AGENT_SCHEMA.to_owned(),
        protocol_revision: 1,
        guest_id: "windows-fallback-test".to_owned(),
        guest_revision: 7,
        guest_sha256: "a".repeat(64),
        workload_arch: "x86_64".to_owned(),
    };
    validate_guest_agent_hello(
        &hello,
        "windows-fallback-test",
        7,
        &"a".repeat(64),
        "x86_64",
    )
    .unwrap();

    let uppercase_digest = GuestAgentHello {
        guest_sha256: "A".repeat(64),
        ..hello.clone()
    };
    assert!(validate_guest_agent_hello(
        &uppercase_digest,
        "windows-fallback-test",
        7,
        &"a".repeat(64),
        "x86_64",
    )
    .is_err());
    assert!(validate_guest_agent_hello(
        &hello,
        "windows-fallback-test",
        7,
        &"A".repeat(64),
        "x86_64",
    )
    .is_err());

    let wrong_arch = GuestAgentHello {
        workload_arch: "x86".to_owned(),
        ..hello.clone()
    };
    assert!(validate_guest_agent_hello(
        &wrong_arch,
        "windows-fallback-test",
        7,
        &"a".repeat(64),
        "x86_64"
    )
    .is_err());

    let wrong_digest = GuestAgentHello {
        guest_sha256: "b".repeat(64),
        ..hello
    };
    assert!(validate_guest_agent_hello(
        &wrong_digest,
        "windows-fallback-test",
        7,
        &"a".repeat(64),
        "x86_64"
    )
    .is_err());
}

#[test]
fn guest_agent_handshake_rejects_undeclared_protocol_fields() {
    use primed::windows_vm::{GuestAgentHello, WINDOWS_VM_AGENT_SCHEMA};

    let mut hello = serde_json::json!({
        "schema": WINDOWS_VM_AGENT_SCHEMA,
        "protocol_revision": 1,
        "guest_id": "windows-fallback-test",
        "guest_revision": 7,
        "guest_sha256": "a".repeat(64),
        "workload_arch": "x86_64"
    });
    assert!(serde_json::from_value::<GuestAgentHello>(hello.clone()).is_ok());
    hello["host_override"] = serde_json::json!({"allow_host_network": true});
    assert!(
        serde_json::from_value::<GuestAgentHello>(hello).is_err(),
        "unknown guest-agent protocol fields must fail closed"
    );
}

#[test]
fn vm_session_paths_are_application_scoped_and_reject_escape() {
    use primed::windows_vm::vm_session_paths;

    let root = private_tempdir();
    let a = vm_session_paths(root.path(), "app-001", "session-a").unwrap();
    let b = vm_session_paths(root.path(), "app-002", "session-b").unwrap();
    assert_ne!(a.runtime_dir, b.runtime_dir);
    assert_ne!(a.overlay_image, b.overlay_image);
    assert!(a.agent_socket.starts_with(&a.runtime_dir));
    assert!(b.agent_socket.starts_with(&b.runtime_dir));

    assert!(vm_session_paths(root.path(), "../escape", "session-a").is_err());
    assert!(vm_session_paths(root.path(), "", "session-a").is_err());
    assert!(vm_session_paths(root.path(), "app-001", "").is_err());
    assert!(vm_session_paths(root.path(), "app-💥", "session-a").is_err());
    assert!(vm_session_paths(root.path(), "app-001", "session\nspoof").is_err());
    assert!(vm_session_paths(root.path(), " app-001", "session-a").is_err());
    assert!(vm_session_paths(root.path(), "app-001", "session-a ").is_err());
    assert!(vm_session_paths(root.path(), &"a".repeat(129), "session-a").is_err());
    assert!(vm_session_paths(root.path(), &"a".repeat(100), "session-a").is_err());
    assert!(vm_session_paths(root.path(), "app-001", &"s".repeat(129)).is_err());
    assert!(vm_session_paths(root.path(), "app-001", "/absolute").is_err());
    assert!(vm_session_paths(std::path::Path::new("/"), "app-001", "session-a").is_err());
    assert!(
        vm_session_paths(std::path::Path::new("/tmp/./prime"), "app-001", "session-a").is_err()
    );
    assert!(vm_session_paths(std::path::Path::new("/tmp/prime/"), "app-001", "session-a").is_err());
    assert!(vm_session_paths(std::path::Path::new("/tmp//prime"), "app-001", "session-a").is_err());
}

#[cfg(unix)]
#[test]
fn vm_session_paths_rejects_symlinked_root_alias() {
    use primed::windows_vm::vm_session_paths;
    use std::os::unix::fs::symlink;

    let dir = private_tempdir();
    let real_root = dir.path().join("real");
    private_runtime_dir(&real_root);
    let alias = dir.path().join("alias");
    symlink(&real_root, &alias).unwrap();

    assert!(vm_session_paths(&alias, "app-001", "session-a").is_err());
    assert!(vm_session_paths(&real_root, "app-001", "session-a").is_ok());
}

#[cfg(unix)]
#[test]
fn vm_session_paths_rejects_existing_symlinked_application_and_session() {
    use primed::windows_vm::vm_session_paths;
    use std::os::unix::fs::symlink;

    let dir = private_tempdir();
    let root = dir.path().join("runtime");
    let outside = dir.path().join("outside");
    private_runtime_dir(&root);
    fs::create_dir(&outside).unwrap();

    symlink(&outside, root.join("app-001")).unwrap();
    assert!(vm_session_paths(&root, "app-001", "session-a").is_err());
    fs::remove_file(root.join("app-001")).unwrap();

    let app = root.join("app-001");
    private_runtime_dir(&app);
    symlink(&outside, app.join("session-a")).unwrap();
    assert!(vm_session_paths(&root, "app-001", "session-a").is_err());
    fs::remove_file(app.join("session-a")).unwrap();

    fs::write(app.join("session-a"), b"not a session directory").unwrap();
    assert!(vm_session_paths(&root, "app-001", "session-a").is_err());
    fs::remove_file(app.join("session-a")).unwrap();

    assert!(vm_session_paths(&root, "app-001", "session-a").is_ok());
}

#[test]
fn vm_session_paths_rejects_regular_file_as_root() {
    use primed::windows_vm::vm_session_paths;

    let dir = private_tempdir();
    let file_root = dir.path().join("not-a-directory");
    fs::write(&file_root, b"not a VM session directory").unwrap();

    assert!(vm_session_paths(&file_root, "app-001", "session-a").is_err());
    assert_eq!(fs::read(&file_root).unwrap(), b"not a VM session directory");
}

#[cfg(unix)]
#[test]
fn vm_session_paths_rejects_shared_writable_root_and_ancestor() {
    use primed::windows_vm::vm_session_paths;
    use std::os::unix::fs::PermissionsExt;

    let dir = private_tempdir();
    let root = dir.path().join("runtime");
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(vm_session_paths(&root, "app-001", "session-a").is_err());

    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let parent = dir.path().join("shared");
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o777)).unwrap();
    let nested = parent.join("runtime");
    fs::create_dir(&nested).unwrap();
    fs::set_permissions(&nested, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(vm_session_paths(&nested, "app-001", "session-a").is_err());

    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(vm_session_paths(&nested, "app-001", "session-a").is_ok());
}

#[cfg(unix)]
#[test]
fn vm_session_paths_rejects_shared_writable_application_and_session_directories() {
    use primed::windows_vm::vm_session_paths;
    use std::os::unix::fs::PermissionsExt;

    let dir = private_tempdir();
    let root = dir.path().join("runtime");
    private_runtime_dir(&root);
    let app = root.join("app-001");
    private_runtime_dir(&app);
    let session = app.join("session-a");
    private_runtime_dir(&session);

    assert!(vm_session_paths(&root, "app-001", "session-a").is_ok());

    fs::set_permissions(&app, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(vm_session_paths(&root, "app-001", "session-a").is_err());
    fs::set_permissions(&app, fs::Permissions::from_mode(0o700)).unwrap();

    fs::set_permissions(&session, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(vm_session_paths(&root, "app-001", "session-a").is_err());
    fs::set_permissions(&session, fs::Permissions::from_mode(0o700)).unwrap();

    assert!(vm_session_paths(&root, "app-001", "session-a").is_ok());
}

#[test]
fn runtime_proof_plan_is_exact_guest_bound_and_requires_all_w8_cases() {
    use primed::windows_vm::{
        create_runtime_proof_observation_from_evidence, evaluate_runtime_proof,
        prepare_runtime_proof_plan, VmRuntimeAdversarialCase, VmRuntimeProofState,
    };

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();

    let root = dir.path().join("runtime");
    private_runtime_dir(&root);
    let plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();

    assert_eq!(plan.proof_id.len(), 64);
    assert_eq!(plan.required_cases.len(), 4);
    assert!(plan
        .required_cases
        .contains(&VmRuntimeAdversarialCase::ConcurrentSessionIsolation));
    assert!(plan
        .required_cases
        .contains(&VmRuntimeAdversarialCase::ForcedProcessTermination));
    assert!(plan
        .required_cases
        .contains(&VmRuntimeAdversarialCase::CorruptGuestAuthority));
    assert!(plan
        .required_cases
        .contains(&VmRuntimeAdversarialCase::DeniedUsbNode));
    assert_eq!(
        evaluate_runtime_proof(&plan, &[]),
        VmRuntimeProofState::Pending
    );

    let observations: Vec<_> = plan
        .required_cases
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, case)| {
            create_runtime_proof_observation_from_evidence(
                &plan,
                case,
                true,
                true,
                format!("runtime-evidence-{index}").as_bytes(),
            )
            .unwrap()
        })
        .collect();

    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pass
    );
}

#[test]
fn runtime_proof_cannot_pass_on_wrong_binding_duplicate_case_or_residual_state() {
    use primed::windows_vm::{
        create_runtime_proof_observation_from_evidence, evaluate_runtime_proof,
        prepare_runtime_proof_plan, VmRuntimeProofState,
    };

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    private_runtime_dir(&root);
    let plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();

    let make = || {
        plan.required_cases
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, case)| {
                create_runtime_proof_observation_from_evidence(
                    &plan,
                    case,
                    true,
                    true,
                    format!("runtime-evidence-{index}").as_bytes(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>()
    };

    let mut observations = make();
    observations[0].proof_id = "b".repeat(64);
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );

    let mut observations = make();
    observations[0].session_id = "session-b".to_owned();
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );

    let mut observations = make();
    observations[1].zero_residual_state = false;
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );

    let mut observations = make();
    observations[3].case = observations[2].case.clone();
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );

    let mut observations = make();
    observations[1] = create_runtime_proof_observation_from_evidence(
        &plan,
        observations[1].case.clone(),
        true,
        true,
        b"runtime-evidence-0",
    )
    .unwrap();
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );
}

#[test]
fn runtime_observation_identity_rejects_mutation_and_noncanonical_evidence_digest() {
    use primed::windows_vm::{
        create_runtime_proof_observation_from_evidence, evaluate_runtime_proof,
        prepare_runtime_proof_plan, VmRuntimeProofState,
    };

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    private_runtime_dir(&root);
    let plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();

    assert!(create_runtime_proof_observation_from_evidence(
        &plan,
        plan.required_cases[0].clone(),
        true,
        true,
        b"",
    )
    .is_err());

    let mut observations: Vec<_> = plan
        .required_cases
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, case)| {
            create_runtime_proof_observation_from_evidence(
                &plan,
                case,
                true,
                true,
                format!("runtime-evidence-{index}").as_bytes(),
            )
            .unwrap()
        })
        .collect();

    // A substituted canonical evidence digest must invalidate the observation
    // identity, even if the other three case digests remain unique.
    let mut substituted = observations.clone();
    substituted[0].evidence_sha256 = digest(b"unrelated-substituted-evidence");
    assert_eq!(
        evaluate_runtime_proof(&plan, &substituted),
        VmRuntimeProofState::Pending
    );

    observations[1].evidence_schema = "prime.windows-vm-runtime-evidence.v0".to_owned();
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );
    observations[1].evidence_schema =
        primed::windows_vm::WINDOWS_VM_RUNTIME_EVIDENCE_SCHEMA.to_owned();

    observations[2].passed = false;
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );
}

#[test]
fn runtime_proof_plan_rejects_relative_runtime_root() {
    use primed::windows_vm::prepare_runtime_proof_plan;

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();

    assert!(prepare_runtime_proof_plan(
        &guest,
        std::path::Path::new("relative-runtime"),
        "app-001",
        "session-a"
    )
    .is_err());
}

#[test]
fn runtime_proof_plan_rejects_parent_traversal_runtime_root() {
    use primed::windows_vm::prepare_runtime_proof_plan;

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime").join("..").join("escape");

    assert!(prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").is_err());
}

#[test]
fn runtime_proof_plan_identity_changes_with_guest_application_or_session_scope() {
    use primed::windows_vm::prepare_runtime_proof_plan;

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    private_runtime_dir(&root);

    let a = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();
    let b = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-b").unwrap();
    let c = prepare_runtime_proof_plan(&guest, &root, "app-002", "session-a").unwrap();
    assert_ne!(a.proof_id, b.proof_id);
    assert_ne!(a.proof_id, c.proof_id);

    let alternate_root = dir.path().join("alternate-runtime");
    private_runtime_dir(&alternate_root);
    let path_scoped =
        prepare_runtime_proof_plan(&guest, &alternate_root, "app-001", "session-a").unwrap();
    assert_ne!(a.proof_id, path_scoped.proof_id);

    let other_image = dir.path().join("other-windows.qcow2");
    let other_bytes = b"prime-w8-runtime-proof-other-guest";
    private_guest_image(&other_image, other_bytes);
    let other = validate_guest_definition(&definition(&other_image, other_bytes)).unwrap();
    let d = prepare_runtime_proof_plan(&other, &root, "app-001", "session-a").unwrap();
    assert_ne!(a.proof_id, d.proof_id);

    let mut forged = guest.clone();
    forged.base_sha256 = "b".repeat(64);
    assert!(prepare_runtime_proof_plan(&forged, &root, "app-001", "session-a").is_err());
}

#[test]
fn runtime_observation_hashes_actual_nonempty_evidence_bytes() {
    use primed::windows_vm::{
        create_runtime_proof_observation_from_evidence, prepare_runtime_proof_plan,
    };

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    private_runtime_dir(&root);
    let plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();
    let evidence = b"forced-termination:zero-qemu-processes";

    let observation = create_runtime_proof_observation_from_evidence(
        &plan,
        plan.required_cases[1].clone(),
        true,
        true,
        evidence,
    )
    .unwrap();
    assert_eq!(observation.evidence_sha256, digest(evidence));
    assert!(create_runtime_proof_observation_from_evidence(
        &plan,
        plan.required_cases[1].clone(),
        true,
        true,
        b"",
    )
    .is_err());
}

#[test]
fn runtime_evidence_envelope_rejects_cross_case_and_cross_session_replay() {
    use primed::windows_vm::{
        create_runtime_proof_observation_from_envelope, prepare_runtime_proof_plan,
        VmRuntimeEvidenceEnvelope, WINDOWS_VM_RUNTIME_EVIDENCE_SCHEMA,
    };

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    private_runtime_dir(&root);
    let plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();

    let envelope = VmRuntimeEvidenceEnvelope {
        schema: WINDOWS_VM_RUNTIME_EVIDENCE_SCHEMA.to_owned(),
        proof_id: plan.proof_id.clone(),
        case: plan.required_cases[0].clone(),
        guest_id: plan.guest_id.clone(),
        guest_revision: plan.guest_revision,
        guest_sha256: plan.guest_sha256.clone(),
        application_id: plan.application_id.clone(),
        session_id: plan.session_id.clone(),
        evidence: b"concurrent-session-isolation:clean".to_vec(),
    };
    create_runtime_proof_observation_from_envelope(&plan, &envelope, true, true).unwrap();
    let mut unexpected = serde_json::to_value(&envelope).unwrap();
    unexpected["authority"] = serde_json::json!("guest-attested");
    assert!(serde_json::from_value::<VmRuntimeEvidenceEnvelope>(unexpected).is_err());

    let mut wrong_session = envelope.clone();
    wrong_session.session_id = "session-b".to_owned();
    assert!(
        create_runtime_proof_observation_from_envelope(&plan, &wrong_session, true, true).is_err()
    );

    let mut wrong_case = envelope;
    wrong_case.case = plan.required_cases[1].clone();
    wrong_case.proof_id = "b".repeat(64);
    assert!(
        create_runtime_proof_observation_from_envelope(&plan, &wrong_case, true, true).is_err()
    );
}

#[test]
fn runtime_proof_plan_rejects_noncanonical_forged_guest_digest() {
    use primed::windows_vm::prepare_runtime_proof_plan;

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    private_guest_image(&image, bytes);
    let mut guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    fs::create_dir(&root).unwrap();

    guest.base_sha256 = "A".repeat(64);
    assert!(prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").is_err());

    guest.base_sha256 = "g".repeat(64);
    assert!(prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").is_err());
}
#[cfg(unix)]
#[test]
fn guest_definition_rejects_symlinked_image_parent() {
    use std::os::unix::fs::symlink;
    let dir = private_tempdir();
    let image = dir.path().join("guest.qcow2");
    let bytes = b"w8-parent";
    private_guest_image(&image, bytes);
    let alias = dir.path().join("alias");
    symlink(dir.path(), &alias).unwrap();
    assert!(validate_guest_definition(&definition(&alias.join("guest.qcow2"), bytes)).is_err());
    assert!(validate_guest_definition(&definition(&image, bytes)).is_ok());
}

#[test]
fn runtime_proof_rejects_rehashed_plan_with_missing_mandatory_case() {
    use primed::windows_vm::{
        create_runtime_proof_observation_from_evidence, evaluate_runtime_proof,
        prepare_runtime_proof_plan, VmRuntimeProofState, WINDOWS_VM_RUNTIME_PROOF_SCHEMA,
    };

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-forged-incomplete-proof-plan";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    private_runtime_dir(&root);
    let mut plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();

    // A consumer can serialize the public plan fields and recompute its SHA-256.
    // A self-consistent digest must never authorize omitting W8's mandatory cases.
    plan.required_cases.truncate(1);
    let body = serde_json::json!({
        "schema": WINDOWS_VM_RUNTIME_PROOF_SCHEMA,
        "guest_id": plan.guest_id,
        "guest_revision": plan.guest_revision,
        "guest_sha256": plan.guest_sha256,
        "application_id": plan.application_id,
        "session_id": plan.session_id,
        "required_cases": plan.required_cases,
        "runtime_dir": plan.paths.runtime_dir,
        "overlay_image": plan.paths.overlay_image,
        "agent_socket": plan.paths.agent_socket,
    });
    plan.proof_id = digest(&serde_json::to_vec(&body).unwrap());
    let observation = create_runtime_proof_observation_from_evidence(
        &plan,
        plan.required_cases[0].clone(),
        true,
        true,
        b"one-case-only",
    )
    .unwrap();
    assert_eq!(
        evaluate_runtime_proof(&plan, &[observation]),
        VmRuntimeProofState::Pending
    );
}

#[test]
fn runtime_proof_rejects_rehashed_overlay_path() {
    use primed::windows_vm::{
        create_runtime_proof_observation_from_evidence, evaluate_runtime_proof,
        prepare_runtime_proof_plan, VmRuntimeProofState, WINDOWS_VM_RUNTIME_PROOF_SCHEMA,
    };

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-forged-overlay-proof-plan";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    private_runtime_dir(&root);
    let mut plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();

    // A public plan can be mutated and rehashed; the overlay must still belong
    // to the canonical application/session runtime directory.
    plan.paths.overlay_image = root.join("unscoped-overlay.qcow2");
    let body = serde_json::json!({
        "schema": WINDOWS_VM_RUNTIME_PROOF_SCHEMA,
        "guest_id": plan.guest_id,
        "guest_revision": plan.guest_revision,
        "guest_sha256": plan.guest_sha256,
        "application_id": plan.application_id,
        "session_id": plan.session_id,
        "required_cases": plan.required_cases,
        "runtime_dir": plan.paths.runtime_dir,
        "overlay_image": plan.paths.overlay_image,
        "agent_socket": plan.paths.agent_socket,
    });
    plan.proof_id = digest(&serde_json::to_vec(&body).unwrap());
    let observations: Vec<_> = plan
        .required_cases
        .iter()
        .enumerate()
        .map(|(index, case)| {
            create_runtime_proof_observation_from_evidence(
                &plan,
                case.clone(),
                true,
                true,
                format!("distinct-case-{index}").as_bytes(),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );
}

#[test]
fn runtime_proof_plan_rejects_unvalidated_guest_identity_and_resources() {
    use primed::windows_vm::prepare_runtime_proof_plan;

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    fs::create_dir(&root).unwrap();

    let mut invalid = guest.clone();
    invalid.guest_id = "../other-guest".to_owned();
    assert!(prepare_runtime_proof_plan(&invalid, &root, "app-001", "session-a").is_err());

    let mut invalid = guest.clone();
    invalid.revision = 0;
    assert!(prepare_runtime_proof_plan(&invalid, &root, "app-001", "session-a").is_err());

    let mut invalid = guest.clone();
    invalid.memory_mib = 0;
    assert!(prepare_runtime_proof_plan(&invalid, &root, "app-001", "session-a").is_err());

    let mut invalid = guest;
    invalid.vcpus = 0;
    assert!(prepare_runtime_proof_plan(&invalid, &root, "app-001", "session-a").is_err());
}

#[test]
fn guest_agent_handshake_rejects_noncanonical_expected_identity() {
    use primed::windows_vm::{
        validate_guest_agent_hello, GuestAgentHello, WINDOWS_VM_AGENT_SCHEMA,
    };
    let mut hello = GuestAgentHello {
        schema: WINDOWS_VM_AGENT_SCHEMA.to_owned(),
        protocol_revision: 1,
        guest_id: "windows-fallback-test".to_owned(),
        guest_revision: 1,
        guest_sha256: "a".repeat(64),
        workload_arch: "x86_64".to_owned(),
    };
    for invalid_id in ["", " leading", "guest/alias"] {
        hello.guest_id = invalid_id.to_owned();
        assert!(
            validate_guest_agent_hello(&hello, invalid_id, 1, &"a".repeat(64), "x86_64").is_err()
        );
    }
    hello.guest_id = "windows-fallback-test".to_owned();
    hello.guest_revision = 0;
    assert!(validate_guest_agent_hello(
        &hello,
        "windows-fallback-test",
        0,
        &"a".repeat(64),
        "x86_64"
    )
    .is_err());
}

#[test]
fn runtime_proof_plan_rejects_guest_image_mutation_after_admission() {
    use primed::windows_vm::prepare_runtime_proof_plan;

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let original = b"prime-w8-runtime-proof-guest";
    private_guest_image(&image, original);
    let guest = validate_guest_definition(&definition(&image, original)).unwrap();

    // Runtime proof authority must bind the bytes still present after admission.
    fs::write(&image, b"modified-after-admission").unwrap();

    let root = dir.path().join("runtime");
    fs::create_dir(&root).unwrap();
    let error = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap_err();
    assert!(error.to_string().contains("digest"));
    assert!(!root.join("app-001").exists());
}

#[test]
fn runtime_proof_plan_rejects_preoccupied_overlay_image() {
    use primed::windows_vm::prepare_runtime_proof_plan;

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-overlay-preoccupation-proof";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    let runtime = root.join("app-001").join("session-a");
    fs::create_dir_all(&runtime).unwrap();
    let overlay = runtime.join("overlay.qcow2");
    fs::write(&overlay, b"stale-overlay").unwrap();

    assert!(prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").is_err());
    assert_eq!(fs::read(&overlay).unwrap(), b"stale-overlay");
}

#[cfg(unix)]
#[test]
fn runtime_proof_plan_rejects_preoccupied_agent_socket() {
    use primed::windows_vm::prepare_runtime_proof_plan;
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-agent-socket-proof";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    let runtime = root.join("app-001").join("session-a");
    private_runtime_dir(&root);
    private_runtime_dir(&root.join("app-001"));
    private_runtime_dir(&runtime);
    let socket = runtime.join("agent.sock");
    let outside = dir.path().join("unrelated-host-file");
    fs::write(&outside, b"must-remain-intact").unwrap();

    symlink(&outside, &socket).unwrap();
    assert!(prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").is_err());
    fs::remove_file(&socket).unwrap();

    fs::write(&socket, b"occupied").unwrap();
    assert!(prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").is_err());
    fs::remove_file(&socket).unwrap();

    let listener = UnixListener::bind(&socket).unwrap();
    assert!(prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").is_err());
    drop(listener);
    fs::remove_file(&socket).unwrap();

    assert_eq!(fs::read(outside).unwrap(), b"must-remain-intact");
    assert!(prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").is_ok());
}

#[cfg(unix)]
#[test]
fn runtime_proof_plan_rejects_preoccupied_overlay_aliases() {
    use primed::windows_vm::prepare_runtime_proof_plan;
    use std::os::unix::fs::symlink;

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-overlay-alias-proof";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    let runtime = root.join("app-001").join("session-a");
    private_runtime_dir(&root);
    private_runtime_dir(&root.join("app-001"));
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    let outside = dir.path().join("outside-host-file");
    fs::write(&outside, b"do-not-touch").unwrap();

    symlink(&outside, &overlay).unwrap();
    assert!(prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").is_err());
    assert_eq!(fs::read(&outside).unwrap(), b"do-not-touch");
    fs::remove_file(&overlay).unwrap();

    symlink(dir.path().join("missing-target"), &overlay).unwrap();
    assert!(prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").is_err());
    fs::remove_file(&overlay).unwrap();

    fs::create_dir(&overlay).unwrap();
    assert!(prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").is_err());
    fs::remove_dir(&overlay).unwrap();

    assert!(prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").is_ok());
    assert_eq!(fs::read(&outside).unwrap(), b"do-not-touch");
}
#[test]
fn vm_fallback_rejects_incompatible_compatibility_states() {
    for state in [
        MechanicalCompatibilityState::Unsupported,
        MechanicalCompatibilityState::RequiresRemoteProvider,
    ] {
        let mut profile = vm_profile(ExecutionBackend::Vm);
        profile.compatibility.state = state;
        assert!(
            validate_vm_profile(&profile).is_err(),
            "incompatible profile must not authorize a VM launch"
        );
    }
}

#[test]
fn vm_fallback_rejects_revoked_profiles() {
    let mut profile = vm_profile(ExecutionBackend::Vm);
    profile.revoked = true;
    assert!(validate_vm_profile(&profile).is_err());
}

#[cfg(unix)]
#[test]
fn guest_definition_rejects_shared_writable_base_image() {
    use std::os::unix::fs::PermissionsExt;

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-mutable-guest";
    private_guest_image(&image, bytes);
    for unsafe_mode in [0o664, 0o666] {
        fs::set_permissions(&image, fs::Permissions::from_mode(unsafe_mode)).unwrap();
        let error = validate_guest_definition(&definition(&image, bytes)).unwrap_err();
        assert!(error.to_string().contains("writable"), "{error}");
    }
}

#[cfg(unix)]
#[test]
fn w8_proof_session_root_rejects_qemu_option_delimiters() {
    use primed::windows_vm::vm_session_paths;

    let dir = private_tempdir();
    for unsafe_name in ["runtime,alias", "runtime\nspoof", "runtime\tspoof"] {
        let root = dir.path().join(unsafe_name);
        private_runtime_dir(&root);
        assert!(
            vm_session_paths(&root, "app-001", "session-a").is_err(),
            "QEMU option delimiter accepted in root: {unsafe_name:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn w8_runtime_directory_requires_private_traversal() {
    use primed::windows_vm::vm_session_paths;
    use std::os::unix::fs::PermissionsExt;
    let dir = private_tempdir();
    let root = dir.path().join("runtime");
    private_runtime_dir(&root);
    assert!(vm_session_paths(&root, "app-001", "session-a").is_ok());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(vm_session_paths(&root, "app-001", "session-a").is_err());
}

#[cfg(unix)]
#[test]
fn w8_qemu_plan_rejects_public_guest_state_paths() {
    use std::os::unix::fs::PermissionsExt;
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);
    let request = VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime.clone(),
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };
    assert!(build_qemu_plan(&request).is_ok());
    fs::set_permissions(&overlay, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(build_qemu_plan(&request).is_err());
    fs::set_permissions(&overlay, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(build_qemu_plan(&request).is_err());
}

#[test]
fn guest_definition_rejects_qemu_option_delimiters_in_image_path() {
    // A digest-valid image path must not be able to introduce QEMU options
    // or control-character ambiguity into later guest preparation.
    let dir = private_tempdir();
    let bytes = b"prime-w8-guest-path-negative";
    for name in ["guest,if=none.qcow2", "guest\nimage.qcow2"] {
        let image = dir.path().join(name);
        private_guest_image(&image, bytes);
        let error = validate_guest_definition(&definition(&image, bytes)).unwrap_err();
        assert!(error.to_string().contains("delimiter"));
    }
}

#[test]
fn runtime_evidence_rejects_oversized_bytes_and_envelope() {
    use primed::windows_vm::{
        create_runtime_proof_observation_from_envelope,
        create_runtime_proof_observation_from_evidence, prepare_runtime_proof_plan,
        VmRuntimeEvidenceEnvelope, WINDOWS_VM_RUNTIME_EVIDENCE_SCHEMA,
    };

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let guest_bytes = b"prime-w8-evidence-limit-guest";
    private_guest_image(&image, guest_bytes);
    let guest = validate_guest_definition(&definition(&image, guest_bytes)).unwrap();
    let root = dir.path().join("runtime");
    private_runtime_dir(&root);
    let plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();
    let case = plan.required_cases[0].clone();
    let evidence = vec![b'x'; 1024 * 1024];

    assert!(create_runtime_proof_observation_from_evidence(
        &plan,
        case.clone(),
        true,
        true,
        &evidence
    )
    .is_ok());
    let mut oversized = evidence.clone();
    oversized.push(b'x');
    assert!(create_runtime_proof_observation_from_evidence(
        &plan,
        case.clone(),
        true,
        true,
        &oversized
    )
    .is_err());

    let mut envelope = VmRuntimeEvidenceEnvelope {
        schema: WINDOWS_VM_RUNTIME_EVIDENCE_SCHEMA.to_owned(),
        proof_id: plan.proof_id.clone(),
        case,
        guest_id: plan.guest_id.clone(),
        guest_revision: plan.guest_revision,
        guest_sha256: plan.guest_sha256.clone(),
        application_id: plan.application_id.clone(),
        session_id: plan.session_id.clone(),
        evidence,
    };
    assert!(create_runtime_proof_observation_from_envelope(&plan, &envelope, true, true).is_ok());
    envelope.evidence.push(b'x');
    assert!(create_runtime_proof_observation_from_envelope(&plan, &envelope, true, true).is_err());
}

#[test]
fn runtime_observation_rejects_unrecognized_authority_fields() {
    use primed::windows_vm::{
        create_runtime_proof_observation_from_evidence, prepare_runtime_proof_plan,
        VmRuntimeProofObservation,
    };

    let dir = private_tempdir();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-observation-admission";
    private_guest_image(&image, bytes);
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    private_runtime_dir(&root);
    let plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();
    let observation = create_runtime_proof_observation_from_evidence(
        &plan,
        plan.required_cases[0].clone(),
        true,
        true,
        b"host-observed-case",
    )
    .unwrap();
    let mut serialized = serde_json::to_value(&observation).unwrap();
    serialized["authority"] = serde_json::json!("guest-attested");
    assert!(serde_json::from_value::<VmRuntimeProofObservation>(serialized).is_err());
}
