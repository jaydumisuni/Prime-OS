#!/usr/bin/env python3
"""Emit a read-only, exact-commit P2 component review-input manifest.

This is construction evidence, never independent qualification or release authority.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import stat
import sys

INPUTS = (
    "Cargo.lock",
    "crates/prime-contracts/Cargo.toml",
    "crates/prime-contracts/src/component.rs",
    "crates/primed/Cargo.toml",
    "crates/primed/src/component_package.rs",
)


class ReviewInputError(Exception):
    pass


def git(repo: Path, *args: str) -> bytes:
    result = subprocess.run(
        ["git", "-C", str(repo), *args], capture_output=True, check=False
    )
    if result.returncode:
        raise ReviewInputError(f"Git verification failed: {' '.join(args)}")
    return result.stdout


def build_manifest(repo: Path, expected_head: str) -> dict:
    if not re.fullmatch(r"[0-9a-f]{40}", expected_head):
        raise ReviewInputError("expected HEAD must be a full lowercase commit SHA")
    root = Path(git(repo, "rev-parse", "--show-toplevel").decode().strip()).resolve()
    if root != repo.resolve():
        raise ReviewInputError("review workspace must be the exact Git worktree root")
    head = git(repo, "rev-parse", "HEAD").decode().strip()
    if head != expected_head:
        raise ReviewInputError("review candidate HEAD changed")
    entries = []
    for relative in INPUTS:
        file = root / relative
        if file.is_symlink() or not file.is_file() or file.resolve() != file:
            raise ReviewInputError(f"noncanonical or missing review input: {relative}")
        if git(root, "status", "--porcelain", "--", relative).strip():
            raise ReviewInputError(f"review input has local changes: {relative}")
        committed = git(root, "show", f"{head}:{relative}")
        before = file.lstat()
        working = file.read_bytes()
        after = file.lstat()
        identity = lambda state: (
            state.st_dev, state.st_ino, state.st_size,
            state.st_mtime_ns, state.st_ctime_ns,
        )
        if (not stat.S_ISREG(after.st_mode) or file.resolve() != file
                or identity(before) != identity(after)):
            raise ReviewInputError(f"review input changed during read: {relative}")
        if git(root, "status", "--porcelain", "--", relative).strip():
            raise ReviewInputError(f"review input changed during read: {relative}")
        if committed != working:
            raise ReviewInputError(f"review input differs from HEAD: {relative}")
        blob = git(root, "rev-parse", f"{head}:{relative}").decode().strip()
        entries.append({
            "path": relative,
            "git_blob": blob,
            "sha256": hashlib.sha256(committed).hexdigest(),
            "bytes": len(committed),
        })
    if git(root, "rev-parse", "HEAD").decode().strip() != head:
        raise ReviewInputError("review candidate HEAD changed during inspection")
    return {
        "schema": "prime.p2.component-review-inputs.v1",
        "candidate_head": head,
        "inputs": entries,
        "independent_qualification": "NOT_PROVEN",
        "authority": "REVIEW_INPUT_ONLY",
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--expected-head", required=True)
    args = parser.parse_args(argv)
    try:
        manifest = build_manifest(args.repo, args.expected_head)
    except (ReviewInputError, OSError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
