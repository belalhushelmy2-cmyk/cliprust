"""Private headless Sway test: monitor starts before display, then reconnects.

Never touches the user's display or clipboard. Requires sway and wl-copy.
"""
import os
import pathlib
import sqlite3
import subprocess
import tempfile
import time

binary = str(pathlib.Path(__file__).resolve().parents[1] / "target/debug/cliprust")

def wait_for(predicate, timeout=20):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if predicate(): return
        time.sleep(.1)
    raise AssertionError("Reconnect test timed out")

with tempfile.TemporaryDirectory(prefix="cliprust-reconnect-") as root:
    runtime = pathlib.Path(root) / "runtime"
    runtime.mkdir(mode=0o700)
    config = pathlib.Path(root) / "sway.conf"
    config.write_text("output HEADLESS-1 resolution 1280x720\nseat seat0 fallback true\n")
    env = dict(os.environ, XDG_RUNTIME_DIR=str(runtime), WLR_BACKENDS="headless",
               WLR_RENDERER="pixman", WLR_HEADLESS_OUTPUTS="1", WLR_LIBINPUT_NO_DEVICES="1")
    for name in ["DISPLAY", "WAYLAND_DISPLAY", "SWAYSOCK"]: env.pop(name, None)
    client = dict(env, WAYLAND_DISPLAY="wayland-1", XDG_SESSION_TYPE="wayland")
    db = str(pathlib.Path(root) / "history.db")
    monitor = subprocess.Popen([binary, "--database", db, "monitor", "--seconds", "60"], env=client,
                               stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    compositor = None
    log = open(pathlib.Path(root) / "sway.log", "wb")
    try:
        time.sleep(1.5)  # Force at least one failed initial connection.
        def start():
            process = subprocess.Popen(["sway", "--config", str(config)], env=env, stdout=log, stderr=log)
            wait_for(lambda: (runtime / "wayland-1").exists() or process.poll() is not None)
            assert process.poll() is None, "Private compositor failed to start"
            return process
        def publish_and_wait(text):
            subprocess.run(["wl-copy", "--type", "text/plain", text], env=client, check=True, timeout=5)
            def captured():
                assert monitor.poll() is None, "Monitor stopped unexpectedly"
                with sqlite3.connect(db) as conn:
                    return conn.execute("SELECT count(*) FROM entries WHERE preview=?", [text]).fetchone()[0] == 1
            wait_for(captured)
        compositor = start()
        publish_and_wait("initial late connection")
        compositor.terminate()
        compositor.wait(timeout=5)
        time.sleep(.5)
        compositor = start()
        publish_and_wait("captured after display restart")
        print("PASS: missing-display startup recovery and capture after compositor restart")
    finally:
        monitor.terminate()
        monitor.communicate(timeout=5)
        if compositor is not None and compositor.poll() is None:
            compositor.terminate()
            compositor.wait(timeout=5)
        log.close()
