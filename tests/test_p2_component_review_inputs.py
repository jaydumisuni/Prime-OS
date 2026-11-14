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


def test_rejects_setid_review_ancestor(tmp_path: Path) -> None:
    root, head = sample_repo(tmp_path)
    parent = tmp_path / "setid-parent"
    parent.mkdir()
    moved = parent / "p2"
    root.rename(moved)
    try:
        for bit in (0o2000, 0o4000):
            parent.chmod(0o755 | bit)
            try:
                module.build_manifest(moved, head)
            except module.ReviewInputError as error:
                assert "special permission bits on review ancestor" in str(error)
            else:
                raise AssertionError(f"setid ancestor {bit:o} admitted")
    finally:
        parent.chmod(0o755)


def test_ambient_git_index_override_cannot_substitute_review_identity(
    tmp_path: Path, monkeypatch
) -> None:
    """A caller-controlled alternate index must not change exact-head evidence."""
    root, head = sample_repo(tmp_path)
    monkeypatch.setenv("GIT_INDEX_FILE", str(tmp_path / "missing-index"))
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert len(manifest["inputs"]) == len(module.INPUTS)


def test_ambient_git_worktree_override_cannot_redirect_review_identity(
    tmp_path: Path, monkeypatch
) -> None:
    """An inherited worktree override must not redirect committed input custody."""
    root, head = sample_repo(tmp_path)
    alternate = tmp_path / "alternate"
    alternate.mkdir()
    monkeypatch.setenv("GIT_WORK_TREE", str(alternate))
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert len(manifest["inputs"]) == len(module.INPUTS)


def test_ambient_git_config_injection_cannot_redirect_review_identity(
    tmp_path: Path, monkeypatch
) -> None:
    """Inherited command-scope Git config must not rewrite review object custody."""
    root, head = sample_repo(tmp_path)
    monkeypatch.setenv("GIT_CONFIG_COUNT", "1")
    monkeypatch.setenv("GIT_CONFIG_KEY_0", "core.worktree")
    monkeypatch.setenv("GIT_CONFIG_VALUE_0", str(tmp_path / "nonexistent-worktree"))
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert len(manifest["inputs"]) == len(module.INPUTS)


def test_ambient_git_object_directory_cannot_redirect_review_identity(
    tmp_path: Path, monkeypatch
) -> None:
    """An inherited alternate object store must not replace committed review inputs."""
    root, head = sample_repo(tmp_path)
    monkeypatch.setenv("GIT_OBJECT_DIRECTORY", str(tmp_path / "missing-objects"))
    monkeypatch.setenv("GIT_ALTERNATE_OBJECT_DIRECTORIES", str(tmp_path / "missing-alternates"))
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert len(manifest["inputs"]) == len(module.INPUTS)


def test_repository_fsmonitor_hook_is_not_executed(tmp_path: Path) -> None:
    """Repository-local fsmonitor commands must not execute during review custody."""
    root, head = sample_repo(tmp_path)
    marker = tmp_path / "fsmonitor-executed"
    hook = tmp_path / "fsmonitor-hook.py"
    hook.write_text(
        "#!/usr/bin/env python3\n"
        "from pathlib import Path\n"
        f"Path({str(marker)!r}).write_text('executed')\n",
        encoding="utf-8",
    )
    hook.chmod(0o700)
    git(root, "config", "core.fsmonitor", str(hook))
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert manifest["authority"] == "REVIEW_INPUT_ONLY"
    assert not marker.exists(), "repository fsmonitor escaped review-input custody"


def test_review_git_disables_optional_index_writes(tmp_path: Path, monkeypatch) -> None:
    """Exact-head inspection must not acquire optional index locks or refresh the index."""
    root, head = sample_repo(tmp_path)
    original_run = module.subprocess.run
    observed = []

    def inspect_run(argv, *args, **kwargs):
        if argv[0] == "git":
            observed.append(kwargs["env"].get("GIT_OPTIONAL_LOCKS"))
        return original_run(argv, *args, **kwargs)

    monkeypatch.setenv("GIT_OPTIONAL_LOCKS", "1")
    monkeypatch.setattr(module.subprocess, "run", inspect_run)
    manifest = module.build_manifest(root, head)
    assert manifest["candidate_head"] == head
    assert observed and set(observed) == {"0"}


