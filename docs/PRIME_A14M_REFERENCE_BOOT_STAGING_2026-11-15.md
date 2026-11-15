# Prime Mobile A14m — source-qualified boot-image staging, NOT a device-boot proof

Date: 2026-11-15. Repository: `jaydumisuni/Prime-OS`; same MOB-0 candidate branch and PR #223. No new repository or shared Core daemon. This work is a continuation of `PRIME_A14M_TWRP_ONLY_SOURCE_LAYOUT_CORRECTION_2026-11-15.md`.

## Decision and scope

The owner's SM-A145P is TWRP-only and does not have Android installed. A running stock Android `getprop` is therefore NOT a build dependency. The published old AWG1 boot/recovery source gives enough information to create an independently verifiable **historical Android boot-header v2 staging image** offline, but it cannot certify the current device's bootloader security state, vendor_boot/init_boot use, working kernel or safe flash route.

**The built `a14m-first-light.reference-v2.boot.img` is not flash-authorized, not production and not called phone-bootable.** Do not install it in Odin, TWRP, Download Mode, fastboot or as an OTA. The preservation of the existing working TWRP recovery is non-negotiable.

## Recovered inputs (exact historical Git objects)

- Dump: `forforksake/a14m_dump` at commit `699855860e672eb5f3582ecf8e2144de0ed8b2d1`, **boot/kernel** Git blob `1d3968ea089f88deb19c0b8e1efe482b85ef7e65` (11,564,226 bytes). **Do not substitute the different TWRP recovery kernel** `cb704f154f9aef83096fcab08db6dbcb871606c2` (11,532,508 bytes).
- **boot/dtb.img** Git blob `db8938c3219ff857dafbf59e0b476ecdb751582b` (135,300 bytes). **ACTUAL CONTAINER FORMAT** recovered from the exact bytes in CI: 32-byte Android big-endian `dt_table_header` (`0xd7b7ab1e`), one 32-byte table entry, one raw FDT at byte offset **64** of size **135,236**. The builder preserves the complete 135,300-byte table container and independently validates the embedded FDT. This matches the historical recovery DTB by Git blob content, but does not prove current physical binding.
- **boot/img_info** Git blob `13c04b85f08d74e3fae7f27cc035feb3ba394d1b`: page 2048, base `0x40078000`, kernel offset `0x00008000`, ramdisk offset `0x07c08000`, tags offset `0x0bc08000`, DTB offset `0x07808000`, board `SRPVJ12A002`, cmdline `bootopt=64S3,32N2,64N2 loop.max_part=7`.
- Historic TWRP `BoardConfig.mk` at Git blob `a57fe4d664e94407771a47e60da6aaaa44578e7a`: recovery build boot header v2, boot partition size assertion 80,740,352 bytes. PIT analysis shows `boot`, `recovery`, `vendor_boot`, `init_boot`, `dtbo`, `vbmeta`, `super`; current active partition semantics remain UNQUALIFIED.
- Boot-image format reference: Android Open Source Project `unpack_bootimg.py`/`mkbootimg.py` and [DTB images documentation](https://source.android.com/docs/core/architecture/bootloader/dtb-images). The v2 header carries separate DTB size/load address and uses page-aligned kernel, ramdisk and DTB sections.

## New implementation

- `mobile/device-ports/samsung/a14m/first_light_init.c`: **static AArch64** Linux early PID1, no libc or dynamically loaded libraries, no device partition/file writes. It logs bounded source-only early-boot markers to console and `/dev/kmsg`, attempts to mount `proc` and `sysfs`, reads only Device Tree model if available, reports `EVIDENCE_ONLY_NO_PRIME_CORE_NO_PHONE_QUALIFICATION`, and remains alive. It does not provide a Prime shell, unlock a bootloader, run `primed`, or use Android framework/HALs. It is deliberately diagnostic and not a replacement stock OS.
- `mobile/tools/a14m_stage_boot.py`: refuses source bytes if the kernel/DTB Git blob IDs differ from pinned AWG1. Parses and validates the historic `boot/img_info` geometry; assembles gzip newc with exactly `init`, `dev/{console,null,kmsg}`, `proc`, `sys`; checks ELF64 AArch64 static executable and truth markers; builds an Android header v2 `ANDROID!` image with *original boot kernel* and pinned DTB; independently parses the finished image and round-trips exact kernel, ramdisk and DTB sections, enforcing historical partition size limit. `os_version_patch_level` is encoded as 0, rather than inventing a current patch level. The original AWG1 cmdline is preserved, without TWRP `selinux=permissive`, disabled AVB flags, spoofed 2099 patch date or fabricated super geometry.
- `mobile/tests/test_a14m_stage_boot.py`: parser, ramdisk, source blob and negative image-format regressions.
- `.github/workflows/mobile-a14m-first-light-stage.yml`: read-only permission, branch-scoped GitHub Ubuntu job, immutable reference downloads verified by **Git object SHA-1** and file sizes, actual Clang AArch64 early init build, actual binary image construction, independent header/section parser, ZIP SHA256 member validation and bounded source evidence artifact. It does not involve KRATOS, a device, external emulator or Samsung flashing tools.

## Trust and completion boundaries

`manifest.json` contains the source commit, historical Git blobs, SHA256 for image, kernel, DTB and ramdisk, exact boot header geometry, and **false** flags for physical boot, Prime Core running, mobile shell, device flash authorization, actual boot-chain qualification and source licensing review. The staging ZIP is named `A14M_REFERENCE_STAGING_DO_NOT_FLASH.zip` to prevent confusing a development artifact with TWRP-flashable ROM.

**Full Prime OS Mobile is not finished by this source build.** The prototype does not prove actual Samsung kernel execution, image signature/boot acceptance, required vendor_boot/init_boot handoff, hardware peripherals, power/suspend, safe restoration, persistent service lifecycle or Prime Mobile UI. Device-specific execution and controlled recovery are separate physical gates. A TWRP-only phone does not need Android reinstalled to continue.

Next gate: independently review the staged boot sections and manufacturer-compatible boot image/AVB chain for the exact phone's unlocked state and tested TWRP stock restore, then attempt only an explicitly authorized, reversible controlled first-boot test. Never treat a CI build as phone boot evidence.
