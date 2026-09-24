# Prime OS Apps Menu

Status: **WORKING / FROZEN UI PROTOTYPE**

This folder owns the approved Prime OS application-menu reference implementation.

## Artifact

- `Prime_OS_Apps_Menu_Standalone.html` — fully standalone launcher/menu prototype.
- No external asset directory is required at runtime; the artwork, CSS, and JavaScript are embedded in the HTML.
- The visual baseline is the approved neon Prime mascot/menu composition.

## Locked interaction model

- Fixed positions are reserved for core / most-used apps.
- Only the final two lower portal positions rotate through installed applications.
- Mouse wheel / trackpad and keyboard navigation can rotate the installed-app lane.
- The bottom search field searches the full application catalogue.
- Installed applications are supplied through `PrimeAppsMenu.setApps([...])`.
- Launch handling is exposed through `PrimeAppsMenu.onLaunch(...)` and the `prime-app-launch` event.

## Proven behavior

The source implementation was mechanically proved before being packaged as this standalone artifact:

- HTML parse: PASS
- Inline JavaScript `node --check`: PASS
- Headless Chromium render: PASS
- Browser console/page errors: 0
- Search: `term` returned Terminal
- Rotation: visible rotating slots advanced from Apps / Media to Media / Gallery
- Launch: Terminal emitted `prime-app-launch` with `{ name: "Terminal" }`

The launcher was proven at a 1440×1000 browser viewport with the UI constrained to approximately 760×694.53 CSS pixels.

## Prime integration boundary

This folder is the UI/reference boundary only. Prime's native shell may consume the visual/interaction contract without redesigning it. Native installed-app enumeration and actual process launching remain Prime runtime responsibilities behind the existing menu contracts.

See `EVIDENCE.md` for repository-sync and proof details.
