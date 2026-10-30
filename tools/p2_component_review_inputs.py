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
MAX_GIT_OBJECT_ENTRIES = 100_000
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



def git_control_path(root: Path, name: str) -> Path:
    """Resolve a Git metadata path before the final read-only Git probes."""
    value = git(root, "rev-parse", "--git-path", name).decode().strip()
    path = Path(value)
    return path if path.is_absolute() else root / path


def control_file_epoch(path: Path, limit: int) -> tuple[tuple[int, ...], bytes, bytes]:
    """Read a private Git control file through its verified regular descriptor."""
    if path.resolve() != path:
        raise ReviewInputError("noncanonical HEAD control path")
    require_private_directory(path.parent)
    require_protected_ancestors(path.parent)
    before = path.lstat()
    if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1
            or before.st_uid not in (0, os.geteuid())
            or before.st_mode & 0o022 or before.st_size > limit):
        raise ReviewInputError("untrusted HEAD control file")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK
                 | getattr(os, "O_CLOEXEC", 0))
    with os.fdopen(fd, "rb") as stream:
        opened = os.fstat(stream.fileno())
        if stat_identity(opened) != stat_identity(before):
            raise ReviewInputError("HEAD control file changed before read")
        data = stream.read(limit + 1)
        if len(data) > limit or stat_identity(os.fstat(stream.fileno())) != stat_identity(before):
            raise ReviewInputError("HEAD control file changed during read")
    return stat_identity(before), hashlib.sha256(data).digest(), data


def capture_gitdir_pointer_epoch(root: Path, head_path: Path) -> tuple | None:
    """Pin the linked-worktree .git pointer to the HEAD metadata directory."""
    dotgit = root / ".git"
    state = dotgit.lstat()
    if stat.S_ISDIR(state.st_mode):
        require_private_directory(dotgit)
        return None
    if not stat.S_ISREG(state.st_mode):
        raise ReviewInputError("untrusted worktree gitdir control")
    epoch, digest, data = control_file_epoch(dotgit, 4096)
    # Git worktree pointers may be relative to the worktree. Do not trust a
    # different gitdir merely because it resolves the same commit and inputs.
    line = data.rstrip(b"\n")
    if (not line.startswith(b"gitdir: ") or not line[8:]
            or b"\n" in line or b"\r" in line):
        raise ReviewInputError("invalid worktree gitdir control")
    target = Path(os.fsdecode(line[8:]))
    if not target.is_absolute():
        target = root / target
    if target.resolve() != head_path.parent.resolve():
        raise ReviewInputError("worktree gitdir control differs from HEAD authority")
    return (dotgit, 4096, epoch, digest)


