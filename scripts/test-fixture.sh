#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "${CNPG_WATCH_E2E:-0}" == 1 && "${E2E_CONTEXT:-}" == k3d-cnpg-watch-e2e-* && -f "${KUBECONFIG:-}" ]] || { echo 'explicit disposable fixture required' >&2; exit 2; }
suite=${1:-operations}
operations=0
if [[ "$suite" == operations ]]; then operations=1; suite=fleet; fi
[[ "$suite" == slice || "$suite" == permissions || "$suite" == fleet ]] || exit 2
if [[ "$suite" == permissions || "$suite" == fleet ]]; then
  cargo test --locked --test permissions_live -- --ignored --nocapture --test-threads=1
fi
if [[ "$suite" == fleet ]]; then
  cargo test --locked --test fleet_live -- --ignored --nocapture --test-threads=1
  cargo test --locked --test probes_live -- --ignored --nocapture --test-threads=1
fi
if [[ "$operations" == 1 ]]; then
  cargo test --locked --test pooler_live -- --ignored --nocapture --test-threads=1
  cargo test --locked --test operational_live -- --ignored --nocapture --test-threads=1
fi
cargo test --locked --test live -- --ignored --nocapture --test-threads=1
cargo build --locked
cargo build --locked --example terminal_lifecycle
cargo test --locked --test terminal_pty -- --ignored --nocapture --test-threads=1
cargo test --locked --test terminal_live -- --ignored --nocapture --test-threads=1
if [[ "${E2E_LEGACY_PTY:-0}" == 1 ]]; then
  python3 tests/app_smoke.py
  if [[ "$suite" == fleet ]]; then python3 tests/fleet_smoke.py; fi
  if [[ "$operations" == 1 ]]; then python3 tests/operational_smoke.py; fi
fi

