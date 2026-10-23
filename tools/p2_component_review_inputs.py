#!/usr/bin/env python3
"""Emit a read-only, exact-commit P2 component review-input manifest.

This is construction evidence, never independent qualification or release authority.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
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
    # Ambient Git overrides may substitute the index, object database, or
    # worktree and falsely certify a different candidate as this exact HEAD.
    # Trust only the repository resolved from the explicit -C path.
    trusted_env = {key: value for key, value in os.environ.items()
                   if not key.startswith("GIT_")}
    result = subprocess.run(
        ["git", "-C", str(repo), *args], capture_output=True, check=False,
        env=trusted_env,
    )
    if result.returncode:
        raise ReviewInputError(f"Git verification failed: {' '.join(args)}")
    return result.stdout


def require_private_directory(directory: Path) -> None:
    """A review input cannot derive authority through a shared-writable parent."""
    mode = directory.lstat().st_mode
    if not stat.S_ISDIR(mode) or mode & 0o022:
        raise ReviewInputError(f"non-directory or shared-writable review path: {directory}")



def require_protected_ancestors(repo: Path) -> None:
    """Reject replaceable worktree ancestors; permit owner-protected sticky /tmp."""
    child = repo
    for parent in repo.parents:
        child_mode = child.lstat()
        parent_mode = parent.lstat()
        if not stat.S_ISDIR(parent_mode.st_mode):
            raise ReviewInputError(f"non-directory review ancestor: {parent}")
        if parent_mode.st_mode & 0o022:
            sticky = bool(parent_mode.st_mode & stat.S_ISVTX)
            trusted_parent = parent_mode.st_uid in (0, os.geteuid())
            protected_child = child_mode.st_uid == os.geteuid()
            if not (sticky and trusted_parent and protected_child):
                raise ReviewInputError(f"shared-writable review ancestor: {parent}")
        child = parent


def build_manifest(repo: Path, expected_head: str) -> dict:
    if not re.fullmatch(r"[0-9a-f]{40}", expected_head):
        raise ReviewInputError("expected HEAD must be a full lowercase commit SHA")
    # The caller's path itself is review authority: resolving an alias and
    # comparing only resolved paths would silently accept symlinked roots.
    if not repo.is_absolute() or repo.resolve() != repo:
        raise ReviewInputError("review workspace must be a canonical absolute worktree root")
    require_private_directory(repo)
    require_protected_ancestors(repo)
    root = Path(git(repo, "rev-parse", "--show-toplevel").decode().strip()).resolve()
    if root != repo.resolve():
        raise ReviewInputError("review workspace must be the exact Git worktree root")
    head = git(repo, "rev-parse", "HEAD").decode().strip()
    if head != expected_head:
        raise ReviewInputError("review candidate HEAD changed")
    entries = []
    for relative in INPUTS:
        directory = root
        for component in Path(relative).parts[:-1]:
            directory /= component
            require_private_directory(directory)
        file = root / relative
        if file.is_symlink() or not file.is_file() or file.resolve() != file:
            raise ReviewInputError(f"noncanonical or missing review input: {relative}")
        if git(root, "status", "--porcelain", "--", relative).strip():
            raise ReviewInputError(f"review input has local changes: {relative}")
        committed = git(root, "show", f"{head}:{relative}")
        before = file.lstat()
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
            raise ReviewInputError(f"hardlinked or nonregular review input: {relative}")
        working = file.read_bytes()
        directory = root
        require_private_directory(directory)
        for component in Path(relative).parts[:-1]:
            directory /= component
            require_private_directory(directory)
        after = file.lstat()
        if after.st_nlink != 1:
            raise ReviewInputError(f"hardlinked review input changed during read: {relative}")
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
