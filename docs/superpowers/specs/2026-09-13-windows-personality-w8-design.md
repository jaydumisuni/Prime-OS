# Prime Windows Personality W8 — VM Fallback Design

## Scope

W8 adds an explicit VM execution lane for Windows workloads whose required semantics cannot be safely provided by the W1–W7 Wine personality. Frozen predecessor: `8115b4c0a85610920febcb3153ee861bf0a7665a` (W7). W8 does not replace Wine and does not silently retry failed Wine launches inside a VM.

## Decision

Use the existing `ExecutionBackend::Vm` authority as the only selector for the fallback lane. `PERSONALITY` continues to mean the proven Wine path; `VM` means the workload must be handed to a Prime-owned QEMU/KVM adapter. Selection is profile authority, not runtime guesswork.

The VM adapter must consume only an immutable, digest-bound guest definition from Prime state. It must not accept arbitrary host block devices, host networking, host filesystem passthrough, or broad `/dev` access. USB passthrough reuses W7 concrete `/dev/bus/usb/BBB/DDD` allowlists and is opt-in per workload. Missing KVM/QEMU/guest authority fails closed before launch.

A guest agent protocol will be the Windows workload handoff boundary. Host proof may certify orchestration, authority validation, lifecycle, and isolation with a minimal deterministic guest harness; actual Windows guest workload execution requires an authorized Windows guest image and remains a separate W8 gate rather than being inferred from QEMU startup.

## Rejected alternatives

1. **Automatic Wine→VM retry:** rejected because a Wine failure is not proof that a VM is authorized, and automatic retry would blur evidence and policy boundaries.
2. **Make VM another ordinary Windows provider manifest:** rejected because the current provider registry selects by PE format/arch and has no explicit backend authority; lexicographic provider selection would make fallback nondeterministic.
3. **Explicit `ExecutionBackend::Vm` lane:** selected because the contract already exists and keeps fallback opt-in, auditable, and mechanically distinguishable from the W1–W7 personality.

## Host-provable benchmark gates

1. Preserve exact shipped W7 predecessor authority.
2. Prove KRATOS virtualization donor closure: QEMU x86_64 plus usable KVM, with exact executable identity recorded.
3. Prove backend separation: `PERSONALITY` remains Wine; only `VM` profiles enter the VM lane; no automatic Wine→VM retry exists.
4. Prove immutable guest authority validation: regular non-symlink guest artifact, digest match, bounded resources, and fail-closed missing/tampered definitions.
5. Prove safe QEMU plan generation: KVM acceleration, isolated display/control channels, no host network by default, no arbitrary disks/filesystems, and only explicit W7 USB nodes when requested.
6. Prove deterministic guest-agent handshake and x64/x86 Windows workload handoff contract using a non-production minimal guest harness if an authorized Windows guest image is not yet present. This gate does not claim Windows application compatibility by itself.
7. Prove authorized Windows guest execution when a digest-bound Windows guest image is available; otherwise this gate remains the first irreducible external-image blocker and W8 cannot ship.
8. Prove concurrent VM isolation and per-application state separation.
9. Prove lifecycle/adversarial recovery: timeout/kill/corrupt authority/denied USB leave zero QEMU/helper processes and never broaden access.
10. Freeze only after source-clean, diff/fmt/clippy/workspace and W1–W8 regressions, current Sergeant approval, exact frozen-SHA runtime rerun, Formula PASS, push, and remote SHA binding.

## Safety

No physical disk passthrough, raw host storage, bridge/TAP networking, arbitrary host filesystem sharing, USB reset/firmware mutation, or implicit device capture is part of host certification.

## Deferred

Full sealed-image Prime physical boot remains deferred. W9 TTG workload certification starts only after W8 is legitimately shipped.