def test_review_git_disables_terminal_prompts_and_inherited_stdin(tmp_path: Path, monkeypatch) -> None:
    root, head = sample_repo(tmp_path)
    original_run = module.subprocess.run
    observed = []

    def checked_run(*args, **kwargs):
        observed.append((kwargs.get("env", {}).get("GIT_TERMINAL_PROMPT"), kwargs.get("stdin")))
        return original_run(*args, **kwargs)

    monkeypatch.setenv("GIT_TERMINAL_PROMPT", "1")
    monkeypatch.setattr(module.subprocess, "run", checked_run)
    assert module.build_manifest(root, head)["candidate_head"] == head
    assert observed and all(prompt == "0" and stdin == subprocess.DEVNULL
                            for prompt, stdin in observed)


def test_rejects_special_permission_bits_on_review_input_parent(tmp_path: Path) -> None:
    """Setid/sticky source directories cannot establish exact-head input custody."""
    root, head = sample_repo(tmp_path)
    parent = root / "crates/primed/src"
    original_mode = parent.stat().st_mode & 0o7777
    try:
        for bit in (0o1000, 0o2000, 0o4000):
            parent.chmod(original_mode | bit)
            try:
                module.build_manifest(root, head)
            except module.ReviewInputError as error:
                assert "special permission bits" in str(error)
            else:
                raise AssertionError(f"special source parent bit {bit:o} admitted")
    finally:
        parent.chmod(original_mode)


def test_review_git_disables_partial_clone_lazy_fetch(tmp_path: Path, monkeypatch) -> None:
    """Read-only exact-head review must never fetch missing promisor objects."""
    root, _ = sample_repo(tmp_path)
    original_run = module.subprocess.run
    observed = []

    def inspected_run(*args, **kwargs):
        observed.append(kwargs.get("env", {}).get("GIT_NO_LAZY_FETCH"))
        return original_run(*args, **kwargs)

    monkeypatch.setattr(module.subprocess, "run", inspected_run)
    module.git(root, "rev-parse", "HEAD")
    assert observed == ["1"]


def test_rejects_ancestor_permission_change_during_input_read(tmp_path: Path, monkeypatch) -> None:
    root, head = sample_repo(tmp_path)
    parent = tmp_path / "ancestor-custody"
    parent.mkdir()
    moved = parent / "p2"
    root.rename(moved)
    target = moved / module.INPUTS[-1]
    original_read = Path.read_bytes

    def change_ancestor(file: Path) -> bytes:
        content = original_read(file)
        if file == target:
            parent.chmod(0o777)
        return content

    monkeypatch.setattr(Path, "read_bytes", change_ancestor)
    try:
        try:
            module.build_manifest(moved, head)
        except module.ReviewInputError as error:
            assert "shared-writable review ancestor" in str(error)
        else:
            raise AssertionError("ancestor custody lost during inspection was admitted")
    finally:
        parent.chmod(0o755)


def test_rejects_earlier_source_parent_changed_after_its_read(tmp_path: Path, monkeypatch) -> None:
    root, head = sample_repo(tmp_path)
    earlier = root / "crates/prime-contracts/src"
    last = root / module.INPUTS[-1]
    original_read = Path.read_bytes

    def change_earlier_parent(file: Path) -> bytes:
        content = original_read(file)
        if file == last:
            earlier.chmod(0o777)
        return content

    monkeypatch.setattr(Path, "read_bytes", change_earlier_parent)
    try:
        try:
            module.build_manifest(root, head)
        except module.ReviewInputError as error:
            assert "shared-writable review path" in str(error)
        else:
            raise AssertionError("earlier source parent changed after read was admitted")
    finally:
        earlier.chmod(0o755)


