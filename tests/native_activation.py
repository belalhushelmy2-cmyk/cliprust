"""Private Sway smoke test of existing-window IPC and xdg-token transport.
An invalid token tests safe handling, NOT compositor acceptance of activation.
"""
import json, os, pathlib, subprocess, tempfile, time
binary = pathlib.Path(__file__).resolve().parents[1] / 'target/release/cliprust'
def wait_for(f):
    for _ in range(100):
        if f(): return
        time.sleep(.05)
    raise AssertionError('Timed out')
with tempfile.TemporaryDirectory(prefix='cliprust-activation-') as directory:
    root=pathlib.Path(directory); runtime=root/'runtime'; runtime.mkdir(mode=0o700)
    config=root/'sway.conf'; config.write_text('output HEADLESS-1 resolution 1280x800\nseat seat0 fallback true\nfor_window [app_id="cliprust"] floating enable\n')
    env=dict(os.environ,XDG_RUNTIME_DIR=str(runtime),WLR_BACKENDS='headless',WLR_RENDERER='pixman',WLR_HEADLESS_OUTPUTS='1',WLR_LIBINPUT_NO_DEVICES='1',XDG_CURRENT_DESKTOP='sway')
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
        subprocess.run([str(binary),'--database',db,'--show'],env=dict(env,XDG_ACTIVATION_TOKEN='invalid-test-token'),check=True,timeout=5)
        time.sleep(.3)
        assert app.poll() is None and len(windows())==1
        subprocess.run(['swaymsg','-s',sock,f'[pid={app.pid}] kill'],check=True,stdout=subprocess.DEVNULL)
        wait_for(lambda:len(windows())==0)
        subprocess.run([str(binary),'--database',db,'--show'],env=env,check=True,timeout=5)
        wait_for(lambda:len(windows())==1)
        assert app.poll() is None
        print('PASS: one existing window, token IPC safely handled, hidden window reopened')
    finally:
        if app:app.terminate();app.wait(timeout=5)
        compositor.terminate();compositor.wait(timeout=5);log.close()
