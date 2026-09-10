#!/usr/bin/env python3
import datetime as dt
import struct
import sys
from pathlib import Path
import olefile

if len(sys.argv) != 2:
    raise SystemExit('usage: normalize_msi.py FILE.msi')
path = Path(sys.argv[1])
stream_name = '\x05SummaryInformation'
fixed = dt.datetime(2024, 1, 1, tzinfo=dt.timezone.utc)
filetime_epoch = dt.datetime(1601, 1, 1, tzinfo=dt.timezone.utc)
fixed_filetime = int((fixed - filetime_epoch).total_seconds() * 10_000_000)

with olefile.OleFileIO(path, write_mode=True) as ole:
    if not ole.exists(stream_name):
        raise SystemExit('SummaryInformation stream missing')
    summary = bytearray(ole.openstream(stream_name).read())
    if len(summary) < 48 or summary[0:2] != b'\xfe\xff':
        raise SystemExit('invalid property-set stream header')
    section_count = struct.unpack_from('<I', summary, 24)[0]
    if section_count != 1:
        raise SystemExit(f'expected one SummaryInformation section, found {section_count}')
    section = struct.unpack_from('<I', summary, 44)[0]
    if section + 8 > len(summary):
        raise SystemExit('invalid SummaryInformation section offset')
    section_size, prop_count = struct.unpack_from('<II', summary, section)
    if section_size < 8 or section + section_size > len(summary):
        raise SystemExit('invalid SummaryInformation section size')
    table_end = section + 8 + prop_count * 8
    if table_end > section + section_size:
        raise SystemExit('truncated SummaryInformation property table')
    props = {}
    for i in range(prop_count):
        pid, rel = struct.unpack_from('<II', summary, section + 8 + i * 8)
        if pid in props:
            raise SystemExit(f'duplicate SummaryInformation property id {pid}')
        props[pid] = rel
    for pid in (12, 13):
        if pid not in props:
            raise SystemExit(f'required FILETIME property {pid} missing')
        value = section + props[pid]
        if value + 12 > section + section_size:
            raise SystemExit(f'FILETIME property {pid} outside section')
        variant_type = struct.unpack_from('<I', summary, value)[0]
        if variant_type != 0x40:
            raise SystemExit(f'property {pid} is not VT_FILETIME: 0x{variant_type:x}')
        struct.pack_into('<Q', summary, value + 4, fixed_filetime)
    ole.write_stream(stream_name, bytes(summary))

# Normalize the only embedded CAB file timestamp after OLE writeback.
data = bytearray(path.read_bytes())
positions = []
start = 0
while True:
    idx = data.find(b'MSCF', start)
    if idx < 0:
        break
    positions.append(idx)
    start = idx + 4
if len(positions) != 1:
    raise SystemExit(f'expected exactly one embedded CAB, found {len(positions)}')
cab = positions[0]
if cab + 36 > len(data):
    raise SystemExit('truncated CAB header')
cb_cabinet = struct.unpack_from('<I', data, cab + 8)[0]
coff_files = struct.unpack_from('<I', data, cab + 16)[0]
if cb_cabinet < 44 or cab + cb_cabinet > len(data):
    raise SystemExit('invalid CAB size')
cffile = cab + coff_files
if cffile + 16 > cab + cb_cabinet:
    raise SystemExit('CFFILE outside CAB')
name_end = data.find(b'\0', cffile + 16, cab + cb_cabinet)
if name_end < 0:
    raise SystemExit('unterminated CFFILE name')
name = bytes(data[cffile + 16:name_end])
if name != b'MsiProofFile':
    raise SystemExit(f'unexpected CAB file name {name!r}')
dos_date = ((2024 - 1980) << 9) | (1 << 5) | 1
struct.pack_into('<HH', data, cffile + 10, dos_date, 0)
path.write_bytes(data)
print(f'normalized SummaryInformation PIDs 12/13 and CAB file {name.decode()} to 2024-01-01T00:00:00Z')