def capture_head_epoch(root: Path, expected_head: str) -> tuple:
    """Pin HEAD, linked gitdir and branch refs without granting release authority."""
    head_path = git_control_path(root, "HEAD")
    symbolic = git(root, "rev-parse", "--symbolic-full-name", "HEAD").decode().strip()
    head_epoch, head_digest, head_data = control_file_epoch(head_path, 4096)
    files = [(head_path, 4096, head_epoch, head_digest)]
    absent_commondir = None
    pointer = capture_gitdir_pointer_epoch(root, head_path)
    if pointer is not None:
        files.append(pointer)
    # commondir can redirect Git's common refs and objects even when .git is
    # an ordinary directory. Capture its presence or absence independently
    # of whether this worktree uses a .git pointer.
    commondir = head_path.parent / "commondir"
    try:
        commondir.lstat()
    except FileNotFoundError:
        if pointer is not None:
            # A standalone --separate-git-dir repository has no commondir.
            # Its Git common directory must remain the pinned HEAD directory.
            common_name = git(
                root, "rev-parse", "--path-format=absolute", "--git-common-dir"
            ).decode().strip()
            if Path(common_name).resolve() != head_path.parent.resolve():
                raise ReviewInputError("separate gitdir differs from HEAD authority")
        absent_commondir = commondir
    else:
        if pointer is None:
            # A normal .git directory has no authorized linked-worktree
            # redirection. Fail closed rather than admitting an unpinned
            # alternate common refs/object store.
            raise ReviewInputError("unbound ordinary Git commondir control")
        common_epoch, common_digest, common_data = control_file_epoch(
            commondir, 4096
        )
        common_line = common_data.rstrip(b"\n")
        if (not common_line or b"\n" in common_line
                or b"\r" in common_line or b"\x00" in common_line):
            raise ReviewInputError("invalid linked worktree commondir control")
        common_target = Path(os.fsdecode(common_line))
        if not common_target.is_absolute():
            common_target = head_path.parent / common_target
        expected_common = head_path.parent.parent.parent
        if common_target.resolve() != expected_common.resolve():
            raise ReviewInputError("linked worktree commondir differs from Git authority")
        require_private_directory(expected_common)
        require_protected_ancestors(expected_common)
        files.append((commondir, 4096, common_epoch, common_digest))
    if symbolic == "HEAD":
        if head_data.strip() != expected_head.encode():
            raise ReviewInputError("detached HEAD control differs from expected commit")
        return files, None, absent_commondir
    if (not symbolic.startswith("refs/heads/")
            or any(part in ("", ".", "..") for part in symbolic.split("/"))):
        raise ReviewInputError("unsupported symbolic HEAD control")
    if head_data.strip() != ("ref: " + symbolic).encode():
        raise ReviewInputError("symbolic HEAD control differs from Git resolution")
    ref_path = git_control_path(root, symbolic)
    try:
        ref_epoch, ref_digest, ref_data = control_file_epoch(ref_path, 4096)
    except FileNotFoundError:
        # Packed branches are valid, but the absence of a loose ref is also
        # part of the epoch: a newly created loose ref must invalidate it.
        packed = git_control_path(root, "packed-refs")
        packed_epoch, packed_digest, packed_data = control_file_epoch(
            packed, MAX_REVIEW_INPUT_BYTES)
        entries = [tuple(line.split(b" ", 1)) for line in packed_data.splitlines()
                   if b" " in line and not line.startswith(b"#")]
        # Git can resolve the last of duplicate packed ref records. A matching
        # final record must not conceal a conflicting or repeated earlier one:
        # review evidence requires one unambiguous authoritative ref identity.
        matches = [oid for oid, ref in entries if ref == symbolic.encode()]
        if len(matches) != 1:
            raise ReviewInputError("ambiguous packed HEAD ref entries")
        if matches[0] != expected_head.encode():
            raise ReviewInputError("packed HEAD ref differs from expected commit")
        files.append((packed, MAX_REVIEW_INPUT_BYTES, packed_epoch, packed_digest))
        return files, ref_path, absent_commondir
    if ref_data.strip() != expected_head.encode():
        raise ReviewInputError("loose HEAD ref differs from expected commit")
    files.append((ref_path, 4096, ref_epoch, ref_digest))
    return files, None, absent_commondir


def verify_head_epoch(snapshot: tuple) -> None:
    """No Git subprocess follows the final HEAD probe; verify pinned ref files."""
    files, absent_ref, absent_commondir = snapshot
    try:
        for path, limit, epoch, digest in files:
            current, current_digest, _ = control_file_epoch(path, limit)
            if current != epoch or current_digest != digest:
                raise ReviewInputError("HEAD ref changed after final Git probe")
        # Both packed-ref absence and standalone gitdir commondir absence
        # are inode claims: even a dangling symlink invalidates the epoch.
        for missing in (absent_ref, absent_commondir):
            if missing is None:
                continue
            try:
                missing.lstat()
            except FileNotFoundError:
                pass
            else:
                if missing == absent_ref:
                    raise ReviewInputError("HEAD ref changed from packed to loose")
                raise ReviewInputError("HEAD commondir appeared after final Git probe")
    except OSError as exc:
        raise ReviewInputError("HEAD ref changed after final Git probe") from exc

