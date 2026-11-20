# Prime Mobile — Shared Rust Core ARM64 Gap Audit (Source-Based)

Date: 2026-11-15. Status: **candidate engineering review**, not merged/frozen or a declaration that the same Core already boots on ARM64. Basis: **exact Prime-OS main source**, read on 2026-11-15, before the draft MOB-1 branch. No speculative vendor driver acceptance.

## Findings recovered from existing implementation

| Observation | Exact source location | Consequence for ARM64 and ownership |
| --- | --- | --- |
| Prime Host and Core are architecturally host-local; no compile-time x86 gate is evident in the main entry point | \`crates/primed/src/main.rs:6-89\`; \`crates/prime-contracts/src/lib.rs\` | The shared Rust code is a plausible cross-build target; **not yet compiled or executed** with an aarch64 Rust toolchain. |
| Main Core assumes \`/usr/sbin/systemd-run\`, \`/usr/sbin/bootc\`, persistent state directories, \`/proc/self/mountinfo\`, generation seed and local socket | \`crates/primed/src/main.rs:8-43\` | A real ARM64 Linux guest requires an actual service manager/generation environment or a separately authorized adaptation. An early \`/init\` marker alone is insufficient. |
| Host arch is actual Rust \`std::env::consts::ARCH\` | \`crates/primed/src/identity.rs:43-55\` | No need for a second Mobile Host ID model. Maintain the same Prime Host Identity authority. |
| Hardware fingerprint currently uses Linux DMI system/board descriptors and private DMI identifiers, with LOW/UNPROBED when weak/missing | \`crates/prime-hardware/src/lib.rs:81-163\` | Phones often lack DMI. Mobile needs a reviewed stable, privacy-preserving enrollment evidence route. **Do not derive host identity from IMEI/ICCID or untrusted user-settable device strings**. |
| Identity reconciliation only enrolls HIGH or MEDIUM confidence hardware fingerprints | \`crates/primed/src/identity.rs:99-129\` | No silent fallback from absent mobile DMI to a fabricated enrolled identity. Unsupported device remains uncertain until reviewed identity evidence exists. |
| Generic CPU parser uses x86-specific \`vendor_id\`, \`model name\`, \`flags\`/VMX/SVM, though it counts \`processor\` entries | \`crates/prime-hardware/src/lib.rs:175-205\` | ARM64 CPU vendor/model/ISA and clusters need a dedicated observed-source projection; should not pretend MediaTek/ARM cores are an Intel CPU. Preserve already-frozen x86 semantics. |
| Generic hardware scanner expects to enumerate PCI and reports unavailability as a limitation | \`crates/prime-hardware/src/lib.rs:222-224\`; \`390-396\` | Typical phone SoC peripherals may be platform devices, not PCI; a genuine absent PCI bus should not automatically mean unsupported mobile hardware, nor be hidden as though inventory succeeded. Requires additive compatibility/limitations design. |
| UEFI is a boolean measured at \`/sys/firmware/efi\`; firmware descriptors otherwise come from DMI | \`crates/prime-hardware/src/lib.rs:165-173\` | Android/vendor phone boot is not UEFI. A valid Mobile boot path requires a distinct qualified boot-chain description, not a fake UEFI flag. |
| Frozen P1 baseline requires x86_64, specific HP system, Intel i7, i915 PCI GPU, PCI USB, Ethernet, UEFI and large local disks | \`crates/primed/src/hardware.rs:63-194\` | **Never invoke Desktop P1 as Mobile hardware acceptance**. Preserve P1 for KRATOS. Create a separately scoped Mobile hardware/telephony baseline on actual board evidence. |
| \`p1_health::build_report\` unconditionally calls that Desktop P1 baseline, and requires HIGH/MEDIUM Host fingerprint | \`crates/primed/src/p1_health.rs:7-55\`; \`59-101\` | Mobile First Light must not satisfy or change this proof gate; it requires a product-specific **MOB** health evaluator, with real Core/shell/recovery and essential phone proof. |
| Existing \`/v1/hardware\` and version-negotiated capability endpoints remain Prime Core authority | \`crates/primed/src/server.rs:416-430\`; \`docs/contracts/PRIME_CAPABILITY_INTERFACE_V1.md\` | PHI/Mobile should extend existing versioned API after review, not expose a second unguarded hardware or mutation service. |

## No duplication rule

Already documented — No update required for existing \`prime.hardware-graph.v1\`, \`prime.capability.v1\`, Workload Policy v1, Host Identity v1, Desk P1 baseline and iAM evidence authority. The **missing** Mobile integration is a small, typed ARM64/Device Tree/vendor-bus observation pathway and an independent product-acceptance profile. The Python read-only PHI/board probes in Prime draft PR #223 are candidate evidence collectors, not new permanent Core semantics or full iAM runtime. Candidate ARM64 \`/init\` in PR #224 is a boot-preflight artifact, not a second Rust Core.

## Proposed order, each with a separate proof gate

1. **Compile shared Rust crates first:** obtain reviewed AArch64 Rust toolchain and target; independently cross-compile \`prime-contracts\`, \`prime-hardware\` and \`primed\`, checking actual dependencies/links and target architecture. A host-side compiler success proves compilation only.
2. **Prove generic Linux guest First Light:** reuse TTG-Virtual-Hardware-Lab pinned ARM64 QEMU, a reviewed ARM64 Linux kernel Image and the PR #224 \`/init\` archive. Capture exact guest UART/process evidence; no Samsung hardware claim.
3. **Actual Core startup under qualified guest userspace:** ensure \`systemd-run\`, \`bootc\`/approved generation alternative, writable state, local sockets, mount namespaces, storage and recovery are real, not fixture strings. Prove the frozen \`prime.capability.v1\` handshake.
4. **Design additive Mobile Hardware Graph coverage:** Device Tree/ACPI where present, platform bus, ARM64 CPU topology, vendor kernel/driver/firmware, DRM/touch/modem/power/USB, real capability health and explicit UNKNOWN. Privacy, trust and backward compatibility must be independently reviewed before freezing an API or changing \`prime-hardware\`.
5. **Separate Mobile health and recovery:** family-bound hardware baseline; no literal reuse of the HP/Intel P1 proof-host evaluator. Never treat a shell prompt or a recognized peripheral as a fully usable phone.
6. **PHI resource adaptation:** carry only qualified Linux memory/thermal/battery/accelerator observations into existing Workload Policy; no model, D-XQT or Colibri required for ordinary boot.
7. **First physical handset admission:** read-only device identity, boot chain, partition map, kernel/DT, stock-restoration evidence, explicit owner authorization, and only then consider a reversible test. No fake daily-driver certification.

## Current status

- **Done this session:** reviewed exact Rust entry, identity, hardware and P1 health code; compiled a real AArch64 diagnostic \`/init\` using Clang/LLD; repeated byte-identical build; host-side 17/17 tests; source/receipt preserved in PR #224.
- **Not done:** Rust compilation for AArch64, qemu-system-aarch64 execution, real Linux guest kernel boot, Core or Shell readiness, actual device/driver/telephony proof, signed image or rollback.
- **Engineering disposition:** **Update existing documentation** through this scoped gap audit. Do **not** alter frozen Core contracts or production code from assumptions. Review this audit against current source before scheduling any Rust/ARM64 changes.

Related:
- [Prime MOB-1 draft PR #224](https://github.com/jaydumisuni/Prime-OS/pull/224)
- [Prime MOB-0 / PHI draft PR #223](https://github.com/jaydumisuni/Prime-OS/pull/223)
- [TTG VLab G0.7 ARM64](https://github.com/jaydumisuni/TTG-Virtual-Hardware-Lab/blob/main/docs/G07_QEMU_GENERIC_ARM_EXECUTION.md)
