# Prime Windows Personality W6 — Supported Windows Services Design

## Scope

W6 proves a bounded Windows Service Control Manager subset through the existing Prime Wine personality. Frozen predecessor: `706e90a7d93b527fdeba0015c19f4f4a7cbb365c` (W5). Prime does not add a second service manager. Application-scoped Wine prefixes remain the isolation boundary.

## Decision

Reuse Wine 11 SCM (`services.exe`, `sc.exe`, advapi32 service APIs) and prove lifecycle through the real Prime provider. Supported service behavior is install/create, start, query running, stop, query stopped, delete, isolated registration, deterministic recovery, and clean process drain. Drivers/kernel services remain out of scope for W6 and belong to later device/VM work.

## Host-provable benchmark gates

1. Preserve exact frozen W5 predecessor authority.
2. Prove sealed-image SCM donor closure for x64 and WoW64.
3. Prove x64 provider-routed service create/start/query/stop/delete.
4. Prove x86/WoW64 provider-routed service create/start/query/stop/delete.
5. Prove service registration is application-prefix isolated.
6. Prove killed-service recovery and restart behavior.
7. Prove stale/missing binary and disabled-service failures are explicit and fail closed.
8. Prove repeated lifecycle and cleanup with no cross-app leakage.
9. Run adversarial regression and prove zero leaked Wine/services processes after drain.
10. Freeze only after source-clean, diff, fmt, clippy, workspace and W1-W6 regressions, current Sergeant approval, exact frozen-SHA runtime replay, Formula PASS, push, and remote binding.

## Proof fixtures

Use a hash-bound native service executable implementing `StartServiceCtrlDispatcher`, `RegisterServiceCtrlHandlerEx`, RUNNING and STOPPED transitions, plus a controller fixture using SCM APIs. Build x64 and x86 outside the product image. The fixture is proof surface only; production routing remains the existing Prime Wine provider.

## Failure policy

Missing donor binaries, architecture mismatch, SCM registration escape, cross-application leakage, failed state transitions, unrecoverable service termination, stale registration, or residual Wine/service processes fails the corresponding gate. No silent fallback may promote W6.

## Deferred

Full physical Prime boot integration remains deferred. W7 USB/device integration, W8 VM fallback, and W9 TTG workload certification remain separate milestones.
