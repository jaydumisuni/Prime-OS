# Prime Mobile MOB-2 — SM-A145P / a14m read-only source intake

**Date:** 2026-11-15. **Scope:** isolated candidate development on existing Prime-OS MOB-0 PR #223, **not** a Samsung-installable build or physical handset acceptance. This extends the already documented Prime Mobile road map / MOB-0 evidence schemas; **do not create a second hardware authority, fork the Prime Core or modify Desktop P1**.

## Source recovery before design

The existing `mobile/device-ports/samsung/a14m/port-candidate.json` and `evidence-candidate.json` already record the reference model SM-A145P/a14m and owner-stated unlocked bootloader. The prior TWRP photo proves a visible TWRP interface, **not** bootloader state. The existing `mobile/tools/mob0_preflight.py` already distinguishes caller-reported, photographic, simulated, stale and live evidence; the existing `port_capability_audit.py` keeps all device claims unqualified. **Already documented — No update required** for those contracts.

Newly verified external source *leads*, not device-current measurements:

| Source | Evidence | Scope/limit |
| --- | --- | --- |
| Samsung release notes: `https://doc.samsungmobile.com/SM-A145P/026283230309/eng.html` | SM-A145P Android 15 `A145PXXUEDZJ2`, Samsung release date **2026-10-28**, patch 2026-10-05 | This is a published regional update, **not** the installed build of the user's device. Don't assume the phone is on Android 15 or bootloader revision 14 |
| `https://github.com/forforksake/android_kernel_samsung_a145p` | A145P `A145PXXU2AWG1` historical kernel source, commit `2ad60c6b7b89f6f37d1eb483696e7d121c8972bd` | Historical Android 13-era reference, **not** current target kernel/image compatibility |
| `https://github.com/forforksake/recovery_device_samsung_a14m` | A145P community recovery tree, commit `cdbc13945796b6863362ed041ce7177ce7cf3ca5` | Recovery/MT6768 hints only; cannot qualify stock restore or present touchscreen / encryption |
| `https://github.com/forforksake/a14m_dump` | Historical A145P firmware dump, commit `699855860e672eb5f3582ecf8e2144de0ed8b2d1` | Published Android 13-era source reference, NOT a complete/current backup |

Public third-party sources also report MediaTek MT6768/MT6769 and `a14m`, but **silicon, panel, storage, bootloader bits, actual current partition mapping, available OEM images and vendor compatibility must be measured on the exact lab phone**. Samsung publishes device update notes by region; a newer build does not imply an older stock image can be downgraded or restored to an arbitrary bootloader revision. Public firmware mirrors are not cryptographic device/owner proof.

## Implementation added to the existing PR

- `mobile/device-ports/samsung/a14m/source-intake.candidate.json` — public references with pinned commits; all physical readiness booleans **false**.
- `mobile/tools/a14m_stock_preflight.py` — optional **read-only** non-root ADB property/partition-name capture from **exactly one** authorized phone; avoids recording serial, IMEI and MAC; pins subsequent commands to the selected transport; only runs `getprop`, `uname`, `ls`. It never invokes `flash`, `push`, `pull`, `root`, `reboot`, `dd`, `fastboot` or partition-writing APIs. The resulting capture is a *claim* from Android runtime, not OEM-download-mode/fuse/bootloader attestation. ADB must be enabled/authorized to collect data. No phone was connected for this source-only checkpoint.
- The same tool's `assess` mode reads the pinned source ledger and any future ADB capture. It compares provisional model/board, extracts a firmware code only when explicitly present in the firmware fingerprint, treats historical source mismatches as blockers and **always returns** `STOP_DEVICE_IMAGE_NOT_READY` / `device_flash_authorized:false` / `boot_image_compatible:false` / `stock_restore_verified:false`.
- `mobile/tests/test_a14m_stock_preflight.py`: positive/negative **synthetic** tests, including exact model, wrong board/model, older-vs-newer firmware, forged bootloader unlocked property, omitted/ambiguous firmware, malformed fields, source pinning, multi-device fail-closed, unauthorized/offline ADB, privacy, and read-only command allowlist. Mocked USB is not a real physical read.
- `.github/workflows/mobile-a14m-readonly-preflight.yml`: branch-scoped **read-only** CI workflow that runs the existing MOB-0 suites and verifies the new stop gate on a source-only manifest; no device, KRATOS, firmware, download or deployment actions.

## Reproducible commands

On any machine with Python 3, including GitHub-hosted runners, source-only:

```sh
python3 -m unittest discover -s mobile/tests -v
python3 mobile/tools/a14m_stock_preflight.py assess --sources mobile/device-ports/samsung/a14m/source-intake.candidate.json
```

**Later, only when the exact lab phone is connected and ADB debugging was explicitly authorized:**

```sh
python3 mobile/tools/a14m_stock_preflight.py capture --adb /path/to/adb --out a14m-local-capture.json
python3 mobile/tools/a14m_stock_preflight.py assess --sources mobile/device-ports/samsung/a14m/source-intake.candidate.json --capture a14m-local-capture.json
```

On ATHENA, use the previously established portable `adb.exe` path if still present, rather than assuming a system-wide adb. **Do not publish device raw evidence or serial identifiers in a public repository.** The captured property digest and source must still be reviewed and bound to the physical phone; the tool never upgrades the evidence to authority.

## Exact blockers before a device boot image

1. Actual phone-side model/variant/product/SoC/installed build and Android/bootloader state, collected read-only and independently reviewed (ADB alone is not download-mode lock proof).
2. Exact active-partition and boot-image layout (boot/recovery/vendor_boot/init_boot/dtbo/vbmeta/super only **where actually present**), verified stock image and hash for the *installed compatible* firmware, signed image/AVB chain and rollback constraints.
3. Right-to-use/correct kernel, Device Tree, firmware/modules/user-space HAL for that exact build and panel; historic public A145P trees are not sufficient.
4. Verified **stock recovery/restore path**, test image rollback and owner approval before any write or transient boot.
5. Only then build a Samsung-specific image in an isolated branch; its file extension, hash or kernel compilation is never proof that flashing is safe.

**Disposition:** Preparation and source-only negative proof; no physical Samsung read, no flash, no system partition write, no Core authority promotion, no MOB-2 hardware acceptance. PR stays draft until actual-device evidence and independent review.
