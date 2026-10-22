"""Measure the release app on a private headless Sway display with 10k entries.

Reports Linux process RSS/PSS, one-core CPU %, and IPC-to-WM-visible latency.
Activation includes swaymsg observation overhead, not proof of frame presentation.
Requires a built release binary, sway, swaymsg and a session D-Bus.
"""
import json
import os
import pathlib
import socket
import statistics
import subprocess
import tempfile
import time

binary = pathlib.Path(__file__).resolve().parents[1] / "target/release/cliprust"

def wait_for(predicate, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate(): return
        time.sleep(.01)
    raise AssertionError("Measurement setup timed out")

with tempfile.TemporaryDirectory(prefix="cliprust-measure-") as directory:
    root = pathlib.Path(directory)
    runtime = root / "runtime"
    runtime.mkdir(mode=0o700)
    config = root / "sway.conf"
    config.write_text("output HEADLESS-1 resolution 1280x720\nseat seat0 fallback true\n")
    env = dict(os.environ, XDG_RUNTIME_DIR=str(runtime), WLR_BACKENDS="headless",
               WLR_RENDERER="pixman", WLR_HEADLESS_OUTPUTS="1", WLR_LIBINPUT_NO_DEVICES="1")
    for name in ["DISPLAY", "WAYLAND_DISPLAY", "SWAYSOCK"]: env.pop(name, None)
    client = dict(env, WAYLAND_DISPLAY="wayland-1", XDG_SESSION_TYPE="wayland")
    app = None
    with open(root / "sway.log", "wb") as log:
        compositor = subprocess.Popen(["sway", "--config", str(config)], env=env, stdout=log, stderr=log)
        try:
            wait_for(lambda: (runtime / "wayland-1").exists() or compositor.poll() is not None)
            assert compositor.poll() is None, "Private compositor failed"
            sway_socket = next(runtime.glob("sway-ipc.*.sock"))
            def sway(*args):
                return json.loads(subprocess.check_output(["swaymsg", "-s", str(sway_socket), "-r", *args], timeout=5))
            db = root / "history.db"
            now = int(time.time())
            archive = {"version": 1, "entries": [], "snippets": []}
            for i in range(10000):
                text = f"project {i} engineering clipboard transaction reference"
                archive["entries"].append([
                    {"id":i+1,"identity":"","kind":"text","preview":text,"created_at":now-i,
                     "last_used":now-i,"copy_count":1,"pinned":False,"pin_order":0,"source":"Editor"},
                    {"formats":{"text/plain":list(text.encode())}}])
            fixture = root / "fixture.json"
            fixture.write_text(json.dumps(archive))
            subprocess.run([str(binary), "--database", str(db), "import", str(fixture)], check=True, env=client)
            del archive
            app = subprocess.Popen([str(binary), "--database", str(db)], env=client, stdout=subprocess.DEVNULL, stderr=log)
            def visible():
                assert app.poll() is None, "App exited during measurement"
                def contains(node):
                    return node.get("pid") == app.pid or any(contains(child) for key in ["nodes", "floating_nodes"] for child in node.get(key, []))
                return contains(sway("-t", "get_tree"))
            wait_for(visible)
            time.sleep(5)
            def hide():
                sway(f"[pid={app.pid}] kill")
                wait_for(lambda: not visible())
            hide()
            time.sleep(2)
            def usage():
                stat = pathlib.Path(f"/proc/{app.pid}/stat").read_text().rsplit(")", 1)[1].split()
                ticks = int(stat[11]) + int(stat[12])
                pages = int(pathlib.Path(f"/proc/{app.pid}/statm").read_text().split()[1])
                return ticks, pages * os.sysconf("SC_PAGE_SIZE") / (1024 * 1024)
            start_ticks, first_rss = usage()
            start = time.monotonic()
            rss = [first_rss]
            for _ in range(20):
                time.sleep(1)
                rss.append(usage()[1])
            elapsed = time.monotonic() - start
            cpu = (usage()[0] - start_ticks) / os.sysconf("SC_CLK_TCK") / elapsed * 100
            rollup = pathlib.Path(f"/proc/{app.pid}/smaps_rollup").read_text()
            pss = next(int(line.split()[1]) / 1024 for line in rollup.splitlines() if line.startswith("Pss:"))
            activations = []
            for _ in range(5):
                started = time.monotonic()
                with socket.socket(socket.AF_UNIX) as ipc:
                    ipc.connect(str(db.with_suffix(".sock")))
                    ipc.sendall(bytes(8))
                wait_for(visible)
                activations.append((time.monotonic() - started) * 1000)
                hide()
            print(json.dumps({"entries":10000,"monitoring":True,"idle_window":"hidden","sample_seconds":elapsed,
                              "rss_mib_min":min(rss),"rss_mib_max":max(rss),"pss_mib":pss,"cpu_percent_one_core":cpu,
                              "ipc_to_wm_visible_ms":activations,"activation_median_ms":statistics.median(activations)}, indent=2))
        finally:
            if app is not None and app.poll() is None:
                app.terminate()
                app.wait(timeout=5)
            compositor.terminate()
            compositor.wait(timeout=5)
