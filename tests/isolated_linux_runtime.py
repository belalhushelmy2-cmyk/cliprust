"""Verify bundled GUI/fonts without host app libraries, fonts, portals or FUSE.
Usage: python3 tests/isolated_linux_runtime.py MINIMAL_ROOT RUNTIME_DIRECTORY
MINIMAL_ROOT supplies only glibc and standard shell tools (not X11/fonts/Wayland).
"""
import os,pathlib,subprocess,sys,tempfile,time
minimal=pathlib.Path(sys.argv[1]).resolve();bundle=pathlib.Path(sys.argv[2]).resolve()
with tempfile.TemporaryDirectory(prefix='cliprust-runtime-test-') as d:
 root=pathlib.Path(d);run=root/'run';run.mkdir(mode=0o700)
 config=root/'sway.conf';config.write_text('output HEADLESS-1 resolution 1280x800\nseat seat0 fallback true\n')
 env=dict(os.environ,XDG_RUNTIME_DIR=str(run),WLR_BACKENDS='headless',WLR_RENDERER='pixman',WLR_LIBINPUT_NO_DEVICES='1')
 for k in ['SWAYSOCK','WAYLAND_DISPLAY','DISPLAY']:env.pop(k,None)
 with (root/'log').open('w') as log:
  sway=subprocess.Popen(['sway','-c',str(config)],env=env,stdout=log,stderr=log)
  try:
   end=time.monotonic()+15
   while not (run/'wayland-1').exists():
    assert time.monotonic()<end,'No compositor';time.sleep(.05)
   command=['bwrap','--ro-bind',str(minimal),'/','--dev','/dev','--proc','/proc','--bind',str(root),'/tmp','--ro-bind',str(bundle),'/app','--clearenv','--setenv','PATH','/bin','--setenv','HOME','/tmp/home','--setenv','XDG_RUNTIME_DIR','/tmp/run','--setenv','WAYLAND_DISPLAY','wayland-1','--setenv','XDG_SESSION_TYPE','wayland','--setenv','LANG','C.UTF-8']
   added=subprocess.run(command+['/app/AppRun','--database','/tmp/history.db','add','Bundled Unicode مرحبا'],capture_output=True,text=True);assert added.returncode==0,added.stderr
   result=subprocess.run(command+['/app/AppRun','--database','/tmp/history.db','--snapshot','/tmp/window.png'],capture_output=True,text=True,timeout=30)
   assert result.returncode==0,result.stderr
   assert (root/'window.png').stat().st_size>10000
   assert 'Fontconfig error' not in result.stderr and 'xkbcommon: ERROR' not in result.stderr,result.stderr
   cached=next((root/'home/.cache/cliprust/fontconfig').glob('*.conf'))
   font=subprocess.check_output(command+['/bin/env','LD_LIBRARY_PATH=/app/usr/lib','FONTCONFIG_FILE=/tmp/'+str(cached.relative_to(root)),'FONTCONFIG_PATH=/app/etc/fonts','/bin/fc-match','-f','%{file}\n'],text=True)
   assert '/app/usr/share/fonts/' in font,font
   print('PASS: isolated GUI snapshot, Unicode SQLite, bundled keyboard data and fallback font; no system application libraries/fonts/FUSE')
   print('Font:',font.strip())
  finally:sway.terminate();sway.wait(timeout=10)
