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
    # Local refs/replace must not reinterpret the object graph being reviewed.
    # Ignore user/system Git configuration while retaining repository-local settings.
    trusted_env["GIT_CONFIG_NOSYSTEM"] = "1"
    trusted_env["GIT_CONFIG_GLOBAL"] = os.devnull
    trusted_env["GIT_NO_REPLACE_OBJECTS"] = "1"
    # Exact-head review is read-only: status must not refresh the index on disk.
    trusted_env["GIT_OPTIONAL_LOCKS"] = "0"
    # A local evidence probe must not wait for credentials or read caller stdin.
    trusted_env["GIT_TERMINAL_PROMPT"] = "0"
    # Partial-clone object reads must not trigger an implicit network fetch.
    trusted_env["GIT_NO_LAZY_FETCH"] = "1"
    # Repository or inherited Git config may name an executable fsmonitor.
    # `git status` must never invoke it while constructing review evidence.
    try:
        result = subprocess.run(
            ["git", "-c", "core.fsmonitor=false", "-C", str(repo), *args],
            capture_output=True, check=False, env=trusted_env,
            stdin=subprocess.DEVNULL, timeout=15,
        )
    except subprocess.TimeoutExpired as exc:
        raise ReviewInputError("Git verification timed out") from exc
    except OSError as exc:
        raise ReviewInputError("Git verification could not start") from exc
    if result.returncode:
        raise ReviewInputError(f"Git verification failed: {' '.join(args)}")
    return result.stdout


