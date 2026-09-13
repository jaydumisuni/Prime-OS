# Prime Windows Personality W6 Implementation Plan

**Goal:** Prove a supported Windows service lifecycle on x64 and WoW64 using the existing Prime Wine provider and application-scoped prefixes.

1. Bind W6 to frozen W5 `706e90a7d93b527fdeba0015c19f4f4a7cbb365c` and record SCM donor closure.
2. Add hash-bound x64/x86 service and controller fixtures.
3. Prove provider-routed create/start/query/stop/delete for both architectures.
4. Prove cross-application service registration isolation.
5. Inject killed-service, stale registration, missing binary and disabled-service failures; prove deterministic recovery or fail-closed behavior.
6. Run repeated lifecycle/adversarial cleanup and zero-leak checks.
7. Run prefreeze regressions, Sergeant, exact frozen runtime replay, Formula, push and remote SHA binding.

Full physical Prime boot remains deferred.
