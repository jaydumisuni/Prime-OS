#!/usr/bin/env python3
"""Bounded W8 host-only synthetic VM runtime probe.

Uses an original 512-byte serial-output BIOS boot sector. This cannot qualify
Windows guest execution, Prime guest-agent handoff, or release authority.
Everything runs in disposable private storage; no network, USB, or host disks.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import stat
import subprocess
import tempfile
import time

MARKER = b"TTG-W8-SYNTHETIC-GUEST-BOOTED\n"
BOOT_SOURCE = r""".code16
.global _start
_start:
cli
xorw %ax,%ax
movw %ax,%ds
movw %ax,%ss
movw $0x7c00,%sp
sti
movw $0x3fb,%dx
movb $0x80,%al
outb %al,%dx
movw $0x3f8,%dx
movb $0x01,%al
outb %al,%dx
movw $0x3f9,%dx
xorb %al,%al
outb %al,%dx
movw $0x3fb,%dx
movb $0x03,%al
outb %al,%dx
movw $0x3fa,%dx
movb $0xc7,%al
outb %al,%dx
movw $0x3fc,%dx
movb $0x0b,%al
outb %al,%dx
movw $message,%si
movw $0x3f8,%dx
nextchar:
lodsb
testb %al,%al
jz loop_forever
outb %al,%dx
jmp nextchar
loop_forever:
hlt
jmp loop_forever
message:
.asciz "TTG-W8-SYNTHETIC-GUEST-BOOTED\n"
.org 510
.word 0xaa55
"""
TIMEOUT_SECONDS = 12


def run(argv: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(argv, capture_output=True, text=True, timeout=30, check=True)


def sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def boot_sector(directory: Path) -> bytes:
    source = directory / "guest.s"
    obj = directory / "guest.o"
    binary = directory / "guest.bin"
    source.write_text(BOOT_SOURCE)
    run(["as", "--32", "-o", str(obj), str(source)])
    run(["ld", "-m", "elf_i386", "-Ttext", "0x7c00",
         "--oformat", "binary", "-o", str(binary), str(obj)])
    data = binary.read_bytes()
    if len(data) != 512 or data[-2:] != b"\x55\xaa" or MARKER not in data:
        raise RuntimeError("synthetic bootsector build failed closed")
    return data


def prepare_session(root: Path, application: str, sector: bytes) -> dict[str, Path]:
    runtime = root / application / "session-1"
    runtime.mkdir(parents=True, mode=0o700)
    raw = runtime / "scratch.raw"
    overlay = runtime / "overlay.qcow2"
    with raw.open("wb") as handle:
        handle.write(sector)
        handle.truncate(8 * 1024 * 1024)
    run(["qemu-img", "convert", "-f", "raw", "-O", "qcow2",
         "-o", "compat=1.1", str(raw), str(overlay)])
    raw.unlink()
    info = json.loads(run(["qemu-img", "info", "--output=json", str(overlay)]).stdout)
    if info.get("format") != "qcow2" or info.get("backing-filename"):
        raise RuntimeError("synthetic guest image has unadmitted backing")
    if overlay.stat().st_mode & 0o077 or runtime.stat().st_mode & 0o077:
        raise RuntimeError("synthetic guest paths are not private")
    return {"runtime": runtime, "overlay": overlay,
            "agent": runtime / "agent.sock",
            "serial": runtime / "serial.log",
            "stderr": runtime / "stderr.log"}


def arguments(qemu: str, session: dict[str, Path]) -> list[str]:
    argv = [
        qemu, "-accel", "kvm", "-machine", "q35", "-nodefaults",
        "-no-user-config", "-display", "none", "-net", "none",
        "-m", "512", "-smp", "1",
        "-drive", f"file={session['overlay']},if=virtio,format=qcow2,cache=none",
        "-chardev", f"socket,id=primeagent,path={session['agent']},server=on,wait=off",
        "-device", "virtio-serial-pci",
        "-device", "virtserialport,chardev=primeagent,name=org.thetechguy.prime.agent",
        "-serial", f"file:{session['serial']}",
        "-monitor", "none", "-no-reboot", "-boot", "c",
    ]
    if "-net" not in argv or argv[argv.index("-net") + 1] != "none":
        raise RuntimeError("host networking not explicitly disabled")
    if any("usb-host" in arg or "hostfwd" in arg or "tap" == arg
           for arg in argv):
        raise RuntimeError("unadmitted device/network argument")
    return argv


def stop(proc: subprocess.Popen[bytes], requested_signal: int) -> int:
    if proc.poll() is None:
        os.killpg(proc.pid, requested_signal)
    try:
        return proc.wait(timeout=3)
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL)
        return proc.wait(timeout=3)


def run_probe() -> dict[str, object]:
    qemu_name = shutil.which("qemu-system-x86_64")
    if not qemu_name:
        raise RuntimeError("missing QEMU x86_64")
    qemu = str(Path(qemu_name).resolve(strict=True))
    meta = Path(qemu).stat()
    if not stat.S_ISREG(meta.st_mode) or meta.st_mode & 0o022:
        raise RuntimeError("QEMU donor identity not privately admitted")
    prior_mask = os.umask(0o077)
    procs: list[subprocess.Popen[bytes]] = []
    streams = []
    try:
        with tempfile.TemporaryDirectory(prefix="ttg-w8-synthetic-host-", dir="/tmp") as temp:
            root = Path(temp)
            sector = boot_sector(root)
            one = prepare_session(root, "app-alpha", sector)
            two = prepare_session(root, "app-beta", sector)
            sessions = [one, two]
            paths = [s["overlay"] for s in sessions] + [s["agent"] for s in sessions]
            if len({str(path) for path in paths}) != len(paths):
                raise RuntimeError("synthetic session state is not independent")
            for session in sessions:
                err = session["stderr"].open("wb")
                streams.append(err)
                proc = subprocess.Popen(arguments(qemu, session),
                                        stdout=subprocess.DEVNULL,
                                        stderr=err, start_new_session=True)
                procs.append(proc)
            started = time.monotonic()
            ready = False
            while time.monotonic() - started < TIMEOUT_SECONDS:
                ready = all(
                    p.poll() is None
                    and s["serial"].exists()
                    and MARKER in s["serial"].read_bytes()
                    and s["agent"].exists()
                    and stat.S_ISSOCK(s["agent"].stat().st_mode)
                    for p, s in zip(procs, sessions)
                )
                if ready:
                    break
                if any(p.poll() is not None for p in procs):
                    break
                time.sleep(0.15)
            if not ready:
                raise RuntimeError("concurrent host synthetic guests did not boot")
            if procs[0].pid == procs[1].pid:
                raise RuntimeError("guest processes are not independent")
            terminated_first = stop(procs[0], signal.SIGTERM)
            other_survived = procs[1].poll() is None
            if not other_survived:
                raise RuntimeError("terminating app-alpha also killed app-beta")
            terminated_second = stop(procs[1], signal.SIGKILL)
            residual_before = [str(s["agent"]) for s in sessions if s["agent"].exists()]
            # SIGKILL may leave a stale UNIX socket. This host harness is the
            # cleanup owner and must not mistake process death for full cleanup.
            for path in [s["agent"] for s in sessions]:
                if path.exists():
                    path.unlink()
            for err in streams:
                err.close()
            streams.clear()
            stopped = all(p.poll() is not None for p in procs)
            sockets_cleared = all(not s["agent"].exists() for s in sessions)
            if not (stopped and sockets_cleared):
                raise RuntimeError("host harness left residual process or socket")
            return {
                "schema": "prime.w8.host-synthetic-runtime.v1",
                "status": "HOST_SYNTHETIC_PASS_NOT_W8_RELEASE",
                "qemu_path": qemu,
                "qemu_sha256": sha256_file(Path(qemu)),
                "boot_sector_sha256": sha256(sector),
                "guest_type": "original x86 BIOS bootsector, not Windows",
                "boot_markers": [MARKER in s["serial"].read_bytes() for s in sessions],
                "serial_sha256": [sha256(s["serial"].read_bytes()) for s in sessions],
                "independent_processes": procs[0].pid != procs[1].pid,
                "distinct_application_storage": True,
                "distinct_agent_sockets": True,
                "other_session_survived_first_termination": other_survived,
                "forced_second_termination": terminated_second,
                "graceful_first_termination": terminated_first,
                "stale_socket_paths_cleaned_by_harness": len(residual_before),
                "managed_qemu_processes_stopped": stopped,
                "agent_sockets_cleared_after_cleanup": sockets_cleared,
                "host_networking": False,
                "usb_passthrough": False,
                "guest_agent_handoff_proven": False,
                "windows_guest_execution_proven": False,
                "release_authority": False,
            }
    finally:
        for proc in procs:
            if proc.poll() is None:
                stop(proc, signal.SIGKILL)
        for stream in streams:
            stream.close()
        os.umask(prior_mask)


if __name__ == "__main__":
    try:
        print(json.dumps(run_probe(), indent=2, sort_keys=True))
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(json.dumps({"status": "FAIL", "error": str(error),
                          "release_authority": False}), flush=True)
        raise SystemExit(1)
