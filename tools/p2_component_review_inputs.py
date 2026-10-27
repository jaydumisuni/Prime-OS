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
import shutil
import subprocess
import stat
import sys

MAX_REVIEW_INPUT_BYTES = 8 * 1024 * 1024
GIT_READ_TIMEOUT_SECONDS = 30
# Resolve Git only from the host system path, never caller-controlled PATH.
SYSTEM_GIT = shutil.which("git", path=os.defpath)

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
    # A pinned Git executable is insufficient if the child inherits loader
    # controls (LD_PRELOAD/LD_AUDIT/LD_DEBUG), Git configuration locations,
    # or other caller-supplied process state. Construct a minimal environment
    # rather than trying to enumerate every hostile inherited variable.
    trusted_env = {
        "PATH": os.defpath,
        "LC_ALL": "C",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_NO_LAZY_FETCH": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
    }
    # Repository-local fsmonitor is explicitly disabled in the argv below.
    # Neither ambient config nor Git helpers may expand this read-only scope.
    if SYSTEM_GIT is None:
        raise ReviewInputError("system Git executable unavailable")
    try:
        result = subprocess.run(
            [SYSTEM_GIT, "-c", "core.fsmonitor=false", "-C", str(repo), *args],
            capture_output=True, check=False, env=trusted_env,
            timeout=GIT_READ_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired as exc:
        raise ReviewInputError(
            f"Git verification timed out: {' '.join(args)}"
        ) from exc
    if result.returncode:
        raise ReviewInputError(f"Git verification failed: {' '.join(args)}")
    return result.stdout


def require_clean_index(repo: Path, head: str, relative: str) -> None:
    """Check staged identity against HEAD without invoking clean filters."""
    try:
        tree = git(repo, "ls-tree", head, "--", relative).decode().splitlines()
        index = git(repo, "ls-files", "--stage", "--", relative).decode().splitlines()
        if len(tree) != 1 or len(index) != 1:
            raise ValueError("missing or unmerged index entry")
        tree_fields, tree_path = tree[0].split("\t", 1)
        index_fields, index_path = index[0].split("\t", 1)
        tree_mode, kind, tree_blob = tree_fields.split()
        index_mode, index_blob, stage = index_fields.split()
        if (kind != "blob" or tree_path != relative or index_path != relative
                or stage != "0" or tree_mode != index_mode or tree_blob != index_blob):
            raise ValueError("index differs from committed tree")
    except (ValueError, UnicodeDecodeError):
        raise ReviewInputError(f"review input has local changes: {relative}") from None


def require_private_directory(directory: Path) -> None:
    """A review input cannot derive authority through a shared-writable parent."""
    state = directory.lstat()
    if not stat.S_ISDIR(state.st_mode) or state.st_mode & 0o022:
        raise ReviewInputError(f"non-directory or shared-writable review path: {directory}")
    if state.st_uid not in (0, os.geteuid()):
        raise ReviewInputError(f"untrusted review directory owner: {directory}")



def require_protected_ancestors(repo: Path) -> None:
    """Reject replaceable worktree ancestors; permit owner-protected sticky /tmp."""
    child = repo
    for parent in repo.parents:
        child_mode = child.lstat()
        parent_mode = parent.lstat()
        if not stat.S_ISDIR(parent_mode.st_mode):
            raise ReviewInputError(f"non-directory review ancestor: {parent}")
        if parent_mode.st_uid not in (0, os.geteuid()):
            raise ReviewInputError(f"untrusted review ancestor owner: {parent}")
        if parent_mode.st_mode & 0o022:
            sticky = bool(parent_mode.st_mode & stat.S_ISVTX)
            trusted_parent = parent_mode.st_uid in (0, os.geteuid())
            protected_child = child_mode.st_uid == os.geteuid()
            if not (sticky and trusted_parent and protected_child):
                raise ReviewInputError(f"shared-writable review ancestor: {parent}")
        child = parent


def stat_identity(state: os.stat_result) -> tuple[int, int, int, int, int]:
    return (state.st_dev, state.st_ino, state.st_size,
            state.st_mtime_ns, state.st_ctime_ns)


def require_index_epoch(index_path: Path) -> tuple[int, int, int, int, int]:
    """Bind final review evidence to one private, unchanged Git index file."""
    if index_path.resolve() != index_path:
        raise ReviewInputError("noncanonical review index path")
    require_private_directory(index_path.parent)
    require_protected_ancestors(index_path.parent)
    state = index_path.lstat()
    if (not stat.S_ISREG(state.st_mode) or state.st_nlink != 1
            or state.st_uid not in (0, os.geteuid())
            or state.st_mode & 0o022):
        raise ReviewInputError("untrusted review index")
    return stat_identity(state)


def read_bounded_working_input(file: Path, before: os.stat_result) -> bytes:
    """Bind bounded input bytes to the checked inode, never a later path target."""
    # The checked regular pathname can be replaced by a FIFO before open.
    # O_NONBLOCK prevents a read-only review from hanging before fstat rejects it.
    fd = os.open(
        file,
        os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | getattr(os, "O_CLOEXEC", 0),
    )
    with os.fdopen(fd, "rb") as stream:
        opened = os.fstat(stream.fileno())
        if (not stat.S_ISREG(opened.st_mode) or opened.st_nlink != 1
                or stat_identity(opened) != stat_identity(before)):
            raise ReviewInputError(f"review input changed before descriptor open: {file}")
        if opened.st_uid not in (0, os.geteuid()) or opened.st_mode & 0o022:
            raise ReviewInputError(f"untrusted opened review input: {file}")
        if opened.st_size > MAX_REVIEW_INPUT_BYTES:
            raise ReviewInputError(f"review input exceeds size limit: {file}")
        working = stream.read(MAX_REVIEW_INPUT_BYTES + 1)
        after_fd = os.fstat(stream.fileno())
        if len(working) > MAX_REVIEW_INPUT_BYTES:
            raise ReviewInputError(f"review input exceeds size limit during read: {file}")
        if stat_identity(opened) != stat_identity(after_fd):
            raise ReviewInputError(f"review input changed during descriptor read: {file}")
        return working


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
    verified_inputs = []
    for relative in INPUTS:
        directory = root
        for component in Path(relative).parts[:-1]:
            directory /= component
            require_private_directory(directory)
        file = root / relative
        if file.is_symlink() or not file.is_file() or file.resolve() != file:
            raise ReviewInputError(f"noncanonical or missing review input: {relative}")
        require_clean_index(root, head, relative)
        blob_size_text = git(root, "cat-file", "-s", f"{head}:{relative}")
        try:
            blob_size = int(blob_size_text.strip())
        except ValueError:
            raise ReviewInputError(f"invalid committed review input size: {relative}") from None
        if blob_size > MAX_REVIEW_INPUT_BYTES:
            raise ReviewInputError(f"committed review input exceeds size limit: {relative}")
        committed = git(root, "show", f"{head}:{relative}")
        if len(committed) != blob_size:
            raise ReviewInputError(f"committed review input size changed: {relative}")
        before = file.lstat()
        if before.st_size > MAX_REVIEW_INPUT_BYTES:
            raise ReviewInputError(f"working review input exceeds size limit: {relative}")
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
            raise ReviewInputError(f"hardlinked or nonregular review input: {relative}")
        if before.st_uid not in (0, os.geteuid()) or before.st_mode & 0o022:
            raise ReviewInputError(f"untrusted owner or shared-writable review input: {relative}")
        working = read_bounded_working_input(file, before)
        # An ancestor may become replaceable after the initial path preflight.
        # Recheck before accepting bytes from the opened descriptor.
        require_protected_ancestors(root)
        directory = root
        require_private_directory(directory)
        for component in Path(relative).parts[:-1]:
            directory /= component
            require_private_directory(directory)
        after = file.lstat()
        if after.st_nlink != 1:
            raise ReviewInputError(f"hardlinked review input changed during read: {relative}")
        if after.st_uid not in (0, os.geteuid()) or after.st_mode & 0o022:
            raise ReviewInputError(f"review input owner or permissions changed during read: {relative}")
        if (not stat.S_ISREG(after.st_mode) or file.resolve() != file
                or stat_identity(before) != stat_identity(after)):
            raise ReviewInputError(f"review input changed during read: {relative}")
        require_clean_index(root, head, relative)
        if committed != working:
            raise ReviewInputError(f"review input has local changes: {relative}")
        blob = git(root, "rev-parse", f"{head}:{relative}").decode().strip()
        verified_inputs.append((relative, stat_identity(after)))
        entries.append({
            "path": relative,
            "git_blob": blob,
            "sha256": hashlib.sha256(committed).hexdigest(),
            "bytes": len(committed),
        })
    # The final filesystem sweep cannot observe staged-only changes. Pin the
    # index inode/epoch before the last Git probes and verify it again after
    # the sweep; no later Git subprocess is permitted.
    index_name = git(root, "rev-parse", "--git-path", "index").decode().strip()
    index_path = Path(index_name)
    if not index_path.is_absolute():
        index_path = root / index_path
    index_epoch = require_index_epoch(index_path)
    # Complete every final index probe before the final HEAD check. A Git
    # subprocess can overlap a concurrent mutation of an earlier input;
    # performing probes after final filesystem checks could admit stale
    # review evidence despite an intervening HEAD or worktree change.
    for relative, _ in verified_inputs:
        require_clean_index(root, head, relative)
    if git(root, "rev-parse", "HEAD").decode().strip() != head:
        raise ReviewInputError("review candidate HEAD changed during inspection")
    # No Git subprocesses follow this point. Recheck every admitted input's
    # filesystem identity after all final Git probes have completed.
    for relative, admitted_identity in verified_inputs:
        directory = root
        for component in Path(relative).parts[:-1]:
            directory /= component
            require_private_directory(directory)
        file = root / relative
        current = file.lstat()
        if (not stat.S_ISREG(current.st_mode) or current.st_nlink != 1
                or current.st_uid not in (0, os.geteuid())
                or current.st_mode & 0o022 or file.resolve() != file
                or stat_identity(current) != admitted_identity):
            raise ReviewInputError(f"review input changed after verification: {relative}")
    # The final Git and filesystem reads are untrusted intervals. Recheck
    # the worktree root and its ancestors before returning review evidence.
    require_private_directory(root)
    require_protected_ancestors(root)
    if require_index_epoch(index_path) != index_epoch:
        raise ReviewInputError("review index changed after final Git probe")
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
