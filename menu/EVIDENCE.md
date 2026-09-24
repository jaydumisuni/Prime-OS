# Prime OS Apps Menu — Evidence

Date: 2026-09-25
Status: **WORKING / FROZEN UI PROTOTYPE / SYNCED TO PRIME**

## Authority and scope

Repository: `jaydumisuni/Prime-OS`
Owning folder: `menu/`

Prime's repository policy was recovered before the change:

- `.ttg/project-policy.yaml` requires `ttg.tenfold.v1`.
- This sync is an atomic, bounded artifact/documentation change.
- The canonical engineering cycle remains Understand → Build → Review → Freeze → Prove → Ship.

No existing `menu/` folder was present on `main`, so this folder was created specifically for the approved Prime application-menu reference.

## Frozen artifact

Path:

`menu/Prime_OS_Apps_Menu_Standalone.html`

Repository artifact facts recovered from KRATOS after fetching `origin/main`:

- File size: **3,308,845 bytes**
- SHA-256: `54963af2ef3ce859a5911725040b8b4fb130a19cfde247ac69ec5539c37d5059`
- Git blob: `941a5f75ffef873ed9db2081bfdd2a553d4ff72e`
- Embedded image payloads: **10**
- External HTTP asset references: **none**
- Inline script blocks: **1**
- `PrimeAppsMenu.setApps(...)` contract: present
- `PrimeAppsMenu.onLaunch(...)` contract: present
- `prime-app-launch` event: present
- Inline JavaScript `node --check`: **PASS**

The KRATOS proof worktree was clean after checkout.

## Functional proof carried from the frozen source implementation

Before standalone packaging, the same implementation was mechanically proved in the workspace:

- HTML parsed successfully.
- All implementation-local image references resolved.
- Inline JavaScript passed `node --check`.
- Headless Chromium rendered the menu at a 1440×1000 viewport.
- Launcher size was approximately 760×694.53 CSS pixels.
- Browser console/page errors: **0**.
- Search proof: typing `term` returned **Terminal**.
- Rotation proof: the rotating lane advanced from **Apps / Media** to **Media / Gallery**.
- Launch proof: selecting Terminal emitted `prime-app-launch` with `{ name: "Terminal" }`.

The standalone artifact embeds the runtime artwork directly, so no separate asset folder or network dependency is required.

## Locked menu behavior

The accepted Prime menu contract is:

1. Core / most-used applications remain in fixed positions.
2. Only the final two lower portal positions rotate through installed applications.
3. Wheel / trackpad, drag, and keyboard navigation can rotate that lane.
4. The bottom search field searches the full application catalogue.
5. Prime supplies installed apps through `PrimeAppsMenu.setApps([...])`.
6. Prime receives selections through `PrimeAppsMenu.onLaunch(...)` / `prime-app-launch`.

## Git sync record

- Standalone artifact commit: `4f60bac92d6533b8dc2cfd260b0ceece65b6770a`
- Menu documentation commit: `4dd77bd3d7455d24e415d8384e5765eec76c1fc7`

A fresh KRATOS detached worktree of `origin/main` confirmed both files were present and the worktree was clean.

## Integration boundary

This freeze proves and preserves the Prime Apps Menu **UI/reference contract**. It does not claim that Prime's native shell already enumerates or launches installed applications through this HTML. Native app discovery/process launch remains a Prime runtime adapter behind the locked menu boundary; integrating that adapter does not require redesigning this UI.
