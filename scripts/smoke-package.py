"""Execute a built package's CLI and terminal lifecycle; no real cluster required."""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import termios
import threading

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tests/support"))
from pty_harness import PtyProcess

SHELLS = ("bash", "zsh", "fish", "powershell", "elvish")


def unpack(tar, root):
    # Reject links and escaping paths, including on Python 3.11 before data_filter.
    root = root.resolve()
    for member in tar:
        destination = (root / member.name).resolve()
        if not member.isfile() or not destination.is_relative_to(root):
            raise ValueError(f"unsafe archive member: {member.name}")
        destination.parent.mkdir(parents=True, exist_ok=True)
        with tar.extractfile(member) as source, destination.open("wb") as output:
            shutil.copyfileobj(source, output)
        destination.chmod(member.mode & 0o777)


def check_cli(binary, prefix, env, target):
    commands = [["--help"], ["info", "--output", "json"], ["command-reference"]]
    commands += [["completions", shell] for shell in SHELLS]
    for args in commands:
        result = subprocess.run(
            [*prefix, str(binary), *args], env=env, capture_output=True,
            timeout=20, check=True,
        )
        assert result.stdout and not result.stderr
        if args[0] == "info":
            info = json.loads(result.stdout)
            assert info["schema_version"] == 1
            assert info["platform"] == {
                "os": "macos" if "darwin" in target else "linux",
                "arch": target.split("-")[0],
            }


class Api(BaseHTTPRequestHandler):
    """Minimal offline API stub; live compatibility is a separate gate."""

    def log_message(self, *args):
        pass

    def do_GET(self):
        if "watch=true" in self.path:
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            try:
                self.wfile.write(b'{"type":"BOOKMARK","object":{"metadata":{"resourceVersion":"1"}}}\n')
                self.wfile.flush()
                self.server.stop.wait(15)
            except (BrokenPipeError, ConnectionResetError):
                pass
            return
        if self.path == "/version":
            value = {"major": "1", "minor": "35", "gitVersion": "v1.35.5"}
        elif self.path == "/apis/postgresql.cnpg.io/v1":
            value = {
                "apiVersion": "v1", "kind": "APIResourceList",
                "groupVersion": "postgresql.cnpg.io/v1",
                "resources": [{
                    "name": "clusters", "namespaced": True, "kind": "Cluster",
                    "verbs": ["get", "list", "watch"],
                }],
            }
        else:
            value = {
                "apiVersion": "v1",
                "kind": "PodList" if "/pods" in self.path else "ClusterList",
                "metadata": {"resourceVersion": "1"}, "items": [],
            }
        body = json.dumps(value).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def write_kubeconfig(path, port):
    path.write_text(json.dumps({
        "apiVersion": "v1", "kind": "Config",
        "clusters": [{"name": "fixture", "cluster": {"server": f"http://127.0.0.1:{port}"}}],
        "contexts": [{"name": "package-smoke", "context": {"cluster": "fixture", "namespace": "smoke"}}],
        "current-context": "package-smoke", "users": [],
    }))


def check_restored(child):
    assert termios.tcgetattr(child.master)[3] & termios.ICANON
    assert b"\x1b[?1049h" in child.data and b"\x1b[?1049l" in child.data


def check_terminal(binary, prefix, env, root, config):
    base = [*prefix, str(binary), "--config", str(config), "--profile", "minimal"]
    for key in [b"q", b"\x03"]:
        args = [*base, "--namespace", "smoke", "--no-operator-watch", "--log-file", str(root / "app.log")]
        with PtyProcess(args, env=env) as child:
            child.wait_for(b"cnpg-watch", 20)
            child.wait_for(b"Fleet", 20)
            assert not termios.tcgetattr(child.master)[3] & termios.ICANON
            child.resize(100, 30)
            child.send(b"?")
            child.wait_for(b"Observe-only", 10)
            child.send(key)
            assert child.wait_exit(10) == 0
            check_restored(child)
    args = [*base, "--kube-context", "missing-context", "--log-file", str(root / "error.log")]
    with PtyProcess(args, env=env) as child:
        assert child.wait_exit(15) == 1
        check_restored(child)


def smoke(archive, output, prefix, execution):
    with tempfile.TemporaryDirectory(prefix="cnpg-package-") as temp:
        root = Path(temp)
        with tarfile.open(archive) as tar:
            unpack(tar, root)
        package = next(root.iterdir())
        manifest = json.loads((package / "manifest.json").read_text())
        for path, metadata in manifest["files"].items():
            assert hashlib.sha256((package / path).read_bytes()).hexdigest() == metadata["sha256"]
            assert (package / path).stat().st_mode & 0o777 == int(metadata["mode"], 8)
        env = {
            k: v for k, v in os.environ.items()
            if not k.startswith("CNPG_WATCH_")
            and k.upper() not in ("HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY")
        }
        config = root / "watch.toml"
        config.write_text("")
        env["CNPG_WATCH_CONFIG"] = str(config)
        binary = (package / "cnpg-watch").resolve()
        check_cli(binary, prefix, env, manifest["target"])
        server = ThreadingHTTPServer(("127.0.0.1", 0), Api)
        server.daemon_threads = True
        server.stop = threading.Event()
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            kubeconfig = root / "kubeconfig"
            write_kubeconfig(kubeconfig, server.server_port)
            env["KUBECONFIG"] = str(kubeconfig)
            check_terminal(binary, prefix, env, root, config)
        finally:
            server.stop.set()
            server.shutdown()
            server.server_close()
            thread.join(2)
            assert not thread.is_alive()
        report = {
            "schema_version": 1, "target": manifest["target"], "archive": archive.name,
            "archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
            "execution": execution,
            "cli_checks": ["help", "info schema 1 and target OS/architecture", "all five completions", "command reference", "manifest hashes and modes"],
            "terminal_checks": ["real binary startup against local API stub", "resize/help", "q", "Ctrl-C", "error restoration"],
            "cluster_fixture": "local API stub; live compatibility tested separately",
            "passed": True,
        }
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--execution", required=True)
    parser.add_argument("--rosetta", action="store_true")
    args = parser.parse_args()
    prefix = ["/usr/bin/arch", "-x86_64"] if args.rosetta else []
    smoke(args.archive.resolve(), args.report, prefix, args.execution)
