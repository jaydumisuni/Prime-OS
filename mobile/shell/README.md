# Prime Mobile Shell — native Android HOME component

One Prime-OS repository, distinct Desktop and Mobile builds. This contains a native Android HOME surface, not a standalone OS, TWRP ROM ZIP, privileged service, GSI, production PHI runtime or physical SM-A145P boot proof.

Current behavior: real launchable-app discovery, search and exact selected activity launch; real time, battery and platform-validated network status; Android Internet panel and Settings. Responsive navy/cyan/violet native UI. No remote assets, browser runtime, mandatory AI, Colibri, D-XQT or always-running daemon.

Build without Android on the lab phone. Requirements: JDK17, Gradle 8.7, Android SDK platform 35 and build-tools 35. Command: gradle -p mobile/shell/android :app:assembleDebug :app:testDebugUnitTest.

Debug APK is engineering proof only. A real Prime Mobile OS needs a pinned AOSP/Treble base, exact-device vendor/HAL and boot compatibility, Android-native Prime Core lifecycle, partition/update handling, recovery and whole-system boot proof. The Shell APK alone is insufficient. Calls, notification shade, SystemUI replacement, biometric unlock, USB host integration and fully featured PHI are not yet implemented.
