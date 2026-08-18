#!/usr/bin/env python3
import argparse
import json
import pathlib

RECOVERY_UNIT = "systemd.unit=prime-recovery.target"
RECOVERY_MARKER = "prime.recovery=1"


def read_json(path: str) -> dict:
    return json.loads(pathlib.Path(path).read_text(encoding="utf-8"))


def read_text(path: str) -> str:
    return pathlib.Path(path).read_text(encoding="utf-8")


def os_release_fields(text: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        value = value.strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
            value = value[1:-1]
        fields[key] = value
    return fields


def require_same_hash(left: dict, right: dict, section: str, label: str) -> None:
    left_hash = left[section]["sha256"]
    right_hash = right[section]["sha256"]
    if left_hash != right_hash:
        raise SystemExit(
            f"{label} {section} hash differs: {left_hash} != {right_hash}"
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--provisional", required=True)
    parser.add_argument("--normal", required=True)
    parser.add_argument("--recovery", required=True)
    parser.add_argument("--normal-cmdline", required=True)
    parser.add_argument("--recovery-cmdline", required=True)
    args = parser.parse_args()

    provisional = read_json(args.provisional)
    normal = read_json(args.normal)
    recovery = read_json(args.recovery)
    normal_cmdline = read_text(args.normal_cmdline)
    recovery_cmdline = read_text(args.recovery_cmdline)

    for section in (".linux", ".initrd", ".osrel", ".uname"):
        require_same_hash(provisional, normal, section, "normal UKI")
    for section in (".linux", ".initrd", ".uname"):
        require_same_hash(provisional, recovery, section, "recovery UKI")

    if normal[".cmdline"]["text"].strip() != normal_cmdline:
        raise SystemExit("normal UKI command line differs from prepared command line")
    if recovery[".cmdline"]["text"].strip() != recovery_cmdline:
        raise SystemExit("recovery UKI command line differs from prepared command line")

    if RECOVERY_UNIT in normal_cmdline or RECOVERY_MARKER in normal_cmdline:
        raise SystemExit("normal UKI contains a recovery selector")
    if RECOVERY_UNIT not in recovery_cmdline or RECOVERY_MARKER not in recovery_cmdline:
        raise SystemExit("recovery UKI is missing a required recovery selector")

    normal_os = os_release_fields(normal[".osrel"]["text"])
    recovery_os = os_release_fields(recovery[".osrel"]["text"])

    expected_normal = {
        "ID": "prime",
        "PRETTY_NAME": "Prime OS P1 First Light",
        "VERSION_ID": "0.1",
    }
    expected_recovery = {
        "ID": "prime",
        "PRETTY_NAME": "Prime OS Recovery",
        "VERSION_ID": "0.0",
        "VARIANT_ID": "recovery",
    }

    for key, expected in expected_normal.items():
        actual = normal_os.get(key)
        if actual != expected:
            raise SystemExit(f"normal UKI {key}={actual!r}, expected {expected!r}")
    for key, expected in expected_recovery.items():
        actual = recovery_os.get(key)
        if actual != expected:
            raise SystemExit(f"recovery UKI {key}={actual!r}, expected {expected!r}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
