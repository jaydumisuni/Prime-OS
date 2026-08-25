# Prime Exec v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Inspection schema: `prime.exec-inspection.v1`

## Authority

Prime Exec answers mechanical executability questions. It does not perform mission composition, AgentOps lifecycle, CodeOps review or Origins Node placement.

Recognition is not execution support.

## P1 inspection

P1 accepts one local regular-file artifact and produces a content-bound inspection from bytes actually read from that file.

Prime Exec v1 recognizes, where mechanically identifiable:

- ELF;
- PE32 / PE32+;
- JVM `.class`;
- JAR;
- APK;
- DEX;
- WASM;
- Mach-O;
- IPA;
- otherwise `OTHER`.

`.app` directories and nested package semantics are deferred; P1 must not pretend a directory was fully inspected as an executable artifact.

## Inspection record

```json
{
  "schema": "prime.exec-inspection.v1",
  "artifact_identity": "sha256:...",
  "size_bytes": 1234,
  "format": "ELF",
  "runtime_family": "NATIVE_LINUX",
  "workload_arch": "x86_64",
  "suggested_backend": "NATIVE",
  "native_compatible": true,
  "limitations": []
}
```

`artifact_identity` is the SHA-256 of the bytes read. The inspection does not expose the source path as portable artifact identity.

## File safety

P1 inspection:

- rejects symlink inputs rather than silently following a moving target;
- rejects non-regular files;
- hashes the entire artifact;
- classifies from bytes captured during the same read pass where possible;
- compares file metadata before/after inspection and rejects an artifact observed changing during inspection;
- does not execute, load or shell-evaluate the artifact.

A later launch must re-bind/revalidate the exact artifact identity. Inspection evidence alone is not an authorization to execute.

## Architecture truth

P1 maps common machine identifiers for x86, x86_64, ARM, AArch64 and RISC-V where present in the artifact format.

An ELF artifact is only a P1 `NATIVE` candidate when:

- its workload architecture exactly matches the Prime Host architecture;
- it is an executable regular file;
- no unsupported format condition prevents safe classification.

P1 does not silently claim 32-bit compatibility, instruction translation or foreign ABI support.

## Backend truth

P1 may recognize Windows/JVM/Android/WASM/Darwin/iOS artifacts before their runtime capability exists. Such an inspection reports the runtime family but leaves the execution backend unavailable/unselected unless a real Host capability exists.

## P1 nonclaims

Inspection does not yet prove:

- dynamic dependency closure;
- runtime API compatibility;
- signing/trust validity;
- application functionality;
- container/VM/remote-provider availability;
- foreign-platform execution.

Those truths belong to later Prime Exec/runtime capability stages and Application Profile evidence.
