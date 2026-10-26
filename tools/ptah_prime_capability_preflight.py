"""Read-only F04-F07 preparation for a future Prime -> Ptah adapter.

This evaluates an explicitly supplied Prime snapshot. It never connects to
Prime, enrolls a Ptah Node, selects a task, or grants execution authority.
A compatible snapshot is NOT a live capability or a Ptah admission decision.
"""

import re
from datetime import datetime, timedelta, timezone
from uuid import UUID


class SnapshotRejected(ValueError):
    """The offline Prime snapshot cannot support even a candidate mapping."""


def _record(value, label):
    if type(value) is not dict:
        raise SnapshotRejected(f"{label} must be an object")
    return value


def _identity(value, label):
    if type(value) is not str or not value or value != value.strip():
        raise SnapshotRejected(f"invalid {label}")
    if any(char.isspace() or ord(char) < 32 or ord(char) == 127 for char in value):
        raise SnapshotRejected(f"invalid {label}")
    return value


def _prime_host_id(value):
    """Bind the independently supplied Host identity to Prime's frozen UUIDv7 contract."""
    value = _identity(value, "expected Prime Host ID")
    try:
        parsed = UUID(value)
    except ValueError as error:
        raise SnapshotRejected("expected Prime Host ID must be canonical UUIDv7") from error
    if parsed.version != 7 or str(parsed) != value:
        raise SnapshotRejected("expected Prime Host ID must be canonical UUIDv7")
    return value


_RFC3339_OBSERVATION = re.compile(
    r"[0-9]{4}-[0-9]{2}-[0-9]{2}[Tt](?:[01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]"
    r"(?P<fraction>\.[0-9]{1,9})?(?:[Zz]|[+-](?:[01][0-9]|2[0-3]):[0-5][0-9])\Z"
)


def _observation(value):
    # datetime.fromisoformat also accepts week dates, basic dates, and offsets
    # without a colon. Those are not Prime's RFC3339 health observations.
    match = _RFC3339_OBSERVATION.fullmatch(value) if type(value) is str else None
    if match is None:
        raise SnapshotRejected("capability observation must be RFC3339")
    try:
        normalized = value.replace("t", "T").replace("z", "Z").replace("Z", "+00:00")
        observed = datetime.fromisoformat(normalized)
    except ValueError as error:
        raise SnapshotRejected("invalid capability observation time") from error
    if observed.tzinfo is None or observed.utcoffset() is None:
        raise SnapshotRejected("capability observation must include timezone")
    fraction = match.group("fraction")
    # Python truncates RFC3339 nanoseconds to microseconds. Retain the
    # remaining 0-999 ns so a future-dated submicrosecond sample cannot pass.
    submicro_ns = int((fraction[1:] if fraction else "").ljust(9, "0")[6:])
    return observed.astimezone(timezone.utc), submicro_ns


def preflight_snapshot(
    host_projection,
    descriptor,
    *,
    expected_host_id,
    expected_generation_id,
    requested_capability_id,
    now,
    max_age=timedelta(minutes=5),
):
    """Check a frozen Prime v1 snapshot without granting runtime authority.

    Caller supplies exact Prime Host and generation identity independently.
    The output is a source-snapshot diagnostic only; live negotiation, Ptah
    enrollment, workload policy, effect witnessing and release remain external.
    """
    expected_host_id = _prime_host_id(expected_host_id)
    expected_generation_id = _identity(expected_generation_id, "expected generation ID")
    requested_capability_id = _identity(requested_capability_id, "requested capability ID")
    if not isinstance(now, datetime) or now.tzinfo is None or now.utcoffset() is None:
        raise SnapshotRejected("current time must be timezone-aware")
    if not isinstance(max_age, timedelta) or max_age <= timedelta(0):
        raise SnapshotRejected("invalid snapshot age limit")
    now_utc = now.astimezone(timezone.utc)
    projection = _record(host_projection, "Prime Host projection")
    capability = _record(descriptor, "Prime capability descriptor")
    host = _record(projection.get("host"), "Prime Host")
    provider = _record(capability.get("provider"), "capability provider")
    placement = _record(capability.get("placement"), "capability placement")
    health = _record(capability.get("health"), "capability health")

    if (projection.get("interface") != "prime.capability.v1"
            or projection.get("interface_version") != "1.0"):
        raise SnapshotRejected("unsupported Prime interface snapshot")
    if (_identity(host.get("host_id"), "Prime Host ID") != expected_host_id
            or _identity(placement.get("host_id"), "placement Host ID") != expected_host_id):
        raise SnapshotRejected("Prime Host identity mismatch")
    if (_identity(host.get("generation_id"), "Prime generation ID") != expected_generation_id
            or _identity(provider.get("generation_id"), "provider generation ID") != expected_generation_id):
        raise SnapshotRejected("Prime generation identity mismatch")
    if provider.get("id") != "prime" or placement.get("scope") != "HOST_LOCAL":
        raise SnapshotRejected("nonlocal or non-Prime capability authority")
    if capability.get("capability_id") != requested_capability_id:
        raise SnapshotRejected("capability identity mismatch")
    if capability.get("availability") != "AVAILABLE" or health.get("status") != "HEALTHY":
        raise SnapshotRejected("capability is not observed healthy and available")
    observed, submicro_ns = _observation(health.get("observed_at"))
    if (observed > now_utc or (observed == now_utc and submicro_ns)
            or now_utc - observed > max_age):
        raise SnapshotRejected("capability observation is stale or in the future")

    observed_text = observed.isoformat()
    if submicro_ns:
        clock, offset = observed_text.rsplit("+", 1)
        if "." not in clock:
            clock += ".000000"
        observed_text = f"{clock}{submicro_ns:03d}+{offset}"

    return {
        "candidate_compatible": True,
        "evidence_scope": "OFFLINE_PRIME_SNAPSHOT_ONLY",
        "prime_host_id": expected_host_id,
        "prime_generation_id": expected_generation_id,
        "capability_id": requested_capability_id,
        "observed_at": observed_text,
        "ptah_node_enrolled": False,
        "execution_authorized": False,
        "live_verified": False,
    }
