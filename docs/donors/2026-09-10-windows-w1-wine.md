# Windows Personality W1 donor decision — Wine

**Date:** 2026-09-10  
**Prime scope:** P4A / W1 portable-simple Win32  
**Decision:** use Fedora 44 `wine-core-11.0-3.fc44` as the first implementation donor behind a Prime-owned provider adapter.

## Authority boundary

Wine is an **implementation donor**, not the Prime Windows architecture and not a Prime-facing execution backend. Prime remains authority for PE inspection, Application Profile selection, Workload Policy enforcement, provider admission, launch supervision, display authorization, evidence and failure semantics.

The public Prime route remains:

```text
PE32 / PE32+
  -> Prime Exec
  -> Application Profile
  -> PERSONALITY / WINDOWS
  -> prime.windows.w1.default
  -> /usr/libexec/prime/prime-windows-provider-w1
  -> implementation donor
```

No Application Profile, Shell request, capability consumer, or normal user action needs to name or launch Wine directly. The provider ID and installed adapter path are deliberately donor-neutral so a future donor can replace or supplement this implementation without changing Prime's execution contract.

## Why this donor is sufficient for W1

Fedora 44 publishes `wine-core-11.0-3.fc44` for x86_64. The package provides `/usr/bin/wine`, `/usr/bin/wine64`, `wine-wow64`, PE runtime modules and Wayland client dependencies. That fits the W1 requirement to attempt portable/simple PE32 x86 and PE32+ x86_64 workloads on an x86_64 Prime Host.

W1 does **not** infer support for installers, .NET, DirectX acceleration, COM completeness, Windows services, USB/device passthrough or VM fallback. Those remain W2-W8 obligations.

## Adapter isolation

The Prime adapter:

- accepts only the frozen Prime provider ABI;
- accepts absolute regular non-symlink PE artifacts;
- limits W1 to x86/x86_64 PE32/PE32+;
- requires `/run/prime-win-<launch-id>` runtime-state binding;
- uses a per-launch prefix/home/cache/config tree;
- clears inherited environment before invoking the donor;
- invokes only the fixed packaged `/usr/bin/wine` path with direct argv, never a shell;
- consumes the Prime compositor readiness file only to discover a safe Wayland socket name;
- does not grant `prime-shell` Core authority.

## Package evidence

Fedora package authority checked on 2026-09-10:

- `https://packages.fedoraproject.org/pkgs/wine/wine-core/fedora-44.html`
- Fedora 44 package version: `wine-core-11.0-3.fc44`
- package file list includes `/usr/bin/wine` and `/usr/bin/wine64`; package provides `wine-wow64`.

## Replacement rule

A later ReactOS-derived, Wine-derived, hybrid, VM, or other provider may be introduced only behind the same or a versioned Prime provider contract. Donor replacement must not silently mutate an existing immutable Application Profile revision or falsify earlier launch evidence.
