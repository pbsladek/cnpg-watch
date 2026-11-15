#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source tests/e2e/versions.env
if [[ -x "$PWD/artifacts/tools/bin/kubectl" ]]; then export PATH="$PWD/artifacts/tools/bin:$PATH"; fi
matrix_cell=${E2E_MATRIX_CELL:-k8s135}
read -r K3S_IMAGE expected_server < <(python3 - "$matrix_cell" <<'PYMATRIX'
import json,sys
cells=json.load(open('tests/e2e/matrix.json'))['cells']
cell=next(c for c in cells if c['id']==sys.argv[1])
print(cell['k3s_image'],cell['server_version'])
PYMATRIX
)
suite=slice
if [[ $# -gt 0 ]]; then
  [[ $# -eq 2 && "$1" == --suite ]] || { echo 'usage: e2e-local-k3d.sh [--suite slice|permissions|fleet|probes|operations|cleanup]' >&2; exit 2; }
  suite=$2
fi
cleanup_probe=0
if [[ "$suite" == cleanup ]]; then cleanup_probe=1; suite=slice; fi
operations=0
if [[ "$suite" == operations ]]; then operations=1; suite=fleet; fi
if [[ "$suite" == probes ]]; then suite=fleet; fi
[[ "$suite" == slice || "$suite" == permissions || "$suite" == fleet ]] || { echo "suite not implemented: $suite" >&2; exit 2; }
for tool in docker k3d kubectl curl openssl python3 cargo rg; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
kubectl version --client -o json | python3 -c 'import json,sys; assert json.load(sys.stdin)["clientVersion"]["gitVersion"]==sys.argv[1], "install pinned fixture tools with scripts/install-e2e-tools.sh"' "$KUBECTL_VERSION"
docker info >/dev/null
[[ "$(k3d version | head -1)" == "k3d version $K3D_VERSION" ]] || { echo "requires k3d $K3D_VERSION" >&2; exit 1; }
run_name="cnpg-watch-e2e-$(date +%s)-$(openssl rand -hex 3)"
export E2E_CONTEXT="k3d-$run_name"
artifact_dir="$PWD/artifacts/e2e/$run_name"
umask 077
mkdir -p "$artifact_dir"
export KUBECONFIG="$artifact_dir/kubeconfig"
export E2E_PASSWORD_FILE="$artifact_dir/password"
export E2E_ADMIN_PASSWORD_FILE="$artifact_dir/admin-password"
export E2E_LOG_FILE="$artifact_dir/app.log"
# Keep user application configuration out of the disposable fixture.
unset CNPG_WATCH_CONFIG CNPG_WATCH_CONTEXT CNPG_WATCH_NAMESPACE \
  CNPG_WATCH_ALL_NAMESPACES CNPG_WATCH_PROFILE CNPG_WATCH_REFRESH \
  CNPG_WATCH_PASSWORD_FILE CNPG_WATCH_DB_ROLE CNPG_WATCH_LOG_FILE
export CNPG_WATCH_E2E=1
created=0
metrics_pid=
k() { kubectl --kubeconfig "$KUBECONFIG" --context "$E2E_CONTEXT" --request-timeout=15s "$@"; }
redact() { python3 scripts/redact-e2e.py "$E2E_PASSWORD_FILE" "$E2E_ADMIN_PASSWORD_FILE"; }
cleanup() {
  result=$?
  trap - EXIT
  if [[ -n "$metrics_pid" ]]; then kill "$metrics_pid" 2>/dev/null || true; wait "$metrics_pid" 2>/dev/null || true; fi
  if [[ "$created" == 1 && -f "$KUBECONFIG" ]]; then
    k get pods -A -o wide 2>&1 | redact > "$artifact_dir/pods.txt" || true
    k -n cnpg-watch-e2e get clusters,events -o yaml 2>&1 | redact > "$artifact_dir/state.yaml" || true
    k -n cnpg-system logs deployment/cnpg-controller-manager --tail=200 --limit-bytes=65536 2>&1 | redact > "$artifact_dir/operator.log" || true
    for pod in payments-1 payments-2; do
      k -n cnpg-watch-e2e logs "$pod" -c postgres --tail=200 --limit-bytes=65536 2>&1 | redact > "$artifact_dir/$pod.log" || true
    done
  fi
  cleanup_status=retained
  if [[ "$created" == 1 && "${E2E_KEEP_CLUSTER:-0}" != 1 ]]; then
    if k3d cluster delete "$run_name"; then cleanup_status=passed; else cleanup_status=failed; result=1; fi
  fi
  python3 - "$artifact_dir/run.json" "$run_name" "$matrix_cell" "$expected_server" "$result" "$cleanup_status" <<'PYRUN'
import json,pathlib,sys
path,name,cell,version,result,cleanup=sys.argv[1:]
pathlib.Path(path).write_text(json.dumps({'schema_version':1,'fixture':name,'matrix_cell':cell,'server_version':version,'exit_code':int(result),'cleanup':cleanup,'fixture_disk_policy':{'eviction_floor':'1Gi','image_gc_high_percent':99,'reason':'shared Docker VM; absolute floor for isolated test nodes'}},indent=2)+'\n')
PYRUN
  echo "e2e artifacts: $artifact_dir (private; contains the fixture kubeconfig/credential)"
  exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
created=1
k3d cluster create "$run_name" --image "$K3S_IMAGE" --servers 1 --agents 0 \
  --kubeconfig-update-default=false --kubeconfig-switch-context=false \
  --k3s-arg '--disable=traefik@server:*' --k3s-arg '--disable=metrics-server@server:*' \
  --k3s-arg '--kubelet-arg=eviction-hard=nodefs.available<1Gi,imagefs.available<1Gi,nodefs.inodesFree<5%@server:*' \
  --k3s-arg '--kubelet-arg=image-gc-high-threshold=99@server:*' \
  --k3s-arg '--kubelet-arg=image-gc-low-threshold=98@server:*' --wait --timeout 180s
k3d kubeconfig get "$run_name" > "$KUBECONFIG"
k version -o json > "$artifact_dir/kubernetes-version.json"
python3 - "$artifact_dir/kubernetes-version.json" "$expected_server" <<'PYVERSION'
import json,sys
assert json.load(open(sys.argv[1]))['serverVersion']['gitVersion']==sys.argv[2], 'fixture server version differs from matrix pin'
PYVERSION
if [[ "$cleanup_probe" == 1 ]]; then echo 'controlled failure to verify diagnostics and cleanup' >&2; exit 42; fi
curl --fail --silent --show-error --location --retry 2 --max-time 60 \
  "https://raw.githubusercontent.com/cloudnative-pg/cloudnative-pg/v$CNPG_VERSION/releases/cnpg-$CNPG_VERSION.yaml" -o "$artifact_dir/operator.yaml"
python3 - "$artifact_dir/operator.yaml" "$CNPG_MANIFEST_SHA256" <<'PY'
import hashlib, pathlib, sys
assert hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest() == sys.argv[2], 'operator manifest checksum mismatch'
PY
k apply --server-side -f "$artifact_dir/operator.yaml"
k -n cnpg-system rollout status deployment/cnpg-controller-manager --timeout=180s --request-timeout=180s
k create namespace cnpg-watch-e2e --dry-run=client -o yaml | k apply -f -
openssl rand -hex 24 | tr -d '\n' > "$E2E_PASSWORD_FILE"
k -n cnpg-watch-e2e create secret generic cnpg-watch-monitor --from-literal=username=watch_monitor --from-file=password="$E2E_PASSWORD_FILE" --dry-run=client -o yaml | k apply -f -
k apply -f tests/e2e/cluster.yaml
k -n cnpg-watch-e2e wait --for=condition=Ready cluster/payments --timeout=360s --request-timeout=360s
if [[ "$suite" == fleet ]]; then
  k create namespace cnpg-watch-e2e-other --dry-run=client -o yaml | k apply -f -
  k -n cnpg-watch-e2e-other create secret generic cnpg-watch-monitor --from-literal=username=watch_monitor --from-file=password="$E2E_PASSWORD_FILE" --dry-run=client -o yaml | k apply -f -
  k apply -f tests/e2e/fleet.yaml
  k -n cnpg-watch-e2e-other wait --for=condition=Ready cluster/inventory --timeout=360s --request-timeout=360s
  k -n cnpg-watch-e2e wait --for=condition=Available deployment/payments-pool --timeout=300s --request-timeout=300s
fi
if [[ "$suite" == permissions || "$suite" == fleet ]]; then
  k -n cnpg-watch-e2e patch cluster payments --type merge -p '{"spec":{"enableSuperuserAccess":true}}'
  for attempt in $(seq 1 60); do
    if k -n cnpg-watch-e2e get secret payments-superuser -o json > "$artifact_dir/admin-secret.json" 2>/dev/null; then break; fi
    sleep 1
  done
  python3 - "$artifact_dir/admin-secret.json" "$E2E_ADMIN_PASSWORD_FILE" <<'PYADMIN'
import base64,json,pathlib,sys
path=pathlib.Path(sys.argv[1])
secret=json.loads(path.read_text())
pathlib.Path(sys.argv[2]).write_bytes(base64.b64decode(secret['data']['password']))
path.unlink()
PYADMIN
fi
if [[ "$operations" == 1 ]]; then
  k apply -f tests/e2e/operational.yaml
  k -n cnpg-system port-forward --address 127.0.0.1 deployment/cnpg-controller-manager 0:8080 > "$artifact_dir/metrics-forward.log" 2>&1 &
  metrics_pid=$!
  for attempt in $(seq 1 100); do
    if rg -q 'Forwarding from 127.0.0.1:' "$artifact_dir/metrics-forward.log"; then break; fi
    kill -0 "$metrics_pid" || { cat "$artifact_dir/metrics-forward.log" >&2; exit 1; }
    sleep .2
  done
  E2E_METRICS_URL=$(python3 - "$artifact_dir/metrics-forward.log" <<'PYMETRICS'
import pathlib,re,sys
match=re.search(r'Forwarding from 127.0.0.1:(\d+)',pathlib.Path(sys.argv[1]).read_text())
assert match, 'metrics forward did not become ready'
print('http://127.0.0.1:'+match[1]+'/metrics')
PYMETRICS
)
  export E2E_METRICS_URL
fi
if [[ "${E2E_PROVISION_ONLY:-0}" == 1 ]]; then
  echo "fixture provisioned for explicitly selected live tests"
  exit 0
fi
test_suite=$suite
if [[ "$operations" == 1 ]]; then test_suite=operations; fi
./scripts/test-fixture.sh "$test_suite"
cp artifacts/phase7/terminal-live.json "$artifact_dir/terminal-live.json"

if [[ "${E2E_MEASURE_IDLE:-0}" == 1 ]]; then
  cargo build --release --locked
  CNPG_WATCH_IDLE_REPORT="$artifact_dir/idle.json" python3 scripts/measure-idle.py
fi