def require_private_directory(directory: Path) -> None:
    """A review input cannot derive authority through a shared-writable parent."""
    try:
        state = directory.lstat()
    except OSError as exc:
        raise ReviewInputError(f"review directory cannot be inspected: {directory}") from exc
    if not stat.S_ISDIR(state.st_mode) or state.st_mode & 0o022:
        raise ReviewInputError(f"non-directory or shared-writable review path: {directory}")
    if state.st_mode & 0o7000:
        raise ReviewInputError(f"special permission bits on review directory: {directory}")
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
        if parent_mode.st_mode & (stat.S_ISUID | stat.S_ISGID):
            raise ReviewInputError(f"special permission bits on review ancestor: {parent}")
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
    executable_modes = {}
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
        # Git may be configured with core.filemode=false, hiding executable-bit
        # drift from status. Bind the worktree mode to the committed tree.
        tree_record = git(root, "ls-tree", head, "--", relative).decode().strip()
        expected_mode = tree_record.split(None, 1)[0] if tree_record else ""
        if expected_mode not in ("100644", "100755"):
            raise ReviewInputError(f"unexpected committed review input mode: {relative}")
        before = file.lstat()
        if before.st_mode & 0o7000:
            raise ReviewInputError(f"special permission bits on review input: {relative}")
        if bool(before.st_mode & 0o111) != (expected_mode == "100755"):
            raise ReviewInputError(f"review input executable mode differs from HEAD: {relative}")
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
            raise ReviewInputError(f"hardlinked or nonregular review input: {relative}")
        if before.st_uid not in (0, os.geteuid()) or before.st_mode & 0o022:
            raise ReviewInputError(f"untrusted owner or shared-writable review input: {relative}")
        working = file.read_bytes()
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
        identity = lambda state: (
            state.st_dev, state.st_ino, state.st_mode, state.st_size,
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
        if not re.fullmatch(r"[0-9a-f]{40}", blob):
            raise ReviewInputError(f"invalid committed blob identity: {relative}")
        executable_modes[relative] = expected_mode == "100755"
        entries.append({
            "path": relative,
            "git_blob": blob,
            "sha256": hashlib.sha256(committed).hexdigest(),
            "bytes": len(committed),
        })
    # Earlier input directories may be replaced after their own read.
    # Recheck all source parents before emitting a complete review manifest.
    for relative in INPUTS:
        directory = root
        require_private_directory(directory)
        for component in Path(relative).parts[:-1]:
            directory /= component
            require_private_directory(directory)
    # A previously inspected source can change while later inputs are read.
    # Revalidate every source against its recorded committed digest before
    # publishing the aggregate manifest.
    for entry in entries:
        file = root / entry["path"]
        require_private_directory(file.parent)
        state = file.lstat()
        if (not stat.S_ISREG(state.st_mode) or state.st_nlink != 1
                or state.st_uid not in (0, os.geteuid())
                or state.st_mode & (0o022 | 0o7000)
                or bool(state.st_mode & 0o111) != executable_modes[entry["path"]]
                or file.is_symlink() or file.resolve() != file):
            raise ReviewInputError(f"review input custody changed: {entry['path']}")
        final_bytes = file.read_bytes()
        final_state = file.lstat()
        if (not stat.S_ISREG(final_state.st_mode) or final_state.st_nlink != 1
                or final_state.st_uid not in (0, os.geteuid())
                or final_state.st_mode & (0o022 | 0o7000)
                or file.is_symlink() or file.resolve() != file
                or identity(state) != identity(final_state)):
            raise ReviewInputError(f"review input custody changed during final read: {entry['path']}")
        if hashlib.sha256(final_bytes).hexdigest() != entry["sha256"]:
            raise ReviewInputError(f"review input changed after inspection: {entry['path']}")
        if git(root, "status", "--porcelain", "--", entry["path"]).strip():
            raise ReviewInputError(f"review input changed after inspection: {entry['path']}")
    # Final digest reads can also mutate an earlier source directory.
    # Recheck every parent after the final read phase, not only before it.
    for relative in INPUTS:
        directory = root
        require_private_directory(directory)
        for component in Path(relative).parts[:-1]:
            directory /= component
            require_private_directory(directory)
    # A later final read may mutate an earlier input after its digest check.
    # Bind the complete set again before releasing aggregate review evidence.
    for entry in entries:
        file = root / entry["path"]
        state = file.lstat()
        if (not stat.S_ISREG(state.st_mode) or state.st_nlink != 1
                or state.st_uid not in (0, os.geteuid())
                or state.st_mode & (0o022 | 0o7000)
                or bool(state.st_mode & 0o111) != executable_modes[entry["path"]]
                or file.is_symlink() or file.resolve() != file
                or hashlib.sha256(file.read_bytes()).hexdigest() != entry["sha256"]
                or identity(state) != identity(file.lstat())
                or git(root, "status", "--porcelain", "--", entry["path"]).strip()):
            raise ReviewInputError(f"review input changed during final verification: {entry['path']}")
    # The last digest pass may itself change a previously checked parent.
    for relative in INPUTS:
        directory = root
        require_private_directory(directory)
        for component in Path(relative).parts[:-1]:
            directory /= component
            require_private_directory(directory)
    # Ancestor custody can change while committed inputs are being inspected.
    require_protected_ancestors(root)
    if git(root, "rev-parse", "HEAD").decode().strip() != head:
        raise ReviewInputError("review candidate HEAD changed during inspection")
    # The final Git identity query is also an observation boundary: a concurrent
    # permission change during that query must not leave a trusted root behind.
    require_private_directory(root)
    require_protected_ancestors(root)
    # The last Git identity query can also race a source rewrite. Rebind each
    # input to its recorded digest after that query before returning evidence.
    for entry in entries:
        file = root / entry["path"]
        require_private_directory(file.parent)
        before = file.lstat()
        if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1
                or before.st_uid not in (0, os.geteuid())
                or before.st_mode & (0o022 | 0o7000)
                or bool(before.st_mode & 0o111) != executable_modes[entry["path"]]
                or file.is_symlink() or file.resolve() != file):
            raise ReviewInputError(f"review input changed after final HEAD query: {entry['path']}")
        content = file.read_bytes()
        after = file.lstat()
        if (identity(before) != identity(after)
                or hashlib.sha256(content).hexdigest() != entry["sha256"]):
            raise ReviewInputError(f"review input changed after final HEAD query: {entry['path']}")
    # The post-HEAD digest reads can themselves change a previously checked
    # parent. Revalidate all source directories at the publication boundary.
    for relative in INPUTS:
        directory = root
        require_private_directory(directory)
        for component in Path(relative).parts[:-1]:
            directory /= component
            require_private_directory(directory)
    require_protected_ancestors(root)
    # A later post-HEAD read can rewrite an earlier source after its own check.
    # Rebind all recorded digests at the final publication boundary.
    for entry in entries:
        file = root / entry["path"]
        before = file.lstat()
        if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1
                or before.st_uid not in (0, os.geteuid())
                or before.st_mode & (0o022 | 0o7000)
                or bool(before.st_mode & 0o111) != executable_modes[entry["path"]]
                or file.is_symlink() or file.resolve() != file):
            raise ReviewInputError(f"review input custody changed at publication: {entry['path']}")
        content = file.read_bytes()
        if (identity(before) != identity(file.lstat())
                or hashlib.sha256(content).hexdigest() != entry["sha256"]):
            raise ReviewInputError(f"review input changed at publication: {entry['path']}")
    require_protected_ancestors(root)
    # Final source reads can race a worktree HEAD switch; never publish
    # a manifest under a candidate identity no longer checked out.
    if git(root, "rev-parse", "HEAD").decode().strip() != head:
        raise ReviewInputError("review candidate HEAD changed at publication")
    # The last identity query must not silently relax root/ancestor custody.
    require_private_directory(root)
    require_protected_ancestors(root)
    # The publication HEAD query can race a source rewrite just as earlier
    # identity queries can. Rebind source bytes after that final query.
    for entry in entries:
        file = root / entry["path"]
        require_private_directory(file.parent)
        before = file.lstat()
        if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1
                or before.st_uid not in (0, os.geteuid())
                or before.st_mode & (0o022 | 0o7000)
                or bool(before.st_mode & 0o111) != executable_modes[entry["path"]]
                or file.is_symlink() or file.resolve() != file):
            raise ReviewInputError(f"review input custody changed after publication HEAD: {entry['path']}")
        content = file.read_bytes()
        if (identity(before) != identity(file.lstat())
                or hashlib.sha256(content).hexdigest() != entry["sha256"]
                or git(root, "status", "--porcelain", "--", entry["path"]).strip()):
            raise ReviewInputError(f"review input changed after publication HEAD: {entry['path']}")
    for relative in INPUTS:
        directory = root
        require_private_directory(directory)
        for component in Path(relative).parts[:-1]:
            directory /= component
            require_private_directory(directory)
    require_protected_ancestors(root)
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
    except (ReviewInputError, OSError, UnicodeError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
