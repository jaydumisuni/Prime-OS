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

## P1 proof target

The HP 290 G4 proof must independently demonstrate at least the expected i7-10700 CPU family/model observation, memory, Intel UHD 630 PCI display function, Samsung NVMe, Crucial SATA storage, Ethernet, USB/input, audio path, UEFI state and virtualization capability where the host exposes them.

The implementation remains generic Linux hardware discovery; HP-specific constants are proof expectations, never probe logic.
