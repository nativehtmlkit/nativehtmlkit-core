from pathlib import Path
import re,json,hashlib,sys
ROOT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else Path.cwd();OUT=Path(sys.argv[2]).resolve() if len(sys.argv)>2 else Path.cwd()
OUT.mkdir(parents=True,exist_ok=True)
def camel_snake(s): return re.sub('([a-z0-9])([A-Z])',r'\1_\2',re.sub('([A-Z]+)([A-Z][a-z])',r'\1_\2',s)).lower()
def rel(path): return str(path.relative_to(ROOT))
def extract_enum(path, name):
 text=path.read_text();line_list=text.splitlines(); start=next(i for i,v in enumerate(line_list) if re.match(r'pub\(super\) enum '+name+r'\s*\{',v));end=next(i for i in range(start+1,len(line_list)) if line_list[i]=='}'); variants=[]; current=None; field_decorators=[]
 for idx in range(start+1,end):
  line=line_list[idx]
  m=re.match(r'^    ([A-Z]\w+)(?: \{|,)$',line)
  if m:
   if current: variants.append(current)
   current={'op':camel_snake(m.group(1)),'rust_variant':m.group(1),'source':f'{rel(path)}:{idx+1}','fields':[]}
   field_decorators=[]
   continue
  if current is None:continue
  if line.strip().startswith('#[serde('):
   field_decorators.append(line.strip());continue
  f=re.match(r'^        (\w+): ([^,]+),\s*$',line)
  if f:
   current['fields'].append({'name':f.group(1),'rust_type':f.group(2).strip(),'optional': f.group(2).strip().startswith('Option<') or any('default' in dec for dec in field_decorators),'attributes':field_decorators, 'line':idx+1});field_decorators=[]
 if current:variants.append(current)
 return variants
cmd=extract_enum(ROOT/'crates/ui4me-host/src/host/document_commands.rs','DocumentCommand')
node=extract_enum(ROOT/'crates/ui4me-host/src/host/document_commands.rs','NodeBatchOp')
batch=extract_enum(ROOT/'crates/ui4me-host/src/host/document_commands.rs','BatchOp')
files=['crates/ui4me-host/src/host/requests.rs','crates/ui4me-host/src/host/documents.rs','crates/ui4me-host/src/host/bitmap.rs']
method_list={}
for name in files:
 p=ROOT/name
 s=p.read_text().splitlines();limit=425 if name.endswith('requests.rs') else (240 if name.endswith('documents.rs') else 314)
 for i,line in enumerate(s[:limit]):
  # dispatch arm begins with a quoted method ending =>, plus alternatives
  if '=>' in line and '"' in line and re.search(r'"(?:host|app|bitmap|document|window|desktop|probe|dialog|clipboard|request)\.',line):
   ms=re.findall(r'"((?:host|app|bitmap|document|window|desktop|probe|dialog|clipboard|request)\.[a-z_]+)"',line)
   if not ms:continue
   for m in ms:
    method_list[m]={'method':m,'source':f'{name}:{i+1}','status':'implemented in dispatch; live behavior not verified'}
for m,line in [('host.hello',723),('app.activate',732)]:method_list[m]={'method':m,'source':f'crates/ui4me-host/src/host/mod.rs:{line}','status':'implemented special-path; live behavior not verified'}
methods=sorted(method_list.values(),key=lambda x:x['method'])
notify=[]
for name,cut in [('crates/ui4me-host/src/host/mod.rs',1015),('crates/ui4me-host/src/host/document_native.rs',270),('crates/ui4me-host/src/host/native.rs',650)]:
 lines=(ROOT/name).read_text().splitlines()
 for i,line in enumerate(lines[:cut]):
  for x in re.findall(r'"((?:document|desktop|app|window|request)\.[a-z_]+)"',line):
   if x in ('document.event','document.error','desktop.preferences_changed','window.event','app.activated','request.cancelled','app.shutdown','window.before_close','app.before_quit'):
    notify.append({'method':x,'source':f'{name}:{i+1}','kind':'reverse_request' if x in ('window.before_close','app.before_quit') else 'notification'})
notify=list({x['method']:x for x in notify}.values());notify.sort(key=lambda x:x['method'])
result={'source':'Uploaded ui4me-core-main(6).zip — source-derived inventory, not independently verified by running host','rust_host_version':re.search(r'version\s*=\s*"([^\"]+)"',(ROOT/'Cargo.toml').read_text()).group(1),'protocol_version':1,'methods':methods,'document_commands':cmd,'batch_ops':batch,'node_batch_ops':node,'host_emitted':notify,'source_archive_sha256':(hashlib.sha256(Path(sys.argv[3]).read_bytes()).hexdigest() if len(sys.argv)>3 else None)}
(OUT/'legacy-source-inventory.json').write_text(json.dumps(result,indent=2)+'\n')
print('methods',len(methods),', doc cmds',len(cmd),', batch ops',len(batch),', node batch ops',len(node),', notifications/requests',len(notify))
print('Methods:',','.join(m['method'] for m in methods));print('Commands:',','.join(c['op'] for c in cmd));print('Host emitted:',','.join(n['method'] for n in notify));print('No field variant',len([c for c in cmd if not c['fields']]));print('sha',result['source_archive_sha256'])
