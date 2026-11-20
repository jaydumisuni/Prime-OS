#!/usr/bin/env python3
"""Run the ACTUAL read-only Rust adapter in a Linux Unix-socket fixture.

Validates wire compatibility with Android HOME's PrimeCoreClient requests.
Not Android init, SELinux, a phone, the full Core or a ROM boot test.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time


def exchange(path: Path, request: bytes) -> tuple[int, dict]:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as conn:
        conn.settimeout(3.0)
        conn.connect(str(path))
        conn.sendall(request)
        data = bytearray()
        while True:
            part = conn.recv(4096)
            if not part:
                break
            if len(data) + len(part) > 131072:
                raise RuntimeError("UNBOUNDED_CORE_RESPONSE")
            data.extend(part)
    raw = bytes(data)
    head, sep, body = raw.partition(b"\r\n\r\n")
    if sep != b"\r\n\r\n" or not head.startswith(b"HTTP/1.1 "):
        raise AssertionError("CORE_INVALID_HTTP")
    code = int(head[9:12])
    header = head.lower()
    length = next(
        (int(row.split(b":", 1)[1].strip()) for row in header.split(b"\r\n")
         if row.startswith(b"content-length:")), None)
    if length != len(body):
        raise AssertionError("CORE_HTTP_BODY_LENGTH_MISMATCH")
    return code, json.loads(body)


def check(binary: Path) -> None:
    if not binary.is_file():
        raise RuntimeError("RUST_CORE_BINARY_MISSING")
    with tempfile.TemporaryDirectory(prefix="prime-mobile-core-test-") as temp:
        unix_path = Path(temp) / "core.sock"
        env = {**os.environ, "PRIME_MOBILE_TEST_SOCKET": str(unix_path)}
        proc = subprocess.Popen(
            [str(binary.resolve())], env=env,
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL)
        try:
            for _ in range(100):
                if unix_path.is_socket():
                    break
                if proc.poll() is not None:
                    raise RuntimeError("CORE_EXITED_BEFORE_SOCKET")
                time.sleep(0.03)
            else:
                raise RuntimeError("CORE_SOCKET_STARTUP_TIMEOUT")
            get = lambda path, header=b"": (
                b"GET " + path.encode("ascii") +
                b" HTTP/1.1\r\nHost: localhost\r\n" + header +
                b"Connection: close\r\n\r\n")
            code, versions = exchange(unix_path, get("/v1/versions"))
            assert code == 200
            assert versions == {
                "interface": "prime.capability.v1",
                "supported_versions": ["1.0"]}
            code, capabilities = exchange(
                unix_path, get("/v1/capabilities", b"Prime-Interface-Accept: 1.0\r\n"))
            assert code == 200
            assert capabilities["interface"] == "prime.capability.v1"
            assert capabilities["interface_version"] == "1.0"
            assert capabilities["generation_id"] == "ANDROID_UNBOUND_NOT_ATTESTED"
            assert len(capabilities["capabilities"]) == 1
            descriptor = capabilities["capabilities"][0]
            assert descriptor["capability_id"] == "prime.hardware.read"
            assert descriptor["availability"] == "DEGRADED"
            assert descriptor["rollback"]["supported"] is False
            code, hardware = exchange(
                unix_path, get("/v1/hardware", b"Prime-Interface-Accept: 1.0\r\n"))
            assert code == 200
            assert hardware["graph"]["schema"] == "prime.hardware-graph.v1"
            assert "No bootc generation binding or phone device admission" in hardware["graph"]["limitations"]
            assert hardware["generation_id"] == "ANDROID_UNBOUND_NOT_ATTESTED"
            code, _ = exchange(unix_path, get("/v1/capabilities"))
            assert code == 400
            code, _ = exchange(
                unix_path, get("/v1/capabilities", b"Prime-Interface-Accept: 9.0\r\n"))
            assert code == 409
            code, _ = exchange(
                unix_path, b"POST /v1/system/power HTTP/1.1\r\n"
                           b"Prime-Interface-Accept: 1.0\r\n\r\n")
            assert code == 405
            print("PRIME_ANDROID_NATIVE_CORE_UNIX_WIRE_ACCEPTANCE_UNQUALIFIED")
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=3)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: verify_runtime.py <compiled-host-core-binary>")
    check(Path(sys.argv[1]))
