"""Exercise automatic fresh-recording loading in the packaged app.

Usage: python3 scripts/autozoom-smoke.py VIDEO
VIDEO must have an aspect ratio below 1.2 and a .cursor.json sidecar with clicks.
The video is read only; test projects and evidence go under test-output.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
source = Path(sys.argv[1]).resolve()
app = root / "dist/SubTake.app/Contents/MacOS/SubTake"
out = root / "test-output/autozoom-regression"
out.mkdir(parents=True, exist_ok=True)
prefs = Path.home() / "Library/Application Support/com.SubTake.SubTake-Native/preferences.json"
before = prefs.read_bytes() if prefs.exists() else None
env = dict(os.environ, SUBTAKE_LAUNCHER_SMOKE="autozoom",
           SUBTAKE_AUTOZOOM_SOURCE=str(source), SUBTAKE_LAUNCHER_TEST_DIRECTORY=str(out))
with (out / "run.log").open("w") as log:
    subprocess.run([str(app)], env=env, stdout=log, stderr=subprocess.STDOUT,
                   check=True, timeout=60)
assert "LAUNCHER_SMOKE_PASSED autozoom" in (out / "run.log").read_text()
assert before == (prefs.read_bytes() if prefs.exists() else None), "User preferences changed"
evidence = json.loads((out / "automatic-zoom.json").read_text())
evidence.update(binary_sha256=hashlib.sha256(app.read_bytes()).hexdigest(),
                user_preferences_unchanged=True, ordinary_reopen_unchanged=True,
                disabled_preference_respected=True)
(out / "verification.json").write_text(json.dumps(evidence, indent=2) + "\n")
print(json.dumps(evidence, indent=2))