def test_rejects_earlier_input_mutated_during_later_read(tmp_path: Path, monkeypatch) -> None:
    """A late mutation of an already inspected input cannot enter the manifest."""
    root, head = sample_repo(tmp_path)
    earlier = root / module.INPUTS[0]
    last = root / module.INPUTS[-1]
    original_read = Path.read_bytes
    changed = False

    def mutate_earlier(file: Path) -> bytes:
        nonlocal changed
        content = original_read(file)
        if file == last and not changed:
            changed = True
            earlier.write_bytes(original_read(earlier) + b"late mutation\n")
        return content

    monkeypatch.setattr(Path, "read_bytes", mutate_earlier)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "changed after inspection" in str(error)
    else:
        raise AssertionError("late mutation of earlier source was admitted")


def test_rejects_final_read_permission_change(tmp_path: Path, monkeypatch) -> None:
    """A late chmod during final digest verification must fail closed."""
    root, head = sample_repo(tmp_path)
    target = root / module.INPUTS[0]
    original_read = Path.read_bytes
    reads = 0

    def mutate_on_final_read(file: Path) -> bytes:
        nonlocal reads
        content = original_read(file)
        if file == target:
            reads += 1
            if reads == 2:
                target.chmod(0o666)
        return content

    monkeypatch.setattr(Path, "read_bytes", mutate_on_final_read)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "custody changed during final read" in str(error)
    else:
        raise AssertionError("late permission mutation was admitted")


def test_rejects_earlier_input_executable_bit_drift_after_inspection(
    tmp_path: Path, monkeypatch
) -> None:
    """Final custody must retain committed file mode even when Git ignores mode changes."""
    root, head = sample_repo(tmp_path)
    git(root, "config", "core.filemode", "false")
    earlier = root / module.INPUTS[0]
    last = root / module.INPUTS[-1]
    original_read = Path.read_bytes
    changed = False

    def change_mode_late(file: Path) -> bytes:
        nonlocal changed
        content = original_read(file)
        if file == last and not changed:
            changed = True
            earlier.chmod(earlier.stat().st_mode | 0o100)
        return content

    monkeypatch.setattr(Path, "read_bytes", change_mode_late)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "custody changed" in str(error)
    else:
        raise AssertionError("late executable-bit drift admitted")


def test_rejects_earlier_source_parent_mutated_during_final_read(
    tmp_path: Path, monkeypatch
) -> None:
    """Final reads must not allow a previously checked source directory to drift."""
    root, head = sample_repo(tmp_path)
    earlier_parent = root / "crates/prime-contracts/src"
    last = root / module.INPUTS[-1]
    original_read = Path.read_bytes
    reads = 0

    def mutate_during_final(file: Path) -> bytes:
        nonlocal reads
        content = original_read(file)
        if file == last:
            reads += 1
            if reads == 2:
                earlier_parent.chmod(0o777)
        return content

    monkeypatch.setattr(Path, "read_bytes", mutate_during_final)
    try:
        try:
            module.build_manifest(root, head)
        except module.ReviewInputError as error:
            assert "shared-writable review path" in str(error)
        else:
            raise AssertionError("late source-parent mutation admitted")
    finally:
        earlier_parent.chmod(0o755)


def test_rejects_earlier_input_mutated_during_final_read(tmp_path: Path, monkeypatch) -> None:
    """A late source rewrite during final verification cannot publish stale custody."""
    root, head = sample_repo(tmp_path)
    earlier = root / module.INPUTS[0]
    last = root / module.INPUTS[-1]
    original_read = Path.read_bytes
    reads = 0

    def mutate_during_final(file: Path) -> bytes:
        nonlocal reads
        content = original_read(file)
        if file == last:
            reads += 1
            if reads == 2:
                earlier.write_bytes(original_read(earlier) + b"final-pass mutation\n")
        return content

    monkeypatch.setattr(Path, "read_bytes", mutate_during_final)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "changed" in str(error) or "custody" in str(error)
    else:
        raise AssertionError("final-pass source rewrite admitted")


