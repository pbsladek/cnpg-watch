"""Phase 1 real-binary smoke; full terminal emulation remains Phase 7."""
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
args = [os.environ.get("CNPG_WATCH_BINARY", "target/debug/cnpg-watch"), "--log-file", os.environ.get("E2E_LOG_FILE", "artifacts/phase1/app.log"), "--config", "tests/e2e/watch.toml", "--kube-context", context, "--password-file", os.environ["E2E_PASSWORD_FILE"]]
for key in [b"q", b"\x03"]:
    with PtyProcess(args) as child:
        original_wait = child.wait_for
        def wait(text):
            try:
                original_wait(text)
            except AssertionError:
                (Path(os.environ["E2E_LOG_FILE"]).parent / "app-terminal.bin").write_bytes(child.data)
                raise
        widths = [120]
        def redraw(data):
            child.send(data)
            deadline = time.monotonic() + .2
            while time.monotonic() < deadline:
                child.read(.02)
            widths[0] = 121 if widths[0] == 120 else 120
            child.resize(widths[0], 40)
        wait(b"payments-1")
        wait(b"fresh")
        # Keep key input beside resize events: the default Unix reader previously
        # lost a readiness notification and left the next panel unopened.
        for _ in range(8):
            redraw(b"d")
            wait(b"Doctor / accepted evidence")
            redraw(b"\x1b")
            wait(b"Instances")
            redraw(b"p")
            wait(b"Change profile")
            redraw(b"\x1b")
            wait(b"Instances")
        redraw(b"p")
        wait(b"Change profile")
        child.data = b""
        redraw(b"1")
        wait(b"Minim")
        child.data = b""
        redraw(b"2")
        wait(b"Stand")
        child.data = b""
        redraw(b"\x1b")
        wait(b"Instances")
        redraw(b"?")
        wait(b"Observe-only")
        child.send(key)
        assert child.wait_exit() == 0
        assert b"\x1b[?1049l" in child.data
        assert termios.tcgetattr(child.master)[3] & termios.ICANON
print("PASS: actual app shows live topology/SQL, opens capability/profile panels, changes profiles, opens help, and exits on q/Ctrl-C")
