# Prime P1 Physical Proof — 2026-09-02

## Authority

- Branch: `design/p1-first-light-visual`
- Proven source revision: `5b43c5c7639f9e7176ef7eedfd43f5a4a969ad87`
- Generation: `p1-first-light-5b43c5c7639f`
- Generation state: `HEALTH_PROVING`
- `KNOWN_GOOD`: **not proven / not promoted**
- Owner visual acceptance: **PENDING_OWNER**
- Canonical proof source: `/var/tmp/prime-p1-visual-proof-20260902T141732Z/work/run/prime-p1-local-proof.json`
- Physical evidence root: `/var/tmp/prime-p1-visual-proof-20260902T144904Z/physical/`

This checkpoint records only evidence actually earned on KRATOS or in the canonical isolated P1 proof. A missing cable, peripheral, or human observation remains an explicit external gate.

## Canonical First Light — PASS

The unchanged canonical P1 proof completed with `P1_LOCAL_PROOF=PASS` for `5b43c5c`.

- OVMF/UEFI normal boot: PASS
- Prime Host identity persistence: PASS
- generation entered `HEALTH_PROVING`: PASS
- mechanical `SHELL_READY`: PASS
- mapped-surface frame retirement: PASS
- normal and recovery UKIs embed the same canonical Composefs digest: PASS
- canonical QCOW2 SHA-256: `75c4fb22d3460552642ce2a0e46966f358a7dcbe3b655235ba50f2dd45a9d25a`
- canonical Composefs digest: `8d0374c1b8f5330c709750a0f16f8fc8bb37b497838f5f9c919e18f3781b8f63cc7dd820353731ed6657bc4e6f81addc4b0dab3135e07b1467bf0f8302a5085a`

The canonical report deliberately keeps `known_good_proven=false`, `owner_visual_acceptance=false`, and `physical_kratos_boot_proven=false`.

## Recovery — PASS, Supplemental Isolated Boot Proof

The exact sealed recovery UKI from the canonical build was staged as the first UEFI boot device on disposable FAT media and booted through OVMF with a disposable QCOW2 overlay backed by the canonical Prime disk.

- Recovery UKI SHA-256: `608e17eab499d4a130241612ad1b23280b32e5c24580cb6eb76abc36cc3beb76`
- embedded command line contains `systemd.unit=prime-recovery.target prime.recovery=1`: PASS
- OVMF firmware handoff to the recovery UKI: PASS
- recovery tty console interaction: PASS
- documented `j` action changed the framebuffer to a stable second recovery state: PASS
- repeated JSON-state framebuffer captures were bit-identical: PASS
- prompt and JSON-state framebuffer hashes differed: PASS
- recovery power-off action: **NOT PROVEN**; the guest was terminated through the QEMU monitor after evidence capture

Structured supplemental evidence: `/var/tmp/prime-p1-visual-proof-20260902T144904Z/physical/recovery-boot-proof.json`.

## Physical KRATOS Graphics — PASS

Prime compositor ran directly on KRATOS through logind/libseat against the real Intel graphics path.

- GPU: Intel UHD 630, `/dev/dri/card1`
- connector: `HDMI-A-1`
- mode: `1920x1080 @ 60 Hz`
- direct tty backend: PASS
- DRM access / renderer / output readiness: PASS
- real Prime glass path: `PRIME_GLASS_EFFECTS=ready`
- glass fallback: not observed
- visible compositor-owned cursor implementation is included in `5b43c5c`
- mapped Prime Shell frames retired on DRM vblank: PASS

Final post-resume readiness reached `phase=SHELL_READY`, `frames_queued=739`, `frames_submitted=739`, `mapped_surface_frames_submitted=737`, `clients_accepted=6`, and `input_events_seen=1114` while keeping output, frame-loop, renderer, pointer and keyboard readiness true.

Evidence snapshot: `/var/tmp/prime-p1-visual-proof-20260902T144904Z/physical/readiness-final-post-suspend.json`.

