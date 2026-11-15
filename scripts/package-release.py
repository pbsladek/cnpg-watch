"""Create deterministic, self-contained packages; never publish them."""
import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import tarfile
import tomllib

TARGETS = {
    "aarch64-apple-darwin": "darwin-arm64",
    "x86_64-apple-darwin": "darwin-amd64",
    "x86_64-unknown-linux-musl": "linux-amd64",
    "aarch64-unknown-linux-musl": "linux-arm64",
}
OPERATOR_GUIDES = (
    "installation", "setup", "operational", "fleet-and-probes", "compatibility",
)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def package(target, binary, out, generated):
    version = tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"]
    label = TARGETS[target]
    name = f"cnpg-watch-{version}-{label}"
    files = {
        "cnpg-watch": (binary.read_bytes(), 0o755),
        "LICENSE": (Path("LICENSE").read_bytes(), 0o644),
        "README.md": (Path("README.md").read_bytes(), 0o644),
    }
    for path in sorted(generated.rglob("*")):
        if path.is_file():
            files[str(path.relative_to(generated))] = (path.read_bytes(), 0o644)
    for guide in OPERATOR_GUIDES:
        path = Path(f"docs/{guide}.md")
        files[str(path)] = (path.read_bytes(), 0o644)
    inputs = [Path(p) for p in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml")]
    inputs += sorted(Path("src").rglob("*.rs"))
    manifest = {
        "schema_version": 1,
        "version": version,
        "target": target,
        "package_label": label,
        "runtime_smoke": "reported separately; compilation alone is not qualification",
        "files": {
            key: {"sha256": digest(data), "mode": oct(mode)}
            for key, (data, mode) in sorted(files.items())
        },
        "source_sha256": {str(p): digest(p.read_bytes()) for p in inputs},
    }
    files["manifest.json"] = (
        json.dumps(manifest, sort_keys=True, indent=2).encode() + b"\n", 0o644,
    )
    out.mkdir(parents=True, exist_ok=True)
    archive = out / (name + ".tar.gz")
    with archive.open("wb") as stream:
        with gzip.GzipFile(fileobj=stream, mode="wb", mtime=0, filename="") as zipped:
            with tarfile.open(fileobj=zipped, mode="w", format=tarfile.USTAR_FORMAT) as tar:
                for key, (data, mode) in sorted(files.items()):
                    info = tarfile.TarInfo(name + "/" + key)
                    info.size = len(data)
                    info.mode = mode
                    info.uid = info.gid = info.mtime = 0
                    info.uname = info.gname = ""
                    tar.addfile(info, io.BytesIO(data))
    archives = sorted(out.glob("cnpg-watch-*.tar.gz"))
    (out / "SHA256SUMS").write_text("".join(
        f"{digest(p.read_bytes())}  {p.name}\n" for p in archives
    ))
    return archive


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, default=Path("dist"))
    parser.add_argument("--generated", type=Path, default=Path("artifacts/release-docs"))
    args = parser.parse_args()
    print(package(args.target, args.binary, args.out_dir, args.generated))
