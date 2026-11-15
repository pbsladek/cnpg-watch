"""Prove failure diagnostics and cleanup using an isolated, deliberately failed fixture."""
import json
import pathlib
import subprocess
import os
before=set(pathlib.Path('artifacts/e2e').glob('*/run.json'))
result=subprocess.run(['./scripts/e2e-local-k3d.sh','--suite','cleanup'],env={**os.environ,'E2E_KEEP_CLUSTER':'0'})
assert result.returncode==42, f'expected injected failure 42, got {result.returncode}'
new=set(pathlib.Path('artifacts/e2e').glob('*/run.json'))-before
assert len(new)==1
path=new.pop()
report=json.loads(path.read_text())
assert report['exit_code']==42 and report['cleanup']=='passed'
clusters=json.loads(subprocess.check_output(['k3d','cluster','list','-o','json']))
assert not any(c['name']==report['fixture'] for c in clusters)
assert (path.parent/'pods.txt').is_file()
assert (path.parent/'operator.log').is_file()
pathlib.Path('artifacts/phase7').mkdir(exist_ok=True)
pathlib.Path('artifacts/phase7/cleanup.json').write_text(json.dumps(report,indent=2)+'\n')
print('PASS: injected failure retained diagnostics and removed only its own fixture')
