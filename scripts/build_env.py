"""Local native build tools shared by dev, packaging and direct Cargo commands.

Install without sudo:
  python3 -m venv target/build-tools
  target/build-tools/bin/python -m pip install ninja==1.13.0 cmake==3.31.6
Run Cargo with the same environment as the app scripts:
  python3 scripts/build_env.py check --locked --all-targets
"""
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def build_environment():
    env = os.environ.copy()
    local_bin = ROOT / "target/build-tools/bin"
    if local_bin.is_dir():
        env["PATH"] = str(local_bin) + os.pathsep + env.get("PATH", os.defpath)
    # Respect explicit overrides, including custom wrappers. rust-skia reads this
    # variable directly when its exact prebuilt archive is unavailable.
    ninja = shutil.which("ninja", path=env.get("PATH"))
    if ninja and "SKIA_NINJA_COMMAND" not in env:
        env["SKIA_NINJA_COMMAND"] = ninja
    return env


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    sys.exit(subprocess.call(["cargo", *sys.argv[1:]], cwd=ROOT, env=build_environment()))
