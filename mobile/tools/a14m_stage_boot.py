#!/usr/bin/env python3
"""Source-pinned, NON-FLASHABLE A14m/A145P first-light Android boot v2 staging.

This accepts exactly the old AWG1 boot kernel and boot DTB identified by their
Git blob IDs and historic boot metadata. It does not touch a phone, install an
OS, modify Samsung AVB, patch bootloader flags, or claim verified boot.
Header v2 is a *historical TWRP-based candidate*, NOT live handset evidence.
"""
from __future__ import annotations
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re
import struct
import zipfile

SCHEMA='prime.mobile.a14m.boot-image-staging.candidate.v0'
KERNEL_GIT_BLOB='1d3968ea089f88deb19c0b8e1efe482b85ef7e65'
DTB_GIT_BLOB='db8938c3219ff857dafbf59e0b476ecdb751582b'
MAX_IMAGE=80_740_352 # historical TWRP BoardConfig assertion ONLY
BOOT_HEADER_SIZE=1660
FILES_MAX=80 * 1024 * 1024
CPIO_MAX=8 * 1024 * 1024

def sha(data:bytes)->str:return hashlib.sha256(data).hexdigest()
def git_blob(data:bytes)->str:
    return hashlib.sha1(f'blob {len(data)}\0'.encode('ascii')+data).hexdigest()
def align(n:int,p:int)->int:return (n+p-1)//p*p

def parse_boot_info(text:str)->dict:
    result={}
    for row in text.splitlines():
        m=re.fullmatch(r'([a-z_]+)=(.*)',row.strip())
        if not m:raise ValueError('UNEXPECTED_BOOT_INFO_LINE')
        key,value=m.groups()
        if key in result:raise ValueError('DUPLICATE_BOOT_INFO_FIELD')
        result[key]=value.strip().strip('"\'')
    required={'kernel','ramdisk','page_size','dt','kernel_size','ramdisk_size','dtb_offset','dtb_size','base_addr','kernel_offset','ramdisk_offset','tags_offset','cmd_line','board'}
    if not required.issubset(result):raise ValueError('BOOT_INFO_REQUIRED_FIELD_MISSING')
    for key in required-{'kernel','ramdisk','dt','cmd_line','board'}:
        try:result[key]=int(result[key],0)
        except ValueError as exc:raise ValueError('BOOT_INFO_INVALID_NUMBER:'+key) from exc
    if not (result['page_size']==2048 and result['base_addr']==0x40078000
            and result['kernel_offset']==0x8000 and result['ramdisk_offset']==0x07c08000
            and result['tags_offset']==0x0bc08000 and result['dtb_offset']==0x07808000):
        raise ValueError('BOOT_INFO_NOT_PINNED_AWG1_GEOMETRY')
    if result['kernel']!='kernel' or result['dt']!='dtb.img' or result['board']!='SRPVJ12A002':
        raise ValueError('BOOT_INFO_NOT_REFERENCE_BOARD')
    if not result['cmd_line'].startswith('bootopt=') or len(result['cmd_line'])>511:
        raise ValueError('BOOT_INFO_CMDLINE_UNEXPECTED')
    if 'permissive' in result['cmd_line'] or 'disable' in result['cmd_line']:
        raise ValueError('UNSAFE_REFERENCE_CMDLINE')
    return result

