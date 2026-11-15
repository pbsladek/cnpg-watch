#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
target=$(rustc -vV | sed -n 's/^host: //p' | sed 's/unknown-linux-gnu/unknown-linux-musl/')
./scripts/build-release.sh "$target"
case "$target" in
  aarch64-apple-darwin) label=darwin-arm64;;
  x86_64-apple-darwin) label=darwin-amd64;;
  aarch64-unknown-linux-musl) label=linux-arm64;;
  x86_64-unknown-linux-musl) label=linux-amd64;;
esac
version=$(python3 -c 'import pathlib,tomllib; print(tomllib.loads(pathlib.Path("Cargo.toml").read_text())["package"]["version"])')
archive="dist/cnpg-watch-$version-$label.tar.gz"
python3 scripts/smoke-package.py "$archive" --execution 'native local host' --report "artifacts/phase7/smoke-$label.json"
