"""Bounded projection of the cursor/erase operations emitted by our TUI.

This supports screen assertions, not general terminal compatibility emulation.
Raw PTY bytes remain available separately for lifecycle/escape assertions.
"""
import codecs
import re
import unicodedata


class TerminalScreen:
    def __init__(self, cols, rows):
        self.cols, self.rows = cols, rows
        self.cells = [[" "] * cols for _ in range(rows)]
        self.x = self.y = 0
        self.pending = ""
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")

    def resize(self, cols, rows):
        self.cells = [(row + [" "] * cols)[:cols] for row in self.cells[:rows]]
        self.cells += [[" "] * cols for _ in range(rows - len(self.cells))]
        self.cols, self.rows = cols, rows
        self.x, self.y = min(self.x, cols - 1), min(self.y, rows - 1)

    def feed(self, data):
        self.pending += self.decoder.decode(data)
        while self.pending:
            if self.pending.startswith("\x1b["):
                match = re.match(r"\x1b\[([0-?]*)([ -/]*)([@-~])", self.pending)
                if not match:
                    break
                params, _, command = match.groups()
                self.pending = self.pending[match.end():]
                if params.startswith("?"):
                    continue  # Private modes and cursor visibility do not contain text.
                values = [int(p) if p else 0 for p in params.split(";")]
                first = values[0] or 1
                if command in "Hf":
                    self.y = min(self.rows - 1, first - 1)
                    self.x = min(self.cols - 1, (values[1] or 1) - 1 if len(values) > 1 else 0)
                elif command == "G": self.x = min(self.cols - 1, first - 1)
                elif command == "d": self.y = min(self.rows - 1, first - 1)
                elif command == "A": self.y = max(0, self.y - first)
                elif command == "B": self.y = min(self.rows - 1, self.y + first)
                elif command == "C": self.x = min(self.cols - 1, self.x + first)
                elif command == "D": self.x = max(0, self.x - first)
                elif command == "J" and values[0] in (2, 3):
                    self.cells = [[" "] * self.cols for _ in range(self.rows)]
                elif command == "K":
                    start, end = (0, self.cols) if values[0] == 2 else ((0, self.x + 1) if values[0] == 1 else (self.x, self.cols))
                    self.cells[self.y][start:end] = [" "] * (end - start)
                continue
            if self.pending[0] == "\x1b":
                if len(self.pending) < 2: break
                self.pending = self.pending[2:]
                continue
            char, self.pending = self.pending[0], self.pending[1:]
            if char == "\r": self.x = 0
            elif char == "\n": self.y = min(self.rows - 1, self.y + 1)
            elif char == "\b": self.x = max(0, self.x - 1)
            elif not unicodedata.category(char).startswith("C"):
                width = 0 if unicodedata.combining(char) else (2 if unicodedata.east_asian_width(char) in "WF" else 1)
                if width == 0:
                    self.cells[self.y][max(0, self.x - 1)] += char
                    continue
                if self.x >= self.cols:
                    self.x, self.y = 0, min(self.rows - 1, self.y + 1)
                self.cells[self.y][self.x] = char
                if width == 2 and self.x + 1 < self.cols:
                    self.cells[self.y][self.x + 1] = ""
                self.x += width
        # An unsupported or malformed sequence must not create unbounded retention.
        if len(self.pending) > 4096:
            self.pending = ""

    def text(self):
        return "\n".join("".join(row) for row in self.cells)
