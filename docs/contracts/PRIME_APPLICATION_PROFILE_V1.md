# Prime Application Profile v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Schema identifier: `prime.application-profile.v1`

## Rule

A launch pins one immutable Application Profile revision. Prime never silently changes policy/backend semantics underneath a running workload.

## Required fields

```json
{
  "schema": "prime.application-profile.v1",
  "application_id": "uuid",
  "profile_revision": 1,
  "profile_digest": "sha256",
  "display_name": "string",
  "artifact": {
    "identity": "string",
    "digest": "sha256-or-null",
    "format": "ELF|PE32|PE32+|JAR|CLASS|APK|DEX|WASM|MACHO|APP_BUNDLE|IPA|OTHER",
    "runtime_family": "NATIVE_LINUX|WINDOWS|JVM|ANDROID|WASM|DARWIN|IOS|OTHER",
    "workload_arch": "string"
  },
  "execution_backend": "NATIVE|PERSONALITY|CONTAINER|VM|REMOTE_PROVIDER|SPECIALIZED_PROVIDER",
  "dependencies": [],
  "workload_policy": {
    "policy_id": "uuid",
    "policy_revision": 1,
    "policy_digest": "sha256"
  },
  "permissions": [],
  "compatibility": {
    "state": "UNKNOWN|RECOGNIZED|INSTALLABLE|LAUNCHES|PARTIALLY_FUNCTIONAL|FUNCTIONAL|BROKEN|UNSUPPORTED|REQUIRES_VM|REQUIRES_REMOTE_PROVIDER",
    "evidence_refs": []
  },
  "revoked": false,
  "revocation_reason": null,
  "created_at": "RFC3339"
}
```

## Revision rules

- revisions are append-only;
- a changed semantic field creates a new revision and digest;
- existing running workloads remain bound to their launch revision;
- ordinary new launches use the currently selected non-revoked revision;
- schema migration preserves old data and must be rollback-aware.

## Revocation

Revocation is explicit and audited. A critical security revocation may block new launches and may request suspension/termination of running workloads, but the action still goes through Prime authorization and Workload Policy. History remains retained.

## P1 scope

P1 must implement the registry and native/Linux profile path. Recognition of other formats may be present in Prime Exec without claiming those runtimes are executable.
