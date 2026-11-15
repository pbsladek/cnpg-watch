"""Regression checks for the limited PTY screen projection used by smoke tests."""
import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent / "support"))
from terminal_screen import TerminalScreen


class ScreenAssertions(unittest.TestCase):
    def test_differential_output_preserves_unchanged_letters(self):
        screen = TerminalScreen(20, 3)
        screen.feed(b"\x1b[1;1HServices")
        # The shared initial S remains in place; it is absent from emitted bytes.
        screen.feed(b"\x1b[1;2Htorage\x1b[1;8H\x1b[K")
        self.assertEqual(screen.text().splitlines()[0].rstrip(), "Storage")

    def test_chunked_cursor_sequence_resize_and_unicode(self):
        screen = TerminalScreen(20, 3)
        screen.feed(b"\x1b[2;")
        screen.feed(b"3HHealth \xe7")
        screen.feed(b"\x95\x8c")
        self.assertIn("  Health 界", screen.text())
        screen.resize(30, 4)
        screen.feed(b"\x1b[2J\x1b[HProfile")
        self.assertEqual(screen.text().splitlines()[0].rstrip(), "Profile")

    def test_incomplete_control_sequences_do_not_accumulate_without_bound(self):
        screen = TerminalScreen(20, 3)
        for _ in range(100):
            screen.feed(b"\x1b[" + b"1;" * 3000)
            self.assertLessEqual(len(screen.pending), 4096)
        self.assertEqual(len(screen.cells), 3)


if __name__ == "__main__":
    unittest.main()
