"""Real wtype/GTK/foot clipboard acceptance in private headless Sway; no user clipboard touched."""
import json,os,pathlib,subprocess,tempfile,time,struct,zlib,shutil
repo=pathlib.Path(__file__).resolve().parents[1]
binary=pathlib.Path(os.environ.get('CLIPRUST_TEST_BINARY',str(repo/'target/debug/cliprust')))
def wait(f,timeout=12):
 end=time.monotonic()+timeout
 while time.monotonic()<end:
  if f():return
  time.sleep(.05)
 raise AssertionError('Timed out waiting for acceptance condition')
with tempfile.TemporaryDirectory(prefix='cliprust-sway-paste-') as directory:
 root=pathlib.Path(directory);runtime=root/'run';runtime.mkdir(mode=0o700)
 config=root/'sway.conf';config.write_text('output HEADLESS-1 resolution 1280x800 position 0 0\noutput HEADLESS-2 resolution 1280x800 position 1280 0\nseat seat0 fallback true\n')
 env=dict(os.environ,XDG_RUNTIME_DIR=str(runtime),WLR_BACKENDS='headless',WLR_RENDERER='pixman',WLR_HEADLESS_OUTPUTS='2',WLR_LIBINPUT_NO_DEVICES='1',XDG_CURRENT_DESKTOP='sway',XDG_SESSION_TYPE='wayland',GDK_BACKEND='wayland')
 for k in ['DISPLAY','WAYLAND_DISPLAY','SWAYSOCK','XDG_ACTIVATION_TOKEN']:env.pop(k,None)
 log=open(root/'log','w');processes=[]
 compositor=subprocess.Popen([os.environ.get('CLIPRUST_TEST_SWAY','sway'),'--config',str(config)],env=env,stdout=log,stderr=log)
 try:
  wait(lambda:(runtime/'wayland-1').exists());env.update(WAYLAND_DISPLAY='wayland-1',SWAYSOCK=str(next(runtime.glob('sway-ipc.*.sock'))))
  def sway(command):
   reply=json.loads(subprocess.check_output(['swaymsg','-s',env['SWAYSOCK'],command],env=env))
   assert all(item.get('success') for item in reply), (command,reply)
   return reply
  def tree():return json.loads(subprocess.check_output(['swaymsg','-s',env['SWAYSOCK'],'-t','get_tree']))
  def nodes(n):return [n]+sum([nodes(c) for k in ['nodes','floating_nodes'] for c in n.get(k,[])],[])
  def find(appid):return next((n for n in nodes(tree()) if (n.get('pid')==app.pid if appid=='cliprust' else n.get('app_id')==appid)),None)
  def focused(appid):return bool(find(appid) and find(appid).get('focused'))
  def key(*args):
   # Let clients process modifier transitions before each simulated physical key.
   paced=[];i=0
   while i<len(args):
    if args[i] in ['-M','-m','-k']:
     option,value=args[i:i+2]
     if option=='-M':paced.extend(['-P',{'ctrl':'Control_L','logo':'Super_L'}[value],'-s','100'])
     paced.extend([option,value])
     if option=='-m':paced.extend(['-p',{'ctrl':'Control_L','logo':'Super_L'}[value]])
     i+=2
    else:paced.append(args[i]);i+=1
    paced.extend(['-s','100'])
   subprocess.run(['wtype','-s','100',*paced],env=env,check=True,timeout=10)
  db=str(root/'history.db')
  def cli(*args):return subprocess.check_output([str(binary),'--database',db,*args],env=env,text=True,timeout=10)
  cli('config','paused','false');cli('add','ArabicAcceptance مرحبا بالعالم — Unicode ∇')
  # Valid 1×1 PNG; decoded and rendered by the receiver.
  def chunk(kind,data):return struct.pack('!I',len(data))+kind+data+struct.pack('!I',zlib.crc32(kind+data))
  png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('!2I5B',1,1,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(b'\x00\xff\x00\x00\xff'))+chunk(b'IEND',b'')
  bundle=root/'bundle.json';bundle.write_text(json.dumps({'formats':{'image/png':list(png)}}));cli('load-bundle',str(bundle))
  bundle.write_text(json.dumps({'formats':{'text/uri-list':list(b'file:///tmp/cliprust-acceptance.txt\r\n')}}));cli('load-bundle',str(bundle))
  (root/'mode').write_text('text')
  receiver=subprocess.Popen(['python3',str(repo/'tests/sway_paste_receiver.py'),str(root)],env=env,stdout=log,stderr=log);processes.append(receiver)
  wait(lambda:find('cliprust-paste-receiver'))
  sway('focus output HEADLESS-1');sway('workspace "acceptance-target"');sway('[app_id="cliprust-paste-receiver"] move container to workspace "acceptance-target"');sway('[app_id="cliprust-paste-receiver"] focus')
  bin_dir=root/'bin';bin_dir.mkdir();wtype=bin_dir/'wtype';wtype.symlink_to(shutil.which('wtype'))
  app_env=dict(env,PATH=str(bin_dir))
  receiver_geometry=find('cliprust-paste-receiver')['rect'].copy()
  app=subprocess.Popen([str(binary),'--database',db],env=app_env,stdout=log,stderr=log);processes.append(app)
  wait(lambda:focused('cliprust'));time.sleep(.6)
  assert find('cliprust')['rect']['x']<1280
  assert find('cliprust-paste-receiver')['rect']==receiver_geometry, 'Popup retiled the destination application'
  initial_geometry=find('cliprust')['rect'].copy()
  assert (initial_geometry['width'],initial_geometry['height'])==(780,540)
  from native_pointer import Pointer
  pointer=Pointer(root,env,log);processes.append(pointer.process);time.sleep(.2)
  # Drag through the application's own header using real pointer events.
  pointer.command("absolute",initial_geometry["x"]+130,initial_geometry["y"]+24)
  pointer.command('button',1);time.sleep(.15)
  pointer.command('relative',40,0);time.sleep(.15)
  pointer.command('relative',70,45);time.sleep(.15)
  pointer.command('button',0);time.sleep(.2)
  dragged=find('cliprust')['rect']
  assert dragged['x']>initial_geometry['x'] and dragged['y']>initial_geometry['y'], ('Header drag did not move the floating window',initial_geometry,dragged)
  pointer.command("absolute",dragged["x"]+dragged["width"]-6,dragged["y"]+dragged["height"]-6)
  pointer.command('button',1);time.sleep(.15)
  pointer.command('relative',-80,-80);time.sleep(.15)
  pointer.command('button',0);time.sleep(.3)
  custom_geometry=find('cliprust')['rect'].copy()
  assert (custom_geometry['width'],custom_geometry['height'])==(700,460), custom_geometry
  # A resize must release Slint's grab, so the next header drag stays a move.
  pointer.command('absolute',custom_geometry['x']+130,custom_geometry['y']+24)
  pointer.command('button',1);time.sleep(.15)
  pointer.command('relative',40,0);time.sleep(.15)
  pointer.command('relative',-40,20);time.sleep(.15)
  pointer.command('button',0);time.sleep(.2)
  moved_again=find('cliprust')['rect'].copy()
  assert (moved_again['width'],moved_again['height'])==(700,460)
  assert moved_again['x']!=custom_geometry['x'] or moved_again['y']!=custom_geometry['y'], 'Resize left the pointer grab stuck'
  custom_geometry=moved_again


  # Install the exact shortcut into the private compositor, then invoke through real key events.
  sway(f'bindsym Mod4+v exec "{binary}" --database "{db}" --toggle')
  def summon():
   key('-M','logo','v','-m','logo');wait(lambda:focused('cliprust'))
   assert len([n for n in nodes(tree()) if n.get('pid')==app.pid])==1
  def search(query):key('-M','ctrl','f','-m','ctrl');key('-M','ctrl','a','-m','ctrl',query);time.sleep(.2)
  search('ArabicAcceptance');key('-k','Return')
  wait(lambda:(root/'text.txt').exists() and 'مرحبا بالعالم' in (root/'text.txt').read_text())
  wait(lambda:find('cliprust') is None)
  first=(root/'text.txt').read_text();assert first=='ArabicAcceptance مرحبا بالعالم — Unicode ∇'
  summon();assert find('cliprust')['rect']==custom_geometry, 'Reopen reset moved/resized geometry'
  search('ArabicAcceptance');key('-M','ctrl','-k','Return','-m','ctrl')
  wait(lambda:(root/'text.txt').read_text()==first*2);wait(lambda:focused('cliprust'))
  key('-M','ctrl','-k','Return','-m','ctrl');wait(lambda:(root/'text.txt').read_text()==first*3);wait(lambda:focused('cliprust'))
  assert find('cliprust')['rect']==custom_geometry, 'Keep Open reset moved/resized geometry'
  # Toggle hides only when focused. The same process returns on another output/workspace.
  key('-M','logo','v','-m','logo');wait(lambda:find('cliprust') is None)
  sway('focus output HEADLESS-2');sway('workspace "acceptance-second"')
  sway('[app_id="cliprust-paste-receiver"] move container to workspace "acceptance-second"');sway('[app_id="cliprust-paste-receiver"] focus')
  summon();assert find('cliprust')['rect']['x']>=1280
  assert [find('cliprust')['rect'][k] for k in ['width','height']]==[700,460]
  # Image and URI selection use the original supported clipboard MIME data.
  (root/'mode').write_text('image');search('image');key('-k','Return')
  wait(lambda:(root/'pixels.json').exists())
  assert json.loads((root/'pixels.json').read_text())==[1,1,[255,0,0,255]]
  (root/'mode').write_text('files');summon();search('cliprust-acceptance');key('-k','Return')
  wait(lambda:(root/'files.txt').exists());assert b'file:///tmp/cliprust-acceptance.txt' in (root/'files.txt').read_bytes()
  print('Backend:',os.environ.get('CLIPRUST_SWAY_INPUT','native'),'(wtype uses an explicit GTK action mapping)')
  print('PASS: draggable header, flexible size, geometry preservation, GTK text/Arabic/Unicode, repeated keep-open, query preservation, Meta+V toggle, two workspaces/outputs, PNG and URI paste')
  # Real terminal receiver configured with Ctrl+V as required by this integration.
  (root/'foot.ini').write_text('[key-bindings]\nclipboard-paste=Control+v\n')
  (root/'capture.py').write_text('import os,tty,sys\ntty.setraw(0)\nf=open(sys.argv[1],"wb",buffering=0)\nwhile True:f.write(os.read(0,4096))\n')
  terminal=subprocess.Popen(['foot','--app-id=cliprust-test-terminal','--config='+str(root/'foot.ini'),'python3',str(root/'capture.py'),str(root/'terminal.txt')],env=env,stdout=log,stderr=log);processes.append(terminal)
  wait(lambda:focused('cliprust-test-terminal'));summon();search('ArabicAcceptance');key('-k','Return')
  wait(lambda:(root/'terminal.txt').exists() and first.encode() in (root/'terminal.txt').read_bytes())
  print('PASS: actual foot terminal paste with Ctrl+V configured')
  # Stock Mousepad editor: changed-document marker plus a screenshot for visual text verification.
  if shutil.which('mousepad') and os.environ.get('CLIPRUST_SWAY_INPUT')!='wtype':
   (root/'editor.txt').write_text('')
   editor_env=dict(env,GSETTINGS_BACKEND='memory',XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_STATE_HOME=str(root/'state'))
   editor=subprocess.Popen(['mousepad','--disable-server',str(root/'editor.txt')],env=editor_env,stdout=log,stderr=log);processes.append(editor)
   wait(lambda:find('mousepad') or find('org.xfce.mousepad'))
   summon();search('ArabicAcceptance');key('-k','Return');time.sleep(.5)
   assert find('org.xfce.mousepad')['name'].startswith('*')
   subprocess.run(['grim','/tmp/cliprust-sway-mousepad.png'],env=env,check=True)
   print('PASS: stock Mousepad document changed; screenshot saved for visual content verification',flush=True)
   editor.terminate();editor.wait(timeout=5)
   sway('[app_id="cliprust-test-terminal"] focus')
  # A closed target must not cause a paste into a different application or hide the launcher.
  summon();search('ArabicAcceptance');terminal.terminate();terminal.wait(timeout=5);key('-k','Return');time.sleep(.3)
  assert find('cliprust') is not None and app.poll() is None
  assert 'closed' in (root/'log').read_text().lower()
  print('PASS: closed target rejected with window still visible')
  sway('[app_id="cliprust-paste-receiver"] focus');summon();search('ArabicAcceptance')
  if os.environ.get('CLIPRUST_SWAY_INPUT')!='wtype':
   (root/'mode').write_text('text');wtype.unlink();key('-k','Return')
   wait(lambda:(root/'text.txt').read_text()==first*4)
   import sqlite3
   with sqlite3.connect(db) as connection:
    assert connection.execute('select count(*) from entries').fetchone()[0]==3
    assert connection.execute('select max(copy_count) from entries').fetchone()[0]==1
   print('PASS: native Wayland virtual keyboard pastes without wtype; active monitor did not recapture text, PNG, or file paste writes');raise SystemExit(0)
  wtype.unlink();key('-k','Return');time.sleep(.3)
  assert find('cliprust') is not None and 'wtype is missing' in (root/'log').read_text()
  wtype.write_text('#!/bin/sh\nexit 42\n');wtype.chmod(0o700)
  key('-k','Return');time.sleep(.3)
  assert find('cliprust') is not None and 'wtype failed' in (root/'log').read_text()
  print('PASS: missing and failing wtype produce diagnostics without hiding Cliprust')
 except Exception:
  subprocess.run(['grim','/tmp/cliprust-sway-failure.png'],env=env)
  print('TEXT:',(root/'text.txt').read_text() if (root/'text.txt').exists() else 'missing')
  print([(n.get('app_id'),n.get('focused'),n.get('rect')) for n in nodes(tree()) if n.get('app_id')])
  print((root/'log').read_text()[-6000:]);raise
 finally:
  for p in processes:
   if p.poll() is None:p.terminate();p.wait(timeout=5)
  compositor.terminate();compositor.wait(timeout=5);log.close()
