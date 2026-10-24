"""P2 review-input manifest must bind exact committed bytes without self-qualification."""
from __future__ import annotations

import importlib.util
import json
import shlex
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
    original_read = Path.read_bytes
    def replaced_read(file: Path) -> bytes:
        content = original_read(file)
        if file == target:
            file.unlink()
            file.write_bytes(content)
        return content
    monkeypatch.setattr(Path, "read_bytes", replaced_read)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "changed during read" in str(error)
    else:
        raise AssertionError("same-byte inode replacement admitted")


def test_rejects_same_bytes_inplace_rewrite_during_read(tmp_path: Path, monkeypatch) -> None:
    root, head = sample_repo(tmp_path)
    target = root / module.INPUTS[-1]
    original_read = Path.read_bytes
    def rewritten_read(file: Path) -> bytes:
        content = original_read(file)
        if file == target:
            file.write_bytes(content)
        return content
    monkeypatch.setattr(Path, "read_bytes", rewritten_read)
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