## Output Topology / Revalidation — PASS for Software DRM Change

A real DRM `change` uevent was injected against `card1` while Prime was live. Prime failed closed rather than claiming stale readiness:

- phase changed to `OUTPUT_REVALIDATION_REQUIRED`
- `outputs_ready=false`
- `shell_ready=false`
- `frame_loop_ready=false`
- `last_udev_event=CHANGED:57857`

The bounded compositor/Shell session was restarted against the unchanged hardware. Prime reacquired `/dev/dri/card1`, HDMI-A-1 and the active seat, then returned to `SHELL_READY` with mapped-frame retirement.

A **real cable disconnect/reconnect or DP hotplug is still BLOCKED_EXTERNAL** because DP is physically disconnected and no one changed the display cable during this proof.

## Wayland / XDG / Isolation — PASS

Post-revalidation normal client proof:

```text
PHYSICAL_OUTPUT_ANNOUNCED=PASS count=1
XDG_TOPLEVEL_INITIAL_CONFIGURE=PASS
XDG_POPUP_INITIAL_CONFIGURE=PASS
PRIME_P1_PHYSICAL_XDG_CLIENT=PASS
```

This was repeated successfully after deep S3 resume.

The earlier disposable malformed-client helper was found to depend on an unreliable assumption: requesting a second `xdg_surface` for the same `wl_surface` did not guarantee rejection at its chosen roundtrip boundary. Product code was not changed to satisfy that helper.

An unambiguous invalid-XDG stimulus was used instead. An invalid configure serial/role sequence was rejected with Wayland protocol error while Prime remained alive:

```text
XDG_INVALID_SERIAL_CLIENT_REJECTED=PASS errno=71
PRIME_P1_PHYSICAL_PROTOCOL_ERROR_INJECTION=PASS
```

Compositor readiness remained `SHELL_READY` after client rejection.

## Input / USB — PASS for Attached USB2 Devices and Prime Interaction

Attached USB2 paths during proof:

- Bus 1 Port 4 — RTL8188EUS Wi-Fi, 480 Mbit/s
- Bus 1 Port 5 — Logitech M185 receiver, 12 Mbit/s
- Bus 1 Port 6 — HP Elite Keyboard, 1.5 Mbit/s
- Bus 1 Port 8 — Xiaomi ADB device, 480 Mbit/s

Prime input proof after output revalidation:

- synthetic pointer + keyboard event delivery: PASS
- pointer Orb activation: PASS (`PRIME_SHELL_ORB_OPEN=pointer`)
- pointer Quick Controls activation: PASS (`PRIME_SHELL_QUICK_CONTROLS_OPEN=pointer`)
- keyboard Quick Controls activation: PASS
- keyboard transient close/navigation: PASS
- input counters advanced while `SHELL_READY` remained true: PASS
- post-S3 pointer Quick Controls interaction: PASS

The USB3 root hub is present as xHCI Bus 2 with eight ports and 10 Gbit/s capability, but **no SuperSpeed peripheral is attached**. Therefore physical USB3 data-path proof is `BLOCKED_EXTERNAL`, not PASS.

No removable USB block-storage device was attached during this checkpoint.

## Network — Partial Hardware Baseline

Wi-Fi is physically live through the USB RTL8188EUS adapter and reconnected after deep S3:

- SSID: `TTG HOME`
- 2.4 GHz / 2462 MHz
- post-resume signal approximately -36 dBm
- 72.2 Mbit/s reported transmit bitrate
- DHCP lease returned after resume

Ethernet baseline:

- device: `enp1s0`
- driver: `r8169`
- firmware: `rtl8168h-2_0.0.2 02/26/15`
- advertised modes include 10/100/1000BASE-T
- `NO-CARRIER`, `Link detected: no`

Live Ethernet carrier/traffic is **BLOCKED_EXTERNAL** pending a connected Ethernet cable/link partner.

