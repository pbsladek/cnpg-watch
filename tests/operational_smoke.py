"""Exercise phase 5/6 production commands and the named auxiliary opt-ins."""
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
dumps = Path(os.environ["E2E_LOG_FILE"]).parent / "operational-dumps"
dumps.mkdir(exist_ok=True)
args = ["target/debug/cnpg-watch", "--log-file", os.environ["E2E_LOG_FILE"],
        "--config", "tests/e2e/watch.toml", "--kube-context", context,
        "--password-file", os.environ["E2E_PASSWORD_FILE"], "--profile", "incident",
        "--operator-namespace", "cnpg-system", "--pooler-stats", "--cluster-resources",
        "--certificate-secret", "cnpg-watch-e2e/payments-server",
        "--operator-metrics-url", os.environ["E2E_METRICS_URL"], "--dump-dir", str(dumps)]
with PtyProcess(args, cols=180, rows=50) as child:
    child.wait_for(b"payments-1", 45)
    width = 180
    def command(text, marker):
        global width
        child.send(text.encode() + b"\r")
        deadline = time.monotonic() + .2
        while time.monotonic() < deadline:
            child.read(.02)
        width = 181 if width == 180 else 180
        child.resize(width, 50)
        try:
            child.wait_for(marker, 60)
        except AssertionError:
            (dumps.parent / "operational-terminal.bin").write_bytes(child.data)
            raise
    command(":backups", b"payments-unavailable-plugin")
    command(":roles", b"watch-fixture-role")
    command(":databases", b"watch-fixture-db")
    command(":publications", b"watch-fixture-publication")
    command(":replicas", b"Replicas")
    command(":plugins", b"Plugins")
    command(":certs", b"SHA256")
    command(":poolers", b"PgBouncer")
    command(":logscope cluster", b"Logs")
    command(":logsource poolers off", b"Logs")
    command(":logscope backup payments-unavailable-plugin", b"Logs")
    command(":logscope operator", b"cnpg-system")
    child.send(b" ")
    child.send(b"G")
    command(":logscope unmatched", b"Logs")
    command(":instances", b"Instances")
    command(":dump", b"saved")
    files = list(dumps.glob("*.json"))
    assert files
    for path in files:
        value = json.loads(path.read_text())
        assert value["operational"]["schema_version"] == 1
        assert value["logs"]["max_timeline_rows"] == 512
        assert not any(marker in path.read_text() for marker in ["BEGIN CERTIFICATE", "PRIVATE KEY", "tls.key", "reconcileID"])
    child.send(b"q")
    assert child.wait_exit() == 0
    assert termios.tcgetattr(child.master)[3] & termios.ICANON
print("PASS: operational views, auxiliary opt-ins, timeline scopes/toggles, bounded dump and terminal cleanup")
