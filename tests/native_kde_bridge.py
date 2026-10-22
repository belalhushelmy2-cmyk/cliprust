"""Run via dbus-run-session. Tests real D-Bus bridge transport with a mock KWin peer, not KDE behavior."""
import json, os, pathlib, subprocess, tempfile, time, shlex
binary = pathlib.Path(__file__).resolve().parents[1] / 'target/release/cliprust'
def wait_for(f):
    for _ in range(100):
        if f(): return
        time.sleep(.05)
    raise AssertionError('Timed out')
with tempfile.TemporaryDirectory(prefix='cliprust-activation-') as directory:
    root=pathlib.Path(directory); runtime=root/'runtime'; runtime.mkdir(mode=0o700)
    config=root/'sway.conf'; config.write_text('output HEADLESS-1 resolution 1280x800\nseat seat0 fallback true\nfor_window [app_id="cliprust"] floating enable\n')
    env=dict(os.environ,XDG_RUNTIME_DIR=str(runtime),WLR_BACKENDS='headless',WLR_RENDERER='pixman',WLR_HEADLESS_OUTPUTS='1',WLR_LIBINPUT_NO_DEVICES='1',XDG_CURRENT_DESKTOP='KDE')
    for key in ['DISPLAY','WAYLAND_DISPLAY','SWAYSOCK','XDG_ACTIVATION_TOKEN']:env.pop(key,None)
    log=open(root/'log','w'); app=None
    compositor=subprocess.Popen(['sway','--config',str(config)],env=env,stdout=log,stderr=log)
    try:
        wait_for(lambda:(runtime/'wayland-1').exists())
        env.update(WAYLAND_DISPLAY='wayland-1',XDG_SESSION_TYPE='wayland')
        sock=str(next(runtime.glob('sway-ipc.*.sock')))
        db=str(root/'history.db')
        subprocess.run([str(binary),'--database',db,'config','paused','true'],check=True,env=env)
        app=subprocess.Popen([str(binary),'--database',db],env=env,stdout=log,stderr=log)
        def windows():
            tree=json.loads(subprocess.check_output(['swaymsg','-s',sock,'-t','get_tree']))
            def visit(n):return ([n] if n.get('pid')==app.pid else [])+sum([visit(c) for k in ['nodes','floating_nodes'] for c in n.get(k,[])],[])
            return visit(tree)
        wait_for(lambda:len(windows())==1)
        # Exercise exported wire signatures on an isolated session bus.
        def call(method, *args):
            return subprocess.Popen(['busctl','--user','call','org.cliprust.KWin','/org/cliprust/KWin','org.cliprust.KWin',method,*args],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        pending=call('NextRequest')
        subprocess.run([str(binary),'--database',db,'--show'],env=env,check=True,timeout=5)
        fields=shlex.split(pending.communicate(timeout=5)[0])
        while fields[2]=='check': # Capability probes from older builds.
            report=call('Report','sbss',fields[1],'false','0','No target prepared')
            assert report.wait(timeout=3)==0
            pending=call('NextRequest')
            fields=shlex.split(pending.communicate(timeout=5)[0])
        assert fields[0]=='ssss' and fields[2]=='prepare', fields
        report=call('Report','sbss',fields[1],'true','1','')
        assert report.wait(timeout=3)==0
        pending=call('NextRequest')
        fields=shlex.split(pending.communicate(timeout=5)[0])
        while fields[2]=='check':
            report=call('Report','sbss',fields[1],'true','1','')
            assert report.wait(timeout=3)==0
            fields=shlex.split(call('NextRequest').communicate(timeout=5)[0])
        assert fields[0]=='ssss' and fields[2]=='show', fields
        report=call('Report','sbss',fields[1],'true','1','')
        assert report.wait(timeout=3)==0
        assert app.poll() is None and len(windows())==1
        print('PASS: actual KWin bridge D-Bus prepare/show/report transport (mock peer)')
    finally:
        if app:app.terminate();app.wait(timeout=5)
        compositor.terminate();compositor.wait(timeout=5);log.close()
