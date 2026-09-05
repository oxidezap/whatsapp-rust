#!/usr/bin/env python3
"""Losslessly encode fixture JSON values as CBOR, then compress with zstd.

Uses only Python's standard library and the zstd CLI. Integer/float distinctions and every numeric value
are preserved; f32 is used only when exactly representable; map keys are sorted for deterministic
encoding. Decoding is performed by the Rust tests with ciborium/ruzstd.
"""
import argparse
import hashlib
import json
import struct
import subprocess
from pathlib import Path

def head(major, n):
    for limit, extra, code in [(24, None, None), (256, 24, '>B'), (65536, 25, '>H'), (2**32, 26, '>I'), (2**64, 27, '>Q')]:
        if n < limit:
            return bytes([(major << 5) | (n if extra is None else extra)]) + (b'' if code is None else struct.pack(code, n))
    raise ValueError('integer exceeds CBOR u64')

def cbor(v):
    if v is None:return b'\xf6'
    if v is False:return b'\xf4'
    if v is True:return b'\xf5'
    if isinstance(v,int):return head(0,v) if v>=0 else head(1,-1-v)
    if isinstance(v,float):
        try:
            single=struct.pack('>f',v)
            if struct.unpack('>f',single)[0]==v:return b'\xfa'+single
        except OverflowError:
            pass
        return b'\xfb'+struct.pack('>d',v)
    if isinstance(v,str):
        b=v.encode();return head(3,len(b))+b
    if isinstance(v,list):return head(4,len(v))+b''.join(map(cbor,v))
    if isinstance(v,dict):return head(5,len(v))+b''.join(cbor(k)+cbor(v[k]) for k in sorted(v))
    raise TypeError(type(v))

def pack(source, output):
    raw=source.read_bytes();value=json.loads(raw)
    encoded=cbor(value)
    compressed=subprocess.run(['zstd','-q','-19','-T1','--stdout'],input=encoded,check=True,stdout=subprocess.PIPE).stdout
    output.write_bytes(compressed)
    return {'json_sha256':hashlib.sha256(raw).hexdigest(),'cbor_sha256':hashlib.sha256(encoded).hexdigest(),
            'zstd_sha256':hashlib.sha256(compressed).hexdigest(),'json_bytes':len(raw),
            'cbor_bytes':len(encoded),'packed_bytes':len(compressed),
            'records':len(value) if isinstance(value,(list,dict)) else None}

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('source',type=Path);p.add_argument('output',type=Path)
    a=p.parse_args();print(json.dumps(pack(a.source,a.output),indent=2))
