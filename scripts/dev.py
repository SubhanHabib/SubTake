"""Run SubTake with debounced, single-instance rebuilds after source saves.

python3 scripts/dev.py          # build, launch, watch
python3 scripts/dev.py --once   # build and launch, no watching
python3 scripts/dev.py --stop   # ask the running supervisor/app to stop
"""
import argparse
import fcntl
import os
from pathlib import Path
import plistlib
import signal
import subprocess
import sys
import time
from build_env import build_environment
from bundle_resources import verify_ui_assets

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / "target/dev"
APP = WORK / "SubTake.app"
REQUEST = WORK / "restart-request"
LOCK = WORK / "watch.lock"
HELPERS = {
    "platform.swift": "subtake-platform",
    "companion.swift": "subtake-companion",
    "ScreenCaptureKitRecorder.swift": "recordly-screencapturekit-helper",
}
stopping = False


def log(message):
    print(f"[SubTake dev] {message}", flush=True)


def run(*args, **kwargs):
    subprocess.run([str(a) for a in args], cwd=ROOT, check=True, **kwargs)


def snapshot():
    files = [ROOT / p for p in ("Cargo.toml", "Cargo.lock", "build.rs")]
    for directory in ("src", "crates/theme", "crates/ui", "scripts", "assets"):
        files.extend(p for p in (ROOT / directory).rglob("*")
                     if p.is_file() and "__pycache__" not in p.parts
                     and p.suffix in {".rs", ".m", ".h", ".c", ".swift", ".metal",
                                      ".py", ".svg", ".png", ".jpg", ".jpeg", ".webp",
                                      ".json", ".toml", ".ttf", ".otf"})
    result = {}
    for path in files:
        try:
            info = path.stat()
            result[str(path)] = (info.st_mtime_ns, info.st_size)
        except FileNotFoundError:
            pass
    return result


