#!/usr/bin/env python3
"""Inspect Sway's loaded config/includes. --apply updates an existing Cliprust binding only.
Unknown syntax/keycode bindings fail closed; this is not a complete Sway parser.
"""
import argparse,datetime,glob,json,os,pathlib,re,shlex,shutil,subprocess,sys
p=argparse.ArgumentParser();p.add_argument('--apply',action='store_true');p.add_argument('--executable',default='cliprust');args=p.parse_args()
def ipc(kind):return json.loads(subprocess.check_output(['swaymsg','-r','-t',kind],text=True))
root=pathlib.Path(ipc('get_version')['loaded_config_file_name'])
loaded=ipc('get_config')['config']
if loaded.strip()!=root.read_text().strip():sys.exit('Root configuration differs from the loaded configuration; review pending changes before setup.')
lines=[];seen=set()
def read(path):
 path=path.resolve()
 if path in seen:return
 seen.add(path)
 pending=''
 for number,line in enumerate(path.read_text().splitlines(),1):
  line=pending+line
  if line.rstrip().endswith('\\'):
   pending=line.rstrip()[:-1]+' ';continue
  pending=''
  words=shlex.split(line,comments=True)
  if words and words[0]=='include':
   for pattern in words[1:]:
    if '$' in pattern or '`' in pattern:sys.exit('Dynamic include needs manual review: '+line)
    pattern=os.path.expanduser(pattern)
    if not os.path.isabs(pattern):pattern=str(path.parent/pattern)
    for item in sorted(glob.glob(pattern)):read(pathlib.Path(item))
  else:lines.append((path,number,words))
read(root)
variables={};block=''
for path,number,w in lines:
 if w==['set','{']:block='set';continue
 if w==['}']:block='';continue
 values=w[1:] if w and w[0]=='set' else w if block=='set' else []
 if len(values)>=2 and values[0].startswith('$'):variables[values[0]]=values[1]
def expand(value):
 for _ in range(10):
  new=re.sub(r'\$\w+',lambda m:variables.get(m[0],m[0]),value)
  if new==value:return value
  value=new
 return value
matches=[];block=''
for path,number,w in lines:
 if w and w[-1]=='{':block=w[0];continue
 if w==['}']:block='';continue
 if not w:continue
 kind=w[0] if w[0] in ['bindsym','bindcode'] else block
 if kind not in ['bindsym','bindcode']:continue
 values=w[1:] if w[0] in ['bindsym','bindcode'] else w
 values=[v for v in values if not v.startswith('--')]
 if not values:continue
 key=expand(values[0]).lower()
 if '$' in key:sys.exit('Unresolved binding variable needs manual review: '+str(path)+':'+str(number))
 if kind=='bindcode' and 'mod4' in key:sys.exit('Meta keycode binding needs manual review: '+str(path)+':'+str(number))
 if set(key.split('+'))=={'mod4','v'}:matches.append((path,number,' '.join(values[1:])))
print('Checked',len(seen),'config files from',root)
for path,line,command in matches:print(f'Meta+V: {path}:{line}: {command}')
binding='bindsym --no-repeat Mod4+v exec '+json.dumps(args.executable)+' --toggle'
if len(matches)!=1 or 'cliprust' not in matches[0][2].lower():
 print('No unique existing Cliprust binding; no files changed. Review conflicts, then add:')
 print(binding);sys.exit(1 if args.apply else 0)
if not args.apply:print('Proposed replacement:',binding);sys.exit(0)
path,line,_=matches[0]
original=path.read_text().splitlines()
if not original[line-1].lstrip().startswith('bindsym ') or (line>1 and original[line-2].rstrip().endswith('\\')):
 sys.exit('Binding uses block/continuation syntax; review manually. No files changed.')
backup_dir=pathlib.Path.home()/'.config/cliprust/sway-backups';backup_dir.mkdir(parents=True,exist_ok=True)
backup=backup_dir/(path.name+'.'+datetime.datetime.now().strftime('%Y%m%d-%H%M%S-%f'))
shutil.copy2(path,backup)
content=path.read_text().splitlines();content[line-1]=binding;path.write_text('\n'.join(content)+'\n')
check=subprocess.run(['sway','--validate','--config',str(root)],capture_output=True,text=True)
if check.returncode:
 shutil.copy2(backup,path);sys.exit('Validation failed; original restored, no reload:\n'+check.stderr)
reload=subprocess.run(['swaymsg','reload'],capture_output=True,text=True)
try:
 replies=json.loads(reload.stdout)
 reloaded=bool(replies) and all(reply.get('success',False) for reply in replies)
except (ValueError,TypeError,AttributeError):reloaded=False
if reload.returncode or not reloaded:
 shutil.copy2(backup,path);sys.exit('Reload failed; configuration file restored. Inspect Sway session before retrying:\n'+reload.stderr)
print('Validated and reloaded. Backup:',backup)
