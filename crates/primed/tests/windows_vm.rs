use prime_contracts::{
    ApplicationArtifact, ApplicationProfile, ArtifactFormat, CompatibilityRecord, ExecutionBackend,
    MechanicalCompatibilityState, PolicyReference, RuntimeFamily, APPLICATION_PROFILE_SCHEMA,
};
use primed::windows_vm::{
    build_qemu_plan, parse_guest_agent_hello_json, validate_guest_definition, validate_vm_profile,
    VmGuestDefinition, VmPlanRequest,
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
    for (version, header_length, extension_offset) in
        [(2u32, 0u32, 72usize), (3, 104, 104), (3, 112, 112)]
    {
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
    for (version, header_length, extension_offset) in
        [(2u32, 0u32, 72usize), (3, 104, 104), (3, 112, 112)]
    {
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
                bytes[field_start..field_start + 8].copy_from_slice(&bad_offset.to_be_bytes());
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
                bytes[field_start..field_start + 8].copy_from_slice(&admitted_offset.to_be_bytes());
                // Both halves of a present metadata table must agree.
                let count_start = if field_start == 40 { 36 } else { 56 };
                let count = if admitted_offset == 0 { 0u32 } else { 1u32 };
                bytes[count_start..count_start + 4].copy_from_slice(&count.to_be_bytes());
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
            // A nonzero aligned pointer paired with an absent table count
            // is also contradictory, even though its alignment is valid.
            for offset in [65536u64, 131072] {
                private_overlay(&overlay);
                let mut bytes = fs::read(&overlay).unwrap();
                bytes[4..8].copy_from_slice(&version.to_be_bytes());
                if version == 2 {
                    bytes.truncate(72);
                }
                bytes[count_start..count_start + 4].fill(0);
                bytes[offset_start..offset_start + 8].copy_from_slice(&offset.to_be_bytes());
                fs::write(&overlay, bytes).unwrap();
                let error = build_qemu_plan(&request).unwrap_err();
                assert!(
                    error.to_string().contains("metadata table count"),
                    "version {version}, offset field {offset_start}, offset {offset}: {error}"
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
fn qemu_plan_rejects_overlapping_or_wrapping_qcow2_metadata_tables() {
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

    // L1 size counts 8-byte entries; refcount size counts whole clusters.
    // Adjacent tables are permitted by this header-only admission rule.
    for version in [2u32, 3] {
        for (l1_entries, l1_offset, ref_clusters, ref_offset, overlaps) in [
            (1u32, 65536u64, 1u32, 65536u64, true),
            (8193, 65536, 1, 131072, true),
            (1, 131072, 2, 65536, true),
            (1, 65536, 1, 131072, false),
            (8193, 65536, 1, 196608, false),
            (1, 131072, 1, 65536, false),
        ] {
            private_overlay(&overlay);
            let mut bytes = fs::read(&overlay).unwrap();
            bytes[4..8].copy_from_slice(&version.to_be_bytes());
            if version == 2 {
                bytes.truncate(72);
            }
            // Keep the overlap fixture within the L1 allocation budget.
            if l1_entries == 8193 {
                bytes[24..32].copy_from_slice(&(8193u64 * 536_870_912).to_be_bytes());
            }
            bytes[36..40].copy_from_slice(&l1_entries.to_be_bytes());
            bytes[40..48].copy_from_slice(&l1_offset.to_be_bytes());
            bytes[48..56].copy_from_slice(&ref_offset.to_be_bytes());
            bytes[56..60].copy_from_slice(&ref_clusters.to_be_bytes());
            fs::write(&overlay, bytes).unwrap();
            if overlaps {
                let error = build_qemu_plan(&request).unwrap_err();
                assert!(
                    error.to_string().contains("overlap"),
                    "version {version}, L1 {l1_entries}@{l1_offset}, refcount {ref_clusters}@{ref_offset}: {error}"
                );
            } else {
                assert!(
                    build_qemu_plan(&request).is_ok(),
                    "version {version}, L1 {l1_entries}@{l1_offset}, refcount {ref_clusters}@{ref_offset}"
                );
            }
        }

        // The final aligned cluster cannot fit a nonempty table span
        // into a u64 address domain; reject before QEMU argument creation.
        let last_aligned_cluster = u64::MAX - 65535;
        for (l1_entries, l1_offset, ref_clusters, ref_offset) in [
            (1u32, last_aligned_cluster, 0u32, 0u64),
            (0, 0, 1, last_aligned_cluster),
        ] {
            private_overlay(&overlay);
            let mut bytes = fs::read(&overlay).unwrap();
            bytes[4..8].copy_from_slice(&version.to_be_bytes());
            if version == 2 {
                bytes.truncate(72);
            }
            bytes[36..40].copy_from_slice(&l1_entries.to_be_bytes());
            bytes[40..48].copy_from_slice(&l1_offset.to_be_bytes());
            bytes[48..56].copy_from_slice(&ref_offset.to_be_bytes());
            bytes[56..60].copy_from_slice(&ref_clusters.to_be_bytes());
            fs::write(&overlay, bytes).unwrap();
            let error = build_qemu_plan(&request).unwrap_err();
            assert!(
                error.to_string().contains("overflows"),
                "version {version}, L1 {l1_entries}@{l1_offset}, refcount {ref_clusters}@{ref_offset}: {error}"
            );
        }
    }
}

#[test]
fn qemu_plan_rejects_qcow2_tables_beyond_signed_file_offsets() {
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

    // The u64 addition need not overflow for a qcow2 table to exceed
    // QEMU's signed block-offset domain. Preserve a nearby valid control.
    let first_unrepresentable = (i64::MAX as u64) + 1;
    let high_nonwrapping_cluster = u64::MAX - 131_071;
    let last_safe_cluster = first_unrepresentable - 131_072;
    for version in [2u32, 3] {
        for (l1_entries, l1_offset, ref_clusters, ref_offset, valid) in [
            (1u32, first_unrepresentable, 0u32, 0u64, false),
            (0, 0, 1, first_unrepresentable, false),
            (1, high_nonwrapping_cluster, 0, 0, false),
            (0, 0, 1, high_nonwrapping_cluster, false),
            (1, last_safe_cluster, 0, 0, true),
            (0, 0, 1, last_safe_cluster, true),
        ] {
            private_overlay(&overlay);
            let mut bytes = fs::read(&overlay).unwrap();
            bytes[4..8].copy_from_slice(&version.to_be_bytes());
            if version == 2 {
                bytes.truncate(72);
            }
            bytes[36..40].copy_from_slice(&l1_entries.to_be_bytes());
            bytes[40..48].copy_from_slice(&l1_offset.to_be_bytes());
            bytes[48..56].copy_from_slice(&ref_offset.to_be_bytes());
            bytes[56..60].copy_from_slice(&ref_clusters.to_be_bytes());
            fs::write(&overlay, bytes).unwrap();
            let result = build_qemu_plan(&request);
            if valid {
                assert!(
                    result.is_ok(),
                    "version {version}, L1 {l1_entries}@{l1_offset}, refcount {ref_clusters}@{ref_offset}: {result:?}"
                );
            } else {
                let error = result.unwrap_err();
                assert!(
                    error.to_string().contains("signed file-offset"),
                    "version {version}, L1 {l1_entries}@{l1_offset}, refcount {ref_clusters}@{ref_offset}: {error}"
                );
            }
        }
    }
}

#[test]
fn qemu_plan_rejects_l1_tables_too_small_for_virtual_disk() {
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

    // At 512-byte clusters, each L1 entry covers 32 KiB; at 64 KiB
    // clusters, each L1 entry covers 512 MiB. Test both sides of each
    // boundary, plus the existing zero-L1 header-only fixture allowance.
    for version in [2u32, 3] {
        for (cluster_bits, virtual_size, l1_entries, admissible) in [
            (9u32, 65_536u64, 1u32, false),
            (9, 65_536, 2, true),
            (9, 32_768, 1, true),
            (16, 536_870_913, 1, false),
            (16, 536_870_913, 2, true),
            (16, 64 * 1024 * 1024, 1, true),
            (9, 65_536, 0, true),
        ] {
            private_overlay(&overlay);
            let mut bytes = fs::read(&overlay).unwrap();
            bytes[4..8].copy_from_slice(&version.to_be_bytes());
            if version == 2 {
                bytes.truncate(72);
            }
            let cluster_size = 1u64 << cluster_bits;
            let l1_offset = if l1_entries == 0 { 0 } else { cluster_size };
            bytes[20..24].copy_from_slice(&cluster_bits.to_be_bytes());
            bytes[24..32].copy_from_slice(&virtual_size.to_be_bytes());
            bytes[36..40].copy_from_slice(&l1_entries.to_be_bytes());
            bytes[40..48].copy_from_slice(&l1_offset.to_be_bytes());
            bytes[48..56].copy_from_slice(&(cluster_size * 2).to_be_bytes());
            bytes[56..60].copy_from_slice(&1u32.to_be_bytes());
            fs::write(&overlay, bytes).unwrap();

            if admissible {
                assert!(
                    build_qemu_plan(&request).is_ok(),
                    "v{version}, bits {cluster_bits}, size {virtual_size}, L1 {l1_entries}"
                );
            } else {
                let error = build_qemu_plan(&request).unwrap_err();
                assert!(
                    error.to_string().contains("does not cover declared virtual size"),
                    "v{version}, bits {cluster_bits}, size {virtual_size}, L1 {l1_entries}: {error}"
                );
            }
        }
    }
}

#[test]
fn qemu_plan_bounds_qcow2_l1_allocation_to_virtual_disk() {
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

    // L1 table capacity may round up to a full cluster, but arbitrary
    // extra clusters would expose QEMU to unadmitted metadata allocation.
    // Keep zero-L1 as the existing header-only fixture exception.
    for version in [2u32, 3] {
        for (cluster_bits, virtual_size, l1_entries, admissible) in [
            (9u32, 32_768u64, 1u32, true),
            (9, 32_768, 64, true),
            (9, 32_768, 65, false),
            (9, 65_536, 64, true),
            (9, 65_536, 65, false),
            (16, 536_870_912, 8192, true),
            (16, 536_870_912, 8193, false),
            (9, 32_768, 0, true),
        ] {
            private_overlay(&overlay);
            let mut bytes = fs::read(&overlay).unwrap();
            bytes[4..8].copy_from_slice(&version.to_be_bytes());
            if version == 2 {
                bytes.truncate(72);
            }
            let cluster_size = 1u64 << cluster_bits;
            let l1_offset = if l1_entries == 0 { 0 } else { cluster_size };
            bytes[20..24].copy_from_slice(&cluster_bits.to_be_bytes());
            bytes[24..32].copy_from_slice(&virtual_size.to_be_bytes());
            bytes[36..40].copy_from_slice(&l1_entries.to_be_bytes());
            bytes[40..48].copy_from_slice(&l1_offset.to_be_bytes());
            bytes[48..56].copy_from_slice(&0u64.to_be_bytes());
            bytes[56..60].copy_from_slice(&0u32.to_be_bytes());
            fs::write(&overlay, bytes).unwrap();

            let result = build_qemu_plan(&request);
            if admissible {
                assert!(
                    result.is_ok(),
                    "v{version}, bits {cluster_bits}, size {virtual_size}, L1 {l1_entries}: {result:?}"
                );
            } else {
                let error = result.unwrap_err();
                assert!(
                    error.to_string().contains("exceeds cluster-rounded"),
                    "v{version}, bits {cluster_bits}, size {virtual_size}, L1 {l1_entries}: {error}"
                );
            }
        }
    }
}

#[test]
fn qemu_plan_bounds_qcow2_refcount_table_allocation() {
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

    // Refcount metadata is not entitled to arbitrary allocation unrelated to
    // the virtual disk. This is W8's conservative host admission budget, not
    // a general qcow2-format restriction or guest boot qualification.
    for version in [2u32, 3] {
        for (cluster_bits, virtual_size, refcount_clusters, admitted) in [
            (9u32, 1u64, 0u32, true),
            (9, 1, 1, true),
            (9, 1, 2, false),
            (9, 512, 1, true),
            (9, 512, 2, false),
            (9, 513, 2, true),
            (9, 513, 3, false),
            (16, 1, 1, true),
            (16, 1, 2, false),
            (16, 65537, 2, true),
            (16, 65537, 3, false),
            (9, 1, u32::MAX, false),
        ] {
            private_overlay(&overlay);
            let mut bytes = fs::read(&overlay).unwrap();
            bytes[4..8].copy_from_slice(&version.to_be_bytes());
            if version == 2 {
                bytes.truncate(72);
            }
            let cluster_size = 1u64 << cluster_bits;
            let refcount_offset = if refcount_clusters == 0 {
                0
            } else {
                cluster_size * 2
            };
            bytes[20..24].copy_from_slice(&cluster_bits.to_be_bytes());
            bytes[24..32].copy_from_slice(&virtual_size.to_be_bytes());
            bytes[36..40].copy_from_slice(&0u32.to_be_bytes());
            bytes[40..48].copy_from_slice(&0u64.to_be_bytes());
            bytes[48..56].copy_from_slice(&refcount_offset.to_be_bytes());
            bytes[56..60].copy_from_slice(&refcount_clusters.to_be_bytes());
            fs::write(&overlay, bytes).unwrap();

            let result = build_qemu_plan(&request);
            if admitted {
                assert!(
                    result.is_ok(),
                    "v{version}, bits {cluster_bits}, disk {virtual_size}, refcount clusters {refcount_clusters}: {result:?}"
                );
            } else {
                let error = result.unwrap_err();
                assert!(
                    error.to_string().contains("refcount table exceeds virtual disk"),
                    "v{version}, bits {cluster_bits}, disk {virtual_size}, refcount clusters {refcount_clusters}: {error}"
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
    // v2 must not retain the v3-only header-length field at byte 100.
    header.truncate(72);
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
fn qemu_plan_rejects_set_id_executable_before_guest_launch() {
    use std::os::unix::fs::PermissionsExt;

    let dir = private_tempdir();
    let qemu = dir.path().join("qemu-test-binary");
    fs::write(&qemu, b"#!/bin/sh\n").unwrap();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);

    let request = || VmPlanRequest {
        qemu_binary: qemu.clone(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime.clone(),
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec![],
    };
    fs::set_permissions(&qemu, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(build_qemu_plan(&request()).is_ok());

    // File mode alone must not let an otherwise trusted QEMU executable
    // cross a UID or GID boundary when a caller eventually launches it.
    // This is a plan-only regression: no test executable is run.
    for unsafe_mode in [0o4700, 0o2700, 0o6700] {
        fs::set_permissions(&qemu, fs::Permissions::from_mode(unsafe_mode)).unwrap();
        assert_eq!(
            fs::metadata(&qemu).unwrap().permissions().mode() & 0o6000,
            unsafe_mode & 0o6000,
        );
        let error = build_qemu_plan(&request()).unwrap_err();
        assert!(error.to_string().contains("set-id"), "{error}");
    }
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

#[test]
fn qemu_plan_rejects_duplicate_usb_device_authority() {
    let dir = private_tempdir();
    let runtime = dir.path().join("runtime");
    private_runtime_dir(&runtime);
    let overlay = runtime.join("overlay.qcow2");
    private_overlay(&overlay);

    let request = |nodes: Vec<&str>| VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime.clone(),
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: nodes.into_iter().map(Into::into).collect(),
    };

    // Repeated authority is not a second device: reject ambiguous input
    // instead of silently shrinking the caller's explicit W7 allowlist.
    let duplicate = build_qemu_plan(&request(vec![
        "/dev/bus/usb/001/018",
        "/dev/bus/usb/001/018",
    ]))
    .unwrap_err();
    assert!(
        duplicate.to_string().contains("duplicate USB"),
        "{duplicate}"
    );

    let distinct = build_qemu_plan(&request(vec![
        "/dev/bus/usb/001/018",
        "/dev/bus/usb/001/019",
    ]))
    .unwrap();
    let arguments = distinct.args.join(" ");
    assert!(arguments.contains("usb-host,hostbus=1,hostaddr=18"));
    assert!(arguments.contains("usb-host,hostbus=1,hostaddr=19"));
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
fn guest_agent_json_admission_bounds_bytes_before_deserialization() {
    use primed::windows_vm::GuestAgentHello;
    let hello = GuestAgentHello {
        schema: "prime.windows-vm-agent.v1".to_owned(),
        protocol_revision: 1,
        guest_id: "windows-fallback-test".to_owned(),
        guest_revision: 1,
        guest_sha256: "a".repeat(64),
        workload_arch: "x86_64".to_owned(),
    };
    let json = serde_json::to_vec(&hello).unwrap();
    let accepted = parse_guest_agent_hello_json(
        &json,
        &hello.guest_id,
        hello.guest_revision,
        &hello.guest_sha256,
        "x86_64",
    )
    .unwrap();
    assert_eq!(accepted, hello);

    // Whitespace is legal JSON. The exact limit is accepted, but one extra
    // byte must fail before the JSON decoder sees a potentially huge payload.
    assert!(json.len() < 8192);
    let mut boundary = vec![b' '; 8192 - json.len()];
    boundary.extend_from_slice(&json);
    assert!(parse_guest_agent_hello_json(
        &boundary,
        &hello.guest_id,
        hello.guest_revision,
        &hello.guest_sha256,
        "x86_64",
    )
    .is_ok());
    let mut oversized = boundary;
    oversized.push(b' ');
    let error = parse_guest_agent_hello_json(
        &oversized,
        &hello.guest_id,
        hello.guest_revision,
        &hello.guest_sha256,
        "x86_64",
    )
    .unwrap_err();
    assert!(error.to_string().contains("size limit"), "{error}");

    for invalid in [b"".as_slice(), b"{".as_slice(), b"[]".as_slice()] {
        assert!(parse_guest_agent_hello_json(
            invalid,
            &hello.guest_id,
            hello.guest_revision,
            &hello.guest_sha256,
            "x86_64",
        )
        .is_err());
    }

    // Exercise the JSON entry point, not just the lower-level validator:
    // decoding a valid but unauthorized hello must never bypass identity custody.
    let mismatches = [
        (
            "guest_id",
            GuestAgentHello {
                guest_id: "different-guest".to_owned(),
                ..hello.clone()
            },
        ),
        (
            "guest_revision",
            GuestAgentHello {
                guest_revision: hello.guest_revision + 1,
                ..hello.clone()
            },
        ),
        (
            "guest_sha256",
            GuestAgentHello {
                guest_sha256: "b".repeat(64),
                ..hello.clone()
            },
        ),
        (
            "workload_arch",
            GuestAgentHello {
                workload_arch: "x86".to_owned(),
                ..hello.clone()
            },
        ),
    ];
    for (field, mismatched) in mismatches {
        let payload = serde_json::to_vec(&mismatched).unwrap();
        let error = parse_guest_agent_hello_json(
            &payload,
            &hello.guest_id,
            hello.guest_revision,
            &hello.guest_sha256,
            "x86_64",
        )
        .unwrap_err();
        assert!(error.to_string().contains("mismatch"), "{field}: {error}");
    }
}

#[test]
fn guest_agent_harness_json_encoder_is_deterministic_and_authority_checked() {
    use primed::windows_vm::{
        encode_guest_agent_hello_json, parse_guest_agent_hello_json, GuestAgentHello,
        WINDOWS_VM_AGENT_HELLO_MAX_BYTES,
    };

    let hello = GuestAgentHello {
        schema: "prime.windows-vm-agent.v1".to_owned(),
        protocol_revision: 1,
        guest_id: "windows-harness-x64".to_owned(),
        guest_revision: 7,
        guest_sha256: "c".repeat(64),
        workload_arch: "x86_64".to_owned(),
    };
    let encode = |candidate: &GuestAgentHello| {
        encode_guest_agent_hello_json(
            candidate,
            &hello.guest_id,
            hello.guest_revision,
            &hello.guest_sha256,
            "x86_64",
        )
    };
    let encoded = encode(&hello).unwrap();
    assert_eq!(encoded, serde_json::to_vec(&hello).unwrap());
    assert!(encoded.len() <= WINDOWS_VM_AGENT_HELLO_MAX_BYTES);
    assert_eq!(encode(&hello).unwrap(), encoded);
    assert_eq!(
        parse_guest_agent_hello_json(
            &encoded,
            &hello.guest_id,
            hello.guest_revision,
            &hello.guest_sha256,
            "x86_64",
        )
        .unwrap(),
        hello
    );

    // A local harness must not manufacture a plausible hello for a different
    // guest, protocol, digest, or architecture. This is not guest attestation.
    let mut invalid = hello.clone();
    invalid.guest_id = "other-guest".to_owned();
    assert!(encode(&invalid).is_err());
    invalid = hello.clone();
    invalid.protocol_revision = 2;
    assert!(encode(&invalid).is_err());
    invalid = hello.clone();
    invalid.guest_sha256 = "d".repeat(64);
    assert!(encode(&invalid).is_err());
    invalid = hello.clone();
    invalid.workload_arch = "x86".to_owned();
    assert!(encode(&invalid).is_err());
    assert!(encode_guest_agent_hello_json(
        &hello,
        &hello.guest_id,
        0,
        &hello.guest_sha256,
        "x86_64",
    )
    .is_err());
}

#[test]
fn guest_agent_length_delimited_reader_admits_only_one_bounded_frame() {
    use primed::windows_vm::{
        read_guest_agent_hello_frame, GuestAgentHello, WINDOWS_VM_AGENT_HELLO_MAX_BYTES,
    };
    use std::io::Cursor;

    let hello = GuestAgentHello {
        schema: "prime.windows-vm-agent.v1".to_owned(),
        protocol_revision: 1,
        guest_id: "windows-fallback-x86".to_owned(),
        guest_revision: 9,
        guest_sha256: "c".repeat(64),
        workload_arch: "x86".to_owned(),
    };
    let frame = serde_json::to_vec(&hello).unwrap();

    // Explicit frame length is the caller's transport boundary. Do not
    // consume bytes belonging to a subsequent message on the same stream.
    let mut stream = Cursor::new([frame.as_slice(), b"NEXT"].concat());
    let accepted = read_guest_agent_hello_frame(
        &mut stream,
        frame.len(),
        &hello.guest_id,
        hello.guest_revision,
        &hello.guest_sha256,
        "x86",
    )
    .unwrap();
    assert_eq!(accepted, hello);
    assert_eq!(stream.position() as usize, frame.len());

    // Reject oversized lengths before touching the reader or allocating a
    // payload. Truncated frames and empty frames are not valid handshakes.
    let mut oversized = Cursor::new(frame.clone());
    let error = read_guest_agent_hello_frame(
        &mut oversized,
        WINDOWS_VM_AGENT_HELLO_MAX_BYTES + 1,
        &hello.guest_id,
        hello.guest_revision,
        &hello.guest_sha256,
        "x86",
    )
    .unwrap_err();
    assert!(error.to_string().contains("size limit"), "{error}");
    assert_eq!(oversized.position(), 0);

    let mut truncated = Cursor::new(frame[..frame.len() - 1].to_vec());
    let error = read_guest_agent_hello_frame(
        &mut truncated,
        frame.len(),
        &hello.guest_id,
        hello.guest_revision,
        &hello.guest_sha256,
        "x86",
    )
    .unwrap_err();
    assert!(error.to_string().contains("truncated"), "{error}");

    let mut empty = Cursor::new(frame.clone());
    assert!(read_guest_agent_hello_frame(
        &mut empty,
        0,
        &hello.guest_id,
        hello.guest_revision,
        &hello.guest_sha256,
        "x86",
    )
    .is_err());
    assert_eq!(empty.position(), 0);

    // A complete frame is still untrusted until identity/digest validation.
    let mut mismatched = Cursor::new(frame);
    let mismatch_len = mismatched.get_ref().len();
    assert!(read_guest_agent_hello_frame(
        &mut mismatched,
        mismatch_len,
        &hello.guest_id,
        hello.guest_revision,
        &"d".repeat(64),
        "x86",
    )
    .is_err());
}

#[test]
fn guest_agent_length_delimited_reader_preserves_split_frames_and_io_failures() {
    use primed::windows_vm::{read_guest_agent_hello_frame, GuestAgentHello, WindowsVmError};
    use std::io::{self, Cursor, Read};

    // A socket-like reader may deliver a complete frame in arbitrarily small
    // chunks. The first admission must not borrow bytes from the next frame.
    struct Fragmented(Cursor<Vec<u8>>);
    impl Read for Fragmented {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let limit = buf.len().min(3);
            self.0.read(&mut buf[..limit])
        }
    }

    let x86 = GuestAgentHello {
        schema: "prime.windows-vm-agent.v1".to_owned(),
        protocol_revision: 1,
        guest_id: "guest-x86".to_owned(),
        guest_revision: 3,
        guest_sha256: "a".repeat(64),
        workload_arch: "x86".to_owned(),
    };
    let mut x64 = x86.clone();
    x64.guest_id = "guest-x64".to_owned();
    x64.guest_revision = 4;
    x64.guest_sha256 = "b".repeat(64);
    x64.workload_arch = "x86_64".to_owned();
    let first = serde_json::to_vec(&x86).unwrap();
    let second = serde_json::to_vec(&x64).unwrap();
    let mut reader = Fragmented(Cursor::new([first.as_slice(), second.as_slice()].concat()));

    assert_eq!(
        read_guest_agent_hello_frame(
            &mut reader,
            first.len(),
            &x86.guest_id,
            x86.guest_revision,
            &x86.guest_sha256,
            "x86",
        )
        .unwrap(),
        x86,
    );
    assert_eq!(reader.0.position() as usize, first.len());
    assert_eq!(
        read_guest_agent_hello_frame(
            &mut reader,
            second.len(),
            &x64.guest_id,
            x64.guest_revision,
            &x64.guest_sha256,
            "x86_64",
        )
        .unwrap(),
        x64,
    );
    assert_eq!(reader.0.position() as usize, first.len() + second.len());

    // A transport permission failure is not a malformed guest identity and
    // must remain an I/O failure for the future socket owner to handle.
    struct Denied;
    impl Read for Denied {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        }
    }
    assert!(matches!(
        read_guest_agent_hello_frame(
            &mut Denied, first.len(), "guest-x86", 3, &"a".repeat(64), "x86",
        ),
        Err(WindowsVmError::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied
    ));
}

#[test]
fn guest_agent_reader_rejects_invalid_host_authority_before_consuming_frame() {
    use primed::windows_vm::{read_guest_agent_hello_frame, GuestAgentHello};
    use std::io::{self, Cursor, Read};

    struct Counted(Cursor<Vec<u8>>, usize);
    impl Read for Counted {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.1 += 1;
            self.0.read(buffer)
        }
    }

    let hello = GuestAgentHello {
        schema: "prime.windows-vm-agent.v1".to_owned(),
        protocol_revision: 1,
        guest_id: "guest-x86".to_owned(),
        guest_revision: 3,
        guest_sha256: "a".repeat(64),
        workload_arch: "x86".to_owned(),
    };
    let frame = serde_json::to_vec(&hello).unwrap();
    for (case, guest_id, revision, digest, arch) in [
        ("invalid ID", "guest/alias", 3, "a".repeat(64), "x86"),
        ("zero revision", "guest-x86", 0, "a".repeat(64), "x86"),
        ("noncanonical digest", "guest-x86", 3, "A".repeat(64), "x86"),
        (
            "unsupported architecture",
            "guest-x86",
            3,
            "a".repeat(64),
            "arm64",
        ),
    ] {
        let mut reader = Counted(Cursor::new(frame.clone()), 0);
        assert!(
            read_guest_agent_hello_frame(
                &mut reader,
                frame.len(),
                guest_id,
                revision,
                &digest,
                arch,
            )
            .is_err(),
            "{case}: invalid caller authority must fail closed"
        );
        assert_eq!(
            reader.1, 0,
            "{case}: invalid host authority consumed a frame"
        );
        assert_eq!(reader.0.position(), 0, "{case}: stream position changed");
    }

    let mut valid = Counted(Cursor::new(frame.clone()), 0);
    assert_eq!(
        read_guest_agent_hello_frame(
            &mut valid,
            frame.len(),
            &hello.guest_id,
            hello.guest_revision,
            &hello.guest_sha256,
            "x86",
        )
        .unwrap(),
        hello
    );
    assert!(valid.1 > 0);
    assert_eq!(valid.0.position() as usize, frame.len());
}

#[test]
fn guest_agent_json_rejects_invalid_caller_expectations() {
    use primed::windows_vm::GuestAgentHello;

    // The frame may be authentic JSON while the host's claimed authority is
    // malformed. Exercise the parser boundary, not only the inner validator.
    let hello = GuestAgentHello {
        schema: "prime.windows-vm-agent.v1".to_owned(),
        protocol_revision: 1,
        guest_id: "windows-fallback-test".to_owned(),
        guest_revision: 1,
        guest_sha256: "a".repeat(64),
        workload_arch: "x86_64".to_owned(),
    };
    let frame = serde_json::to_vec(&hello).unwrap();

    for invalid_id in ["", "guest/alias", " leading", "guest\nname"] {
        assert!(
            parse_guest_agent_hello_json(
                &frame,
                invalid_id,
                hello.guest_revision,
                &hello.guest_sha256,
                "x86_64",
            )
            .is_err(),
            "invalid host guest identity {invalid_id:?} was accepted"
        );
    }

    assert!(parse_guest_agent_hello_json(
        &frame,
        &hello.guest_id,
        0,
        &hello.guest_sha256,
        "x86_64",
    )
    .is_err());

    for invalid_digest in ["A".repeat(64), "a".repeat(63)] {
        assert!(
            parse_guest_agent_hello_json(
                &frame,
                &hello.guest_id,
                hello.guest_revision,
                &invalid_digest,
                "x86_64",
            )
            .is_err(),
            "invalid host digest was accepted"
        );
    }

    for invalid_arch in ["arm64", "X86_64", ""] {
        assert!(
            parse_guest_agent_hello_json(
                &frame,
                &hello.guest_id,
                hello.guest_revision,
                &hello.guest_sha256,
                invalid_arch,
            )
            .is_err(),
            "invalid host workload architecture {invalid_arch:?} was accepted"
        );
    }
}

#[test]
fn guest_agent_json_parser_preflights_host_authority_before_untrusted_decode() {
    use primed::windows_vm::parse_guest_agent_hello_json;

    // Malformed guest input must not determine the admission outcome when
    // the host's own claimed identity, digest, or architecture is invalid.
    let malformed_wire = br#"{not-a-json-document"#;
    for (case, id, revision, digest, arch, expected_error) in [
        (
            "invalid id",
            "guest/name",
            1,
            "a".repeat(64),
            "x86_64",
            "invalid expected guest identity",
        ),
        (
            "zero revision",
            "guest-test",
            0,
            "a".repeat(64),
            "x86_64",
            "invalid expected guest identity",
        ),
        (
            "invalid digest",
            "guest-test",
            1,
            "A".repeat(64),
            "x86_64",
            "guest-agent digest mismatch",
        ),
        (
            "invalid arch",
            "guest-test",
            1,
            "a".repeat(64),
            "arm64",
            "guest-agent workload architecture mismatch",
        ),
    ] {
        let error =
            parse_guest_agent_hello_json(malformed_wire, id, revision, &digest, arch).unwrap_err();
        assert!(
            error.to_string().contains(expected_error),
            "{case}: {error}"
        );
    }
}

#[test]
fn guest_agent_json_rejects_ambiguous_and_unauthorized_wire_shapes() {
    use primed::windows_vm::{parse_guest_agent_hello_json, GuestAgentHello};

    let hello = GuestAgentHello {
        schema: "prime.windows-vm-agent.v1".to_owned(),
        protocol_revision: 1,
        guest_id: "windows-fallback-test".to_owned(),
        guest_revision: 1,
        guest_sha256: "a".repeat(64),
        workload_arch: "x86_64".to_owned(),
    };
    let canonical = serde_json::to_string(&hello).unwrap();

    // An attacker must not hide a conflicting identity behind duplicate JSON
    // keys, add host-control fields, or append a second protocol document.
    let duplicate_id = format!(r#"{{"guest_id":"shadow",{}"#, &canonical[1..]);
    let host_override = format!(r#"{{"host_override":true,{}"#, &canonical[1..]);
    let trailing_document = format!("{canonical}{{}}");
    for (case, payload) in [
        ("duplicate guest ID", duplicate_id),
        ("undeclared host override", host_override),
        ("trailing document", trailing_document),
    ] {
        assert!(
            parse_guest_agent_hello_json(
                payload.as_bytes(),
                &hello.guest_id,
                hello.guest_revision,
                &hello.guest_sha256,
                "x86_64",
            )
            .is_err(),
            "{case} must not be admitted as guest identity"
        );
    }

    for (case, field, replacement) in [
        (
            "unsupported schema",
            "schema",
            serde_json::json!("prime.windows-vm-agent.v2"),
        ),
        (
            "unsupported revision",
            "protocol_revision",
            serde_json::json!(2),
        ),
    ] {
        let mut payload = serde_json::to_value(&hello).unwrap();
        payload[field] = replacement;
        assert!(
            parse_guest_agent_hello_json(
                &serde_json::to_vec(&payload).unwrap(),
                &hello.guest_id,
                hello.guest_revision,
                &hello.guest_sha256,
                "x86_64",
            )
            .is_err(),
            "{case} must fail closed"
        );
    }
}

#[test]
fn guest_agent_json_x86_handoff_is_arch_and_digest_bound() {
    use primed::windows_vm::{parse_guest_agent_hello_json, GuestAgentHello};

    // Gate 6 explicitly requires both x86 and x86_64 workload handoff.
    // The x86 JSON path must admit its own identity while refusing an x64
    // expectation, an x64 wire identity, or a different pinned image digest.
    let hello = GuestAgentHello {
        schema: "prime.windows-vm-agent.v1".to_owned(),
        protocol_revision: 1,
        guest_id: "windows-fallback-x86".to_owned(),
        guest_revision: 9,
        guest_sha256: "c".repeat(64),
        workload_arch: "x86".to_owned(),
    };
    let wire = serde_json::to_vec(&hello).unwrap();
    assert_eq!(
        parse_guest_agent_hello_json(
            &wire,
            &hello.guest_id,
            hello.guest_revision,
            &hello.guest_sha256,
            "x86",
        )
        .unwrap(),
        hello
    );

    let wrong_expected_arch = parse_guest_agent_hello_json(
        &wire,
        &hello.guest_id,
        hello.guest_revision,
        &hello.guest_sha256,
        "x86_64",
    )
    .unwrap_err();
    assert!(wrong_expected_arch
        .to_string()
        .contains("architecture mismatch"));

    let wrong_expected_digest = parse_guest_agent_hello_json(
        &wire,
        &hello.guest_id,
        hello.guest_revision,
        &"d".repeat(64),
        "x86",
    )
    .unwrap_err();
    assert!(wrong_expected_digest
        .to_string()
        .contains("digest mismatch"));

    let mut wrong_wire_arch = hello.clone();
    wrong_wire_arch.workload_arch = "x86_64".to_owned();
    let wrong_wire = serde_json::to_vec(&wrong_wire_arch).unwrap();
    let wrong_wire_arch_error = parse_guest_agent_hello_json(
        &wrong_wire,
        &hello.guest_id,
        hello.guest_revision,
        &hello.guest_sha256,
        "x86",
    )
    .unwrap_err();
    assert!(wrong_wire_arch_error
        .to_string()
        .contains("architecture mismatch"));
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

#[test]
fn guest_agent_harness_payload_writer_is_bounded_and_fail_closed() {
    use primed::windows_vm::{
        encode_guest_agent_hello_json, write_guest_agent_hello_payload, GuestAgentHello,
        WindowsVmError, WINDOWS_VM_AGENT_SCHEMA,
    };
    use std::io::{self, Write};

    struct CountedWriter {
        bytes: Vec<u8>,
        calls: usize,
        fail: bool,
    }
    impl Write for CountedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            if self.fail {
                return Err(io::Error::from(io::ErrorKind::PermissionDenied));
            }
            // Model a fragmented transport, without owning its framing.
            let count = buf.len().min(3);
            self.bytes.extend_from_slice(&buf[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    for arch in ["x86", "x86_64"] {
        let hello = GuestAgentHello {
            schema: WINDOWS_VM_AGENT_SCHEMA.to_owned(),
            protocol_revision: 1,
            guest_id: "harness-guest".to_owned(),
            guest_revision: 7,
            guest_sha256: "a".repeat(64),
            workload_arch: arch.to_owned(),
        };
        let mut writer = CountedWriter {
            bytes: Vec::new(),
            calls: 0,
            fail: false,
        };
        let expected =
            encode_guest_agent_hello_json(&hello, &hello.guest_id, 7, &hello.guest_sha256, arch)
                .unwrap();
        let written = write_guest_agent_hello_payload(
            &mut writer,
            &hello,
            &hello.guest_id,
            7,
            &hello.guest_sha256,
            arch,
        )
        .unwrap();
        assert_eq!(written, expected.len());
        assert_eq!(writer.bytes, expected);
        assert!(writer.calls > 1, "fragmented writer was not exercised");

        for (id, revision, digest, workload_arch) in [
            ("guest/invalid", 7, "a".repeat(64), arch),
            ("harness-guest", 0, "a".repeat(64), arch),
            ("harness-guest", 7, "A".repeat(64), arch),
            ("harness-guest", 7, "a".repeat(64), "arm64"),
            ("foreign-guest", 7, "a".repeat(64), arch),
        ] {
            let mut denied = CountedWriter {
                bytes: Vec::new(),
                calls: 0,
                fail: false,
            };
            assert!(write_guest_agent_hello_payload(
                &mut denied,
                &hello,
                id,
                revision,
                &digest,
                workload_arch,
            )
            .is_err());
            assert_eq!(denied.calls, 0, "invalid authority touched the writer");
            assert!(denied.bytes.is_empty());
        }

        let mut denied = CountedWriter {
            bytes: Vec::new(),
            calls: 0,
            fail: true,
        };
        assert!(matches!(
            write_guest_agent_hello_payload(
                &mut denied, &hello, &hello.guest_id, 7, &hello.guest_sha256, arch,
            ),
            Err(WindowsVmError::Io(error))
                if error.kind() == io::ErrorKind::PermissionDenied
        ));
        assert_eq!(denied.calls, 1);
        assert!(denied.bytes.is_empty());

        // A transport failure after a partial write must not report success.
        struct PartialThenDenied {
            bytes: Vec<u8>,
        }
        impl Write for PartialThenDenied {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                if self.bytes.is_empty() {
                    self.bytes.extend_from_slice(&buf[..buf.len().min(3)]);
                    return Ok(buf.len().min(3));
                }
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut partial = PartialThenDenied { bytes: Vec::new() };
        assert!(matches!(
            write_guest_agent_hello_payload(
                &mut partial, &hello, &hello.guest_id, 7, &hello.guest_sha256, arch,
            ),
            Err(WindowsVmError::Io(error))
                if error.kind() == io::ErrorKind::PermissionDenied
        ));
        assert_eq!(partial.bytes, expected[..3]);
    }
}

#[test]
fn guest_agent_harness_payload_writer_handles_interrupted_and_zero_progress_io() {
    use primed::windows_vm::{
        write_guest_agent_hello_payload, GuestAgentHello, WindowsVmError, WINDOWS_VM_AGENT_SCHEMA,
    };
    use std::io::{self, Write};

    #[derive(Default)]
    struct InterruptedOrZeroWriter {
        bytes: Vec<u8>,
        calls: usize,
        zero_progress: bool,
    }
    impl Write for InterruptedOrZeroWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            if self.calls == 1 {
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            if self.zero_progress {
                return Ok(0);
            }
            let count = bytes.len().min(2);
            self.bytes.extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let hello = GuestAgentHello {
        schema: WINDOWS_VM_AGENT_SCHEMA.to_owned(),
        protocol_revision: 1,
        guest_id: "harness-guest".to_owned(),
        guest_revision: 7,
        guest_sha256: "a".repeat(64),
        workload_arch: "x86_64".to_owned(),
    };
    let expected = serde_json::to_vec(&hello).unwrap();

    let mut interrupted = InterruptedOrZeroWriter::default();
    let written = write_guest_agent_hello_payload(
        &mut interrupted,
        &hello,
        &hello.guest_id,
        7,
        &hello.guest_sha256,
        "x86_64",
    )
    .unwrap();
    assert_eq!(written, expected.len());
    assert_eq!(interrupted.bytes, expected);
    assert!(
        interrupted.calls > 2,
        "interrupted and fragmented I/O was not exercised"
    );

    let mut zero = InterruptedOrZeroWriter {
        zero_progress: true,
        ..Default::default()
    };
    assert!(matches!(
        write_guest_agent_hello_payload(
            &mut zero, &hello, &hello.guest_id, 7, &hello.guest_sha256, "x86_64",
        ),
        Err(WindowsVmError::Io(error)) if error.kind() == io::ErrorKind::WriteZero
    ));
    assert_eq!(zero.calls, 2);
    assert!(
        zero.bytes.is_empty(),
        "zero-progress writer must not be accepted"
    );
}

#[test]
fn guest_agent_harness_length_prefixed_packet_roundtrip_and_adversarial() {
    use primed::windows_vm::{
        read_guest_agent_hello_packet, write_guest_agent_hello_packet, GuestAgentHello,
        WindowsVmError, WINDOWS_VM_AGENT_HELLO_MAX_BYTES, WINDOWS_VM_AGENT_SCHEMA,
    };
    use std::io::{self, Cursor, Write};

    struct FragmentedWriter(Vec<u8>);
    impl Write for FragmentedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let n = bytes.len().min(2);
            self.0.extend_from_slice(&bytes[..n]);
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    for arch in ["x86", "x86_64"] {
        let hello = GuestAgentHello {
            schema: WINDOWS_VM_AGENT_SCHEMA.to_owned(),
            protocol_revision: 1,
            guest_id: "w8-harness".to_owned(),
            guest_revision: 9,
            guest_sha256: "a".repeat(64),
            workload_arch: arch.to_owned(),
        };
        let mut wire = FragmentedWriter(Vec::new());
        let written = write_guest_agent_hello_packet(
            &mut wire,
            &hello,
            &hello.guest_id,
            9,
            &hello.guest_sha256,
            arch,
        )
        .unwrap();
        let payload = serde_json::to_vec(&hello).unwrap();
        assert_eq!(written, payload.len() + 4);
        assert_eq!(&wire.0[..4], &(payload.len() as u32).to_be_bytes());
        assert_eq!(&wire.0[4..], payload.as_slice());

        let mut stream = Cursor::new([wire.0.as_slice(), b"NEXT"].concat());
        let admitted = read_guest_agent_hello_packet(
            &mut stream,
            &hello.guest_id,
            9,
            &hello.guest_sha256,
            arch,
        )
        .unwrap();
        assert_eq!(admitted, hello);
        assert_eq!(stream.position() as usize, written);

        let mut denied_writer = FragmentedWriter(Vec::new());
        assert!(write_guest_agent_hello_packet(
            &mut denied_writer,
            &hello,
            "wrong-guest",
            9,
            &hello.guest_sha256,
            arch,
        )
        .is_err());
        assert!(denied_writer.0.is_empty());
        let mut denied_reader = Cursor::new(wire.0.clone());
        assert!(read_guest_agent_hello_packet(
            &mut denied_reader,
            "invalid/guest",
            9,
            &hello.guest_sha256,
            arch,
        )
        .is_err());
        assert_eq!(
            denied_reader.position(),
            0,
            "host preflight must not read header"
        );

        for length in [0u32, WINDOWS_VM_AGENT_HELLO_MAX_BYTES as u32 + 1, u32::MAX] {
            let mut hostile = Cursor::new([length.to_be_bytes().as_slice(), b"NEXT"].concat());
            assert!(read_guest_agent_hello_packet(
                &mut hostile,
                &hello.guest_id,
                9,
                &hello.guest_sha256,
                arch,
            )
            .is_err());
            assert_eq!(hostile.position(), 4, "invalid length consumed payload");
        }

        for truncated in [&wire.0[..3], &wire.0[..wire.0.len() - 1]] {
            let mut stream = Cursor::new(truncated);
            assert!(read_guest_agent_hello_packet(
                &mut stream,
                &hello.guest_id,
                9,
                &hello.guest_sha256,
                arch,
            )
            .is_err());
        }
    }

    struct PartialThenDenied {
        writes: usize,
        bytes: Vec<u8>,
    }
    impl Write for PartialThenDenied {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            if self.writes > 1 {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            self.bytes.push(bytes[0]);
            Ok(1)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let hello = GuestAgentHello {
        schema: WINDOWS_VM_AGENT_SCHEMA.to_owned(),
        protocol_revision: 1,
        guest_id: "w8-harness".to_owned(),
        guest_revision: 9,
        guest_sha256: "a".repeat(64),
        workload_arch: "x86_64".to_owned(),
    };
    let mut broken = PartialThenDenied {
        writes: 0,
        bytes: Vec::new(),
    };
    assert!(matches!(write_guest_agent_hello_packet(
        &mut broken, &hello, &hello.guest_id, 9, &hello.guest_sha256, "x86_64",
    ), Err(WindowsVmError::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied));
    assert_eq!(
        broken.bytes.len(),
        1,
        "partial frame must not report success"
    );
}

#[test]
fn guest_agent_harness_packet_stream_two_packets_are_independently_admitted() {
    use primed::windows_vm::{
        read_guest_agent_hello_packet, write_guest_agent_hello_packet, GuestAgentHello,
        WINDOWS_VM_AGENT_SCHEMA,
    };
    use std::io::{self, Cursor, Read};

    // The caller owns stream framing. A fragmented, once-interrupted reader
    // must admit two distinct packets without borrowing the next frame's bytes.
    struct FragmentedInterruptedReader {
        inner: Cursor<Vec<u8>>,
        interrupt_once: bool,
    }
    impl Read for FragmentedInterruptedReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.interrupt_once {
                self.interrupt_once = false;
                return Err(io::ErrorKind::Interrupted.into());
            }
            let chunk = buf.len().min(1);
            self.inner.read(&mut buf[..chunk])
        }
    }

    let first = GuestAgentHello {
        schema: WINDOWS_VM_AGENT_SCHEMA.to_owned(),
        protocol_revision: 1,
        guest_id: "first-harness".to_owned(),
        guest_revision: 11,
        guest_sha256: "a".repeat(64),
        workload_arch: "x86_64".to_owned(),
    };
    let second = GuestAgentHello {
        guest_id: "second-harness".to_owned(),
        guest_revision: 12,
        guest_sha256: "b".repeat(64),
        workload_arch: "x86".to_owned(),
        ..first.clone()
    };
    let mut wire = Vec::new();
    let first_bytes = write_guest_agent_hello_packet(
        &mut wire,
        &first,
        &first.guest_id,
        first.guest_revision,
        &first.guest_sha256,
        &first.workload_arch,
    )
    .unwrap();
    let second_bytes = write_guest_agent_hello_packet(
        &mut wire,
        &second,
        &second.guest_id,
        second.guest_revision,
        &second.guest_sha256,
        &second.workload_arch,
    )
    .unwrap();
    assert_eq!(wire.len(), first_bytes + second_bytes);
    wire.extend_from_slice(b"TRAILER");
    let mut reader = FragmentedInterruptedReader {
        inner: Cursor::new(wire),
        interrupt_once: true,
    };
    assert_eq!(
        read_guest_agent_hello_packet(
            &mut reader,
            &first.guest_id,
            first.guest_revision,
            &first.guest_sha256,
            &first.workload_arch,
        )
        .unwrap(),
        first
    );
    assert_eq!(reader.inner.position() as usize, first_bytes);
    // Invalid caller authority must not consume the second packet's prefix.
    assert!(read_guest_agent_hello_packet(
        &mut reader,
        "invalid/host",
        second.guest_revision,
        &second.guest_sha256,
        &second.workload_arch,
    )
    .is_err());
    assert_eq!(reader.inner.position() as usize, first_bytes);
    assert_eq!(
        read_guest_agent_hello_packet(
            &mut reader,
            &second.guest_id,
            second.guest_revision,
            &second.guest_sha256,
            &second.workload_arch,
        )
        .unwrap(),
        second
    );
    assert_eq!(reader.inner.position() as usize, first_bytes + second_bytes);
    let mut trailer = [0u8; 7];
    reader.read_exact(&mut trailer).unwrap();
    assert_eq!(&trailer, b"TRAILER");
}

#[test]
fn guest_agent_harness_rejected_packet_preserves_following_frame() {
    use primed::windows_vm::{
        read_guest_agent_hello_packet, write_guest_agent_hello_packet, GuestAgentHello,
        WINDOWS_VM_AGENT_SCHEMA,
    };
    use std::io::{Cursor, Read};

    let authorized = GuestAgentHello {
        schema: WINDOWS_VM_AGENT_SCHEMA.to_owned(),
        protocol_revision: 1,
        guest_id: "authorized-harness".to_owned(),
        guest_revision: 13,
        guest_sha256: "a".repeat(64),
        workload_arch: "x86_64".to_owned(),
    };
    let wrong_identity = GuestAgentHello {
        guest_id: "untrusted-harness".to_owned(),
        ..authorized.clone()
    };
    // Both a structurally valid but unauthorized frame and malformed JSON
    // must be rejected without consuming the next independently framed hello.
    for (case, rejected) in [
        (
            "wrong identity",
            serde_json::to_vec(&wrong_identity).unwrap(),
        ),
        ("malformed JSON", b"{invalid".to_vec()),
    ] {
        let mut wire = Vec::new();
        wire.extend_from_slice(&(rejected.len() as u32).to_be_bytes());
        wire.extend_from_slice(&rejected);
        let next_frame_start = wire.len();
        let next_frame_len = write_guest_agent_hello_packet(
            &mut wire,
            &authorized,
            &authorized.guest_id,
            authorized.guest_revision,
            &authorized.guest_sha256,
            &authorized.workload_arch,
        )
        .unwrap();
        wire.extend_from_slice(b"TRAILER");

        let mut reader = Cursor::new(wire);
        assert!(
            read_guest_agent_hello_packet(
                &mut reader,
                &authorized.guest_id,
                authorized.guest_revision,
                &authorized.guest_sha256,
                &authorized.workload_arch,
            )
            .is_err(),
            "{case} must not be admitted"
        );
        assert_eq!(reader.position() as usize, next_frame_start, "{case}");
        assert_eq!(
            read_guest_agent_hello_packet(
                &mut reader,
                &authorized.guest_id,
                authorized.guest_revision,
                &authorized.guest_sha256,
                &authorized.workload_arch,
            )
            .unwrap(),
            authorized,
            "{case}"
        );
        assert_eq!(
            reader.position() as usize,
            next_frame_start + next_frame_len,
            "{case}"
        );
        let mut trailer = [0u8; 7];
        reader.read_exact(&mut trailer).unwrap();
        assert_eq!(&trailer, b"TRAILER", "{case}");
    }
}

#[test]
fn guest_agent_harness_maximum_length_packet_keeps_next_packet_isolated() {
    use primed::windows_vm::{
        read_guest_agent_hello_packet, write_guest_agent_hello_packet, GuestAgentHello,
        WINDOWS_VM_AGENT_HELLO_MAX_BYTES, WINDOWS_VM_AGENT_SCHEMA,
    };
    use std::io::{Cursor, Read};

    // The direct JSON parser's 8 KiB bound must also hold at the actual
    // length-prefixed stream boundary. A full-sized rejected frame may not
    // borrow any bytes from the next independent packet.
    for arch in ["x86", "x86_64"] {
        let hello = GuestAgentHello {
            schema: WINDOWS_VM_AGENT_SCHEMA.to_owned(),
            protocol_revision: 1,
            guest_id: "w8-boundary-harness".to_owned(),
            guest_revision: 17,
            guest_sha256: "c".repeat(64),
            workload_arch: arch.to_owned(),
        };
        let canonical = serde_json::to_vec(&hello).unwrap();
        assert!(canonical.len() < WINDOWS_VM_AGENT_HELLO_MAX_BYTES);
        let mut boundary = vec![b' '; WINDOWS_VM_AGENT_HELLO_MAX_BYTES - canonical.len()];
        boundary.extend_from_slice(&canonical);

        let mut wire = Vec::new();
        wire.extend_from_slice(&(boundary.len() as u32).to_be_bytes());
        wire.extend_from_slice(&boundary);
        let second_start = wire.len();
        let second_len = write_guest_agent_hello_packet(
            &mut wire,
            &hello,
            &hello.guest_id,
            hello.guest_revision,
            &hello.guest_sha256,
            arch,
        )
        .unwrap();
        wire.extend_from_slice(b"TRAILER");
        let mut stream = Cursor::new(wire);
        assert_eq!(
            read_guest_agent_hello_packet(
                &mut stream,
                &hello.guest_id,
                hello.guest_revision,
                &hello.guest_sha256,
                arch,
            )
            .unwrap(),
            hello,
        );
        assert_eq!(stream.position() as usize, second_start);
        assert_eq!(
            read_guest_agent_hello_packet(
                &mut stream,
                &hello.guest_id,
                hello.guest_revision,
                &hello.guest_sha256,
                arch,
            )
            .unwrap(),
            hello,
        );
        assert_eq!(stream.position() as usize, second_start + second_len);
        let mut trailer = [0u8; 7];
        stream.read_exact(&mut trailer).unwrap();
        assert_eq!(&trailer, b"TRAILER");

        let mut untrusted = hello.clone();
        untrusted.guest_id = "unauthorized-boundary".to_owned();
        let unauthorized_json = serde_json::to_vec(&untrusted).unwrap();
        let mut unauthorized =
            vec![b' '; WINDOWS_VM_AGENT_HELLO_MAX_BYTES - unauthorized_json.len()];
        unauthorized.extend_from_slice(&unauthorized_json);
        let mut rejected_wire = Vec::new();
        rejected_wire.extend_from_slice(&(unauthorized.len() as u32).to_be_bytes());
        rejected_wire.extend_from_slice(&unauthorized);
        let next_start = rejected_wire.len();
        write_guest_agent_hello_packet(
            &mut rejected_wire,
            &hello,
            &hello.guest_id,
            hello.guest_revision,
            &hello.guest_sha256,
            arch,
        )
        .unwrap();
        let mut rejected_stream = Cursor::new(rejected_wire);
        assert!(read_guest_agent_hello_packet(
            &mut rejected_stream,
            &hello.guest_id,
            hello.guest_revision,
            &hello.guest_sha256,
            arch,
        )
        .is_err());
        assert_eq!(rejected_stream.position() as usize, next_start);
        assert_eq!(
            read_guest_agent_hello_packet(
                &mut rejected_stream,
                &hello.guest_id,
                hello.guest_revision,
                &hello.guest_sha256,
                arch,
            )
            .unwrap(),
            hello,
        );

        // An oversized declaration is rejected after its prefix only. It is
        // not safe to infer a new frame boundary after a dishonest length.
        let mut oversized = Cursor::new(
            [
                (WINDOWS_VM_AGENT_HELLO_MAX_BYTES as u32 + 1)
                    .to_be_bytes()
                    .as_slice(),
                b"NEXT",
            ]
            .concat(),
        );
        assert!(read_guest_agent_hello_packet(
            &mut oversized,
            &hello.guest_id,
            hello.guest_revision,
            &hello.guest_sha256,
            arch,
        )
        .is_err());
        assert_eq!(oversized.position(), 4);
    }
}
