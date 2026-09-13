# Prime Windows Personality W7 — USB/Device Integration Design

## Scope

W7 adds explicit, policy-bound USB device access to the existing Prime Wine personality. Frozen predecessor: `042841390f1b0456bf1da125350653766523bd36` (W6). Prime reuses Linux USB ownership plus Wine 11 `winebus`/`wineusb`; it does not add a second USB stack.

## Decision

Treat `WorkloadPolicy.devices.usb` as an allowlist of concrete absolute `/dev/bus/usb/BBB/DDD` nodes resolved by Prime before launch. The systemd enforcement plan must use `DevicePolicy=closed` plus one read/write `DeviceAllow=` entry per admitted USB node. Empty device policy retains `PrivateDevices=yes`. Non-USB paths, malformed bus/device names, duplicates, and broad device access fail closed. Wine remains the Windows user-mode bridge.

## Host-provable benchmark gates

1. Preserve exact frozen W6 predecessor authority.
2. Prove sealed-image donor closure for Wine USB bus support and libusb.
3. Prove policy compiler admits only strict `/dev/bus/usb/BBB/DDD` allowlist entries and emits closed systemd device mediation.
4. Prove x64 provider-routed Windows enumeration can identify an allowlisted physical KRATOS USB device.
5. Prove x86/WoW64 provider-routed enumeration can identify the same allowlisted device.
6. Prove empty/denied/malformed device policies fail closed and do not expose the target USB device.
7. Prove missing-device/re-enumeration behavior is explicit without broad fallback.
8. Prove per-application device allowlists are isolated across independent Wine prefixes.
9. Run repeated lifecycle/adversarial regression and prove zero leaked Wine/device-helper processes after drain.
10. Freeze only after source-clean, diff, fmt, clippy, workspace and W1-W7 regressions, current Sergeant approval, exact frozen-SHA runtime replay, Formula PASS, push, and remote binding.

## Safety

W7 proof is enumeration/read-only. No USB control transfer, firmware mutation, reset, detach, interface claim, storage write, or phone/router service action is part of certification.

## Deferred

Kernel-mode Windows USB drivers and unsupported device stacks belong to W8 VM fallback. Full sealed-image Prime physical boot integration remains deferred. W9 TTG workload certification remains separate.