def require_no_pending_git_control_locks(paths: tuple[Path, ...]) -> None:
    """Reject in-flight Git metadata writes, including dangling lock symlinks."""
    for lock in paths:
        try:
            lock.lstat()
        except FileNotFoundError:
            continue
        except OSError as exc:
            raise ReviewInputError("cannot inspect pending Git control update") from exc
        raise ReviewInputError(f"pending Git control update: {lock}")


def capture_config_epochs(paths: tuple[Path, ...]) -> tuple[list, list]:
    """Pin present Git config files and absence of optional worktree config."""
    present, absent = [], []
    for path in paths:
        try:
            epoch, digest, _ = control_file_epoch(path, MAX_REVIEW_INPUT_BYTES)
        except FileNotFoundError:
            # Missing config is a meaningful state. Its parent must be
            # trustworthy, and later creation invalidates the review epoch.
            require_private_directory(path.parent)
            require_protected_ancestors(path.parent)
            absent.append(path)
        else:
            present.append((path, MAX_REVIEW_INPUT_BYTES, epoch, digest))
    return present, absent


def require_no_unbound_git_config_includes(root: Path) -> None:
    """Reject Git config sources outside the pinned local/worktree config epoch.

    An include.path or includeIf path can load mutable external configuration
    without changing either captured repository config inode. Query Git's own
    case-normalized parser with includes disabled, including inactive rules:
    their future activation must not silently change review interpretation.
    """
    records = git(root, "config", "--no-includes", "--null", "--list")
    for record in records.split(b"\x00"):
        key = record.split(b"\n", 1)[0].lower()
        if key.startswith((b"include.", b"includeif.")):
            raise ReviewInputError("unbound Git configuration include")


def require_no_object_alternates(path: Path) -> None:
    """No mutable external Git object store may supply committed review bytes."""
    require_private_directory(path.parent)
    require_protected_ancestors(path.parent)
    try:
        # lstat also catches dangling symlinks, unlike Path.exists().
        path.lstat()
    except FileNotFoundError:
        return
    except OSError as exc:
        raise ReviewInputError("cannot inspect Git object alternates") from exc
    raise ReviewInputError("unbound Git object alternates authority")



