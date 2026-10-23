"""Frozen history and initial keyboard selection in private headless Sway."""
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
 config=root/'sway.conf';config.write_text('output HEADLESS-1 resolution 1280x800 position 0 0\noutput HEADLESS-2 resolution 1280x800 position 1280 0\nseat seat0 fallback true\ndefault_border none\ndefault_floating_border none\nfor_window [title="^Cliprust · Clipboard History$"] floating enable\nfor_window [app_id="^(cliprust|org\\.cliprust\\.Cliprust)$"] floating enable\n')
 env=dict(os.environ,XDG_RUNTIME_DIR=str(runtime),WLR_BACKENDS='headless',WLR_RENDERER='pixman',WLR_HEADLESS_OUTPUTS='2',WLR_LIBINPUT_NO_DEVICES='1',XDG_CURRENT_DESKTOP='sway',XDG_SESSION_TYPE='wayland',GDK_BACKEND='wayland')
 for k in ['DISPLAY','WAYLAND_DISPLAY','SWAYSOCK','XDG_ACTIVATION_TOKEN']:env.pop(k,None)
 log=open(root/'log','w');processes=[]
 compositor=subprocess.Popen(['sway','--config',str(config)],env=env,stdout=log,stderr=log)
 try:
  wait(lambda:(runtime/'wayland-1').exists());env.update(WAYLAND_DISPLAY='wayland-1',SWAYSOCK=str(next(runtime.glob('sway-ipc.*.sock'))))
  def sway(command):return json.loads(subprocess.check_output(['swaymsg','-s',env['SWAYSOCK'],command],env=env))
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
  import sqlite3
  from PIL import Image
  def rows():
   with sqlite3.connect(db) as connection:
    return connection.execute('select id,preview,copy_count,last_used from entries').fetchall()
  def copies(text):return next(row[2] for row in rows() if row[1]==text)
  # Enough pins to force the latest actual clipboard entry below the first viewport.
  for index in range(12):
   cli('add',f'Pinned {index:02}')
  with sqlite3.connect(db) as connection:connection.execute('update entries set pinned=1,pin_order=id')
  cli('snippet','save','SnippetAcceptance','expanded snippet مرحبا')
  (root/'mode').write_text('text')
  receiver=subprocess.Popen(['python3',str(repo/'tests/sway_paste_receiver.py'),str(root)],env=env,stdout=log,stderr=log);processes.append(receiver)
  wait(lambda:find('cliprust-paste-receiver'))
  sway('focus output HEADLESS-1');sway('workspace "history-target"')
  sway('[app_id="cliprust-paste-receiver"] move container to workspace "history-target"')
  sway('[app_id="cliprust-paste-receiver"] focus')
  app=subprocess.Popen([str(binary),'--database',db],env=env,stdout=log,stderr=log);processes.append(app)
  wait(lambda:focused('cliprust'));time.sleep(.5)
  sway(f'bindsym Mod4+v exec "{binary}" --database "{db}" --toggle')
  def summon():
   key('-M','logo','v','-m','logo');wait(lambda:focused('cliprust'));wait(lambda:find('cliprust')['rect']['width']==860);time.sleep(.3)
  def hide():
   key('-M','logo','v','-m','logo');wait(lambda:find('cliprust') is None)
  def publish(value):
   subprocess.run(['wl-copy','--type','text/plain'],input=value.encode(),env=env,check=True)
   wait(lambda:any(row[1]==value for row in rows()))
  def text():return (root/'text.txt').read_text() if (root/'text.txt').exists() else ''
  def list_pixels():
   time.sleep(.6)
   rectangle=find('cliprust')['rect']
   path=root/'screen.png';subprocess.run(['grim','-o','HEADLESS-1',str(path)],env=env,check=True)
   x,y,w,h=[rectangle[k] for k in ['x','y','width','height']]
   # The list viewport only, excluding the status line, search caret, and target editor.
   cropped=Image.open(path).convert('RGB').crop((x+25,y+245,x+w//2,y+h-65))
   return cropped.tobytes()
  def assert_selection_visible():
   list_pixels() # Refresh the screenshot after layout/rendering.
   rectangle=find('cliprust')['rect']
   image=Image.open(root/'screen.png').convert('RGB')
   x,y=rectangle['x'],rectangle['y']
   # The selected row's vertical border must be fully inside the viewport.
   border=(112,97,143)
   ys=[dy for dy in range(245,rectangle['height']-65)
       if image.getpixel((x+24,y+dy))==border]
   assert ys and max(ys)-min(ys)>=48, 'Keyboard selection is clipped or outside the viewport'
  hide()
  for value in ['First copy','Second copy','Newest مرحبا']:
   publish(value);time.sleep(1.05)
  sway('[app_id="cliprust-paste-receiver"] focus');summon()
  assert_selection_visible()
  for direction in ['Up']*12+['Down']*14+['Up']*2:
   key('-k',direction)
   assert_selection_visible()
  key('-k','Return');wait(lambda:text()=='Newest مرحبا');wait(lambda:find('cliprust') is None)
  assert copies('Newest مرحبا')==1 # Only the genuine copy, no self capture.
  summon();key('-k','Down');key('-k','Up');key('-k','Down');before=list_pixels();key('-M','ctrl','-k','Return','-m','ctrl')
  wait(lambda:text()=='Newest مرحباSecond copy');wait(lambda:focused('cliprust'))
  assert before==list_pixels(),'Paste changed row order, selection, or scrolled viewport'
  key('-M','ctrl','-k','Return','-m','ctrl');wait(lambda:text()=='Newest مرحباSecond copySecond copy');wait(lambda:focused('cliprust'))
  assert before==list_pixels(),'Repeated keep-open changed list viewport'
  assert copies('Second copy')==1
  # Genuine external offers, even with the same payload, must still be recorded.
  old_count=copies('Second copy');publish('Second copy');wait(lambda:copies('Second copy')==old_count+1)
  assert before==list_pixels(),'Identical external copy changed visible list'
  publish('External newest');assert before==list_pixels(),'New external entry changed visible list'
  assert len(rows())==16
  # Printable input starts search without Ctrl+F, selecting the first match.
  key('First');time.sleep(.3);search_view=list_pixels()
  key('-M','ctrl','-k','Return','-m','ctrl');wait(lambda:text().endswith('First copy'));wait(lambda:focused('cliprust'))
  assert search_view==list_pixels(),'Search selection/viewport changed after paste'
  key('-M','ctrl','-k','Return','-m','ctrl');wait(lambda:text().endswith('First copyFirst copy'));wait(lambda:focused('cliprust'))
  assert search_view==list_pixels()
  key('-k','Escape');time.sleep(.2);assert focused('cliprust') # Clears query first.
  key('-k','Escape');wait(lambda:find('cliprust') is None)
  summon();key('-k','Return');wait(lambda:text().endswith('External newest'));wait(lambda:find('cliprust') is None)
  summon();key('SnippetAcceptance');time.sleep(.3);snippet_view=list_pixels()
  key('-M','ctrl','-k','Return','-m','ctrl');wait(lambda:text().endswith('expanded snippet مرحبا'));wait(lambda:focused('cliprust'))
  assert snippet_view==list_pixels()
  key('-M','ctrl','-k','Return','-m','ctrl');wait(lambda:text().endswith('expanded snippet مرحباexpanded snippet مرحبا'));wait(lambda:focused('cliprust'))
  assert len(rows())==16 and snippet_view==list_pixels()
  if binary.suffix=='.AppImage':
   assert 'Fontconfig error:' not in (root/'log').read_text()
   assert 'xkbcommon: ERROR:' not in (root/'log').read_text()
  print('PASS: newest external copy selected beneath pins; Enter without click; full Up/Down traversal keeps selection visible; typing-to-search; Escape clear/close; repeated Ctrl+Enter preserves exact list pixels/selection/scroll; self writes suppressed; identical/new external copies recorded without visible changes; reopen refreshes and selects latest')
 except Exception:
  subprocess.run(['grim','/tmp/cliprust-sway-failure.png'],env=env)
  print('CLIPRUST:',find('cliprust'))
  print('TEXT:',(root/'text.txt').read_text() if (root/'text.txt').exists() else 'missing')
  print([(n.get('app_id'),n.get('focused'),n.get('rect')) for n in nodes(tree()) if n.get('app_id')])
  print((root/'log').read_text()[-6000:]);raise
 finally:
  for p in processes:
   if p.poll() is None:p.terminate();p.wait(timeout=5)
  compositor.terminate();compositor.wait(timeout=5);log.close()