def parse_newc(raw:bytes)->dict:
    entries={};pos=0
    for _ in range(32):
        if pos+110>len(raw) or raw[pos:pos+6]!=b'070701':raise ValueError('BAD_NEWC_HEADER')
        try:v=[int(raw[pos+6+i*8:pos+14+i*8],16) for i in range(13)]
        except ValueError as exc:raise ValueError('BAD_NEWC_FIELDS') from exc
        name_start=pos+110; size=v[6];namesize=v[11]
        if not 1<=namesize<=256 or name_start+namesize>len(raw):raise ValueError('BAD_NEWC_NAME')
        nb=raw[name_start:name_start+namesize]
        if not nb.endswith(b'\0'):raise ValueError('BAD_NEWC_NAME_TERM')
        name=nb[:-1].decode('ascii');start=align(name_start+namesize,4)
        if name.startswith('/') or '..' in name.split('/') or name in entries or start+size>len(raw):
            raise ValueError('BAD_NEWC_ENTRY')
        entries[name]=(v[1],v[9],v[10],raw[start:start+size]);pos=align(start+size,4)
        if name=='TRAILER!!!':break
    else:raise ValueError('TRAILER_MISSING')
    if pos!=len(raw):raise ValueError('NECW_EXTRA_DATA')
    if set(entries)!={'dev','dev/console','dev/null','dev/kmsg','proc','sys','init','TRAILER!!!'}:
        raise ValueError('NECW_ENTRIES_NOT_ALLOWLISTED')
    for name in ('dev','proc','sys'):
        if entries[name][0]&0o170000!=0o040000:raise ValueError('DIR_MODE_INVALID')
    for name,maj,minor in [('dev/console',5,1),('dev/null',1,3),('dev/kmsg',1,11)]:
        mode,ma,mi,data=entries[name]
        if mode&0o170000!=0o020000 or (ma,mi)!=(maj,minor) or data:
            raise ValueError('CHAR_DEVICE_INVALID:'+name)
    init=entries['init'][3]
    if entries['init'][0]&0o170000!=0o100000 or not (entries['init'][0]&0o111):
        raise ValueError('INVALID_INIT_MODE')
    if init[:6]!=b'\x7fELF\x02\x01' or struct.unpack_from('<H',init,18)[0]!=183:
        raise ValueError('INIT_NOT_AARCH64_ELF')
    if b'PRIME_A14M_FIRST_LIGHT|stage=ENTERED_UNQUALIFIED' not in init or b'NO_PRIME_CORE_NO_PHONE_QUALIFICATION' not in init:
        raise ValueError('INIT_MISSING_TRUTH_MARKERS')
    # Reject PT_INTERP and PT_DYNAMIC: this is a freestanding static init.
    phoff=struct.unpack_from('<Q',init,32)[0];entsize,num=struct.unpack_from('<HH',init,54)
    if not 1<=num<=64 or entsize<56 or phoff+num*entsize>len(init):raise ValueError('INIT_ELF_PHDR_INVALID')
    if any(struct.unpack_from('<I',init,phoff+i*entsize)[0] in (2,3) for i in range(num)):
        raise ValueError('INIT_NOT_STATIC')
    return {'entry_count':len(entries),'init_sha256':sha(init),'names':sorted(entries)}

def newc(name:str,data:bytes,mode:int,inode:int,maj:int=0,minor:int=0)->bytes:
    if len(name)>255 or not name.isascii():raise ValueError('BAD_NEWC_PATH')
    names=name.encode()+b'\0'
    fields=(inode,mode,0,0,2 if mode&0o170000==0o040000 else 1,0,len(data),0,0,maj,minor,len(names),0)
    head=b'070701'+b''.join(f'{v:08x}'.encode() for v in fields)
    prefix=head+names
    return prefix+b'\0'*((-len(prefix))%4)+data+b'\0'*((-len(data))%4)

def make_ramdisk(init_elf:bytes)->tuple[bytes,dict]:
    names=[('dev',b'',0o040755,0,0),('dev/console',b'',0o020600,5,1),
           ('dev/null',b'',0o020666,1,3),('dev/kmsg',b'',0o020600,1,11),
           ('proc',b'',0o040755,0,0),('sys',b'',0o040755,0,0),
           ('init',init_elf,0o100755,0,0),('TRAILER!!!',b'',0,0,0)]
    raw=b''.join(newc(n,b,mode,i+1,maj,mi) for i,(n,b,mode,maj,mi) in enumerate(names))
    inspected=parse_newc(raw)
    return gzip.compress(raw,compresslevel=9,mtime=0),inspected

