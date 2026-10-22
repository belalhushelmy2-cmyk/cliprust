"""Isolated Sway shortcut edits: live rebinding, persistence, conflicts and rollback."""
import json,os,pathlib,subprocess,tempfile,time
repo=pathlib.Path(__file__).resolve().parents[1]
binary=pathlib.Path(os.environ.get('CLIPRUST_TEST_BINARY',str(repo/'target/debug/cliprust'))).resolve()
def wait(test):
 end=time.monotonic()+15
 while time.monotonic()<end:
  if test():return
  time.sleep(.05)
 raise AssertionError('Timed out')
with tempfile.TemporaryDirectory(prefix='cliprust-shortcut-test-') as d:
 root=pathlib.Path(d);run=root/'run';run.mkdir(mode=0o700);config=root/'sway.conf'
 config.write_text('output HEADLESS-1 resolution 1280x800\nseat seat0 fallback true\nbindsym Mod4+x exec true\n')
 env=dict(os.environ,XDG_RUNTIME_DIR=str(run),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),WLR_BACKENDS='headless',WLR_RENDERER='pixman',WLR_LIBINPUT_NO_DEVICES='1',XDG_CURRENT_DESKTOP='sway',XDG_SESSION_TYPE='wayland')
 for k in ['SWAYSOCK','WAYLAND_DISPLAY','DISPLAY','APPIMAGE','CLIPRUST_LAUNCHER']:env.pop(k,None)
 log=open(root/'log','w');sway=subprocess.Popen(['sway','-c',str(config)],env=env,stdout=log,stderr=log);app=None
 try:
  wait(lambda:(run/'wayland-1').exists());env.update(WAYLAND_DISPLAY='wayland-1',SWAYSOCK=str(next(run.glob('sway-ipc.*.sock'))))
  def cli(*args):return subprocess.run([str(binary),*args],env=env,text=True,capture_output=True,timeout=20)
  def ipc(command):return json.loads(subprocess.check_output(['swaymsg','-s',env['SWAYSOCK'],command],env=env))
  def nodes(n):return [n]+sum([nodes(c) for k in ['nodes','floating_nodes'] for c in n.get(k,[])],[])
  def windows():return [n for n in nodes(json.loads(subprocess.check_output(['swaymsg','-s',env['SWAYSOCK'],'-t','get_tree']))) if n.get('pid')==(app.pid if app else -1)]
  def press(shift=False):subprocess.run(['wtype','-s','200','-P','Super_L','-M','logo','-s','100',*(['-P','Shift_L','-M','shift','-s','100'] if shift else []),'-k','v','-s','100',*(['-m','shift','-p','Shift_L'] if shift else []),'-m','logo','-p','Super_L'],env=env,check=True)
  r=cli('setup-shortcut','Mod+V');assert r.returncode==0,r.stderr
  assert 'bindsym --no-repeat Mod4+v' in config.read_text()
  app=subprocess.Popen([str(binary)],env=env,stdout=log,stderr=log);wait(lambda:any(n.get("focused") for n in windows()));time.sleep(.5)
  press();wait(lambda:not windows())
  r=cli('setup-shortcut','Super+Shift+V');assert r.returncode==0,r.stderr
  assert 'Mod4+Shift+v' in config.read_text() and 'Mod4+v ' not in config.read_text()
  press();time.sleep(.3);assert not windows(),'Old shortcut still active'
  press(True);wait(windows)
  original=config.read_text();r=cli('setup-shortcut','Super+X');assert r.returncode!=0 and 'another binding' in r.stderr
  assert config.read_text()==original
  # A syntax error must roll back the attempted binding change and not reload it.
  include=root/'included';include.write_text('invalid_cliprust_test_command yes\n')
  config.write_text(original+'include '+str(include)+'\n');ipc('reload')
  before=config.read_text();r=cli('setup-shortcut','Super+Alt+V');assert r.returncode!=0,r.stdout
  assert config.read_text()==before
  assert len(list((root/'config/cliprust/sway-backups').glob('*')))>=3
  print('PASS: Sway shortcut apply, old-key removal, new-key activation, conflict protection, backups and validation rollback')
 except Exception:
  print(config.read_text());print((root/'log').read_text());raise
 finally:
  if app:
   app.terminate();app.wait(timeout=10)
  sway.terminate();sway.wait(timeout=10)
