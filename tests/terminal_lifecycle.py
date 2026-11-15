"""Real guard restoration on return, error, panic, and Ctrl-C."""
import sys
import termios
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent / "support"))
from pty_harness import PtyProcess

for key, expected in [(b"q", 0), (b"\x03", 0), (b"e", 1), (b"p", 101)]:
    with PtyProcess(["target/debug/examples/terminal_lifecycle"]) as child:
        child.wait_for(b"READY")
        assert not termios.tcgetattr(child.master)[3] & termios.ICANON
        child.send(key)
        assert child.wait_exit() == expected
        flags = termios.tcgetattr(child.master)[3]
        assert flags & termios.ICANON and flags & termios.ECHO
        assert b"\x1b[?1049l" in child.data
print("PASS: q, Ctrl-C, error, and panic restore termios and alternate screen")
