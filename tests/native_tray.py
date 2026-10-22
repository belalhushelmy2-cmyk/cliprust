"""Explicit Linux SNI acceptance test. Requires a running desktop/tray host.

Uses an isolated database and does not replace the clipboard. Temporarily resumes
capture to test the menu; any captured content is deleted with the test directory.
"""
import json
import pathlib
import sqlite3
import subprocess
import tempfile
import time

binary = str(pathlib.Path(__file__).resolve().parents[1] / "target/debug/cliprust")

def bus(*args):
    output = subprocess.check_output(["busctl", "--user", "--json=short", "--", *args], timeout=5)
    return json.loads(output) if output.strip() else None

def wait_for(predicate):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.1)
    raise AssertionError("Tray operation timed out")

with tempfile.TemporaryDirectory(prefix="cliprust-tray-test-") as directory:
    db = str(pathlib.Path(directory) / "history.db")
    subprocess.run([binary, "--database", db, "config", "paused", "true"], check=True)
    process = subprocess.Popen([binary, "--database", db], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    try:
        service = None
        def registered():
            global service
            assert process.poll() is None, "Cliprust exited before tray registration"
            output = subprocess.check_output(["busctl", "--user", "list", "--no-pager"], text=True)
            matches = [line.split()[0] for line in output.splitlines() if line.startswith(f"org.kde.StatusNotifierItem-{process.pid}-")]
            if matches:
                service = matches[0]
            return bool(matches)
        wait_for(registered)
        wait_for(lambda: service in json.dumps(bus("get-property", "org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher", "org.kde.StatusNotifierWatcher", "RegisteredStatusNotifierItems")))
        def menu():
            layout = bus("call", service, "/MenuBar", "com.canonical.dbusmenu", "GetLayout", "iias", "0", "-1", "0")
            labels = {}
            def visit(value):
                if isinstance(value, dict):
                    for child in value.values(): visit(child)
                elif isinstance(value, list):
                    if len(value) == 3 and isinstance(value[0], int) and isinstance(value[1], dict):
                        label = value[1].get("label")
                        if label:
                            labels[label["data"]] = value[0]
                    for child in value: visit(child)
            visit(layout)
            return labels
        def click(label):
            item = menu()[label]
            bus("call", service, "/MenuBar", "com.canonical.dbusmenu", "Event", "isvu", str(item), "clicked", "s", "", "0")
        def paused():
            with sqlite3.connect(db) as conn:
                return conn.execute("SELECT value FROM settings WHERE key='paused'").fetchone()[0] == "true"
        wait_for(lambda: "Resume Monitoring" in menu())
        click("Show History")
        click("Resume Monitoring")
        wait_for(lambda: not paused())
        wait_for(lambda: "Pause Monitoring" in menu())
        click("Pause Monitoring")
        wait_for(paused)
        click("Quit Cliprust")
        assert process.wait(timeout=10) == 0
        print("PASS: SNI registration, Show dispatch, Resume/Pause persistence and menu updates, Quit")
    finally:
        if process.poll() is None:
            process.terminate()
        process.communicate(timeout=5)
