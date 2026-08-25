# Prime Host Identity v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Schema identifier: `prime.host-identity.v1`

## Authority

A Prime Host is one local physical or virtual machine running Prime. Host authority is self-local only.

`Prime Host ID != Origins Node ID != Ptah Node ID`

## Host ID

`host_id` is an opaque UUIDv7 generated on first Prime enrollment. It is not a MAC address, disk serial, SMBIOS serial or hash of one hardware property.

Durable record:

`/var/lib/prime/identity/host.json`

The file is root-owned and atomically replaced. Public interfaces expose normalized identity fields and fingerprint digests, not raw hardware serials by default.

## Required record

```json
{
  "schema": "prime.host-identity.v1",
  "host_id": "uuidv7",
  "lineage_id": "uuidv7",
  "created_at": "RFC3339",
  "host_arch": "x86_64",
  "hardware_fingerprint": {
    "algorithm": "sha256",
    "digest": "hex-or-null",
    "confidence": "HIGH|MEDIUM|LOW|UNPROBED",
    "observed_at": "RFC3339-or-null"
  },
  "rebind_revision": 0,
  "supersedes_host_id": null
}
```

## Hardware evidence

Prime may use locally available stable evidence such as SMBIOS system UUID, baseboard/chassis identity and TPM endorsement-key public identity where available. Raw evidence is treated as privileged machine data.

Disk identity is inventory evidence, not Host identity. RAM, GPU, NIC, SSD and ordinary peripheral changes do not by themselves create a new Host.

## Classification

- ordinary hardware change: retain Host ID; increment hardware graph revision;
- material machine migration: default to new Host ID;
- explicit owner-approved rebind: retain lineage through an auditable supersession/rebind record.

A materially changed SMBIOS/baseboard identity is sufficient to require new-host/rebind evaluation. When firmware exposes generic or unreliable identifiers, Prime must lower confidence and require explicit evidence rather than silently deciding continuity.

## Rebind evidence

A rebind record contains old Host ID, resulting Host ID, old/new fingerprint digests, reason, timestamp, actor/authority and evidence references.

Rebind never rewrites historical generations or evidence to pretend they ran on different hardware.

## Failure behavior

Unreadable/corrupt identity state blocks ordinary Prime Host authority startup and enters recovery/repair handling. Prime must not silently generate a replacement Host ID over an existing corrupt record.
