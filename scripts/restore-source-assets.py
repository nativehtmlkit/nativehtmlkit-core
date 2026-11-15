#!/usr/bin/env python3
"""Restore original test and fallback fonts from a locally owned UI4ME source ZIP.

No fonts are distributed with this handoff. Restore these licensed assets from
an original source checkout/archive before compiling; verify SHA256.
"""
from __future__ import annotations
import argparse, hashlib, json, pathlib, sys, zipfile
ROOT=pathlib.Path(__file__).resolve().parents[1]
MANIFEST=json.loads((ROOT/'assets-manifest.json').read_text())['assets']
p=argparse.ArgumentParser()
p.add_argument('archive',type=pathlib.Path,nargs='?',help='Original ui4me-core-main.zip owned by the developer')
p.add_argument('--check',action='store_true',help='Only verify current assets')
a=p.parse_args()
# Keep check mode fully independent of source archive bytes.
archive = None if a.check else zipfile.ZipFile(a.archive or sys.exit('Missing original archive path'))
try:
    for item in MANIFEST:
        dest=ROOT/item['path']
        if dest.is_file():
            data=dest.read_bytes()
        elif a.check:
            raise FileNotFoundError(dest)
        else:
            old=item['path'].replace('nativehtmlkit-', 'ui4me-')
            path='ui4me-core-main/'+old
            data=archive.read(path)
        if len(data)!=item['size'] or hashlib.sha256(data).hexdigest()!=item['sha256']:
            raise ValueError('Source asset hash mismatch: '+item['path'])
        if not dest.is_file():
            dest.parent.mkdir(parents=True,exist_ok=True)
            dest.write_bytes(data)
        print('verified',item['path'])
finally:
    if archive:archive.close()
