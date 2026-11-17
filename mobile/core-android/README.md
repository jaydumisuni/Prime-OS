# Prime Mobile — native Android Core transport adapter (MOB-4)

**Scope:** This is an Android/Bionic platform adapter built from the existing shared Prime contract and PHI crates. It is not a renamed Linux primed daemon, not an alternative authority, not bootc-qualified, not installed on the A145P, and not a flashable ROM.

## What is implemented

- Real Rust ARM64 Android target (aarch64-linux-android) with a separate native entrypoint.
- Imports the shared prime-contracts VersionsProjection, CapabilitiesProjection, CapabilityDescriptor, HardwareProjection, and prime-hardware::probe. No alternate capability schema or reasoning agent.
- Responds only to read-only GET /v1/versions, GET /v1/capabilities, and GET /v1/hardware over one Unix stream inherited from Android init.
- Enforces interface version 1.0 negotiation with HTTP 400/409 failure states and bounded input/output.
- Uses an explicitly unbound generation (ANDROID_UNBOUND_NOT_ATTESTED), nil unqualified host ID, degraded capability and unknown health. No image-generation, host-identity, bootc, TWRP or device-identity promotion.
- Refuses all POST/mutation operations, alternative listeners and public TCP. On Android, it requires an actual ANDROID_SOCKET_prime_mobile_core inherited file descriptor from Android init and verifies that the FD is a listening stream socket.
- The ordinary Android HOME client (PrimeCoreClient and AndroidPrimeCoreSocket) connects via Android's RESERVED namespace socket prime_mobile_core, asynchronously. It displays **Core unavailable** until a real bounded 1.0 contract response occurs; after that it shows **Core API N capabilities (unverified)**, never production-ready.
- No reasoner, Colibri, remote agent, systemd, or mandatory service in the HOME UI.

## Why not enabled in the A14m product yet

A full Android product integration must provide: (1) actual platform-signed/approved read-only native binary at the expected system path, (2) Android init service lifecycle with safe inherited stream socket, (3) correct enforcing SELinux file, domain, socket, app-client permissions and process UID isolation, (4) verified vendor/VINTF/partition compatibility and signed system image, (5) real Android framework/UI startup, and (6) phone recovery acceptance. An init .rc file or PRODUCT_PACKAGES entry without these would not make the service work.

Do not enable a world-writable Core socket, leave SELinux permissive, declare the ordinary HOME app privileged, grant unverified permissions, or transplant glibc primed/systemd-run/bootc unchanged. There is no fallback that silently disables AVB or alters recovery. A debug binary is not a phone-ready package.

## Build and evidence

CI .github/workflows/prime-mobile-android-core.yml runs the real shared-crate Rust tests from the Prime-OS workspace and produces a non-installable, internally retained ARM64/Bionic ELF ABI candidate using Android NDK 27.2.12479018 (Android 24 C API target). It verifies ELF Machine AArch64 and interpreter /system/bin/linker64. All dependencies are recorded in root Cargo.lock.

The Android HOME APK and JVM read-only client tests build in .github/workflows/prime-mobile-shell-android.yml independently.

The 2023 source donor VINTF is separately inspected by mobile/rom/aosp/a14m_vendor_interface.py: target-level=5, NDK 31, system-sdk 31, with declared historical camera/audio/graphics/radio/security HAL entries. These are XML declarations, not a running owner phone or a complete vendor image. A Google developer GSI may be used for the permitted developer-app compatibility checks only under its specific license; do not redistribute it as a Prime Mobile OS.

## Status

**Platform adapter source and tests:** under active draft review.

**Working Android init/SELinux deployment, whole AOSP system.img, correct vendor/HAL, Android boot, TWRP rollback, radio/touch/graphics and owner visual sign-off:** NOT QUALIFIED.

Continue all development in the existing Prime-OS repository. Draft branches remain unmerged. Do not ask the owner to download intermediate artifacts.
