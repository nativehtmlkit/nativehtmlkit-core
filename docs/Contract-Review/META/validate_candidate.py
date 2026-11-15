#!/usr/bin/env python3
"""Local static validation of candidate contract files. Does NOT run Rust host or certify protocol."""
from pathlib import Path
import json,sys
try:
    from jsonschema import Draft202012Validator
    from referencing import Registry, Resource
except ImportError:
    raise SystemExit('Install jsonschema and referencing to validate the candidate')
ROOT=Path(__file__).resolve().parents[1]
S=ROOT/'02-proposed-contract'/'schemas'
INV=ROOT/'legacy-source-inventory.json'
def main():
    files=sorted(S.glob('*.schema.json'))
    schemas={p.name:json.loads(p.read_text()) for p in files}
    registry=Registry()
    for obj in schemas.values():
        Draft202012Validator.check_schema(obj)
        if obj.get('$id'):
            registry=registry.with_resource(obj['$id'],Resource.from_contents(obj))
    assert len(json.loads(INV.read_text())['methods'])==32
    assert len(json.loads(INV.read_text())['document_commands'])==81
    assert len(json.loads(INV.read_text())['host_emitted'])==9
    fixture=json.loads((ROOT/'02-proposed-contract'/'examples'/'schema-fixtures.json').read_text())
    choose={'request':schemas['rpc-request.schema.json'],'notification':schemas['rpc-notification.schema.json'],
            'response':schemas['rpc-response.schema.json'],'hello':schemas['hello-params.schema.json'],
            'command':schemas['document-command.schema.json']}
    for entry in fixture['fixtures']:
        obj=choose[entry['id'].split('_',1)[0]]
        result=list(Draft202012Validator(obj,registry=registry).iter_errors(entry['example']))
        assert bool(result)==(not entry['should_validate']), (entry['id'],[x.message for x in result])
    print(f'STATIC candidate validation PASS: {len(files)} schema files, {len(fixture["fixtures"])} fixtures; no live host executed')
    return 0
if __name__=='__main__':raise SystemExit(main())
