#!/usr/bin/env python3
import argparse
import json
import pathlib
import re

RECOVERY_UNIT = "systemd.unit=prime-recovery.target"
RECOVERY_MARKER = "prime.recovery=1"
COMPOSEFS_PATTERN = re.compile(r"composefs=\??[0-9a-f]{128}")
COMPOSEFS_DIGEST = re.compile(r"[0-9a-f]{128}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--inspect-json", required=True)
    parser.add_argument("--normal-out", required=True)
    parser.add_argument("--recovery-out", required=True)
    parser.add_argument("--composefs-digest", default="")
    args = parser.parse_args()

    data = json.loads(pathlib.Path(args.inspect_json).read_text(encoding="utf-8"))
    cmdline = data[".cmdline"]["text"].strip()
    if not cmdline:
        raise SystemExit("provisional UKI command line is empty")
    if RECOVERY_UNIT in cmdline or RECOVERY_MARKER in cmdline:
        raise SystemExit("provisional UKI unexpectedly contains Prime recovery markers")

    normal = cmdline
    if args.composefs_digest:
        if COMPOSEFS_DIGEST.fullmatch(args.composefs_digest) is None:
            raise SystemExit("invalid canonical Composefs digest")
        normal, replacements = COMPOSEFS_PATTERN.subn(
            f"composefs={args.composefs_digest}", normal
        )
        if replacements != 1:
            raise SystemExit(
                f"expected exactly one Composefs token in provisional UKI, found {replacements}"
            )

    recovery = f"{normal} {RECOVERY_UNIT} {RECOVERY_MARKER}"
    pathlib.Path(args.normal_out).write_text(normal, encoding="utf-8")
    pathlib.Path(args.recovery_out).write_text(recovery, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
