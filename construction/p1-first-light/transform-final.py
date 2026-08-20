#!/usr/bin/env python3
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()


def replace_exact(text: str, old: str, new: str, expected: int = 1) -> str:
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"replacement fence failed: expected {expected}, found {count}: {old[:100]!r}")
    return text.replace(old, new)


p = root / "image/Containerfile"
s = p.read_text()
s = replace_exact(
    s,
    """    cargo clippy --locked -p prime-compositor --all-targets -- -D warnings; \\
    cargo build --locked --release -p prime-compositor; \\
    test -x target/release/prime-compositor; \\
    ! ldd target/release/prime-compositor | grep -q 'not found'; \\
    sha256sum target/release/prime-compositor > /prime-compositor.sha256
""",
    """    cargo clippy --locked -p prime-compositor -p prime-shell --all-targets -- -D warnings; \\
    cargo build --locked --release -p prime-compositor -p prime-shell; \\
    test -x target/release/prime-compositor; \\
    test -x target/release/prime-shell; \\
    ! ldd target/release/prime-compositor | grep -q 'not found'; \\
    ! ldd target/release/prime-shell | grep -q 'not found'; \\
    sha256sum target/release/prime-compositor > /prime-compositor.sha256; \\
    sha256sum target/release/prime-shell > /prime-shell.sha256
""",
)
s = replace_exact(
    s,
    """        libseat-0.9.3-1.fc44 \\
        mesa-dri-drivers-26.1.7-1.fc44 \\
""",
    """        libseat-0.9.3-1.fc44 \\
        libwayland-client-1.25.0-1.fc44 \\
        libxkbcommon-1.13.1-2.fc44 \\
        mesa-dri-drivers-26.1.7-1.fc44 \\
""",
    expected=2,
)
s = replace_exact(
    s,
    """RUN install -d -m 0755 /usr/libexec/prime /usr/lib/bootc/install
COPY target/release/primed /usr/libexec/prime/primed
COPY target/release/prime-recovery /usr/libexec/prime/prime-recovery
COPY --from=compositor-builder /source/target/release/prime-compositor /usr/libexec/prime/prime-compositor
COPY image/systemd/primed.service /usr/lib/systemd/system/primed.service
COPY image/systemd/prime-recovery.service /usr/lib/systemd/system/prime-recovery.service
COPY image/systemd/prime-recovery.target /usr/lib/systemd/system/prime-recovery.target
COPY image/bootc/10-prime.toml /usr/lib/bootc/install/10-prime.toml
""",
    """RUN install -d -m 0755 /usr/libexec/prime /usr/lib/bootc/install /usr/lib/sysusers.d
COPY target/release/primed /usr/libexec/prime/primed
COPY target/release/prime-recovery /usr/libexec/prime/prime-recovery
COPY --from=compositor-builder /source/target/release/prime-compositor /usr/libexec/prime/prime-compositor
COPY --from=compositor-builder /source/target/release/prime-shell /usr/libexec/prime/prime-shell
COPY image/scripts/prime-shell-session /usr/libexec/prime/prime-shell-session
COPY image/scripts/prime-first-light-witness /usr/libexec/prime/prime-first-light-witness
COPY image/sysusers/prime-shell.conf /usr/lib/sysusers.d/prime-shell.conf
COPY image/systemd/primed.service /usr/lib/systemd/system/primed.service
COPY image/systemd/prime-compositor.service /usr/lib/systemd/system/prime-compositor.service
COPY image/systemd/prime-shell.service /usr/lib/systemd/system/prime-shell.service
COPY image/systemd/prime-first-light-witness.service /usr/lib/systemd/system/prime-first-light-witness.service
COPY image/systemd/prime-recovery.service /usr/lib/systemd/system/prime-recovery.service
COPY image/systemd/prime-recovery.target /usr/lib/systemd/system/prime-recovery.target
COPY image/bootc/10-prime.toml /usr/lib/bootc/install/10-prime.toml
""",
)
s = replace_exact(
    s,
    """        /usr/libexec/prime/primed \\
        /usr/libexec/prime/prime-recovery \\
        /usr/libexec/prime/prime-compositor; \\
""",
    """        /usr/libexec/prime/primed \\
        /usr/libexec/prime/prime-recovery \\
        /usr/libexec/prime/prime-compositor \\
        /usr/libexec/prime/prime-shell \\
        /usr/libexec/prime/prime-shell-session \\
        /usr/libexec/prime/prime-first-light-witness; \\
""",
)
s = replace_exact(
    s,
    """    ! ldd /usr/libexec/prime/prime-compositor | grep -q 'not found'; \\
""",
    """    ! ldd /usr/libexec/prime/prime-compositor | grep -q 'not found'; \\
    ! ldd /usr/libexec/prime/prime-shell | grep -q 'not found'; \\
""",
)
s = replace_exact(
    s,
    """    systemctl enable primed.service; \\
    systemctl disable bootc-fetch-apply-updates.timer >/dev/null 2>&1 || true; \\
    test -f /usr/lib/systemd/system/prime-recovery.service; \\
    test -f /usr/lib/systemd/system/prime-recovery.target; \\
    test ! -e /etc/systemd/system/multi-user.target.wants/prime-compositor.service; \\
""",
    """    systemctl enable primed.service prime-compositor.service prime-shell.service prime-first-light-witness.service; \\
    systemctl set-default graphical.target; \\
    systemctl mask getty@tty1.service; \\
    systemctl disable bootc-fetch-apply-updates.timer >/dev/null 2>&1 || true; \\
    test -f /usr/lib/systemd/system/prime-recovery.service; \\
    test -f /usr/lib/systemd/system/prime-recovery.target; \\
    test -L /etc/systemd/system/graphical.target.wants/prime-compositor.service; \\
    test -L /etc/systemd/system/graphical.target.wants/prime-shell.service; \\
    test -L /etc/systemd/system/graphical.target.wants/prime-first-light-witness.service; \\
""",
)
s = replace_exact(
    s,
    """    test -x /usr/libexec/prime/prime-compositor; \\
    test -L /etc/systemd/system/multi-user.target.wants/primed.service; \\
""",
    """    test -x /usr/libexec/prime/prime-compositor; \\
    test -x /usr/libexec/prime/prime-shell; \\
    test -x /usr/libexec/prime/prime-shell-session; \\
    test -x /usr/libexec/prime/prime-first-light-witness; \\
    test -f /usr/lib/sysusers.d/prime-shell.conf; \\
    test -L /etc/systemd/system/multi-user.target.wants/primed.service; \\
    test -L /etc/systemd/system/graphical.target.wants/prime-compositor.service; \\
    test -L /etc/systemd/system/graphical.target.wants/prime-shell.service; \\
    test -L /etc/systemd/system/graphical.target.wants/prime-first-light-witness.service; \\
""",
)
s = replace_exact(
    s,
    """    test -x /usr/libexec/prime/prime-compositor; \\
    test ! -e /etc/systemd/system/multi-user.target.wants/prime-compositor.service; \\
    test -f /usr/lib/systemd/system/prime-recovery.target; \\
""",
    """    test -x /usr/libexec/prime/prime-compositor; \\
    test -x /usr/libexec/prime/prime-shell; \\
    test -x /usr/libexec/prime/prime-shell-session; \\
    test -x /usr/libexec/prime/prime-first-light-witness; \\
    test -L /etc/systemd/system/graphical.target.wants/prime-compositor.service; \\
    test -L /etc/systemd/system/graphical.target.wants/prime-shell.service; \\
    test -L /etc/systemd/system/graphical.target.wants/prime-first-light-witness.service; \\
    test -f /usr/lib/systemd/system/prime-recovery.target; \\
""",
)
p.write_text(s)