## Audio — Hardware Stream Path PASS; Audibility PENDING_EXTERNAL

Detected playback hardware includes ALC897 analog and the ASUS `VG249Q1R` HDMI audio endpoint.

Real ALSA hardware-stream tests accepted 48 kHz S16_LE stereo playback on:

- `hw:0,3` HDMI: PASS
- `hw:0,0` ALC897 analog: PASS
- HDMI stream acceptance was repeated successfully after deep S3 resume.

This proves the kernel/ALSA playback path. **Audible sound is PENDING_EXTERNAL** because remote mechanical evidence cannot establish what a person or attached speaker actually heard.

## Suspend / Resume — PASS

A bounded RTC-wake test executed on KRATOS:

```text
rtcwake -m mem -s 10
```

The kernel journal proves a genuine deep ACPI S3 cycle:

- `PM: suspend entry (deep)` at 16:19:01 +02
- `ACPI: PM: Preparing to enter system sleep state S3`
- CPUs were offlined
- `ACPI: PM: Low-level resume complete`
- CPUs were restored
- `ACPI: PM: Waking up from system sleep state S3`
- `PM: suspend exit` at 16:19:12 +02
- Wi-Fi re-associated and reacquired DHCP by 16:19:14 +02
- Oracle/Cloudflare connectivity returned automatically

Prime compositor and Prime Shell both survived the cycle without restart and remained active. The existing readiness object remained `SHELL_READY`; post-resume XDG toplevel/popup and pointer Quick Controls probes also passed.

Concise journal evidence: `/var/tmp/prime-p1-visual-proof-20260902T144904Z/physical/suspend-resume.log`.

## Performance / Thermal — PASS for P1 Baseline

Idle sample:

- Prime compositor CPU: 0%
- Prime Shell CPU: 0%
- queued-frame delta: 0
- submitted-frame delta: 0
- mapped-frame delta: 0
- input-event delta: 0

Active capture around physical interaction observed approximately 0.13% average compositor CPU with 1% peak samples; Shell remained approximately 0% in the sample. No glass fallback or repeated missed-frame condition was observed.

Thermals remained ordinary during/after proof:

- x86 package: approximately 42–43°C
- PCH: approximately 46°C
- ACPI zones: approximately 28–30°C

## Display / Port External Gates

Current connector truth after resume:

- HDMI-A-1: connected
- DP-1: disconnected

Software DRM topology invalidation/recovery is proven. Physical cable removal/reinsertion, DP output, SuperSpeed USB data, Ethernet carrier, and audible audio require corresponding external hardware/actions and remain explicitly unproven.

## KRATOS Restoration — PASS

The bounded Prime physical-proof units were stopped after evidence capture. Normal workstation graphics were restored:

- `gdm.service`: active
- `gdm-x-session startxfce4`: running
- Xorg: running
- `xfce4-session`: running
- temporary Prime physical compositor: inactive

KRATOS was not left on a TTY or stale Prime framebuffer.

## Remaining Acceptance Gates

| Gate | State | Requirement |
|---|---|---|
| USB3/SuperSpeed physical data path | `BLOCKED_EXTERNAL` | Attach an actual SuperSpeed device to a USB3-capable physical port |
| Ethernet live carrier/traffic | `BLOCKED_EXTERNAL` | Attach an Ethernet cable to an active link partner |
| Audible HDMI/analog audio | `PENDING_EXTERNAL` | Human/physical confirmation of audible playback |
| Physical display cable hotplug / DP | `BLOCKED_EXTERNAL` | Perform cable hotplug and/or attach a DP display |
| Owner visual acceptance | `PENDING_OWNER` | Owner explicitly judges the final Prime monitor experience |
| `KNOWN_GOOD` promotion | `NOT_EARNED` | Separate generation promotion authority must be satisfied |

No external gate is converted to PASS by inference.
