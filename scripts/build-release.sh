#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
target=${1:?usage: build-release.sh RUST_TARGET [OUTPUT_DIRECTORY]}
out=${2:-dist}
case "$target" in
  aarch64-apple-darwin|x86_64-apple-darwin|x86_64-unknown-linux-musl|aarch64-unknown-linux-musl) ;;
  *) echo 'unsupported package target' >&2; exit 2;;
esac
host=$(rustc -vV | sed -n 's/^host: //p')
if [[ "$target" == "$host" ]]; then
  cargo build --release --locked --bin cnpg-watch
  binary=target/release/cnpg-watch
elif [[ "$target" == *linux-musl ]]; then
  cargo zigbuild --release --locked --target "$target" --bin cnpg-watch
  binary="target/$target/release/cnpg-watch"
else
  cargo build --release --locked --target "$target" --bin cnpg-watch
  binary="target/$target/release/cnpg-watch"
fi
cargo run --locked --example generate_cli_artifacts -- artifacts/release-docs
python3 scripts/package-release.py --target "$target" --binary "$binary" --out-dir "$out"
