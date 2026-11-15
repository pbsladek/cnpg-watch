#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source tests/e2e/versions.env
tool_dir="$PWD/artifacts/tools/bin"
mkdir -p "$tool_dir"
platform=$(uname -s | tr '[:upper:]' '[:lower:]')
case "$(uname -m)" in arm64|aarch64) arch=arm64;; x86_64) arch=amd64;; *) echo 'unsupported fixture-tool architecture' >&2; exit 2;; esac
case "$platform" in darwin|linux) ;; *) echo 'unsupported fixture-tool OS' >&2; exit 2;; esac
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
curl -fsSL --max-time 60 "https://github.com/k3d-io/k3d/releases/download/$K3D_VERSION/k3d-$platform-$arch" -o "$scratch/k3d"
curl -fsSL --max-time 30 "https://github.com/k3d-io/k3d/releases/download/$K3D_VERSION/checksums.txt" -o "$scratch/checksums.txt"
curl -fsSL --max-time 60 "https://dl.k8s.io/release/$KUBECTL_VERSION/bin/$platform/$arch/kubectl" -o "$scratch/kubectl"
curl -fsSL --max-time 30 "https://dl.k8s.io/release/$KUBECTL_VERSION/bin/$platform/$arch/kubectl.sha256" -o "$scratch/kubectl.sha256"
python3 - "$scratch" "$platform" "$arch" "$tool_dir" <<'PY'
import hashlib,pathlib,sys,shutil,tempfile,os
root,os_name,arch,destination=sys.argv[1:];root=pathlib.Path(root);destination=pathlib.Path(destination)
rows=[line.split() for line in (root/'checksums.txt').read_text().splitlines() if line.strip()]
expected=next(parts[0] for parts in rows if pathlib.PurePosixPath(parts[-1].lstrip('*')).name==f'k3d-{os_name}-{arch}')
assert hashlib.sha256((root/'k3d').read_bytes()).hexdigest()==expected,'k3d checksum mismatch'
assert hashlib.sha256((root/'kubectl').read_bytes()).hexdigest()==(root/'kubectl.sha256').read_text().strip(),'kubectl checksum mismatch'
for name in ['k3d','kubectl']:
    # Replacing a running Mach-O file in place invalidates its code signature.
    # Install a new inode atomically so existing fixture processes can finish.
    with tempfile.NamedTemporaryFile(dir=destination,prefix=name+'.',delete=False) as staged:
        pending=pathlib.Path(staged.name)
    try:
        shutil.copyfile(root/name,pending);pending.chmod(0o755)
        os.replace(pending,destination/name)
    finally:pending.unlink(missing_ok=True)
PY
"$tool_dir/k3d" version
"$tool_dir/kubectl" version --client -o json