def capture_git_object_epoch(objects: Path) -> dict[str, tuple]:
    """Pin private, non-aliased local Git objects across review probes.

    Loose objects and pack/index sidecars share one object directory. A
    hardlinked object has an independently mutable external pathname, even
    when HEAD and every working review input remain unchanged. Refuse such
    storage rather than presenting its bytes as independently held evidence.
    """
    # Caller resolved this path before the final Git probes. Inventory must
    # remain filesystem-only so no later Git subprocess can race input checks.
    if objects.resolve() != objects:
        raise ReviewInputError("noncanonical Git object store")
    require_private_directory(objects)
    require_protected_ancestors(objects)
    # Mode 0700 on the object root protects descendants even when Git creates
    # group-writable fanout directories. If group traversal is allowed, a
    # group-writable descendant is independently mutable and must be rejected.
    group_can_traverse_objects = bool(objects.lstat().st_mode & stat.S_IXGRP)
    entries: dict[str, tuple] = {}
    pending = [objects]
    while pending:
        directory = pending.pop()
        directory_state = directory.lstat()
        # A group-writable descendant is unsafe only when the group can
        # traverse the object root. World-writable entries remain forbidden.
        if (not stat.S_ISDIR(directory_state.st_mode)
                or directory_state.st_uid not in (0, os.geteuid())
                or directory_state.st_mode & stat.S_IWOTH
                or (group_can_traverse_objects
                    and directory_state.st_mode & stat.S_IWGRP)):
            raise ReviewInputError("untrusted Git object store directory")
        for item in directory.iterdir():
            if len(entries) >= MAX_GIT_OBJECT_ENTRIES:
                raise ReviewInputError("Git object store exceeds bounded review inventory")
            state = item.lstat()
            relative = item.relative_to(objects).as_posix()
            if stat.S_ISDIR(state.st_mode):
                pending.append(item)
            elif stat.S_ISREG(state.st_mode):
                if state.st_nlink != 1:
                    raise ReviewInputError("Git object store hardlinked external storage")
                if (state.st_uid not in (0, os.geteuid())
                        or state.st_mode & stat.S_IWOTH
                        or (group_can_traverse_objects
                            and state.st_mode & stat.S_IWGRP)):
                    raise ReviewInputError("untrusted Git object store file")
            else:
                raise ReviewInputError("nonregular Git object store entry")
            entries[relative] = (
                stat.S_IFMT(state.st_mode), state.st_uid,
                state.st_mode & 0o7777, state.st_nlink,
                *stat_identity(state),
            )
    return entries

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
    # Resolve Git's common object directory (also for linked worktrees).
    # Refuse external alternates before reading any committed review blobs.
    alternates_path = git_control_path(root, "objects/info/alternates")
    objects_path = git_control_path(root, "objects")
    require_no_object_alternates(alternates_path)
    object_epoch = capture_git_object_epoch(objects_path)
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
    head_epoch = capture_head_epoch(root, head)
    # A split index delegates tracked entries to sharedindex.<oid>, outside
    # the index inode pinned below. A late replacement of that external file
    # can change Git's staged interpretation without changing the pinned
    # index or HEAD. Fail closed until sharedindex epochs are independently
    # captured and verified; ordinary and linked full indexes remain valid.
    if git(root, "rev-parse", "--shared-index-path").strip():
        raise ReviewInputError("unbound Git split index sharedindex authority")
    index_name = git(root, "rev-parse", "--git-path", "index").decode().strip()
    index_path = Path(index_name)
    if not index_path.is_absolute():
        index_path = root / index_path
    index_epoch = require_index_epoch(index_path)
    # Git writes metadata via adjacent *.lock files before atomically
    # replacing the authoritative pathname. A stable current inode alone
    # cannot prove that no pending update is already underway.
    head_files, absent_ref, absent_commondir = head_epoch
    # Git config updates can rebind object and worktree interpretation during
    # the final read-only probes. Treat pending repository or worktree config
    # writes as an unstable review epoch, just like HEAD/index updates.
    config_paths = (
        git_control_path(root, "config"),
        git_control_path(root, "config.worktree"),
    )
    config_files, absent_config = capture_config_epochs(config_paths)
    require_no_unbound_git_config_includes(root)
    # Git config can change without leaving a .lock file (e.g. direct
    # replacement). Bind its actual inode/bytes and optional-file absence.
    head_files.extend(config_files)
    control_paths = [
        index_path,
        git_control_path(root, "packed-refs"),
        *config_paths,
    ]
    control_paths.extend(path for path, _, _, _ in head_files)
    if absent_ref is not None:
        control_paths.append(absent_ref)
    if absent_commondir is not None:
        control_paths.append(absent_commondir)
    lock_paths = tuple(dict.fromkeys(
        path.with_name(path.name + ".lock") for path in control_paths
    ))
    require_no_pending_git_control_locks(lock_paths)
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
    verify_head_epoch(head_epoch)
    for missing in absent_config:
        try:
            missing.lstat()
        except FileNotFoundError:
            pass
        else:
            raise ReviewInputError("Git config appeared after final Git probe")
    require_no_pending_git_control_locks(lock_paths)
    # No Git subprocess follows the final HEAD probe; the absence of an
    # external object-store pointer is part of the final review epoch.
    require_no_object_alternates(alternates_path)
    if capture_git_object_epoch(objects_path) != object_epoch:
        raise ReviewInputError("Git object store changed after final Git probe")
    # The bounded object inventory is a filesystem-only operation. Reconfirm
    # the control epoch afterward without executing another Git subprocess.
    if require_index_epoch(index_path) != index_epoch:
        raise ReviewInputError("review index changed after object-store verification")
    verify_head_epoch(head_epoch)
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
