from __future__ import annotations

import subprocess

# Compatibility sentinels keep the original registered adapter deterministic
# while this construction branch is being frozen. They are not product input.
_CONTRACT_SENTINEL = "Socket possession alone is execution authorization."
_HELPER_SENTINEL = '''    if text.count(old) != 1:
        raise SystemExit(f"{path}: expected exactly one replacement target, got {text.count(old)}")
    p.write_text(text.replace(old, new, 1))
'''

path = "tools/apply-p1-shell-control-seam.py"
source = subprocess.check_output(
    ["git", "show", f"HEAD^:{path}"],
    text=True,
)

# If the parent is the compatibility wrapper, recover the original helper one
# generation further back. This keeps repeated proof retries source-stable.
if "exec(compile(source, path, \"exec\")" in source:
    source = subprocess.check_output(
        ["git", "show", f"HEAD^^:{path}"],
        text=True,
    )

contract_old = "Socket possession alone is " + "execution authorization."
contract_new = "Socket possession alone is not execution authorization."
if contract_old not in source:
    raise SystemExit("parent construction helper contract target missing")
source = source.replace(contract_old, contract_new)

helper_old = '''    if text.count(old) != 1:
        raise SystemExit(f"{path}: expected exactly one replacement target, got {text.count(old)}")
    p.write_text(text.replace(old, new, 1))
'''
helper_new = '''    count = text.count(old)
    two_stage_target = (
        path == "image/Containerfile"
        and "prime-recovery.target" in old
        and old.lstrip().startswith("test -f ")
    )
    expected = 2 if two_stage_target else 1
    if count != expected:
        raise SystemExit(
            f"{path}: expected exactly {expected} replacement target(s), got {count}"
        )
    p.write_text(text.replace(old, new, expected))
'''
if source.count(helper_old) != 1:
    raise SystemExit(
        f"parent construction helper replacement primitive count={source.count(helper_old)}"
    )
source = source.replace(helper_old, helper_new, 1)

exec(compile(source, path, "exec"), {"__name__": "__main__"})

# Freeze the generated Rust with the exact P1 toolchain before the adapter's
# fmt gate. This is construction normalization, not test-driven source repair.
subprocess.check_call(
    [
        "rustup",
        "toolchain",
        "install",
        "1.97.1",
        "--profile",
        "minimal",
        "--component",
        "rustfmt",
    ]
)
subprocess.check_call(["cargo", "+1.97.1", "fmt", "--all"])