def inspect_reference_dt_table(dtb:bytes)->dict:
    """Admit AWG1's actual dt_table container with one bounded FDT entry.

    The pinned 135300-byte boot/dtb.img is NOT a raw FDT at offset 0.
    Its first 32 bytes are Android's big-endian dt_table_header, then
    one 32-byte dt_table_entry, then a single big-endian FDT at byte 64.
    Preserve it as-is: stripping the table would change published boot bytes.
    """
    if len(dtb)<72:raise ValueError('DT_TABLE_TRUNCATED')
    magic,total,header,entry_size,count,entry_offset,page,version=struct.unpack_from('>8I',dtb,0)
    if (magic,total,header,entry_size,count,entry_offset,page,version)!=(
            0xd7b7ab1e,len(dtb),32,32,1,32,2048,0):
        raise ValueError('AWG1_DT_TABLE_HEADER_UNEXPECTED')
    size,offset,board_id,revision,*custom=struct.unpack_from('>8I',dtb,entry_offset)
    if (offset,size)!=(64,len(dtb)-64) or offset+size!=len(dtb):
        raise ValueError('AWG1_DT_TABLE_ENTRY_INVALID')
    if dtb[offset:offset+4]!=bytes.fromhex('d00dfeed'):
        raise ValueError('AWG1_FDT_MAGIC_MISSING')
    size_fdt=struct.unpack_from('>I',dtb,offset+4)[0]
    if size_fdt!=size:raise ValueError('AWG1_FDT_SIZE_MISMATCH')
    return {'format':'ANDROID_DT_TABLE_V0_WITH_ONE_FDT','total_bytes':len(dtb),
            'fdt_offset':offset,'fdt_bytes':size,'entry_count':count,
            'page_size':page,'board_id':board_id,'revision':revision,
            'custom_fields':custom}

def boot_image(kernel:bytes,ramdisk:bytes,dtb:bytes,info:dict)->bytes:
    page=info['page_size'];base=info['base_addr'];name=info['board'].encode()
    cmdline=info['cmd_line'].encode()
    if len(name)>15 or len(cmdline)>511:raise ValueError('OVERSIZED_BOOT_STRING')
    if not 0<len(kernel)<=30*1024*1024 or not 0<len(ramdisk)<=CPIO_MAX or len(dtb)!=info['dtb_size']:
        raise ValueError('BOOT_SECTION_SIZE_INVALID')
    if len(kernel)!=info['kernel_size']:raise ValueError('KERNEL_SIZE_MISMATCH')
    inspect_reference_dt_table(dtb)  # AWG1 has dt_table header; FDT starts at offset 64
    # Android v2 BOOT_MAGIC, ten u32s, 16/512 fields, 32-byte id, 1024 extra,
    # v1 recovery_dtbo size/offset/header_size and v2 dtb size/address.
    base_header=struct.pack('<8s10I16s512s',b'ANDROID!',len(kernel),base+info['kernel_offset'],
                            len(ramdisk),base+info['ramdisk_offset'],0,base+0x00f00000,
                            base+info['tags_offset'],page,2,0,name,cmdline)
    identity=hashlib.sha1()
    for part in (kernel,ramdisk,b'',b'',dtb):
        identity.update(part);identity.update(struct.pack('<I',len(part)))
    header=base_header+identity.digest().ljust(32,b'\0')+b'\0'*1024+struct.pack('<IQIIQ',0,0,BOOT_HEADER_SIZE,len(dtb),base+info['dtb_offset'])
    if len(header)!=BOOT_HEADER_SIZE:raise AssertionError('HEADER_V2_CONSTRUCTION_ERROR')
    layout=header.ljust(page,b'\0')
    for payload in (kernel,ramdisk,dtb):
        layout+=payload+b'\0'*(align(len(payload),page)-len(payload))
    if len(layout)>MAX_IMAGE:raise ValueError('REFERENCE_BOOT_PARTITION_LIMIT_EXCEEDED')
    return layout

