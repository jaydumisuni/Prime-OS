#!/usr/bin/env bash
set -euo pipefail

run() {
  local label="$1"; shift
  "$@" >/tmp/prime-w2-negative.log 2>&1 || {
    cat /tmp/prime-w2-negative.log >&2
    echo "FAIL: ${label}" >&2
    exit 1
  }
  echo "PASS: ${label}"
}

run "malformed component reference" cargo test --locked -p prime-contracts windows_component_reference_rejects_malformed_identity_or_pin -- --nocapture
run "missing component revision" cargo test --locked -p primed --test windows_components resolver_rejects_missing_component_revision -- --nocapture
run "wrong component digest" cargo test --locked -p primed --test windows_components resolver_rejects_reference_digest_mismatch -- --nocapture
run "component dependency cycle" cargo test --locked -p primed --test windows_components resolver_orders_dependencies_before_dependents_and_rejects_cycles -- --nocapture
run "unsupported component architecture" cargo test --locked -p primed --test windows_components resolver_rejects_duplicate_roots_and_wrong_architecture -- --nocapture
run "unavailable Windows provider" cargo test --locked -p primed --test windows_personality missing_provider_directory_is_explicitly_unavailable -- --nocapture
run "denied unsupported policy" cargo test --locked -p primed policy::tests::filesystem_exposure_fails_closed_until_landlock_backend_exists --lib -- --nocapture
run "installer exit zero with failed post-state" cargo test --locked -p primed --bin prime-windows-component-engine successful_installer_exit_does_not_count_when_probe_fails -- --nocapture

echo "PASS_TOTAL=8"