def test_rejects_final_pass_executable_drift_with_filemode_disabled(
    tmp_path: Path, monkeypatch
) -> None:
    """Last-pass digest alone cannot admit a chmod hidden by core.filemode=false."""
    root, head = sample_repo(tmp_path)
    git(root, "config", "core.filemode", "false")
    first = root / module.INPUTS[0]
    original_read = Path.read_bytes
    reads = 0

    def drift_on_last_pass(file: Path) -> bytes:
        nonlocal reads
        content = original_read(file)
        if file == first:
            reads += 1
            if reads == 3:
                first.chmod(first.stat().st_mode | 0o100)
        return content

    monkeypatch.setattr(Path, "read_bytes", drift_on_last_pass)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "final verification" in str(error)
    else:
        raise AssertionError("last-pass executable-bit drift was admitted")


def test_rejects_source_parent_mutated_during_last_digest_pass(
    tmp_path: Path, monkeypatch
) -> None:
    """The final digest pass must not authorize a newly shared-writable parent."""
    root, head = sample_repo(tmp_path)
    earlier_parent = root / "crates/prime-contracts/src"
    last = root / module.INPUTS[-1]
    original_read = Path.read_bytes
    reads = 0

    def mutate_on_last_pass(file: Path) -> bytes:
        nonlocal reads
        content = original_read(file)
        if file == last:
            reads += 1
            if reads == 3:
                earlier_parent.chmod(0o777)
        return content

    monkeypatch.setattr(Path, "read_bytes", mutate_on_last_pass)
    try:
        try:
            module.build_manifest(root, head)
        except module.ReviewInputError as error:
            assert "shared-writable review path" in str(error)
        else:
            raise AssertionError("last-pass parent permission mutation admitted")
    finally:
        earlier_parent.chmod(0o755)


def test_rejects_root_permission_drift_during_final_head_query(
    tmp_path: Path, monkeypatch
) -> None:
    """Final Git identity inspection cannot admit a shared-writable root."""
    root, head = sample_repo(tmp_path)
    original_git = module.git
    head_reads = 0

    def mutate_on_final_head(repo: Path, *args: str) -> bytes:
        nonlocal head_reads
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_reads += 1
            if head_reads == 2:
                root.chmod(0o777)
        return result

    monkeypatch.setattr(module, "git", mutate_on_final_head)
    try:
        try:
            module.build_manifest(root, head)
        except module.ReviewInputError as error:
            assert "shared-writable review path" in str(error)
        else:
            raise AssertionError("final HEAD query admitted shared-writable root")
    finally:
        root.chmod(0o755)


def test_rejects_source_rewrite_during_final_head_query(tmp_path: Path, monkeypatch) -> None:
    """Final Git identity inspection must not admit a source rewrite."""
    root, head = sample_repo(tmp_path)
    source = root / module.INPUTS[0]
    original_git = module.git
    head_reads = 0

    def mutate_on_final_head(repo: Path, *args: str) -> bytes:
        nonlocal head_reads
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_reads += 1
            if head_reads == 2:
                source.write_bytes(source.read_bytes() + b"late rewrite\n")
        return result

    monkeypatch.setattr(module, "git", mutate_on_final_head)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "after final HEAD query" in str(error)
    else:
        raise AssertionError("source rewrite during final HEAD query was admitted")


def test_rejects_parent_permission_drift_during_post_head_digest(
    tmp_path: Path, monkeypatch
) -> None:
    """A final source read must not invalidate a previously checked parent."""
    root, head = sample_repo(tmp_path)
    earlier_parent = root / "crates/prime-contracts/src"
    last = root / module.INPUTS[-1]
    original_read = Path.read_bytes
    reads = 0

    def drift_on_post_head_read(file: Path) -> bytes:
        nonlocal reads
        content = original_read(file)
        if file == last:
            reads += 1
            if reads == 4:
                earlier_parent.chmod(0o777)
        return content

    monkeypatch.setattr(Path, "read_bytes", drift_on_post_head_read)
    try:
        try:
            module.build_manifest(root, head)
        except module.ReviewInputError as error:
            assert "shared-writable review path" in str(error)
        else:
            raise AssertionError("post-HEAD parent permission drift admitted")
    finally:
        earlier_parent.chmod(0o755)


