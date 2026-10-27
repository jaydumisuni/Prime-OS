"""P2 review-input manifest must bind exact committed bytes without self-qualification."""
from __future__ import annotations

import importlib.util
import json
import os
import shlex
import shutil
from pathlib import Path
import subprocess
import sys

SCRIPT = Path(__file__).resolve().parents[1] / "tools/p2_component_review_inputs.py"
spec = importlib.util.spec_from_file_location("p2_review_inputs", SCRIPT)
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.modules[spec.name] = module
spec.loader.exec_module(module)


def git(root: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()


def sample_repo(tmp_path: Path) -> tuple[Path, str]:
    root = tmp_path / "p2"
    root.mkdir()
    git(root, "init", "-q")
    for relative in module.INPUTS:
        file = root / relative
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(f"p2 committed input: {relative}\n", encoding="utf-8")
    git(root, "add", "--", *module.INPUTS)
    git(root, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
        "commit", "-qm", "test: P2 committed review inputs")
    return root, git(root, "rev-parse", "HEAD")


def test_exact_head_manifest_is_deterministic_and_not_a_certificate(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    first = module.build_manifest(root, head)
    assert first == module.build_manifest(root, head)
    assert first["candidate_head"] == head
    assert first["independent_qualification"] == "NOT_PROVEN"
    assert first["authority"] == "REVIEW_INPUT_ONLY"
    assert len(first["inputs"]) == len(module.INPUTS)
    assert all(len(item["sha256"]) == 64 for item in first["inputs"])


def test_ambient_path_cannot_replace_review_git_binary(
    tmp_path: Path, monkeypatch,
) -> None:
    root, head = sample_repo(tmp_path)
    system_git = shutil.which("git", path=os.defpath)
    assert system_git is not None
    shadow_bin = tmp_path / "shadow-bin"
    shadow_bin.mkdir()
    marker = tmp_path / "shadow-git-was-executed"
    shadow_git = shadow_bin / "git"
    shadow_git.write_text(
        "#!/bin/sh\n"
        + "printf invoked > " + shlex.quote(str(marker)) + "\n"
        + "exec " + shlex.quote(system_git) + " \"$@\"\n",
        encoding="utf-8",
    )
    shadow_git.chmod(0o755)
    monkeypatch.setenv("PATH", str(shadow_bin) + os.pathsep + os.environ.get("PATH", ""))
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert not marker.exists(), "ambient PATH must not run a shadow Git executable"


def test_ambient_dynamic_loader_controls_cannot_reach_review_git(
    tmp_path: Path, monkeypatch,
) -> None:
    # Git is a child process: inheriting LD_DEBUG/LD_PRELOAD can expose or
    # execute attacker-controlled loader state despite a pinned Git binary.
    if sys.platform != "linux":
        import pytest
        pytest.skip("Linux dynamic-loader custody regression")
    root, head = sample_repo(tmp_path)
    trace_prefix = tmp_path / "untrusted-loader-trace"
    monkeypatch.setenv("LD_DEBUG", "libs")
    monkeypatch.setenv("LD_DEBUG_OUTPUT", str(trace_prefix))
    assert module.build_manifest(root, head)["candidate_head"] == head
    assert not list(tmp_path.glob("untrusted-loader-trace.*")), (
        "ambient loader configuration reached a Git subprocess"
    )


def test_rejects_wrong_head(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    try:
        module.build_manifest(root, "0" * 40)
    except module.ReviewInputError as error:
        assert "HEAD changed" in str(error)
    else:
        raise AssertionError("mismatched HEAD admitted")


def test_rejects_modified_input_without_changing_git(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    (root / module.INPUTS[-1]).write_text("uncommitted mutation\n", encoding="utf-8")
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "local changes" in str(error)
    else:
        raise AssertionError("dirty review input admitted")
    assert git(root, "rev-parse", "HEAD") == head


def test_rejects_staged_input_even_when_worktree_bytes_match_head(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    relative = module.INPUTS[0]
    file = root / relative
    committed = file.read_bytes()
    file.write_bytes(b"staged alternate component review input\n")
    git(root, "add", "--", relative)
    file.write_bytes(committed)
    assert file.read_bytes() == committed
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "local changes" in str(error)
    else:
        raise AssertionError("staged index substitution admitted as committed review input")


def test_rejects_symlink_substitution(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    file = root / module.INPUTS[-1]
    original = file.read_bytes()
    file.unlink()
    target = root / "outside-input"
    target.write_bytes(original)
    file.symlink_to(target)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "noncanonical" in str(error)
    else:
        raise AssertionError("symlink substitution admitted")


def test_rejects_symlinked_worktree_root_even_when_git_head_matches(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    alias = tmp_path / "aliased-worktree"
    alias.symlink_to(root, target_is_directory=True)
    try:
        module.build_manifest(alias, head)
    except module.ReviewInputError as error:
        assert "canonical" in str(error)
    else:
        raise AssertionError("symlinked worktree root admitted as review authority")


def test_rejects_hardlinked_review_input_with_identical_bytes(tmp_path: Path) -> None:
    import os
    root, head = sample_repo(tmp_path)
    file = root / module.INPUTS[-1]
    outside = tmp_path / "outside-review-input"
    os.link(file, outside)
    assert outside.read_bytes() == file.read_bytes()
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "hardlink" in str(error)
    else:
        raise AssertionError("hardlinked review input admitted as isolated authority")


def test_cli_fails_closed_and_emits_no_manifest_on_head_mismatch(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    run = subprocess.run(
        [sys.executable, str(SCRIPT), "--repo", str(root), "--expected-head", "0" * 40],
        capture_output=True, text=True
    )
    assert run.returncode != 0
    assert not run.stdout.strip()
    assert "HEAD changed" in run.stderr


def test_rejects_same_bytes_path_replacement_during_read(tmp_path: Path, monkeypatch) -> None:
    root, head = sample_repo(tmp_path)
    target = root / module.INPUTS[-1]
    original_read = module.read_bounded_working_input
    def replaced_read(file: Path, before) -> bytes:
        content = original_read(file, before)
        if file == target:
            file.unlink()
            file.write_bytes(content)
        return content
    monkeypatch.setattr(module, "read_bounded_working_input", replaced_read)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "changed during read" in str(error)
    else:
        raise AssertionError("same-byte inode replacement admitted")


def test_rejects_same_bytes_inplace_rewrite_during_read(tmp_path: Path, monkeypatch) -> None:
    root, head = sample_repo(tmp_path)
    target = root / module.INPUTS[-1]
    original_read = module.read_bounded_working_input
    def rewritten_read(file: Path, before) -> bytes:
        content = original_read(file, before)
        if file == target:
            file.write_bytes(content)
        return content
    monkeypatch.setattr(module, "read_bounded_working_input", rewritten_read)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "changed during read" in str(error)
    else:
        raise AssertionError("same-byte in-place rewrite admitted")


def test_rejects_group_writable_review_root(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    root.chmod(0o770)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "writable" in str(error)
    else:
        raise AssertionError("group-writable review root admitted")


def test_rejects_world_writable_input_parent(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    parent = root / "crates/primed/src"
    parent.chmod(0o777)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "writable" in str(error)
    else:
        raise AssertionError("world-writable review input parent admitted")


def test_rejects_shared_writable_ancestor_above_review_root(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    parent = tmp_path / "shared-parent"
    parent.mkdir()
    moved = parent / "p2"
    root.rename(moved)
    parent.chmod(0o777)
    try:
        try:
            module.build_manifest(moved, head)
        except module.ReviewInputError as error:
            assert "writable" in str(error)
        else:
            raise AssertionError("shared-writable ancestor admitted as review authority")
    finally:
        parent.chmod(0o755)


def test_ambient_git_index_override_cannot_hide_staged_review_input(tmp_path: Path) -> None:
    import os

    root, head = sample_repo(tmp_path)
    clean_index = tmp_path / "clean-index"
    clean_index.write_bytes((root / ".git" / "index").read_bytes())
    relative = module.INPUTS[0]
    file = root / relative
    committed = file.read_bytes()
    file.write_bytes(b"staged hostile component input\n")
    git(root, "add", "--", relative)
    file.write_bytes(committed)

    env = os.environ.copy()
    env["GIT_INDEX_FILE"] = str(clean_index)
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--repo", str(root), "--expected-head", head],
        capture_output=True, text=True, env=env,
    )
    assert result.returncode != 0
    assert "local changes" in result.stderr


def test_rejects_group_writable_review_input_file(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    file = root / module.INPUTS[-1]
    file.chmod(0o664)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "writable" in str(error)
    else:
        raise AssertionError("group-writable review input admitted")


def test_rejects_foreign_owned_input_parent_even_if_not_shared_writable(
    tmp_path: Path, monkeypatch
) -> None:
    from types import SimpleNamespace
    root, head = sample_repo(tmp_path)
    target = root / "crates/primed/src"
    original_lstat = Path.lstat

    def foreign_lstat(path: Path, *args, **kwargs):
        state = original_lstat(path, *args, **kwargs)
        if path == target:
            return SimpleNamespace(st_mode=state.st_mode, st_uid=module.os.geteuid() + 1000)
        return state

    monkeypatch.setattr(Path, "lstat", foreign_lstat)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "owner" in str(error)
    else:
        raise AssertionError("foreign-owned input parent admitted")


def test_rejects_foreign_owned_nonwritable_ancestor(
    tmp_path: Path, monkeypatch
) -> None:
    from types import SimpleNamespace
    root, head = sample_repo(tmp_path)
    target = root.parent
    original_lstat = Path.lstat

    def foreign_lstat(path: Path, *args, **kwargs):
        state = original_lstat(path, *args, **kwargs)
        if path == target:
            return SimpleNamespace(st_mode=state.st_mode, st_uid=module.os.geteuid() + 1000)
        return state

    monkeypatch.setattr(Path, "lstat", foreign_lstat)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "owner" in str(error)
    else:
        raise AssertionError("foreign-owned ancestor admitted")


def test_repository_fsmonitor_is_never_executed_by_review_git(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    marker = tmp_path / "fsmonitor-invoked"
    hook = tmp_path / "hostile-fsmonitor.sh"
    hook.write_text(
        "#!/bin/sh\nprintf invoked > " + shlex.quote(str(marker)) + "\n",
        encoding="utf-8",
    )
    hook.chmod(0o700)
    git(root, "config", "core.fsmonitor", str(hook))

    result = module.build_manifest(root, head)
    assert result["candidate_head"] == head
    assert not marker.exists(), "review inspection executed repository-configured fsmonitor"

def test_git_replace_refs_cannot_substitute_pinned_commit_inputs(
    tmp_path: Path, monkeypatch
) -> None:
    # Git replacement refs can report clean status and the expected HEAD
    # while returning bytes from a different commit for HEAD:path.
    monkeypatch.delenv("GIT_NO_REPLACE_OBJECTS", raising=False)
    root, head = sample_repo(tmp_path)
    for relative in module.INPUTS:
        (root / relative).write_text(f"substituted input: {relative}\n", encoding="utf-8")
    git(root, "add", "--", *module.INPUTS)
    git(root, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
        "commit", "-qm", "untrusted substitute commit")
    alternate = git(root, "rev-parse", "HEAD")
    git(root, "checkout", "-q", head)
    git(root, "replace", head, alternate)
    for relative in module.INPUTS:
        (root / relative).write_text(f"substituted input: {relative}\n", encoding="utf-8")
    git(root, "add", "--", *module.INPUTS)
    assert git(root, "rev-parse", "HEAD") == head
    assert not git(root, "status", "--porcelain", "--", *module.INPUTS)
    assert git(root, "show", f"{head}:{module.INPUTS[0]}").startswith("substituted input")
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "local changes" in str(error) or "differs from HEAD" in str(error)
    else:
        raise AssertionError("Git replace ref substituted exact-head review authority")


def test_review_input_status_never_executes_repository_clean_filter(
    tmp_path: Path
) -> None:
    # Git status can run a repository-configured clean filter to compare a
    # tracked file; review evidence must not execute checkout-supplied code.
    root, head = sample_repo(tmp_path)
    marker = tmp_path / "filter-executed"
    script = tmp_path / "hostile-clean-filter.sh"
    script.write_text(
        "#!/bin/sh\ncat\nprintf executed > " + shlex.quote(str(marker)) + "\n",
        encoding="utf-8",
    )
    script.chmod(0o700)
    git(root, "config", "filter.reviewtest.clean", str(script))
    relative = module.INPUTS[-1]
    (root / ".gitattributes").write_text(relative + " filter=reviewtest\n", encoding="utf-8")
    (root / relative).touch()
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError:
        pass  # Rejecting untrusted filter authority is permitted.
    assert not marker.exists(), "review preflight executed repository clean filter"


def test_promisor_missing_blob_never_executes_repository_uploadpack(tmp_path: Path) -> None:
    """Pinned review evidence must fail closed rather than fetch missing objects."""
    root, head = sample_repo(tmp_path)
    marker = tmp_path / "untrusted-uploadpack-invoked"
    uploadpack = tmp_path / "untrusted-uploadpack.sh"
    uploadpack.write_text(
        "#!/bin/sh\nprintf invoked >> " + shlex.quote(str(marker)) + "\nexit 1\n",
        encoding="utf-8",
    )
    uploadpack.chmod(0o700)
    git(root, "config", "remote.origin.url", str(tmp_path / "missing-remote"))
    git(root, "config", "remote.origin.promisor", "true")
    git(root, "config", "remote.origin.uploadpack", str(uploadpack))
    git(root, "config", "extensions.partialClone", "origin")

    blob = git(root, "rev-parse", f"HEAD:{module.INPUTS[0]}")
    loose = root / ".git" / "objects" / blob[:2] / blob[2:]
    assert loose.is_file(), "disposable fixture must have a loose committed blob"
    loose.unlink()

    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--repo", str(root), "--expected-head", head],
        capture_output=True, text=True,
    )
    assert result.returncode == 2, result.stderr
    assert not marker.exists(), "review inspection launched repository-selected upload-pack"
    assert "FAIL:" in result.stderr


def test_rejects_oversized_committed_review_blob(tmp_path: Path) -> None:
    """Review evidence must not materialize arbitrarily large committed blobs."""
    root, _ = sample_repo(tmp_path)
    relative = module.INPUTS[0]
    (root / relative).write_bytes(b"x" * (8 * 1024 * 1024 + 1))
    git(root, "add", "--", relative)
    git(root, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
        "commit", "-qm", "test: oversized review input")
    head = git(root, "rev-parse", "HEAD")
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "size" in str(error) or "large" in str(error)
    else:
        raise AssertionError("oversized committed blob admitted as bounded review evidence")


def test_fifo_swap_at_descriptor_open_is_nonblocking_and_rejected(
    tmp_path: Path, monkeypatch
) -> None:
    """An attacker must not stall read-only evidence by swapping in a FIFO."""
    import os

    root, head = sample_repo(tmp_path)
    target = root / module.INPUTS[-1]
    real_open = os.open
    swapped = False

    def swap_to_fifo(path, flags, *args, **kwargs):
        nonlocal swapped
        if Path(path) == target and not swapped:
            assert flags & os.O_NONBLOCK, "review input open could block on a FIFO"
            target.unlink()
            os.mkfifo(target, 0o600)
            swapped = True
        return real_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(module.os, "open", swap_to_fifo)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "changed before descriptor open" in str(error)
    else:
        raise AssertionError("FIFO substitution admitted as regular review input")
    assert swapped, "adversarial FIFO replacement was not exercised"


def test_rejects_symlink_swap_at_descriptor_open(tmp_path: Path, monkeypatch) -> None:
    """The verified input pathname cannot redirect the actual file open."""
    root, head = sample_repo(tmp_path)
    target = root / module.INPUTS[-1]
    saved = tmp_path / "same-bytes-external-input"
    real_open = module.os.open
    opened = []

    def substitute(path, flags, *args, **kwargs):
        if Path(path) == target:
            opened.append(True)
            saved.write_bytes(target.read_bytes())
            target.unlink()
            target.symlink_to(saved)
        return real_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(module.os, "open", substitute)
    try:
        module.build_manifest(root, head)
    except (module.ReviewInputError, OSError):
        pass
    else:
        raise AssertionError("review input path substitution admitted")
    assert opened, "review input was not opened through descriptor-bound custody"

def test_git_verification_timeout_fails_closed(tmp_path: Path, monkeypatch) -> None:
    """An unresponsive Git metadata probe must not hang review custody."""
    root, head = sample_repo(tmp_path)

    def timeout_git(*args, **kwargs):
        assert kwargs["timeout"] == module.GIT_READ_TIMEOUT_SECONDS
        raise subprocess.TimeoutExpired(args[0], kwargs["timeout"])

    monkeypatch.setattr(module.subprocess, "run", timeout_git)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "Git verification timed out" in str(error)
    else:
        raise AssertionError("timed-out Git probe admitted review evidence")


def test_rejects_ancestor_becoming_shared_writable_during_input_read(
    tmp_path: Path, monkeypatch,
) -> None:
    """Review authority must not outlive the protected worktree ancestry."""
    import pytest

    root, head = sample_repo(tmp_path)
    target = root / module.INPUTS[-1]
    original_read = module.read_bounded_working_input
    original_mode = tmp_path.stat().st_mode & 0o777

    def change_ancestor_after_read(file: Path, before) -> bytes:
        content = original_read(file, before)
        if file == target:
            tmp_path.chmod(0o777)
        return content

    monkeypatch.setattr(module, "read_bounded_working_input", change_ancestor_after_read)
    try:
        with pytest.raises(module.ReviewInputError, match="ancestor"):
            module.build_manifest(root, head)
    finally:
        tmp_path.chmod(original_mode)


def test_rejects_review_root_becoming_shared_writable_during_final_git_read(
    tmp_path: Path, monkeypatch,
) -> None:
    """The final Git read must not outlive private worktree-root custody."""
    import pytest

    root, head = sample_repo(tmp_path)
    original_git = module.git
    original_mode = root.stat().st_mode & 0o777
    head_reads = 0

    def change_root_after_final_git(repo: Path, *args: str) -> bytes:
        nonlocal head_reads
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_reads += 1
            if head_reads == 2:
                root.chmod(0o777)
        return result

    monkeypatch.setattr(module, "git", change_root_after_final_git)
    try:
        with pytest.raises(module.ReviewInputError, match="shared-writable review path"):
            module.build_manifest(root, head)
        assert head_reads == 2, "final Git HEAD read was not exercised"
    finally:
        root.chmod(original_mode)



def test_rejects_earlier_input_changed_after_last_input_read(
    tmp_path: Path, monkeypatch,
) -> None:
    """A prior accepted input cannot change while a later input is read."""
    import pytest

    root, head = sample_repo(tmp_path)
    earlier = root / module.INPUTS[0]
    last = root / module.INPUTS[-1]
    original_read = module.read_bounded_working_input

    def change_earlier_after_later_read(file: Path, before) -> bytes:
        content = original_read(file, before)
        if file == last:
            earlier.write_bytes(b"late replacement of earlier review input\n")
        return content

    monkeypatch.setattr(module, "read_bounded_working_input",
                        change_earlier_after_later_read)
    with pytest.raises(module.ReviewInputError, match="changed|local changes"):
        module.build_manifest(root, head)


def test_rejects_earlier_index_restage_after_last_input_read(
    tmp_path: Path, monkeypatch,
) -> None:
    """A late staged-only replacement must not retain clean review authority."""
    import pytest

    root, head = sample_repo(tmp_path)
    earlier = root / module.INPUTS[0]
    last = root / module.INPUTS[-1]
    committed = earlier.read_bytes()
    original_read = module.read_bounded_working_input

    def stage_earlier_after_later_read(file: Path, before) -> bytes:
        content = original_read(file, before)
        if file == last:
            earlier.write_bytes(b"late staged review input\n")
            git(root, "add", "--", module.INPUTS[0])
            earlier.write_bytes(committed)
        return content

    monkeypatch.setattr(module, "read_bounded_working_input",
                        stage_earlier_after_later_read)
    with pytest.raises(module.ReviewInputError, match="changed|local changes"):
        module.build_manifest(root, head)


def test_rejects_head_and_prior_input_mutation_during_final_index_probe(
    tmp_path: Path, monkeypatch,
) -> None:
    """A late Git subprocess must not invalidate already-finalized evidence."""
    import pytest

    root, head = sample_repo(tmp_path)
    earlier = root / module.INPUTS[0]
    last = module.INPUTS[-1]
    original_git = module.git
    last_index_reads = 0

    def mutate_during_last_index_probe(repo: Path, *args: str) -> bytes:
        nonlocal last_index_reads
        result = original_git(repo, *args)
        if args == ("ls-files", "--stage", "--", last):
            last_index_reads += 1
            if last_index_reads == 3:
                earlier.write_bytes(b"changed during final Git probe\n")
                git(root, "-c", "user.name=Test", "-c",
                    "user.email=test@example.invalid", "commit",
                    "--allow-empty", "-qm", "test: concurrent HEAD advance")
        return result

    monkeypatch.setattr(module, "git", mutate_during_last_index_probe)
    with pytest.raises(module.ReviewInputError, match="HEAD changed|changed after verification|local changes"):
        module.build_manifest(root, head)
    assert last_index_reads == 3, "adversarial final Git probe was not exercised"
    assert git(root, "rev-parse", "HEAD") != head


def test_rejects_prior_input_mutation_during_last_index_probe_without_head_change(
    tmp_path: Path, monkeypatch,
) -> None:
    """The last index subprocess must precede the final filesystem sweep."""
    import pytest

    root, head = sample_repo(tmp_path)
    earlier = root / module.INPUTS[0]
    last = module.INPUTS[-1]
    original_git = module.git
    last_index_reads = 0

    def mutate_earlier_during_last_probe(repo: Path, *args: str) -> bytes:
        nonlocal last_index_reads
        result = original_git(repo, *args)
        if args == ("ls-files", "--stage", "--", last):
            last_index_reads += 1
            if last_index_reads == 3:
                earlier.write_bytes(b"changed during final Git probe\n")
        return result

    monkeypatch.setattr(module, "git", mutate_earlier_during_last_probe)
    with pytest.raises(module.ReviewInputError, match="changed after verification"):
        module.build_manifest(root, head)
    assert last_index_reads == 3
    assert git(root, "rev-parse", "HEAD") == head


def test_rejects_index_only_restage_after_final_git_probe(
    tmp_path: Path, monkeypatch,
) -> None:
    """A post-probe staged-only edit must invalidate review-input authority."""
    import pytest

    root, head = sample_repo(tmp_path)
    relative = module.INPUTS[0]
    blob = subprocess.check_output(
        ["git", "-C", str(root), "hash-object", "-w", "--stdin"],
        input=b"late index-only replacement\n",
    ).decode().strip()
    original_git = module.git
    original_private_directory = module.require_private_directory
    head_reads = 0
    after_final_git = False
    injected = False

    def observe_final_head(repo: Path, *args: str) -> bytes:
        nonlocal head_reads, after_final_git
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_reads += 1
            if head_reads == 2:
                after_final_git = True
        return result

    def restage_after_git(directory: Path) -> None:
        nonlocal injected
        if after_final_git and not injected:
            git(root, "update-index", "--cacheinfo", f"100644,{blob},{relative}")
            injected = True
        original_private_directory(directory)

    monkeypatch.setattr(module, "git", observe_final_head)
    monkeypatch.setattr(module, "require_private_directory", restage_after_git)
    with pytest.raises(module.ReviewInputError, match="index changed"):
        module.build_manifest(root, head)
    assert injected and head_reads == 2
    assert (root / relative).read_text() == f"p2 committed input: {relative}\n"
    assert git(root, "rev-parse", "HEAD") == head
    assert git(root, "rev-parse", f":{relative}") == blob



def test_rejects_head_ref_advance_after_final_git_probe(
    tmp_path: Path, monkeypatch,
) -> None:
    """A late ref-only HEAD advance cannot retain an old exact-head manifest."""
    import pytest

    root, head = sample_repo(tmp_path)
    env = os.environ.copy()
    env.update({
        "GIT_AUTHOR_NAME": "Test",
        "GIT_AUTHOR_EMAIL": "test@example.invalid",
        "GIT_COMMITTER_NAME": "Test",
        "GIT_COMMITTER_EMAIL": "test@example.invalid",
    })
    successor = subprocess.check_output(
        ["git", "-C", str(root), "commit-tree", f"{head}^{{tree}}", "-p", head],
        input=b"concurrent ref-only advance\n", env=env,
    ).decode().strip()
    original_git = module.git
    original_private = module.require_private_directory
    final_head_seen = False
    injected = False
    head_reads = 0

    def observe_final_head(repo: Path, *args: str) -> bytes:
        nonlocal final_head_seen, head_reads
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_reads += 1
            if head_reads == 2:
                final_head_seen = True
        return result

    def advance_ref_after_final_git(directory: Path) -> None:
        nonlocal injected
        if final_head_seen and not injected:
            git(root, "update-ref", "HEAD", successor)
            injected = True
        original_private(directory)

    monkeypatch.setattr(module, "git", observe_final_head)
    monkeypatch.setattr(module, "require_private_directory",
                        advance_ref_after_final_git)
    with pytest.raises(module.ReviewInputError, match="HEAD.*changed|ref.*changed"):
        module.build_manifest(root, head)
    assert injected and head_reads == 2
    assert git(root, "rev-parse", "HEAD") == successor
    assert not git(root, "diff", "--cached", "--name-only")



def test_head_epoch_supports_packed_detached_and_linked_worktrees(
    tmp_path: Path,
) -> None:
    """Ref custody must not reject ordinary Git storage and worktree forms."""
    root, head = sample_repo(tmp_path)
    git(root, "pack-refs", "--all")
    assert module.build_manifest(root, head)["candidate_head"] == head

    git(root, "checkout", "-q", "--detach", head)
    assert module.build_manifest(root, head)["candidate_head"] == head

    linked = tmp_path / "linked-review"
    git(root, "worktree", "add", "-q", "-b", "linked-review", str(linked), head)
    assert module.build_manifest(linked, head)["candidate_head"] == head


def test_rejects_dangling_loose_ref_after_packed_head_snapshot(
    tmp_path: Path,
) -> None:
    """A dangling symlink is an existing loose ref inode, despite exists() false."""
    import pytest

    root, head = sample_repo(tmp_path)
    git(root, "pack-refs", "--all")
    snapshot = module.capture_head_epoch(root, head)
    _, absent_ref = snapshot
    assert absent_ref is not None, "fixture must use a packed branch ref"
    assert not absent_ref.exists()

    # The packed branch was valid at capture time. A late untrusted loose-ref
    # pathname must invalidate the snapshot even if its symlink target is absent.
    absent_ref.symlink_to(tmp_path / "nonexistent-ref-target")
    assert absent_ref.is_symlink()
    assert not absent_ref.exists()
    with pytest.raises(module.ReviewInputError, match="HEAD ref changed"):
        module.verify_head_epoch(snapshot)



def test_rejects_linked_worktree_gitdir_pointer_swap_after_final_git(
    tmp_path: Path, monkeypatch,
) -> None:
    """A late .git redirection cannot rebind an already verified worktree."""
    import pytest

    root, head = sample_repo(tmp_path)
    first = tmp_path / "linked-first"
    second = tmp_path / "linked-second"
    git(root, "worktree", "add", "-q", "-b", "linked-first", str(first), head)
    git(root, "worktree", "add", "-q", "-b", "linked-second", str(second), head)
    pointer = first / ".git"
    original = pointer.read_bytes()
    alternate = (second / ".git").read_bytes()
    assert original != alternate
    assert module.build_manifest(first, head)["candidate_head"] == head

    original_git = module.git
    head_reads = 0
    swapped = False

    def redirect_after_final_git(repo: Path, *args: str) -> bytes:
        nonlocal head_reads, swapped
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_reads += 1
            if head_reads == 2:
                pointer.write_bytes(alternate)
                swapped = True
        return result

    monkeypatch.setattr(module, "git", redirect_after_final_git)
    try:
        with pytest.raises(module.ReviewInputError, match="gitdir|control|changed"):
            module.build_manifest(first, head)
        assert swapped and head_reads == 2
    finally:
        pointer.write_bytes(original)


def test_rejects_duplicate_packed_head_ref_entries(tmp_path: Path) -> None:
    # A duplicate earlier entry can be ignored by Git's last-entry-wins
    # resolution, but is not a canonical source of review authority.
    for name, duplicate in (("conflicting", "0" * 40), ("identical", None)):
        case_dir = tmp_path / name
        case_dir.mkdir()
        root, head = sample_repo(case_dir)
        git(root, "pack-refs", "--all", "--prune")
        packed = root / ".git" / "packed-refs"
        branch = git(root, "symbolic-ref", "HEAD")
        lines = packed.read_text(encoding="utf-8").splitlines()
        assert len(lines) == 2
        packed.write_text(
            lines[0] + "\n"
            + (duplicate or head) + " " + branch + "\n"
            + lines[1] + "\n",
            encoding="utf-8",
        )
        assert git(root, "rev-parse", "HEAD") == head
        try:
            module.build_manifest(root, head)
        except module.ReviewInputError as error:
            assert "ambiguous packed HEAD ref" in str(error)
        else:
            raise AssertionError("duplicate packed HEAD ref admitted")


def test_rejects_pending_git_control_lockfiles(tmp_path: Path) -> None:
    """A pending Git update must not be presented as a stable review epoch."""
    import pytest

    for kind in ("index", "HEAD", "branch", "packed-refs"):
        case = tmp_path / kind
        case.mkdir()
        root, head = sample_repo(case)
        gitdir = root / ".git"
        if kind == "branch":
            ref = git(root, "symbolic-ref", "HEAD")
            lock = gitdir / (ref + ".lock")
        else:
            lock = gitdir / (kind + ".lock")
        lock.parent.mkdir(parents=True, exist_ok=True)
        lock.write_bytes(b"pending update\n")
        with pytest.raises(module.ReviewInputError, match="pending Git control update"):
            module.build_manifest(root, head)


def test_rejects_late_git_control_lock_after_final_probe(
    tmp_path: Path, monkeypatch,
) -> None:
    """A lock arriving after final Git probes invalidates the review manifest."""
    import pytest

    root, head = sample_repo(tmp_path)
    lock = root / ".git" / "index.lock"
    original_git = module.git
    original_private = module.require_private_directory
    head_reads = 0
    after_final_git = False
    injected = False

    def observe_head(repo: Path, *args: str) -> bytes:
        nonlocal head_reads, after_final_git
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_reads += 1
            if head_reads == 2:
                after_final_git = True
        return result

    def introduce_pending_update(directory: Path) -> None:
        nonlocal injected
        if after_final_git and not injected:
            lock.write_bytes(b"pending update\n")
            injected = True
        original_private(directory)

    monkeypatch.setattr(module, "git", observe_head)
    monkeypatch.setattr(module, "require_private_directory", introduce_pending_update)
    with pytest.raises(module.ReviewInputError, match="pending Git control update"):
        module.build_manifest(root, head)
    assert injected and head_reads == 2



def test_rejects_late_linked_worktree_commondir_redirection(
    tmp_path: Path, monkeypatch,
) -> None:
    """A post-probe commondir redirect must invalidate exact-head custody."""
    import pytest

    root, head = sample_repo(tmp_path)
    linked = tmp_path / "linked-review"
    git(root, "worktree", "add", "-q", "-b", "linked-review", str(linked), head)
    alternate = tmp_path / "alternate"
    subprocess.check_call(
        ["git", "clone", "-q", "--no-hardlinks", str(root), str(alternate)]
    )
    assert git(alternate, "rev-parse", "HEAD") == head
    gitdir = Path((linked / ".git").read_text().split("gitdir: ", 1)[1].strip())
    commondir = gitdir / "commondir"
    original = commondir.read_bytes()
    original_git = module.git
    head_reads = 0
    injected = False

    def redirect_after_final_git(repo: Path, *args: str) -> bytes:
        nonlocal head_reads, injected
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_reads += 1
            if head_reads == 2:
                commondir.write_text(str(alternate / ".git") + "\n")
                injected = True
        return result

    monkeypatch.setattr(module, "git", redirect_after_final_git)
    try:
        with pytest.raises(module.ReviewInputError, match="commondir|HEAD.*changed"):
            module.build_manifest(linked, head)
        assert injected and head_reads == 2
    finally:
        commondir.write_bytes(original)
