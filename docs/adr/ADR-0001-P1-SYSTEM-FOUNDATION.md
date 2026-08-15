# ADR-0001 — P1 First Light System Foundation

Status: **FROZEN FOR P1 IMPLEMENTATION**

Scope: close only the concrete P0 implementation choices needed to begin P1 First Light. This ADR does not change Prime/Origins/Ptah/Oracle ownership boundaries.

## Decision

### 1. P1 target

The first proof target is x86_64 UEFI on the HP 290 G4 Microtower recorded by Prime authority. All contracts carry architecture explicitly so this does not make Prime x86_64-only.

### 2. Linux/package substrate

P1 uses a **version-pinned Fedora Linux 44 package/kernel substrate** as a build input. Fedora is not Prime product identity and no Fedora Workstation/GNOME/KDE desktop session or Fedora visual branding is part of the Prime shell.

The exact base image is pinned by immutable OCI digest in the image build manifest before a proof image is accepted.

### 3. Canonical system image and updates

Prime P1 system generations are bootable OCI images consumed by **bootc**.

P1 uses bootc's **Composefs deployment backend** rather than its OSTree backend. This correction is required by the frozen systemd-boot/UKI architecture: current upstream bootc supports systemd-boot with Composefs, while systemd-boot is not supported by the OSTree backend. P1 is a lab/First-Light proof lane, so it may consume that explicitly experimental bootc backend only while its limitations remain visible and the exact bootc version is pinned/proven. Prime does not expose Composefs identifiers as Prime Host or generation identity.

Prime owns the higher-level generation state machine and policy. Prime does not fork or duplicate bootc's transaction/deployment engine.

P1 supports staging and generation identity. P1.5 owns the exhaustive failure/rollback campaign.

Automatic system updates are **disabled by default** in P1. An update becomes bootable only after Prime has recorded the candidate image digest and local verification result.

### 4. UEFI boot

P1 uses:

- UEFI;
- systemd-boot;
- Unified Kernel Images (UKI);
- Boot Loader Specification entries;
- boot-attempt counting/automatic boot assessment capability.

The P1 image follows bootc's Composefs + UKI path: the input image must provide systemd-boot and must not retain `bootupd`; the UKI is generated from the image root/kernel material rather than merely installing UKI tooling. Secure Boot signing is not a P1 First-Light completion requirement, but any unsigned P1 proof must report Secure Boot as disabled/unproven rather than claiming a signed chain of trust.

Prime's current, previous-known-good and recovery choices remain explicit Prime generation concepts even when the underlying boot/update machinery provides the deployment mechanics.

### 5. Init/service model

`systemd` is PID 1 and the P1 service manager.

Prime-owned long-lived components are ordinary hardened systemd services/sockets. `journald` is the initial mechanical service log transport. Prime does not invent a second init system.

P1 service set starts deliberately small:

- `primed` — Prime Core and Host authority;
- `prime-compositor` — Prime Wayland compositor/window authority;
- `prime-shell` — Prime Shell/Orb user experience;
- supporting one-shot/systemd units only where a separate daemon is not justified.

Subsystems may split into dedicated services later when isolation, privilege separation or lifecycle evidence demonstrates the need.

### 6. Prime Core implementation and IPC

Prime Core is Rust.

The P1 local machine interface is **HTTP/1.1 + JSON over an AF_UNIX socket** at `/run/prime/core.sock`.

Rules:

- no TCP listener in P1;
- filesystem ownership/mode plus Unix peer credentials are the first local authorization boundary;
- mutating operations require an explicit permission decision in Prime Core;
- version negotiation is part of the protocol;
- Origins later consumes this through an adapter; Prime Core does not expose Origins Node semantics;
- remote/distributed access is not a P1 Prime Core responsibility.

### 7. Base component/package model

The booted Prime base is image-owned.

- `/usr` and Prime base components come from the selected bootc image.
- Runtime mutation of the base with `dnf install` is not an ordinary Prime system-management path.
- Build-time RPM/DNF use is allowed inside the image pipeline.
- Optional SDKs/runtimes/applications remain separately activatable capabilities rather than permanently bloating the base.
- P1 installs only the components needed for First Light.

### 8. Persistent state and storage boundary

Machine identity and durable Prime state live under `/var/lib/prime`, not inside image-owned `/usr` and not solely in rollback-sensitive `/etc`.

P1 reserves these logical classes from the first image:

- EFI/boot material;
- image/system generations;
- persistent Prime Host state;
- user/project data;
- application/profile state;
- recovery state;
- logs/evidence;
- scratch/cache.

The physical partition sizing remains install-policy data rather than Host identity. The secondary SATA SSD is optional capacity, not a boot dependency.

### 9. Prime Shell/compositor

P1 uses a **custom Wayland compositor built in Rust on Smithay**. Prime owns window management, layout, system surfaces and visual behavior; Smithay supplies Wayland/system plumbing.

The Shell is not GNOME Shell, KWin/Plasma, COSMIC, or a themed downstream desktop.

Rich shell surfaces use React + TypeScript where suitable through a lightweight Rust native host backed by the system WebKit stack; no Electron/duplicated Chromium runtime is required for P1. The compositor remains the authority for privileged shell roles.

XWayland is optional compatibility plumbing and is not a P1 completion requirement.

### 10. P1 security mechanism baseline

All non-core workloads enter through Prime Workload Policy. Native P1 enforcement uses Linux primitives where applicable:

- systemd transient scopes + cgroup v2 for resource/process accounting;
- namespaces for isolation boundaries;
- seccomp for syscall reduction;
- Landlock for supported filesystem restriction;
- network namespaces/nftables for restricted network classes;
- explicit device and secret mediation.

If a requested policy cannot be enforced, Prime reports the limitation and fails the affected launch closed rather than pretending the policy is active.

## Rejected for P1

- building a second init/service manager;
- mutating a conventional distro install and calling it Prime;
- silently using bootc's GRUB/bootupd path while claiming the frozen systemd-boot architecture;
- GNOME/KDE/COSMIC as the Prime Shell;
- Electron as a mandatory shell runtime;
- GitHub/Oracle queue transport as Prime authority;
- Origins/Ptah semantics inside Prime Core;
- automatic APFS writes;
- full Grid-Knight integration.

## First implementation structure

```text
Cargo.toml
crates/
  prime-contracts/
  primed/
  prime-hardware/
  prime-exec/
  prime-policy/
  prime-storage/
  prime-compositor/
  prime-shell-host/
shell-ui/
image/
  Containerfile
  systemd/
  boot/
proof/
```

This is an ownership map, not a requirement that every crate become a daemon.

## P1 proof consequences

A P1 image does not pass because it compiles. It must prove, at minimum:

- boots through UEFI on the HP target;
- reports exact Prime Host and generation identity;
- reports a truthful hardware graph;
- exposes Capability Interface v1 locally;
- enforces at least one positive and one denied Workload Policy case;
- preserves Host/user state across a generation transition rehearsal;
- enters recovery without Prime Shell;
- presents unmistakable Prime Shell/Orb identity;
- records exact source/image/generation evidence.

P1.5 remains responsible for exhaustive transactional update/rollback failure proof.