def prepare():
    # Compile while the last working instance stays open. Failed builds never
    # replace it. Build outputs are outside the watched paths.
    run("python3", ROOT / "scripts/build-icons.py")
    run("cargo", "build", "--locked", "--bin", "subtake-native", env=build_environment())
    resources = APP / "Contents/Resources"
    bins = resources / "bin"
    bins.mkdir(parents=True, exist_ok=True)
    legacy = ROOT / "legacy-electron"
    packaged = ROOT / "dist/SubTake.app/Contents/Resources"
    # Development uses linked resources; release packaging remains self-contained.
    for name, source in (("public", legacy / "public"), ("src", legacy / "src"),
                         ("assets", ROOT / "assets")):
        link = resources / name
        if link.is_symlink() and link.resolve() != source.resolve():
            # Repair only a development resource link, never a real directory.
            link.unlink()
        if not link.exists():
            link.symlink_to(source, target_is_directory=True)
    # The assets link includes icons/ and branding/ at the same paths used by
    # GPUI in the release bundle. Reject stale real directories/missing assets.
    verify_ui_assets(ROOT, resources, development=True)
    for source in (packaged / "bin").glob("*"):
        dest = bins / source.name
        if not dest.exists():
            dest.symlink_to(source)
    for source_name, binary_name in HELPERS.items():
        source = ROOT / "scripts" / source_name
        dest = bins / binary_name
        if not dest.exists() or source.stat().st_mtime_ns > dest.stat().st_mtime_ns:
            pending = bins / (binary_name + ".pending")
            run("xcrun", "swiftc", "-O", "-target",
                f"{os.uname().machine}-apple-macos14.0", source, "-o", pending)
            # Replace only this dev helper/link, never the packaged helper.
            pending.replace(dest)
    info = {
        "CFBundleName": "SubTake", "CFBundleDisplayName": "SubTake",
        "CFBundleIdentifier": "com.subtake.native",
        "CFBundleExecutable": "SubTake", "CFBundlePackageType": "APPL",
        "CFBundleVersion": str(time.time_ns()), "CFBundleShortVersionString": "0.1.0",
        "CFBundleIconFile": "SubTake.icns", "LSUIElement": True,
        "NSHighResolutionCapable": True, "LSMinimumSystemVersion": "14.0",
        "NSPrincipalClass": "NSApplication",
        "NSMicrophoneUsageDescription": "SubTake records microphone audio when enabled.",
        "NSCameraUsageDescription": "SubTake records the camera when enabled.",
    }
    (APP / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
    return resources


def processes():
    lines = subprocess.check_output(
        ["ps", "-axo", "pid=,ppid=,comm="], text=True
    ).splitlines()
    result = {}
    for line in lines:
        parts = line.strip().split(None, 2)
        if len(parts) == 3:
            result[int(parts[0])] = (int(parts[1]), parts[2])
    return result


def close_old_instances():
    table = processes()
    targets = {pid for pid, (_, command) in table.items()
               if Path(command).name in {"subtake-native", "SubTake"}}
    # Also clean orphaned SubTake helpers from earlier manually interrupted runs.
    targets.update(pid for pid, (_, command) in table.items()
                   if Path(command).name in {"subtake-platform", "subtake-companion"}
                   and str(ROOT) in command)
    while True:
        children = {pid for pid, (parent, _) in table.items() if parent in targets}
        if children <= targets:
            break
        targets |= children
    if not targets:
        return
    log(f"Closing previous app/helper processes: {sorted(targets)}")
    for pid in targets:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        remaining = targets & processes().keys()
        if not remaining:
            return
        time.sleep(.1)
    raise RuntimeError(f"Previous instances still running: {sorted(remaining)}. Close them and retry.")


def install_and_launch(resources, gallery=False):
    import shutil
    macos = APP / "Contents/MacOS"
    macos.mkdir(exist_ok=True)
    executable = macos / "SubTake"
    shutil.copy2(ROOT / "target/debug/subtake-native", executable)
    shutil.copy2(ROOT / "assets/branding/SubTake.icns", resources / "SubTake.icns")
    run("codesign", "--force", "--sign", "-", APP)
    REQUEST.unlink(missing_ok=True)
    environment = dict(os.environ, SUBTAKE_RESOURCES=str(resources),
                       SUBTAKE_DEV_RESTART_FILE=str(REQUEST))
    if gallery:
        environment["SUBTAKE_GALLERY"] = gallery
    child = subprocess.Popen([str(executable)], cwd=ROOT, env=environment,
                             start_new_session=True)
    log(f"Running fresh build — PID {child.pid} — {APP}")
    return child


def graceful_stop(child):
    if child.poll() is not None:
        return True
    REQUEST.touch()
    log("Restart requested; waiting for recording/export to finish. Save/discard any unsaved project when prompted.")
    consumed = None
    while child.poll() is None:
        if not REQUEST.exists():
            consumed = consumed or time.monotonic()
            if time.monotonic() - consumed > 2:
                # The user cancelled the quit dialog, or a quit was refused.
                log("App kept open. The next source save will retry.")
                return False
        time.sleep(.2)
    # The app has exited normally; clean only its own remaining process group.
    try:
        os.killpg(child.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    REQUEST.unlink(missing_ok=True)
    return True


def main():
    global stopping
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--once", action="store_true")
    parser.add_argument("--stop", action="store_true")
    parser.add_argument("--gallery", nargs="?", const="1", default=None,
                        metavar="APPEARANCE",
                        help="launch the UI-only gallery (fixture data, no project); "
                             "pass 'light' to start in light mode")
    args = parser.parse_args()
    WORK.mkdir(parents=True, exist_ok=True)
    with LOCK.open("a+") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            lock.seek(0)
            pid = int(lock.read())
            if args.stop:
                command = subprocess.check_output(["ps", "-p", str(pid), "-o", "command="], text=True)
                if "scripts/dev.py" not in command:
                    raise RuntimeError("Supervisor identity changed; refusing to signal it.")
                os.kill(pid, signal.SIGTERM)
                log(f"Stop requested for supervisor {pid}.")
                return
            raise RuntimeError("Dev mode is already running. Use python3 scripts/dev.py --stop first.")
        if args.stop:
            log("No development supervisor is running.")
            return
        lock.seek(0)
        lock.truncate()
        lock.write(str(os.getpid()))
        lock.flush()

        def request_stop(*_):
            global stopping
            stopping = True
        signal.signal(signal.SIGINT, request_stop)
        signal.signal(signal.SIGTERM, request_stop)
        child = None
        try:
            baseline = snapshot()
            resources = prepare()
            close_old_instances()
            child = install_and_launch(resources, args.gallery)
            log("Watching src/, crates/theme/, crates/ui/, scripts/, assets/ and Cargo files. Ctrl+C stops. Compile errors keep the last working app.")
            while not stopping:
                if child.poll() is not None:
                    log("App exited; supervisor stopped.")
                    break
                time.sleep(.5)
                current = snapshot()
                if args.once or current == baseline:
                    continue
                # Coalesce editor atomic saves and several files saved together.
                time.sleep(.4)
                baseline = snapshot()
                log("Saved source changed — rebuilding…")
                try:
                    resources = prepare()
                except subprocess.CalledProcessError:
                    log("Build failed. Fix and save again; current app remains open.")
                    continue
                if stopping:
                    break
                if graceful_stop(child):
                    child = install_and_launch(resources, args.gallery)
        finally:
            if child is not None and child.poll() is None:
                if not graceful_stop(child):
                    log("Stop cancelled in app; supervisor leaving the current app open.")
            REQUEST.unlink(missing_ok=True)


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError) as error:
        log(str(error))
        sys.exit(1)