p = root / "tools/prove-p1-local.sh"
s = p.read_text()
s = replace_exact(
    s,
    "cargo clippy --locked --workspace --exclude prime-compositor --all-targets -- -D warnings\ncargo test --locked --workspace --exclude prime-compositor\n",
    "cargo clippy --locked --workspace --exclude prime-compositor --exclude prime-shell --all-targets -- -D warnings\ncargo test --locked --workspace --exclude prime-compositor --exclude prime-shell\n",
)
s = replace_exact(
    s,
    """    test -x /usr/libexec/prime/prime-compositor
    test -x /usr/sbin/bootc
""",
    """    test -x /usr/libexec/prime/prime-compositor
    test -x /usr/libexec/prime/prime-shell
    test -x /usr/libexec/prime/prime-shell-session
    test -x /usr/libexec/prime/prime-first-light-witness
    test -f /usr/lib/sysusers.d/prime-shell.conf
    test -x /usr/sbin/bootc
""",
)
s = replace_exact(
    s,
    """    test ! -e /etc/systemd/system/multi-user.target.wants/prime-compositor.service
""",
    """    test -L /etc/systemd/system/graphical.target.wants/prime-compositor.service
    test -L /etc/systemd/system/graphical.target.wants/prime-shell.service
    test -L /etc/systemd/system/graphical.target.wants/prime-first-light-witness.service
""",
)
s = replace_exact(
    s,
    """      libseat-0.9.3-1.fc44 \\
      mesa-dri-drivers-26.1.7-1.fc44 \\
""",
    """      libseat-0.9.3-1.fc44 \\
      libwayland-client-1.25.0-1.fc44 \\
      libxkbcommon-1.13.1-2.fc44 \\
      mesa-dri-drivers-26.1.7-1.fc44 \\
""",
)
s = replace_exact(
    s,
    """    ! ldd /usr/libexec/prime/prime-compositor | grep -q \"not found\"
""",
    """    ! ldd /usr/libexec/prime/prime-compositor | grep -q \"not found\"
    ! ldd /usr/libexec/prime/prime-shell | grep -q \"not found\"
""",
)
s = replace_exact(s, "timeout --signal=TERM 90s qemu-system-x86_64 \\", "timeout --signal=TERM 120s qemu-system-x86_64 \\")
marker = """sudo -n env EXPECTED_GENERATION_ID=\"$GENERATION_ID\" python3 -c 'import json,os,sys; g=json.load(open(sys.argv[1])); assert g[\"generation_id\"]==os.environ[\"EXPECTED_GENERATION_ID\"],g; assert g[\"state\"]==\"HEALTH_PROVING\",g; assert \"prime.core.socket.bound.v1\" in g.get(\"evidence_refs\",[]),g; assert g.get(\"boot_attempts_remaining\")==3,g' \"$GENERATION_FILE\"
HOST_ID=\"$(sudo -n python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))[\"host_id\"])' \"$IDENTITY_FILE\")\"
cleanup_nbd
"""
replacement = """sudo -n env EXPECTED_GENERATION_ID=\"$GENERATION_ID\" python3 -c 'import json,os,sys; g=json.load(open(sys.argv[1])); assert g[\"generation_id\"]==os.environ[\"EXPECTED_GENERATION_ID\"],g; assert g[\"state\"]==\"HEALTH_PROVING\",g; assert \"prime.core.socket.bound.v1\" in g.get(\"evidence_refs\",[]),g; assert g.get(\"boot_attempts_remaining\")==3,g' \"$GENERATION_FILE\"
WITNESS_FILE=\"$PRIME_DIR/first-light/mechanical.json\"
sudo -n test -f \"$WITNESS_FILE\"
sudo -n python3 -c 'import json,sys; w=json.load(open(sys.argv[1])); assert w[\"schema\"]==\"prime.first-light-mechanical.v1\",w; assert w[\"status\"]==\"SHELL_READY\",w; assert w[\"compositor_phase\"]==\"SHELL_READY\",w; assert w[\"shell_ready\"] is True,w; assert w[\"frame_loop_ready\"] is True,w; assert w[\"wayland_listener_ready\"] is True,w; assert w[\"clients_accepted\"]>=1,w; assert w[\"mapped_surface_frames_submitted\"]>=1,w; assert w[\"core_socket_group_nonzero\"] is True,w; assert w[\"owner_visual_acceptance\"] is False,w' \"$WITNESS_FILE\"
HOST_ID=\"$(sudo -n python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))[\"host_id\"])' \"$IDENTITY_FILE\")\"
cleanup_nbd
"""
s = replace_exact(s, marker, replacement)
s = replace_exact(
    s,
    '"physical_kratos_boot_proven":False',
    '"mechanical_shell_ready":True,"owner_visual_acceptance":False,"physical_kratos_boot_proven":False',
)
p.write_text(s)
