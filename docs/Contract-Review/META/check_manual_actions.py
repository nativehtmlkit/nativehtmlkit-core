#!/usr/bin/env python3
"""NativeHTMLKit policy: ALL GitHub Actions triggers must be workflow_dispatch ONLY.
Requires PyYAML (python -m pip install pyyaml), no GitHub credentials or network access.
Usage: python check_manual_actions.py /path/to/repo [other-repo ...]
"""
import sys
from pathlib import Path
try:
    import yaml
except ImportError:
    raise SystemExit('Install PyYAML to check workflow trigger policy')

def scan(repo: Path):
    workflows = repo/'.github'/'workflows'
    failures=[]; inspected=0
    for p in sorted(list(workflows.glob('*.yml'))+list(workflows.glob('*.yaml'))):
        inspected+=1
        try:
            x=yaml.load(p.read_text(encoding='utf-8'), Loader=yaml.BaseLoader)
            on=x.get('on') if isinstance(x,dict) else None
            events=[on] if isinstance(on,str) else list(on) if isinstance(on,(dict,list)) else []
            if events!=['workflow_dispatch']:
                failures.append(f'{p}: expected only workflow_dispatch, found {events!r}')
        except Exception as e:
            failures.append(f'{p}: cannot parse YAML: {e}')
    return inspected,failures

def main():
    args=[Path(p) for p in sys.argv[1:]]
    if not args:raise SystemExit('Usage: python check_manual_actions.py REPO [REPO ...]')
    allfail=[];count=0
    for p in args:
        n,fail=scan(p);count+=n;allfail+=fail
    for x in allfail:print('FAIL',x)
    print(f'checked={count} workflows, violations={len(allfail)}')
    return 1 if allfail else 0
if __name__=='__main__':raise SystemExit(main())
