# Prime P1 First Light Visual Acceptance — 2026-09-03

## Accepted candidate

- Branch: `design/p1-first-light-visual`
- Accepted runtime source: `a4ae2c25f0d9d63119d2dd3d3d959ffd3a7f0330`
- Runtime Shell SHA-256: `4e30bc047c4a3d75d010ce92dd0c7e244d1e3822a167f4ccdabb5a6cd9ba507a`
- Owner visual acceptance: **ACCEPTED**
- Owner acceptance is the subjective authority; it is not replaced by a screenshot or machine readiness field.

The owner had already approved the Prime First Light visual direction and subsequently reconfirmed that approval while the accepted candidate was being closed out. No second owner-approval gate is required for this visual candidate.

## Visual authority and accepted states

The accepted native Rust Prime Shell preserves the approved First Light visual language while incorporating the later approved rail-behavior correction:

- Prime is the fixed rail anchor;
- Apps and Search are default configurable pins;
- duplicate Network/Audio/Storage/Health rail entries are not mandatory because those controls remain available in Quick Controls;
- approved Prime mark, dark cyan/violet First Light field, glass rail, top status chrome, Prime launcher and Quick Controls composition are present;
- launcher uses the reference four-column/two-row application-card composition and bounded Quick Actions;
- Quick Controls uses the accepted tall right-side control-center hierarchy;
- unavailable capabilities are presented truthfully rather than painted as functioning controls.

Accepted production-painter images are frozen under `docs/evidence/visual-acceptance/2026-09-03/`:

- `01-baseline.png`
- `02-prime_launcher.png`
- `03-quick-controls.png`
- `oracle-proof-image-manifest.json`

These images are production Rust painter output, not the temporary HTML/CSS design laboratory.

## Native host verification

Before physical deployment of the accepted Shell:

- `cargo test -p prime-shell`: **46/46 PASS**
- `cargo test --workspace --locked`: **PASS**
- `cargo clippy -p prime-shell --all-targets --locked`: **PASS, zero warnings**
- `cargo fmt --all -- --check`: **PASS**
- `git diff --check`: **PASS**
- optimized `prime-shell` release build: **PASS**

## KRATOS physical deployment

The previously proven compositor remained in control of the physical seat while only the Shell was replaced. The accepted Shell ran as:

`/home/kratos/prime-p1-fidelity-20260902/target/release/prime-shell`

The live compositor accepted the candidate's persistent surfaces at the intended geometry:

- background: `1920×1080`;
- rail: `132×316`, margin `(top=140,left=56)`;
- status cluster: `480×44`;
- output: `HDMI-A-1`;
- mode: `1920×1080 @ 60 Hz`;
- seat: `seat0`;
- Noto Sans production font selected.

The compositor readiness record after the Shell swap remained:

- `phase=SHELL_READY`;
- `shell_ready=true`;
- `session_active=true`;
- `renderer_ready=true`;
- `outputs_ready=true`;
- `frame_loop_ready=true`;
- `keyboard_ready=true`;
- `pointer_ready=true`;
- `input_delivery_ready=true`;
- `frames_submitted=2250`;
- `mapped_surface_frames_submitted=2226`;
- `input_events_seen=3232`.

The fbdev buffer was explicitly rejected as visual evidence because it represented the black console buffer rather than active KMS scanout. A KMS probe independently identified the live i915 plane/framebuffer as 1920×1080 RGBA; modifier mapping prevented exporting the scanout as an independent screenshot. This limitation does not reopen owner acceptance and is not represented as a visual PASS mechanism.

## Fresh canonical First Light proof

A fresh canonical proof was run against the exact accepted runtime source.

- `P1_LOCAL_PROOF=PASS`
- source revision: `a4ae2c25f0d9d63119d2dd3d3d959ffd3a7f0330`
- generation: `p1-first-light-a4ae2c25f0d9`
- generation state: `HEALTH_PROVING`
- mechanical `SHELL_READY`: **true**
- OVMF/UEFI normal QCOW2 boot: **PASS**
- canonical/final Composefs digest: `e33744ad51ab1b38fd45461eeb256d2d803a0e028d9ae0f06f942a7d6035f3469ebac1c06719c38fe79913b3b5754264a47d33f3d8c0ec05e7ccc09903fcef58`
- normal/recovery UKI embedded digest: same canonical digest
- QCOW2 SHA-256: `373394139b21b1af52d86e83146c44fea315f924cc304f19957f9a94d67ce700`

The machine-generated canonical report correctly retains `owner_visual_acceptance=false` because a canonical VM proof cannot assert a human decision. This document records that separate owner authority as **ACCEPTED**.

Canonical report copy: `docs/evidence/visual-acceptance/2026-09-03/prime-p1-local-proof-a4ae2c2.json`.

## Boundary after visual acceptance

The Prime P1 **visual gate is closed** for `a4ae2c2`.

This does not manufacture proof for unrelated external hardware gates. USB3/SuperSpeed with a real device, Ethernet carrier, audible playback, physical display cable/DP hotplug, and separate `KNOWN_GOOD` generation-promotion authority remain governed by their own evidence requirements.
