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


# macOS keys a Screen Recording grant to the signature it was given to. An
# ad-hoc signature changes with every build, so each rebuild loses the grant;
# a certificate that stays the same keeps it.
LOCAL_IDENTITY = "SubTake Local"


def signing_identity():
    """SUBTAKE_SIGN_IDENTITY, else the "SubTake Local" certificate when the
    keychain has it, else ad-hoc."""
    if os.environ.get("SUBTAKE_SIGN_IDENTITY"):
        return os.environ["SUBTAKE_SIGN_IDENTITY"]
    found = subprocess.run(["security", "find-certificate", "-c", LOCAL_IDENTITY],
                           capture_output=True).returncode == 0
    return LOCAL_IDENTITY if found else "-"


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    sys.exit(subprocess.call(["cargo", *sys.argv[1:]], cwd=ROOT, env=build_environment()))
