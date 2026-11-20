# Prime Mobile AOSP system-image integration

The existing repository contains the shared Linux Prime Core plus a new native Android HOME application. This directory provides **only an AOSP build integration fragment** for the Prime graphical shell. It is not a ROM or an installable image.

A properly built Android system image must select and pin a complete AOSP/GSI framework, preserve its applicable SELinux file-contexts and AVB chain, and pair with an actually compatible SM-A145P kernel/vendor/HAL/partition layout. The phone currently runs TWRP only; no Android installation is required to compile this module. No automatic flashing, vbmeta disable, stock firmware downgrade, bootloader mutation or generic GSI-as-ready claim is permitted.

The `prime-mobile-shell.mk` fragment adds the ordinary `PrimeMobileShell` Android application to the **product image** of an authorized, otherwise complete AOSP product build. The matching `mobile/shell/Android.bp` provides a Soong module from the same sources as the independently built APK. This app is not platform-privileged. Calling it Prime Mobile does not make it a complete OS.

**Admission before a real image**: (1) pinned complete AOSP build artifacts or source, (2) exact device vendor interface/partition geometry, (3) concrete Android-compatible Prime Core service bridge, (4) SELinux/AVB and installer/recovery chain, (5) boot/UI and phone functions in actual guest or device tests. Until then **no TWRP ZIP** is generated and **no phone is modified**. Device vendor and signing materials must be handled according to their licenses; copying Samsung/Google proprietary blobs from public dumps without redistribution rights is prohibited.