def inspect_boot_image(raw:bytes,info:dict,expected_kernel:bytes,expected_rd:bytes,expected_dtb:bytes)->dict:
    if len(raw)<2048 or raw[:8]!=b'ANDROID!':raise ValueError('BOOT_MAGIC_MISSING')
    vals=struct.unpack_from('<10I',raw,8)
    kernel_size,kaddr,rd_size,rdaddr,second_size,_,tags,page,ver,os_version=vals
    if (ver,page,second_size,os_version)!=(2,2048,0,0):raise ValueError('BOOT_HEADER_VERSION_INVALID')
    if (kernel_size,rd_size,kaddr,rdaddr,tags)!=(len(expected_kernel),len(expected_rd),info['base_addr']+info['kernel_offset'],info['base_addr']+info['ramdisk_offset'],info['base_addr']+info['tags_offset']):
        raise ValueError('BOOT_IMAGE_FIELDS_MISMATCH')
    dtbsize,dtbaddr=struct.unpack_from('<IQ',raw,BOOT_HEADER_SIZE-12)
    if (dtbsize,dtbaddr)!=(len(expected_dtb),info['base_addr']+info['dtb_offset']):raise ValueError('DTB_IMAGE_ADDRESS_INVALID')
    p=page
    kernelstart=p;ramstart=p+align(kernel_size,p);dtbstart=ramstart+align(rd_size,p)
    if not (raw[kernelstart:kernelstart+kernel_size]==expected_kernel and raw[ramstart:ramstart+rd_size]==expected_rd and raw[dtbstart:dtbstart+dtbsize]==expected_dtb):
        raise ValueError('ROUNDTRIP_IMAGE_CONTENTS_MISMATCH')
    if len(raw)!=dtbstart+align(dtbsize,p):raise ValueError('UNEXPECTED_BOOT_IMAGE_TRAILING_BYTES')
    return {'header_version':ver,'page_size':page,'kernel_bytes':kernel_size,'ramdisk_bytes':rd_size,'dtb_bytes':dtbsize,
            'kernel_load_address':hex(kaddr),'ramdisk_load_address':hex(rdaddr),'dtb_load_address':hex(dtbaddr),'total_bytes':len(raw),
            'identity_sha1':raw[576:596].hex()}

