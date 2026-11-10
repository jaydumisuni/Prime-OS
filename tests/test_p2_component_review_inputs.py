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


def test_rejects_permission_change_during_review_input_read(tmp_path: Path, monkeypatch) -> None:
    root, head = sample_repo(tmp_path)
    target = root / module.INPUTS[-1]
    original_read = Path.read_bytes

    def changed_mode_during_read(file: Path) -> bytes:
        content = original_read(file)
        if file == target:
            file.chmod(0o600)
        return content

    monkeypatch.setattr(Path, "read_bytes", changed_mode_during_read)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "changed during read" in str(error)
    else:
        raise AssertionError("permission-changing review input admitted")


def test_rejects_group_writable_review_root(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    root.chmod(0o770)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "writable" in str(error)
    else:
        raise AssertionError("group-writable review root admitted")


def test_rejects_special_permission_bits_on_input_parent(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    parent = root / "crates/primed/src"
    for bit in (0o1000, 0o2000, 0o4000):
        parent.chmod(0o755 | bit)
        try:
            module.build_manifest(root, head)
        except module.ReviewInputError as error:
            assert "special permission bits" in str(error)
        else:
            raise AssertionError(f"special directory mode {bit:o} admitted")
        finally:
            parent.chmod(0o755)


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


def test_local_git_replace_ref_cannot_reinterpret_committed_review_bytes(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    relative = module.INPUTS[0]
    original = git(root, "rev-parse", f"{head}:{relative}")
    alternate = subprocess.check_output(
        ["git", "-C", str(root), "hash-object", "-w", "--stdin"],
        input=b"untrusted replacement object\n",
    ).decode().strip()
    git(root, "replace", original, alternate)
    assert subprocess.check_output(
        ["git", "-C", str(root), "show", f"{head}:{relative}"]
    ) == b"untrusted replacement object\n"
    result = module.build_manifest(root, head)
    assert result["inputs"][0]["sha256"] == module.hashlib.sha256(
        (root / relative).read_bytes()
    ).hexdigest()
    assert module.git(root, "show", f"{head}:{relative}") == (root / relative).read_bytes()


def test_rejects_executable_mode_drift_with_core_filemode_disabled(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    relative = module.INPUTS[-1]
    (root / relative).chmod(0o755)
    git(root, "config", "core.filemode", "false")
    assert not git(root, "status", "--porcelain", "--", relative)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "executable mode differs" in str(error)
    else:
        raise AssertionError("uncommitted executable mode drift admitted")


def test_ambient_git_config_count_cannot_invoke_fsmonitor(tmp_path: Path, monkeypatch) -> None:
    root, head = sample_repo(tmp_path)
    marker = tmp_path / "ambient-fsmonitor-executed"
    hook = tmp_path / "ambient-fsmonitor.sh"
    hook.write_text("#!/bin/sh\nprintf invoked > " + shlex.quote(str(marker)) + "\n", encoding="utf-8")
    hook.chmod(0o700)
    monkeypatch.setenv("GIT_CONFIG_COUNT", "1")
    monkeypatch.setenv("GIT_CONFIG_KEY_0", "core.fsmonitor")
    monkeypatch.setenv("GIT_CONFIG_VALUE_0", str(hook))
    result = module.build_manifest(root, head)
    assert result["candidate_head"] == head
    assert not marker.exists(), "ambient Git config executed a review-time hook"


def test_global_git_fsmonitor_cannot_execute_during_review(tmp_path: Path, monkeypatch) -> None:
    root, head = sample_repo(tmp_path)
    marker = tmp_path / "global-fsmonitor-executed"
    hook = tmp_path / "global-fsmonitor.sh"
    hook.write_text("#!/bin/sh\nprintf invoked > " + shlex.quote(str(marker)) + "\n", encoding="utf-8")
    hook.chmod(0o700)
    global_config = tmp_path / "global.gitconfig"
    global_config.write_text("[core]\n\tfsmonitor = " + str(hook) + "\n", encoding="utf-8")
    monkeypatch.setenv("GIT_CONFIG_GLOBAL", str(global_config))
    result = module.build_manifest(root, head)
    assert result["candidate_head"] == head
    assert not marker.exists(), "global Git config executed a review-time hook"


def test_global_core_bare_cannot_redirect_review_repository(tmp_path: Path, monkeypatch) -> None:
    root, head = sample_repo(tmp_path)
    config = tmp_path / "hostile-global-config"
    config.write_text("[core]\n\tbare = true\n", encoding="utf-8")
    monkeypatch.setenv("GIT_CONFIG_GLOBAL", str(config))
    result = module.build_manifest(root, head)
    assert result["candidate_head"] == head



def test_ambient_git_config_count_cannot_redirect_review(tmp_path: Path, monkeypatch) -> None:
    """Caller-supplied Git config parameters cannot override exact-head inspection."""
    root, head = sample_repo(tmp_path)
    monkeypatch.setenv("GIT_CONFIG_COUNT", "1")
    monkeypatch.setenv("GIT_CONFIG_KEY_0", "core.bare")
    monkeypatch.setenv("GIT_CONFIG_VALUE_0", "true")
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert manifest["authority"] == "REVIEW_INPUT_ONLY"


def test_ambient_git_dir_and_worktree_cannot_redirect_review(tmp_path: Path, monkeypatch) -> None:
    """A caller's alternate repository cannot become exact-head authority."""
    root, head = sample_repo(tmp_path)
    alternate_parent = tmp_path / "alternate-parent"
    alternate_parent.mkdir()
    alternate, _ = sample_repo(alternate_parent)
    monkeypatch.setenv("GIT_DIR", str(alternate / ".git"))
    monkeypatch.setenv("GIT_WORK_TREE", str(alternate))
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert manifest["authority"] == "REVIEW_INPUT_ONLY"
    assert manifest["inputs"][0]["sha256"] == module.hashlib.sha256(
        (root / module.INPUTS[0]).read_bytes()
    ).hexdigest()


def test_ambient_git_object_and_index_redirects_cannot_replace_review(tmp_path: Path, monkeypatch) -> None:
    """Untrusted alternate object stores and indexes cannot become review authority."""
    root, head = sample_repo(tmp_path)
    alternate_parent = tmp_path / "alternate-parent"
    alternate_parent.mkdir()
    alternate, _ = sample_repo(alternate_parent)
    monkeypatch.setenv("GIT_OBJECT_DIRECTORY", str(alternate / ".git" / "objects"))
    monkeypatch.setenv("GIT_ALTERNATE_OBJECT_DIRECTORIES", str(alternate / ".git" / "objects"))
    monkeypatch.setenv("GIT_INDEX_FILE", str(alternate / ".git" / "index"))
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert manifest["authority"] == "REVIEW_INPUT_ONLY"
    assert manifest["inputs"][0]["sha256"] == module.hashlib.sha256(
        (root / module.INPUTS[0]).read_bytes()
    ).hexdigest()


def test_ambient_git_trace2_cannot_write_outside_review(tmp_path: Path, monkeypatch) -> None:
    """Exact-head review must not inherit Git tracing side effects from callers."""
    root, head = sample_repo(tmp_path)
    trace = tmp_path / "untrusted-trace.json"
    monkeypatch.setenv("GIT_TRACE2_EVENT", str(trace))
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert not trace.exists(), "ambient Git tracing wrote outside the review workspace"


def test_ambient_git_trace_and_performance_cannot_write_outside_review(tmp_path: Path, monkeypatch) -> None:
    """Inherited Git trace destinations must not leak review execution evidence."""
    root, head = sample_repo(tmp_path)
    traces = {
        "GIT_TRACE": tmp_path / "ambient-git-trace.log",
        "GIT_TRACE_PERFORMANCE": tmp_path / "ambient-git-performance.log",
        "GIT_TRACE_SETUP": tmp_path / "ambient-git-setup.log",
    }
    for variable, destination in traces.items():
        monkeypatch.setenv(variable, str(destination))
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert not any(destination.exists() for destination in traces.values())


def test_ambient_git_trace2_normal_and_perf_cannot_write_outside_review(tmp_path: Path, monkeypatch) -> None:
    """Other Trace2 sinks must not escape the exact-head review boundary."""
    root, head = sample_repo(tmp_path)
    destinations = {
        "GIT_TRACE2": tmp_path / "ambient-trace2-normal.log",
        "GIT_TRACE2_PERF": tmp_path / "ambient-trace2-performance.log",
    }
    for variable, destination in destinations.items():
        monkeypatch.setenv(variable, str(destination))
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert not any(destination.exists() for destination in destinations.values())


def test_ambient_git_ref_and_pack_traces_cannot_write_outside_review(tmp_path: Path, monkeypatch) -> None:
    """Inherited Git ref and pack-access trace sinks cannot write review side effects."""
    root, head = sample_repo(tmp_path)
    destinations = {
        "GIT_TRACE_REFS": tmp_path / "ambient-refs.log",
        "GIT_TRACE_PACK_ACCESS": tmp_path / "ambient-pack-access.log",
    }
    for variable, destination in destinations.items():
        monkeypatch.setenv(variable, str(destination))
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert not any(destination.exists() for destination in destinations.values())


def test_ambient_git_setup_and_packet_traces_cannot_write_outside_review(tmp_path: Path) -> None:
    import os

    root, head = sample_repo(tmp_path)
    trace_names = ("GIT_TRACE_SETUP", "GIT_TRACE_PACKET", "GIT_TRACE_CURL")
    env = os.environ.copy()
    sinks = []
    for name in trace_names:
        sink = tmp_path / (name.lower() + ".log")
        env[name] = str(sink)
        sinks.append(sink)
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--repo", str(root), "--expected-head", head],
        capture_output=True, text=True, env=env,
    )
    assert result.returncode == 0, result.stderr
    manifest = json.loads(result.stdout)
    assert manifest["candidate_head"] == head
    assert manifest["authority"] == "REVIEW_INPUT_ONLY"
    assert all(not sink.exists() for sink in sinks), (
        "ambient Git tracing escaped the exact-head review boundary"
    )


def test_git_verification_timeout_fails_closed(tmp_path: Path, monkeypatch) -> None:
    """An unresponsive Git subprocess cannot hang exact-head evidence generation."""
    root, head = sample_repo(tmp_path)
    original_run = module.subprocess.run

    def timed_out_run(argv, *args, **kwargs):
        if argv[0] == "git":
            assert kwargs["timeout"] == 15
            raise subprocess.TimeoutExpired(argv, 15)
        return original_run(argv, *args, **kwargs)

    monkeypatch.setattr(module.subprocess, "run", timed_out_run)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "timed out" in str(error)
    else:
        raise AssertionError("unresponsive Git subprocess was admitted")


def test_git_spawn_failure_fails_closed(tmp_path: Path, monkeypatch) -> None:
    """Missing or inaccessible Git must not generate review authority."""
    root, head = sample_repo(tmp_path)

    def cannot_spawn(*args, **kwargs):
        raise FileNotFoundError("git executable unavailable")

    monkeypatch.setattr(module.subprocess, "run", cannot_spawn)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "could not start" in str(error)
    else:
        raise AssertionError("missing Git executable was admitted")


def test_invalid_git_output_encoding_fails_closed_without_manifest(tmp_path: Path, monkeypatch, capsys) -> None:
    """Invalid Git metadata must produce a bounded failure, not a traceback or manifest."""
    root, head = sample_repo(tmp_path)
    original_git = module.git

    def corrupt_head(repo: Path, *args: str) -> bytes:
        if args == ("rev-parse", "HEAD"):
            return b"\xff\xfe\n"
        return original_git(repo, *args)

    monkeypatch.setattr(module, "git", corrupt_head)
    assert module.main(["--repo", str(root), "--expected-head", head]) == 2
    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err.startswith("FAIL:")
    assert "Traceback" not in captured.err


def test_git_nonzero_exit_fails_closed_without_manifest(tmp_path: Path, monkeypatch, capsys) -> None:
    """Failed Git verification cannot produce partial review authority."""
    root, head = sample_repo(tmp_path)
    original_run = module.subprocess.run

    def failed_git(argv, *args, **kwargs):
        if argv[0] == "git":
            return subprocess.CompletedProcess(argv, 128, stdout=b"partial", stderr=b"secret detail")
        return original_run(argv, *args, **kwargs)

    monkeypatch.setattr(module.subprocess, "run", failed_git)
    assert module.main(["--repo", str(root), "--expected-head", head]) == 2
    captured = capsys.readouterr()
    assert captured.out == ""
    assert "Git verification failed" in captured.err
    assert "secret detail" not in captured.err


def test_rejects_committed_symlink_review_input_even_when_head_is_clean(tmp_path: Path) -> None:
    root, _ = sample_repo(tmp_path)
    relative = module.INPUTS[-1]
    file = root / relative
    file.unlink()
    file.symlink_to(module.INPUTS[0])
    git(root, "add", "--", relative)
    git(root, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
        "commit", "-qm", "test: committed symlink must not become review authority")
    head = git(root, "rev-parse", "HEAD")
    assert not git(root, "status", "--porcelain")
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "noncanonical" in str(error)
    else:
        raise AssertionError("committed symlink admitted as P2 review input")


def test_invalid_committed_blob_identity_fails_closed(tmp_path: Path, monkeypatch) -> None:
    """A malformed Git blob identity cannot enter an exact-head review manifest."""
    root, head = sample_repo(tmp_path)
    original_git = module.git

    def malformed_blob(repo: Path, *args: str) -> bytes:
        if len(args) == 2 and args[0] == "rev-parse" and args[1] == f"{head}:{module.INPUTS[0]}":
            return b"not-a-blob-id\n"
        return original_git(repo, *args)

    monkeypatch.setattr(module, "git", malformed_blob)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "invalid committed blob identity" in str(error)
    else:
        raise AssertionError("malformed committed blob identity admitted")


def test_rejects_symlinked_component_parent_with_unchanged_bytes(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    parent = root / "crates/primed/src"
    moved = root / "actual-primed-src"
    parent.rename(moved)
    parent.symlink_to(moved, target_is_directory=True)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "non-directory" in str(error) or "noncanonical" in str(error)
    else:
        raise AssertionError("symlinked component parent admitted as review authority")


def test_uppercase_commit_identity_fails_closed_without_manifest(tmp_path: Path, capsys) -> None:
    """Review authority requires canonical lowercase exact-commit identities."""
    root, head = sample_repo(tmp_path)
    assert module.main(["--repo", str(root), "--expected-head", head.upper()]) == 2
    captured = capsys.readouterr()
    assert captured.out == ""
    assert "full lowercase commit SHA" in captured.err


def test_rejects_world_writable_review_root(tmp_path: Path) -> None:
    """A world-writable exact-head checkout cannot establish review custody."""
    root, head = sample_repo(tmp_path)
    root.chmod(root.stat().st_mode | 0o002)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "shared-writable" in str(error)
    else:
        raise AssertionError("world-writable review root admitted")


def test_rejects_special_permission_bits_on_committed_review_input(tmp_path: Path) -> None:
    """Setuid/setgid/sticky metadata cannot masquerade as an ordinary committed input."""
    root, head = sample_repo(tmp_path)
    file = root / module.INPUTS[-1]
    file.chmod(file.stat().st_mode | 0o2000)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "special permission bits" in str(error)
    else:
        raise AssertionError("special permission bits admitted as exact-head review input")


def test_rejects_group_writable_review_input_parent(tmp_path: Path) -> None:
    """Group-writable source parents cannot hold exact-head review inputs."""
    root, head = sample_repo(tmp_path)
    parent = root / "crates/primed/src"
    parent.chmod(parent.stat().st_mode | 0o020)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "shared-writable" in str(error)
    else:
        raise AssertionError("group-writable review input parent admitted")
