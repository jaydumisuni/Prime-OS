# Prime Hardware Graph v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Schema identifier: `prime.hardware-graph.v1`

## Purpose

Prime Hardware Graph is Host-local mechanical inventory truth. It describes what the running Linux kernel and firmware interfaces expose; it is not an Origins Node, a Ptah Node, a cybersecurity verdict, or a vendor-support promise.

## Source rule

P1 probes Linux kernel/firmware interfaces directly and read-only. Primary sources include:

- `/proc/cpuinfo` and `/proc/meminfo` for CPU/memory observations;
- `/sys/class/dmi/id` for sanitized SMBIOS/DMI system descriptors and private fingerprint inputs;
- `/sys/bus/pci/devices` for PCI functions;
- `/sys/bus/usb/devices` for USB devices;
- `/sys/class/block` for kernel block devices;
- `/sys/class/net` for network interfaces;
- `/sys/class/drm`, `/sys/class/input`, `/sys/class/sound`, `/sys/class/thermal`, and `/sys/class/tpm` where present;
- `/sys/firmware/efi` for UEFI presence;
- `/dev/kvm` plus CPU virtualization flags for the initial virtualization observation.

A missing family is reported as a limitation. Prime does not invent devices to satisfy the HP proof-host specification.

## Graph record

```json
{
  "schema": "prime.hardware-graph.v1",
  "revision": 1,
  "topology_digest": "sha256:...",
  "observed_at": "RFC3339",
  "inventory": {},
  "limitations": []
}
```

The graph is persisted under `/var/lib/prime/hardware/current.json`.

`revision` starts at 1 and advances only when the normalized topology/probe-result digest changes. Re-observing the same topology does not create a fake hardware revision.

## Normalized inventory

P1 records, where mechanically available:

- host architecture;
- sanitized system/board descriptors;
- UEFI and BIOS descriptors;
- logical CPU count, CPU vendor/model, VT-x/AMD-V flag observations;
- total physical memory observation;
- PCI address/vendor/device/class/subsystem/driver;
- USB path/vendor/product/class/manufacturer/product/speed without USB serial numbers;
- block-device kernel name/type/capacity/read-only/removable/rotational/block-size/vendor/model without storage serial numbers;
- network interface name/type/driver/wireless classification without exporting raw MAC addresses;
- DRM connector names/current connection state/modes;
- input-device names;
- sound-card identifiers;
- thermal-zone types;
- TPM device presence;
- `/dev/kvm` presence.

Dynamic performance/temperature/network-health telemetry is not folded into the topology digest merely to make revisions move.

## Privacy / identity boundary

Raw SMBIOS UUIDs and hardware serial numbers are privileged fingerprint inputs and are **not serialized into the public Hardware Graph**.

Prime derives a versioned SHA-256 Host hardware fingerprint from meaningful stable DMI identity evidence. A high/medium-confidence fingerprint may be enrolled into Prime Host Identity. A changed enrolled fingerprint fails closed for migration/rebind handling; Prime does not silently carry the old Host ID onto materially different hardware.

Low-confidence model-only evidence is never sufficient by itself to rewrite an enrolled Host fingerprint.

Disk, NIC, GPU and removable-device identities are inventory evidence, not Prime Host identity.

## Kernel-path rule

Prime treats sysfs bus/class entries as kernel projections and does not make their symlink text a durable identity. Normalized bus addresses/kernel names are recorded instead.

## Degraded truth

Probe gaps are explicit `limitations`. A partial graph is allowed so recovery remains diagnosable, but Prime health/capability projection reports degradation rather than claiming complete hardware truth.

## Generic discovery vs P1 proof-host acceptance

The reusable `prime-hardware` scanner remains generic Linux discovery. It contains no branch that pretends every machine is the HP proof target.

P1-specific acceptance is evaluated separately by `primed::hardware::p1_baseline_limitations`. That function consumes the already-normalized `prime.hardware-graph.v1` record and returns an empty limitation set only when the frozen First-Light proof-host baseline is mechanically present.

This separation is permanent in principle:

```text
Linux/kernel discovery
        ↓
prime.hardware-graph.v1
        ↓
phase/product acceptance evaluator
```

A future Prime Host may be supported without satisfying the HP 290 G4 **P1 proof fixture**. Conversely, detecting an HP model string alone never proves P1 hardware acceptance.

## P1 proof target

For the frozen HP 290 G4 / Kratos First-Light target, the current P1 baseline evaluator requires:

- `x86_64` Host architecture;
- DMI vendor `HP`;
- DMI product `HP 290 G4 Microtower PC`;
- UEFI boot state;
- CPU vendor `GenuineIntel`;
- CPU model containing `i7-10700`;
- at least 8,000,000,000 bytes of observed physical memory;
- an Intel (`0x8086`) PCI `DISPLAY` function bound to the `i915` kernel driver;
- a PCI USB controller (`0x0c03xx`) with a bound kernel driver;
- at least one connected DRM connector with at least one advertised mode;
- at least one discovered input device;
- at least one discovered sound card;
- at least one non-wireless Ethernet interface with a bound kernel driver;
- one writable, non-removable disk of at least 900,000,000,000 bytes;
- a second writable, non-removable disk of at least 450,000,000,000 bytes;
- no unresolved generic Hardware Graph probe limitations.

The disk thresholds intentionally validate the frozen approximately 1 TB + 500 GB proof-host topology without depending on model/serial strings. Storage serials remain private/not exported, and the evaluator does not require a brand label to masquerade as mechanical disk identity.

The graphics gate intentionally requires the active Linux `i915` binding rather than merely seeing an Intel PCI ID. A display controller that exists but has no usable P1 driver is not accepted.

The USB discovery gate intentionally checks the controller/driver path rather than requiring a peripheral to happen to be attached during startup. Actual USB hotplug/device operation remains a live behavioral proof obligation.

The Ethernet, input, audio and connected-output requirements are discovery/driver-presence gates. Functional transfer, audio playback, pointer/keyboard interaction, USB hotplug and compositor rendering still require live P1 proof; inventory presence alone is not promoted into behavioral success.

## Generation-health relationship

The eventual P1 `prime.generation-health.v1.hardware_baseline_ready` field may be `true` only when this P1 baseline evaluator returns no limitations for the exact booted Host graph used by that health campaign.

Hosted QEMU evidence is useful engineering proof but is not expected to satisfy the HP/Kratos baseline and therefore cannot by itself promote a P1 physical generation to `KNOWN_GOOD`.