def safe_zip(out:Path,files:dict[str,bytes])->None:
    out.parent.mkdir(parents=True,exist_ok=True)
    with zipfile.ZipFile(out,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as z:
        for name,data in sorted(files.items()):
            if '/' in name or name.startswith('.'):raise ValueError('UNSAFE_ARCHIVE_NAME')
            item=zipfile.ZipInfo('A14M_REFERENCE_STAGING_DO_NOT_FLASH/'+name,date_time=(2026,11,15,0,0,0))
            item.compress_type=zipfile.ZIP_DEFLATED
            item.external_attr=0o100644<<16
            z.writestr(item,data,compress_type=zipfile.ZIP_DEFLATED,compresslevel=9)
    with zipfile.ZipFile(out) as z:
        if z.testzip():raise ValueError('ZIP_CRC_FAILURE')

def stage(kernel_path:Path,dtb_path:Path,info_path:Path,init_path:Path,out:Path,source_sha:str)->dict:
    if not re.fullmatch('[a-f0-9]{40}',source_sha):raise ValueError('INVALID_SOURCE_SHA')
    files=[kernel_path,dtb_path,info_path,init_path]
    if any(not p.is_file() or p.stat().st_size<=0 or p.stat().st_size>FILES_MAX for p in files):
        raise ValueError('MISSING_OR_OVERSIZED_INPUT')
    kernel,dtb=kernel_path.read_bytes(),dtb_path.read_bytes()
    meta=parse_boot_info(info_path.read_text())
    if git_blob(kernel)!=KERNEL_GIT_BLOB:raise ValueError('HISTORICAL_KERNEL_BLOB_MISMATCH')
    if git_blob(dtb)!=DTB_GIT_BLOB:raise ValueError('HISTORICAL_DTB_BLOB_MISMATCH')
    init=init_path.read_bytes();ramdisk,ramdetail=make_ramdisk(init)
    candidate=boot_image(kernel,ramdisk,dtb,meta)
    geometry=inspect_boot_image(candidate,meta,kernel,ramdisk,dtb)
    out.mkdir(parents=True,exist_ok=True)
    manifest={'schema':SCHEMA,'scope':'HISTORICAL_AWG1_A145P_V2_REFERENCE_ONLY',
      'source_commit':source_sha,'kernel_git_blob':KERNEL_GIT_BLOB,'dtb_git_blob':DTB_GIT_BLOB,
      'boot_img_sha256':sha(candidate),'ramdisk_sha256':sha(ramdisk),'kernel_sha256':sha(kernel),'dtb_sha256':sha(dtb),
      'early_init_sha256':sha(init),'image':geometry,'dtb_table':inspect_reference_dt_table(dtb),'ramdisk':ramdetail,
      'boot_image_ready':False,'boot_chain_qualified':False,'physical_boot_observed':False,
      'twrp_recovery_modified':False,'flash_authorized':False,'bootc_generation_bound':False,
      'prime_core_running':False,'mobile_shell_running':False,'source_license_review_complete':False,
      'limitations':['Old AWG1 source kernel not proven compatible with current TWRP-only handset',
          'Historical v2 header and DTB offset are a candidate, not active boot-chain proof',
          'No vendor_boot/init_boot/AVB trust chain reconstructed; do not install or flash',
          'Early PID1 is a static diagnostic, NOT the Prime Rust Core, Shell or production OS',
          'Kernel GPL/source/vendor redistribution rights need independent review prior to release']}
    image_name='a14m-first-light.reference-v2.boot.img'
    receipt=(json.dumps(manifest,sort_keys=True,indent=2)+'\n').encode()
    readme=(
      'Prime Mobile A14m — historical AWG1 first-light IMAGE STAGING\n\n'
      'NON-FLASHABLE. DO NOT INSTALL IN TWRP, ODIN, FASTBOOT, DOWNLOAD MODE, OR AS OTA.\n'
      'This is a structurally validated Android boot header v2 candidate using an old AWG1\n'
      'boot kernel and DTB, plus a standalone AArch64 logging PID1. It is not a Prime OS\n'
      'Mobile release, cannot be considered compatible with the current handset, and is not\n'
      'tested on physical MT6768 hardware. The current TWRP remains untouched.\n\n'
      'Sources: https://github.com/forforksake/a14m_dump/tree/699855860e672eb5f3582ecf8e2144de0ed8b2d1\n'
      'Kernel source: https://github.com/forforksake/android_kernel_samsung_a145p/tree/2ad60c6b7b89f6f37d1eb483696e7d121c8972bd\n'
      'Android boot layout: https://source.android.com/docs/core/architecture/bootloader/dtb-images\n'
      'The package contains no recovery image, vbmeta disable flags, user data or device secrets.\n').encode()
    payload={image_name:candidate,'manifest.json':receipt,'README_NOT_FOR_FLASH.txt':readme,
             'first-light-init-aarch64.elf':init}
    checksums=''.join(f'{sha(b)}  {name}\n' for name,b in sorted(payload.items())).encode()
    payload['SHA256SUMS']=checksums
    safe_zip(out/'A14M_REFERENCE_STAGING_DO_NOT_FLASH.zip',payload)
    (out/image_name).write_bytes(candidate)
    (out/'manifest.json').write_bytes(receipt)
    (out/'first-light-init-aarch64.elf').write_bytes(init)
    (out/'SHA256SUMS').write_bytes(checksums)
    return manifest

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for arg in ('kernel','dtb','boot-info','init','out'):
        p.add_argument('--'+arg,type=Path,required=True)
    p.add_argument('--source-commit',required=True)
    a=p.parse_args()
    r=stage(a.kernel,a.dtb,a.boot_info,a.init,a.out,a.source_commit)
    print(json.dumps(r,sort_keys=True,indent=2))
if __name__=='__main__':main()
