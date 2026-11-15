"""Deterministic archives, manifest integrity and safe package smoke extraction."""
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


packager = load("packager", "scripts/package-release.py")
smoke = load("package_smoke", "scripts/smoke-package.py")


class Release(unittest.TestCase):
    def test_reproducible_archive_and_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "binary"
            binary.write_bytes(b"package fixture")
            generated = root / "generated"
            generated.mkdir()
            (generated / "command-reference.md").write_text("generated fixture\n")
            first = packager.package("aarch64-apple-darwin", binary, root / "one", generated)
            second = packager.package("aarch64-apple-darwin", binary, root / "two", generated)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            with tarfile.open(first) as tar:
                self.assertTrue(all(p.mtime == p.uid == p.gid == 0 for p in tar.getmembers()))
                item = next(p for p in tar.getmembers() if p.name.endswith("/manifest.json"))
                manifest = json.load(tar.extractfile(item))
                self.assertEqual(manifest["schema_version"], 1)
                self.assertEqual(
                    manifest["files"]["cnpg-watch"]["sha256"],
                    packager.digest(binary.read_bytes()),
                )
                extracted = root / "extracted"
                extracted.mkdir()
                smoke.unpack(tar, extracted)
                executable = next(extracted.glob("*/cnpg-watch"))
                self.assertEqual(executable.read_bytes(), binary.read_bytes())
                self.assertEqual(executable.stat().st_mode & 0o777, 0o755)
            self.assertIn(packager.digest(first.read_bytes()), (root / "one/SHA256SUMS").read_text())

    def test_extraction_rejects_escaping_paths_and_links(self):
        for name, kind in [("../escape", tarfile.REGTYPE), ("link", tarfile.SYMTYPE)]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                archive = io.BytesIO()
                with tarfile.open(fileobj=archive, mode="w") as tar:
                    entry = tarfile.TarInfo(name)
                    entry.type = kind
                    entry.linkname = "../escape" if kind == tarfile.SYMTYPE else ""
                    tar.addfile(entry, io.BytesIO())
                archive.seek(0)
                with tarfile.open(fileobj=archive) as tar:
                    with self.assertRaises(ValueError):
                        smoke.unpack(tar, Path(directory))


if __name__ == "__main__":
    unittest.main()