def test_rejects_earlier_source_rewrite_during_post_head_digest(
    tmp_path: Path, monkeypatch
) -> None:
    """Later post-HEAD reads must not invalidate earlier committed bytes."""
    root, head = sample_repo(tmp_path)
    earlier = root / module.INPUTS[0]
    last = root / module.INPUTS[-1]
    original_read = Path.read_bytes
    reads = 0

    def rewrite_on_post_head_read(file: Path) -> bytes:
        nonlocal reads
        content = original_read(file)
        if file == last:
            reads += 1
            if reads == 4:
                earlier.write_bytes(original_read(earlier) + b"late rewrite\n")
        return content

    monkeypatch.setattr(Path, "read_bytes", rewrite_on_post_head_read)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "changed at publication" in str(error)
    else:
        raise AssertionError("post-HEAD earlier source rewrite admitted")


def test_rejects_head_switch_during_publication_digest(tmp_path: Path, monkeypatch) -> None:
    """The final digest pass must not publish a stale candidate identity."""
    root, head = sample_repo(tmp_path)
    last = root / module.INPUTS[-1]
    original_read = Path.read_bytes
    original_git = module.git
    reads = 0
    switched = False

    def switch_on_publication_read(file: Path) -> bytes:
        nonlocal reads, switched
        content = original_read(file)
        if file == last:
            reads += 1
            if reads == 5:
                switched = True
        return content

    def observe_switched_head(repo: Path, *args: str) -> bytes:
        if switched and args == ("rev-parse", "HEAD"):
            return b"0" * 40 + b"\n"
        return original_git(repo, *args)

    monkeypatch.setattr(Path, "read_bytes", switch_on_publication_read)
    monkeypatch.setattr(module, "git", observe_switched_head)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "HEAD changed at publication" in str(error)
    else:
        raise AssertionError("publication admitted a changed candidate HEAD")
    assert switched


def test_rejects_root_permission_change_during_publication_head_query(tmp_path: Path, monkeypatch) -> None:
    root, head = sample_repo(tmp_path)
    original_git = module.git
    head_queries = 0

    def mutate_on_last_head(repo: Path, *args: str) -> bytes:
        nonlocal head_queries
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_queries += 1
            if head_queries == 3:
                root.chmod(0o777)
        return result

    monkeypatch.setattr(module, "git", mutate_on_last_head)
    try:
        try:
            module.build_manifest(root, head)
        except module.ReviewInputError as error:
            assert "writable" in str(error)
        else:
            raise AssertionError("publication HEAD query relaxed root custody")
    finally:
        root.chmod(0o755)


def test_rejects_ancestor_permission_change_during_publication_head_query(
    tmp_path: Path, monkeypatch
) -> None:
    """The last Git identity read cannot relax a protected ancestor."""
    root, head = sample_repo(tmp_path)
    ancestor = root.parent
    original_git = module.git
    head_queries = 0

    def mutate_on_last_head(repo: Path, *args: str) -> bytes:
        nonlocal head_queries
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_queries += 1
            if head_queries == 3:
                ancestor.chmod(0o777)
        return result

    monkeypatch.setattr(module, "git", mutate_on_last_head)
    original_mode = ancestor.stat().st_mode & 0o777
    try:
        try:
            module.build_manifest(root, head)
        except module.ReviewInputError as error:
            assert "writable" in str(error)
        else:
            raise AssertionError("publication HEAD query relaxed ancestor custody")
    finally:
        ancestor.chmod(original_mode)


