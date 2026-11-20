#!/usr/bin/env python3
"""Verify *captured* ARM64 guest UART Core HTTP transport receipts.

The input must originate from a separately pinned QEMU/kernel/initrd run.
Synthetic strings cannot prove Linux boot or Prime production generation.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path

PREFIX='PRIME_MOB1_CORE_V0|'
REQUIRED=['stage=ENTERED','core_process_spawned=true',
          'versions_status=200','missing_version_status=400',
          'missing_version_rejection_verified=true',
          'incompatible_version_status=409',
          'incompatible_version_rejection_verified=true','capabilities_status=200',
          'stage=TRANSPORT_ONLY_UNQUALIFIED']

def evaluate(uart:str)->dict:
    marks=[];j={};blocked=[]
    for line in uart.splitlines():
        if PREFIX not in line:continue
        if not line.startswith(PREFIX):blocked.append('EMBEDDED_MARKER');continue
        candidate=line[len(PREFIX):]
        if len(candidate)>65536 or '\r' in candidate:blocked.append('INVALID_MARKER');continue
        if candidate.startswith('error='):blocked.append(candidate);continue
        if '_json=' in candidate:
            k,payload=candidate.split('_json=',1)
            if k in j:blocked.append('DUPLICATE_JSON:'+k);continue
            try:j[k]=json.loads(payload)
            except json.JSONDecodeError:blocked.append('INVALID_JSON:'+k)
        else:marks.append(candidate)
    if marks!=REQUIRED:blocked.append('MISSING_OR_OUT_OF_ORDER_MARKERS')
    versions=j.get('versions',{}); caps=j.get('capabilities',{})
    if not isinstance(versions,dict) or versions.get('interface')!='prime.capability.v1' or '1.0' not in versions.get('supported_versions',[]):
        blocked.append('VERSIONS_INVALID')
    if not isinstance(caps,dict) or caps.get('interface')!='prime.capability.v1' or caps.get('interface_version')!='1.0':
        blocked.append('CAPABILITIES_INTERFACE_INVALID')
    descriptors=caps.get('capabilities',[]) if isinstance(caps,dict) else []
    if not isinstance(descriptors,list) or not any(isinstance(c,dict) and c.get('capability_id')=='prime.capability.interface' for c in descriptors):
        blocked.append('CORE_INTERFACE_DESCRIPTOR_MISSING')
    if not isinstance(caps.get('generation_id'),str) or not caps['generation_id'].startswith('mob1-unqualified-'):
        blocked.append('NOT_EXPLICITLY_LAB_UNQUALIFIED')
    if isinstance(descriptors,list):
        if not any(isinstance(c,dict) and c.get('capability_id')=='prime.host.identity' and c.get('availability')!='AVAILABLE' for c in descriptors):
            blocked.append('HOST_IDENTITY_FALSIFIED_AS_AVAILABLE')
    if 'Kernel panic' in uart: blocked.append('GUEST_KERNEL_PANIC')
    return {'schema':'prime.mob1.primed-guest-transport-proof.v0',
     'transport_responded':not blocked,'status':'CORE_TRANSPORT_RESPONDED_UNQUALIFIED' if not blocked else 'UNQUALIFIED',
     'blockers':blocked,'version':versions,'capability_count':len(descriptors) if isinstance(descriptors,list) else 0,
     'generation_id':caps.get('generation_id') if isinstance(caps,dict) else None,
     'bootc_generation_bound':False,'core_deployment_accepted':False,
     'shell_ready':False,'phone_qualified':False,'flash_authorization':False}

if __name__=='__main__':
    a=argparse.ArgumentParser();a.add_argument('uart',type=Path);v=a.parse_args()
    print(json.dumps(evaluate(v.uart.read_text(errors='replace')),indent=2,sort_keys=True))
