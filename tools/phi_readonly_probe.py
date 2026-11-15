#!/usr/bin/env python3
"""Development-only PHI memory-tier observation. No mutations or host authority.

This is NOT Prime Core's runtime source of truth. It gathers source-labelled,
optional Linux evidence for contract prototyping, Desktop and Mobile alike.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import re
from typing import Any

SCHEMA = 'prime.phi.memory-observation.candidate.v0'
MAX_BYTES = (1 << 64) - 1


def _safe_read(root: Path, name: str) -> str | None:
    """Only read a file beneath the selected synthetic or actual Linux root."""
    try:
        base = root.resolve(strict=True)
        p = (base / name.lstrip('/')).resolve(strict=True)
        if not p.is_relative_to(base) or not p.is_file():
            return None
        return p.read_text(encoding='utf-8', errors='replace')
    except (OSError, ValueError):
        return None


def _ki_bytes(value: str) -> int | None:
    match = re.fullmatch(r'\s*(\d+)\s+kB\s*', value)
    if not match:
        return None
    value = int(match.group(1))
    return value * 1024 if value <= MAX_BYTES // 1024 else None


def _u64(value: str) -> int | None:
    if not re.fullmatch(r'\d+', value):
        return None
    number = int(value)
    return number if number <= MAX_BYTES else None


def _meminfo(content: str | None) -> dict[str, int | None]:
    result = {'MemTotal': None, 'MemAvailable': None, 'SwapTotal': None, 'SwapFree': None}
    if content is None:
        return result
    for line in content.splitlines():
        key, sep, value = line.partition(':')
        if sep and key in result and result[key] is None:
            result[key] = _ki_bytes(value)
    return result


def _pressure(content: str | None) -> dict[str, dict[str, int | float] | None]:
    result: dict[str, dict[str, int | float] | None] = {'some': None, 'full': None}
    if content is None:
        return result
    for line in content.splitlines():
        tokens = line.split()
        if not tokens:
            continue
        name, *pairs = tokens
        if name not in result or result[name] is not None:
            continue
        fields = {}
        for pair in pairs:
            key, sep, raw = pair.partition('=')
            if not sep or key not in ('avg10','avg60','avg300','total'):
                continue
            try:
                if key == 'total':
                    value = _u64(raw)
                else:
                    value = float(raw)
                    if not (0 <= value <= 100):
                        value = None
                if value is not None:
                    fields[key] = value
            except ValueError:
                pass
        if set(fields) == {'avg10','avg60','avg300','total'}:
            result[name] = fields
    return result


def _swap_entries(content: str | None) -> tuple[list[dict[str, Any]], bool]:
    if content is None:
        return [], False
    lines = content.splitlines()
    if not lines or lines[0].split()[:2] != ['Filename','Type']:
        return [], False
    entries: list[dict[str, Any]] = []
    for row in lines[1:]:
        fields = row.split()
        if len(fields) != 5:
            return entries, False
        # Deliberately avoid storing pathname: it might contain private user data.
        _, kind, size, used, priority = fields
        size_bytes = _ki_bytes(size + ' kB')
        used_bytes = _ki_bytes(used + ' kB')
        try:
            p = int(priority)
        except ValueError:
            return entries, False
        if kind not in ('file','partition') or None in (size_bytes,used_bytes) or used_bytes > size_bytes:
            return entries, False
        entries.append({'kind': kind, 'size_bytes': size_bytes,
                        'used_bytes': used_bytes, 'priority': p})
    return entries, True


def snapshot(root: Path = Path('/'), observed_at: str | None = None) -> dict[str, Any]:
    root = Path(root)
    observed_at = observed_at or datetime.now(timezone.utc).isoformat(timespec='seconds')
    memraw = _safe_read(root, '/proc/meminfo')
    presraw = _safe_read(root, '/proc/pressure/memory')
    swapraw = _safe_read(root, '/proc/swaps')
    mem = _meminfo(memraw)
    pressure = _pressure(presraw)
    swap_entries, swap_valid = _swap_entries(swapraw)
    limitations: list[str] = []
    sources = {}
    for family, path, text in (
        ('physical_and_swap', '/proc/meminfo', memraw),
        ('memory_pressure', '/proc/pressure/memory', presraw),
        ('swap_devices', '/proc/swaps', swapraw),
    ):
        sources[family] = {'path': path, 'readable': text is not None}
        if text is None:
            limitations.append(f'{family.upper()}_UNAVAILABLE')
    if memraw is not None and any(v is None for v in mem.values()):
        limitations.append('MEMINFO_INCOMPLETE')
    if presraw is not None and any(v is None for v in pressure.values()):
        limitations.append('PRESSURE_INCOMPLETE')
    if swapraw is not None and not swap_valid:
        limitations.append('SWAPS_MALFORMED')

    zram = []
    block = root / 'sys/block'
    if block.exists() and block.is_dir():
        names = sorted(p.name for p in block.iterdir() if p.name.startswith('zram') and p.name[4:].isdigit())
        for name in names:
            mm = _safe_read(root, f'/sys/block/{name}/mm_stat')
            values = mm.split() if mm else []
            parsed = [_u64(x) for x in values[:3]] if len(values) >= 3 else []
            if len(parsed) == 3 and all(x is not None for x in parsed):
                zram.append({'kernel_name': name, 'original_data_bytes': parsed[0],
                             'compressed_data_bytes': parsed[1], 'memory_used_bytes': parsed[2],
                             'source': f'/sys/block/{name}/mm_stat'})
            else:
                zram.append({'kernel_name': name, 'original_data_bytes': None,
                             'compressed_data_bytes': None, 'memory_used_bytes': None,
                             'source': f'/sys/block/{name}/mm_stat'})
                limitations.append(f'ZRAM_MM_STAT_UNAVAILABLE:{name}')
    else:
        limitations.append('ZRAM_INVENTORY_UNAVAILABLE')

    zswap_enabled = _safe_read(root, '/sys/module/zswap/parameters/enabled')
    if zswap_enabled is None:
        zswap_state = 'UNKNOWN'
        limitations.append('ZSWAP_STATE_UNAVAILABLE')
    else:
        option = zswap_enabled.strip().lower()
        zswap_state = 'ENABLED' if option in ('y','1') else 'DISABLED' if option in ('n','0') else 'UNKNOWN'
        if zswap_state == 'UNKNOWN':
            limitations.append('ZSWAP_STATE_UNRECOGNIZED')

    if mem['MemAvailable'] is not None and mem['MemTotal'] is not None and mem['MemAvailable'] > mem['MemTotal']:
        limitations.append('MEMORY_COUNTER_INCONSISTENT')
    if mem['SwapFree'] is not None and mem['SwapTotal'] is not None and mem['SwapFree'] > mem['SwapTotal']:
        limitations.append('SWAP_COUNTER_INCONSISTENT')

    return {
        'schema': SCHEMA,
        'status': 'DEVELOPMENT_READ_ONLY_NOT_OS_AUTHORITY',
        'observed_at': observed_at,
        'memory': {'physical_total_bytes': mem['MemTotal'], 'available_bytes': mem['MemAvailable'],
                   'swap_total_bytes': mem['SwapTotal'], 'swap_free_bytes': mem['SwapFree']},
        'memory_pressure': pressure,
        'swap_devices': swap_entries,
        'zram': zram,
        'zswap': {'state': zswap_state, 'enabled_does_not_prove_usage': True},
        'sources': sources,
        'limitations': sorted(set(limitations)),
        'facts': {'swap_is_not_physical_ram': True, 'model_streaming_not_counted_as_swap': True},
    }


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--root', type=Path, default=Path('/'), help='Read-only synthetic /proc,/sys fixture root')
    ap.add_argument('--output', type=Path, help='JSON output (otherwise stdout)')
    a = ap.parse_args()
    data = json.dumps(snapshot(a.root), indent=2, sort_keys=True) + '\n'
    if a.output:
        a.output.write_text(data, encoding='utf-8')
    else:
        print(data, end='')


if __name__ == '__main__':
    main()