#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p artifacts/linux-target artifacts/linux-native
# Read-only sources and registry; build/output directories belong to this task.
docker build -t cnpg-watch-validation:phase7 -f tests/release/Containerfile tests/release
docker run --rm --name "cnpg-watch-linux-validation-$$" --network none \
  --mount "type=bind,src=$PWD,dst=/work,readonly" \
  --mount "type=bind,src=$PWD/artifacts/linux-target,dst=/work/target" \
  --mount "type=bind,src=$PWD/artifacts/linux-native,dst=/work/artifacts" \
  --mount "type=bind,src=$HOME/.cargo/registry,dst=/cargo/registry,readonly" \
  --tmpfs /cargo:rw,mode=1777 --env CARGO_HOME=/cargo --env CARGO_NET_OFFLINE=true \
  --env CARGO_BUILD_JOBS=2 --env INSTA_UPDATE=no --env INSTA_FORCE_PASS=0 \
  cnpg-watch-validation:phase7 bash -c '
    set -euo pipefail
    rustc -vV > artifacts/rustc.txt
    uname -a > artifacts/kernel.txt
    make check > artifacts/check.log 2>&1
    make test > artifacts/tests.log 2>&1
    CNPG_WATCH_LOAD_SECONDS=60 CNPG_WATCH_LOAD_REPORT=artifacts/load.json cargo test --release --locked --test load -- --ignored --nocapture > artifacts/load.log 2>&1
  '
