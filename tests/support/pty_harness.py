"""Small bounded POSIX terminal harness shared by lifecycle and live smoke checks."""
import errno
import fcntl
import os
import pty
import select
import signal
import struct
import termios
import time
from terminal_screen import TerminalScreen


class PtyProcess:
    def __init__(self, args, cols=120, rows=40, env=None):
        self.pid, self.master = pty.fork()
        if self.pid == 0:
            environment = dict(os.environ if env is None else env)
            environment["TERM"] = "xterm-256color"
            fcntl.ioctl(1, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
            os.execve(args[0], args, environment)
        os.set_blocking(self.master, False)
        self.data = b""
        self.status = None
        self.screen = TerminalScreen(cols, rows)

    def read(self, timeout=0.01):
        if select.select([self.master], [], [], timeout)[0]:
            try:
                data = os.read(self.master, 65536)
                self.data = (self.data + data)[-1024 * 1024:]
                self.screen.feed(data)
            except BlockingIOError:
                pass
            except OSError as error:
                if error.errno != errno.EIO:
                    raise

    def wait_for(self, text, timeout=15):
        deadline = time.monotonic() + timeout
        while text.decode() not in self.screen.text():
            assert time.monotonic() < deadline, f"missing terminal text {text!r}"
            self.read(0.05)

    def resize(self, cols, rows):
        self.screen.resize(cols, rows)
        fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        os.kill(self.pid, signal.SIGWINCH)

    def send(self, data):
        os.write(self.master, data)

    def wait_exit(self, timeout=10):
        deadline = time.monotonic() + timeout
        while self.status is None:
            # Drain while waiting: a screen flush can otherwise fill the PTY.
            self.read()
            found, status = os.waitpid(self.pid, os.WNOHANG)
            if found:
                self.status = os.waitstatus_to_exitcode(status)
                break
            assert time.monotonic() < deadline, "terminal child did not exit"
        for _ in range(10):
            self.read()
        return self.status

    def __enter__(self):
        return self

    def __exit__(self, *_):
        try:
            if self.status is None:
                os.kill(self.pid, signal.SIGKILL)
                self.wait_exit()
        finally:
            os.close(self.master)