def test_rejects_source_rewrite_during_publication_head_query(tmp_path: Path, monkeypatch) -> None:
    """The final HEAD query cannot silently admit a late source rewrite."""
    root, head = sample_repo(tmp_path)
    source = root / module.INPUTS[0]
    original_git = module.git
    head_queries = 0

    def mutate_on_last_head(repo: Path, *args: str) -> bytes:
        nonlocal head_queries
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_queries += 1
            if head_queries == 3:
                source.write_bytes(source.read_bytes() + b"late rewrite\n")
        return result

    monkeypatch.setattr(module, "git", mutate_on_last_head)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "after publication HEAD" in str(error)
    else:
        raise AssertionError("publication HEAD query source rewrite admitted")
    assert head_queries == 3


def test_rejects_staged_index_swap_during_publication_head_query(
    tmp_path: Path, monkeypatch
) -> None:
    """Matching worktree bytes cannot conceal a late staged index substitution."""
    root, head = sample_repo(tmp_path)
    relative = module.INPUTS[0]
    source = root / relative
    committed = source.read_bytes()
    original_git = module.git
    head_queries = 0

    def stage_on_last_head(repo: Path, *args: str) -> bytes:
        nonlocal head_queries
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_queries += 1
            if head_queries == 3:
                source.write_bytes(b"alternate staged source\n")
                git(root, "add", "--", relative)
                source.write_bytes(committed)
        return result

    monkeypatch.setattr(module, "git", stage_on_last_head)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "after publication HEAD" in str(error)
    else:
        raise AssertionError("late staged index substitution admitted")
    assert head_queries == 3


def test_missing_review_directory_is_fail_closed(tmp_path: Path) -> None:
    """Disappearing source directories produce a bounded review rejection."""
    missing = tmp_path / "vanished"
    try:
        module.require_private_directory(missing)
    except module.ReviewInputError as error:
        assert "review directory cannot be inspected" in str(error)
    else:
        raise AssertionError("missing review directory was admitted")


