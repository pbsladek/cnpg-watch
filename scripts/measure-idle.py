"""Measure the actual release TUI's quiescent CPU and RSS in the isolated fixture."""
import json
import os
import subprocess
import sys
import time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tests" / "support"))
from pty_harness import PtyProcess
assert os.environ.get("CNPG_WATCH_E2E") == "1"
context = os.environ["E2E_CONTEXT"]
assert context.startswith("k3d-cnpg-watch-")


def process_stats(pid):
    text = subprocess.check_output(["ps", "-p", str(pid), "-o", "time=", "-o", "rss="], text=True).strip()
    clock, rss = text.split()
    parts = [float(p) for p in clock.split(":")]
    cpu = sum(value * (60 ** index) for index, value in enumerate(reversed(parts)))
    return cpu, int(rss)


profile = os.environ.get("CNPG_WATCH_IDLE_PROFILE", "minimal")
poolers = os.environ.get("CNPG_WATCH_IDLE_POOLER_STATS") == "1"
args = ["target/release/cnpg-watch", "--log-file", os.environ.get("E2E_LOG_FILE", "artifacts/performance/app.log"), "--config", "tests/e2e/watch.toml", "--kube-context", context, "--profile", profile]
if profile != "minimal":
    args += ["--password-file", os.environ["E2E_PASSWORD_FILE"]]
observed_pooler_stats = False
if poolers:
    dump_dir = Path(os.environ["E2E_LOG_FILE"]).parent / ("idle-dumps-" + str(time.time_ns()))
    dump_dir.mkdir(mode=0o700)
    args += ["--pooler-stats", "--dump-dir", str(dump_dir)]
with PtyProcess(args) as child:
    child.wait_for(b"payments-1")
    if profile != "minimal":
        child.wait_for(b"fresh", 60)
    if poolers:
        child.send(b":poolers\r")
        child.wait_for(b"Poolers", 30)
        # Inspect canonical samples: the table may clip a long metadata cell.
        deadline = time.monotonic() + 45
        while not observed_pooler_stats:
            before = set(dump_dir.glob("*.json"))
            child.send(b":dump\r")
            until = time.monotonic() + 1
            while time.monotonic() < until:
                child.read(.05)
            for path in set(dump_dir.glob("*.json")) - before:
                value = json.loads(path.read_text())
                observed_pooler_stats = any(p.get("sample") and p["sample"].get("version") == "1.25.1" and p["sample"].get("age_ms", 999999) < 15000 for p in value["poolers"])
            assert time.monotonic() < deadline, "native Pooler sample did not become fresh"
    warm = time.monotonic() + (8 if poolers else 2)
    while time.monotonic() < warm:
        child.read(0.05)
    cpu_before, rss = process_stats(child.pid)
    start = time.monotonic()
    while time.monotonic() - start < 10:
        child.read(0.05)
    cpu_after, end_rss = process_stats(child.pid)
    elapsed = time.monotonic() - start
    child.send(b"q")
    assert child.wait_exit() == 0
    report = {"scenario": f"actual release TUI, {profile} profile, 1 cluster/2 instances, pooler stats {poolers}, quiescent watches", "observed_pooler_stats": observed_pooler_stats, "elapsed_seconds": elapsed, "cpu_percent_one_core": 100 * (cpu_after - cpu_before) / elapsed, "rss_kib": max(rss, end_rss)}
    print(json.dumps(report))
    report_path=Path(os.environ.get("CNPG_WATCH_IDLE_REPORT", "artifacts/performance/idle.json"))
    report_path.parent.mkdir(parents=True,exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2) + "\n")
    assert report["cpu_percent_one_core"] <= 1
    assert report["rss_kib"] <= 128 * 1024
