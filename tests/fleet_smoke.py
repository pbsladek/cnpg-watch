"""Exercise production navigation, bounded filter errors, diagnostics, logs and dumps."""
import json
import os
import sys
import termios
import time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent / "support"))
from pty_harness import PtyProcess

assert os.environ.get("CNPG_WATCH_E2E") == "1"
context = os.environ["E2E_CONTEXT"]
assert context.startswith("k3d-cnpg-watch-")
dumps = Path(os.environ["E2E_LOG_FILE"]).parent / "smoke-dumps"
dumps.mkdir(exist_ok=True)
args = [os.environ.get("CNPG_WATCH_BINARY", "target/debug/cnpg-watch"),
        "--log-file", os.environ["E2E_LOG_FILE"], "--config", "tests/e2e/watch.toml",
        "--kube-context", context, "--password-file", os.environ["E2E_PASSWORD_FILE"],
        "--profile", "incident", "--dump-dir", str(dumps)]
with PtyProcess(args, cols=160, rows=50) as child:
    child.wait_for(b"payments-1", 30)
    child.wait_for(b"fresh", 30)
    width = [160]
    def command(text, marker=None):
        child.send(text.encode() + b"\r")
        until = time.monotonic() + .2
        while time.monotonic() < until:
            child.read(.02)
        width[0] = 161 if width[0] == 160 else 160
        child.resize(width[0], 50)
        if marker:
            try:
                child.wait_for(marker, 30)
            except AssertionError:
                (dumps.parent / "fleet-terminal.bin").write_bytes(child.data)
                raise
        until = time.monotonic() + .2
        while time.monotonic() < until:
            child.read(.02)
    command(":namespace all", b"fenced")
    child.wait_for(b"inventory", 30)
    command(":fleet", b"Fleet")
    command("/[", b"oversized")
    command("/-f payments")
    command(":clear")
    command(":instances", b"Instances")
    command(":health", b"Health")
    command(":hints", b"Hints")
    command(":services", b"Services")
    command(":storage", b"Storage")
    command(":poolers", b"Poolers")
    command(":operator", b"Operator")
    command(":connections", b"Connections")
    command(":waits", b"Waits")
    command(":logs", b"Logs")
    command(":since 15m")
    command(":severity warning")
    child.send(b" ")
    command(":doctor", b"accepted")
    child.send(b"\x1b")
    until = time.monotonic() + .2
    while time.monotonic() < until:
        child.read(.02)
    command(":instances")
    command(":dump", b"saved")
    files = list(dumps.glob("*.json"))
    assert files, "dump command did not create an artifact"
    for path in files:
        value = json.loads(path.read_text())
        assert value["schema_version"] == 1
        assert all("query_text" not in item.get("last_good", {}).get("summary", {}) for item in value["diagnostics"] if item.get("last_good"))
    child.send(b"q")
    assert child.wait_exit() == 0
    assert b"\x1b[?1049l" in child.data
    assert termios.tcgetattr(child.master)[3] & termios.ICANON
print("PASS: fleet scope, views, invalid/fuzzy filters, diagnostics, logs, private dumps and terminal cleanup")