def test_missing_review_ancestor_is_controlled_error(tmp_path: Path, monkeypatch) -> None:
    import importlib.util
    import sys
    module_path = Path(__file__).resolve().parents[1] / "tools/p2_component_review_inputs.py"
    spec = importlib.util.spec_from_file_location("p2_ancestor_probe", module_path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    missing = tmp_path / "vanished" / "review"
    try:
        module.require_protected_ancestors(missing)
    except module.ReviewInputError as error:
        assert "review ancestor cannot be inspected" in str(error)
    else:
        raise AssertionError("missing ancestor escaped controlled failure")


def test_missing_source_after_publication_head_is_controlled(tmp_path: Path, monkeypatch) -> None:
    """A disappearing final source is rejected as review evidence, not an OS traceback."""
    root, head = sample_repo(tmp_path)
    source = root / module.INPUTS[0]
    original_git = module.git
    head_queries = 0

    def remove_after_last_head(repo: Path, *args: str) -> bytes:
        nonlocal head_queries
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_queries += 1
            if head_queries == 3:
                source.unlink()
        return result

    monkeypatch.setattr(module, "git", remove_after_last_head)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "vanished after publication HEAD" in str(error)
    else:
        raise AssertionError("missing final source was admitted")
    assert head_queries == 3


def test_missing_source_during_post_publication_read_is_controlled(
    tmp_path: Path, monkeypatch
) -> None:
    """A source disappearing after lstat must fail as bounded review evidence."""
    root, head = sample_repo(tmp_path)
    source = root / module.INPUTS[0]
    original_read = Path.read_bytes
    original_git = module.git
    head_queries = 0

    def count_head(repo: Path, *args: str) -> bytes:
        nonlocal head_queries
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_queries += 1
        return result

    def vanish_during_read(file: Path) -> bytes:
        if file == source and head_queries >= 3:
            source.unlink()
            raise FileNotFoundError(source)
        return original_read(file)

    monkeypatch.setattr(module, "git", count_head)
    monkeypatch.setattr(Path, "read_bytes", vanish_during_read)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "vanished during post-publication read" in str(error)
    else:
        raise AssertionError("post-publication read failure escaped fail-closed boundary")


def test_source_read_failure_is_controlled(tmp_path: Path, monkeypatch) -> None:
    """A read failure must never escape as a raw filesystem exception."""
    missing = tmp_path / "missing-source"
    try:
        module.read_review_bytes(missing)
    except module.ReviewInputError as error:
        assert "review input cannot be read" in str(error)
    else:
        raise AssertionError("missing source escaped review boundary")


def test_missing_source_after_final_head_is_controlled(tmp_path: Path, monkeypatch) -> None:
    """A source removed immediately before the publication read fails closed."""
    root, head = sample_repo(tmp_path)
    source = root / module.INPUTS[0]
    original_lstat = Path.lstat
    original_git = module.git
    head_queries = 0

    def count_head(repo: Path, *args: str) -> bytes:
        nonlocal head_queries
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_queries += 1
        return result

    def disappear_at_publication(path: Path):
        if path == source and head_queries >= 2:
            raise FileNotFoundError(path)
        return original_lstat(path)

    monkeypatch.setattr(module, "git", count_head)
    monkeypatch.setattr(Path, "lstat", disappear_at_publication)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "vanished after final HEAD query" in str(error)
    else:
        raise AssertionError("publication lstat failure escaped fail-closed boundary")


def test_rejects_source_permission_change_during_publication_head_query(
    tmp_path: Path, monkeypatch
) -> None:
    """A late mode relaxation cannot authorize review-input publication."""
    root, head = sample_repo(tmp_path)
    source = root / module.INPUTS[0]
    original_git = module.git
    head_queries = 0

    def relax_on_publication_head(repo: Path, *args: str) -> bytes:
        nonlocal head_queries
        result = original_git(repo, *args)
        if args == ("rev-parse", "HEAD"):
            head_queries += 1
            if head_queries == 3:
                source.chmod(0o666)
        return result

    monkeypatch.setattr(module, "git", relax_on_publication_head)
    try:
        try:
            module.build_manifest(root, head)
        except module.ReviewInputError as error:
            assert "custody changed after publication HEAD" in str(error)
        else:
            raise AssertionError("late permission relaxation admitted")
    finally:
        source.chmod(0o644)
    assert head_queries == 3


def test_rejects_late_index_swap_during_final_status_sweep(tmp_path: Path, monkeypatch) -> None:
    """A later status query must not conceal a staged earlier source."""
    root, head = sample_repo(tmp_path)
    earlier = module.INPUTS[0]
    later = module.INPUTS[-1]
    source = root / earlier
    committed = source.read_bytes()
    original_git = module.git
    status_reads = 0

    def stage_during_last_status(repo: Path, *args: str) -> bytes:
        nonlocal status_reads
        if args == ("status", "--porcelain", "--", later):
            status_reads += 1
            if status_reads == 5:
                source.write_bytes(b"alternate staged source\n")
                git(root, "add", "--", earlier)
                source.write_bytes(committed)
        return original_git(repo, *args)

    monkeypatch.setattr(module, "git", stage_during_last_status)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "index changed at publication" in str(error)
    else:
        raise AssertionError("late staged index substitution admitted")
    assert status_reads >= 5


def test_rejects_index_swap_on_last_status_query(tmp_path: Path, monkeypatch) -> None:
    """A final status query cannot stage an already checked input unnoticed."""
    root, head = sample_repo(tmp_path)
    earlier, later = module.INPUTS[0], module.INPUTS[-1]
    source = root / earlier
    committed = source.read_bytes()
    original_git = module.git
    last_status_count = 0

    def stage_on_last_status(repo: Path, *args: str) -> bytes:
        nonlocal last_status_count
        result = original_git(repo, *args)
        if args == ("status", "--porcelain", "--", later):
            last_status_count += 1
            if last_status_count == 6:
                source.write_bytes(b"alternate staged source\n")
                git(root, "add", "--", earlier)
                source.write_bytes(committed)
        return result

    monkeypatch.setattr(module, "git", stage_on_last_status)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "index changed at publication boundary" in str(error)
    else:
        raise AssertionError("final status index substitution admitted")
    assert last_status_count == 6


def test_missing_source_during_final_verification_is_controlled(tmp_path: Path, monkeypatch) -> None:
    """An input removed at the final digest sweep must fail closed."""
    root, head = sample_repo(tmp_path)
    source = root / module.INPUTS[0]
    original_lstat = Path.lstat
    calls = 0

    def disappear_on_final_sweep(path: Path):
        nonlocal calls
        if path == source:
            calls += 1
            if calls == 5:
                raise FileNotFoundError(path)
        return original_lstat(path)

    monkeypatch.setattr(Path, "lstat", disappear_on_final_sweep)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "vanished during final verification" in str(error)
    else:
        raise AssertionError("final verification disappearance escaped fail-closed boundary")
    assert calls == 5


def test_missing_source_after_publication_read_is_controlled(tmp_path: Path, monkeypatch) -> None:
    """The post-read lstat must not leak an unhandled filesystem exception."""
    root, head = sample_repo(tmp_path)
    source = root / module.INPUTS[0]
    original_lstat = Path.lstat
    original_read = module.read_review_bytes
    publication_read = False

    def mark_publication_read(file: Path) -> bytes:
        nonlocal publication_read
        result = original_read(file)
        if file == source:
            # The last publication loop is reached after several source reads.
            mark_publication_read.calls += 1
            if mark_publication_read.calls == 3:
                publication_read = True
        return result

    mark_publication_read.calls = 0

    def vanish_after_read(file: Path):
        if file == source and publication_read:
            raise FileNotFoundError(file)
        return original_lstat(file)

    monkeypatch.setattr(module, "read_review_bytes", mark_publication_read)
    monkeypatch.setattr(Path, "lstat", vanish_after_read)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "review input cannot be inspected" in str(error)
    else:
        raise AssertionError("post-read disappearance admitted")


def test_rejects_final_index_probe_failure(tmp_path: Path, monkeypatch) -> None:
    """An unavailable final index custody probe cannot authorize a manifest."""
    root, head = sample_repo(tmp_path)
    original_git = module.git
    seen = []

    def fail_final_index_probe(repo: Path, *args: str) -> bytes:
        if args[:4] == ("diff", "--cached", "--name-only", head):
            seen.append(args)
            raise module.ReviewInputError("Git verification failed: final index probe")
        return original_git(repo, *args)

    monkeypatch.setattr(module, "git", fail_final_index_probe)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "final index probe" in str(error)
    else:
        raise AssertionError("missing final index custody probe admitted")
    assert len(seen) == 1


def test_rejects_source_mutation_during_final_index_probe(tmp_path: Path, monkeypatch) -> None:
    """The final index probe cannot conceal a concurrent source rewrite."""
    root, head = sample_repo(tmp_path)
    source = root / module.INPUTS[0]
    original_git = module.git
    seen = []

    def mutate_on_final_probe(repo: Path, *args: str) -> bytes:
        result = original_git(repo, *args)
        if args[:4] == ("diff", "--cached", "--name-only", head):
            seen.append(args)
            source.write_bytes(b"post-index-probe mutation\n")
        return result

    monkeypatch.setattr(module, "git", mutate_on_final_probe)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError:
        pass
    else:
        raise AssertionError("source mutation during final index probe admitted")
    assert len(seen) == 1


def test_rejects_executable_mode_flip_during_final_index_probe(tmp_path: Path, monkeypatch) -> None:
    """A clean index probe cannot authorize an executable source mode flip."""
    root, head = sample_repo(tmp_path)
    source = root / module.INPUTS[0]
    original_git = module.git
    seen = []

    def flip_mode_after_index_probe(repo: Path, *args: str) -> bytes:
        result = original_git(repo, *args)
        if args[:4] == ("diff", "--cached", "--name-only", head):
            seen.append(args)
            source.chmod(0o755)
        return result

    monkeypatch.setattr(module, "git", flip_mode_after_index_probe)
    try:
        module.build_manifest(root, head)
    except module.ReviewInputError as error:
        assert "custody changed after index probe" in str(error)
    else:
        raise AssertionError("post-index executable mode flip admitted")
    assert len(seen) == 1
