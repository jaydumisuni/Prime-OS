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
  "profile_digest": "sha256:...",
  "display_name": "string",
  "artifact": {
    "identity": "sha256:...",
    "format": "ELF|PE32|PE32+|JAR|CLASS|APK|DEX|WASM|MACHO|APP_BUNDLE|IPA|OTHER",
    "runtime_family": "NATIVE_LINUX|WINDOWS|JVM|ANDROID|WASM|DARWIN|IOS|OTHER",
    "workload_arch": "string-or-null"
  },
  "execution_backend": "NATIVE|PERSONALITY|CONTAINER|VM|REMOTE_PROVIDER|SPECIALIZED_PROVIDER",
  "dependencies": [],
  "workload_policy": {
    "policy_id": "uuid",
    "policy_revision": 1,
    "policy_digest": "sha256:..."
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

## Digest rule

For v1, `profile_digest` is SHA-256 over the compact UTF-8 JSON serialization of the typed profile with `profile_digest` set to the empty string. Struct field order and list order are therefore part of the v1 canonical encoding.

The stored profile must re-compute to its recorded digest before use. Digest mismatch fails closed.

## Persistence

P1 durable layout:

```text
/var/lib/prime/applications/<application_id>/
  revisions/<20-digit-revision>.json
  selected
```

- revision files are create-once and never overwritten;
- `selected` contains only the selected decimal revision and is atomically replaced;
- selecting a missing, corrupt, revoked or digest-invalid revision fails closed;
- a newer revision does not mutate a running workload bound to an older revision.

## Revision rules

- revisions start at 1 and are append-only;
- a changed semantic field creates a new revision and digest;
- existing running workloads remain bound to their launch revision;
- ordinary new launches use the explicitly selected non-revoked revision;
- schema migration preserves old data and must be rollback-aware.

The registry does not infer the next revision by overwriting history. Prime Core owns revision allocation/authorization when mutation routes are activated.

## Revocation

Revocation is explicit and audited. A critical security revocation may block new launches and may request suspension/termination of running workloads, but the action still goes through Prime authorization and Workload Policy. History remains retained.

## P1 scope

P1 implements the registry and native/Linux profile path. Recognition of other formats may be present in Prime Exec without claiming those runtimes are executable.
