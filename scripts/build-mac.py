"""Build a self-contained local .app; pass --release for optimized Rust code.

Signing uses an ad-hoc identity by default. Developer ID signing and notarization
are distribution steps and are not claimed by this local build.
"""
import argparse, os, platform, plistlib, shutil, subprocess, tempfile
from pathlib import Path
from build_env import build_environment
from bundle_resources import copy_ui_assets, verify_ui_assets

parser=argparse.ArgumentParser()
parser.add_argument("--release",action="store_true")
parser.add_argument("--identity",default="-")
args=parser.parse_args()
root=Path(__file__).resolve().parents[1]
repo=root/"legacy-electron"
def run(*cmd): subprocess.run([str(c) for c in cmd],check=True,cwd=root,env=build_environment())
run("cargo","build","--locked","--bin","subtake-native",*(["--release"] if args.release else []))
run("python3",root/"scripts/build-icons.py")
destination=root/"dist"/"SubTake.app"
destination.parent.mkdir(parents=True,exist_ok=True)
staging=tempfile.TemporaryDirectory(prefix=".build-",dir=destination.parent)
app=Path(staging.name)/"SubTake.app"
contents=app/"Contents";macos=contents/"MacOS";resources=contents/"Resources";bins=resources/"bin"
macos.mkdir(parents=True,exist_ok=True);bins.mkdir(parents=True,exist_ok=True)
shutil.copy2(root/"target"/("release" if args.release else "debug")/"subtake-native",macos/"SubTake")
info={"CFBundleName":"SubTake","CFBundleDisplayName":"SubTake","CFBundleExecutable":"SubTake","CFBundleIdentifier":"com.subtake.native","CFBundleVersion":"1","CFBundleShortVersionString":"0.1.0","CFBundlePackageType":"APPL","LSMinimumSystemVersion":"14.0","NSHighResolutionCapable":True,"NSMicrophoneUsageDescription":"SubTake records microphone audio when you enable it for a recording.","NSCameraUsageDescription":"SubTake records your camera when you enable the webcam overlay.","NSPrincipalClass":"NSApplication","CFBundleDocumentTypes":[{"CFBundleTypeName":"SubTake Project","CFBundleTypeRole":"Editor","CFBundleTypeExtensions":["recordly","openscreen"]}]}
info["CFBundleIconFile"]="SubTake.icns"
info["LSUIElement"]=True
shutil.copy2(root/"assets/branding/SubTake.icns",resources/"SubTake.icns")
shutil.copy2(repo/"LICENSE.md",resources/"LICENSE.md")
(resources/"licenses").mkdir(exist_ok=True)
shutil.copy2(root/"assets/icons/LICENSE",resources/"licenses/Phosphor.txt")
shutil.copy2(root/"assets/fonts/licenses/Geist-OFL.txt",resources/"licenses/Geist-OFL.txt")
(resources/"NOTICE.md").write_text("SubTake Native contains code adapted from Recordly and OpenScreen, including motion work by @webadderall. Its presentation layer (theme tokens, control metrics and hover motion) is adapted from Zeron (github.com/zeronsh/zeron), MIT (c) 2026 Wing. Bundled interface fonts are Geist and Geist Mono, (c) 2024 The Geist Project Authors, SIL Open Font License 1.1 (see licenses/Geist-OFL.txt). The application is licensed under AGPL-3.0-only. Runtime components retain their own licenses. See the repository-root Rust source and Cargo.lock for dependency versions.\n")
(contents/"Info.plist").write_bytes(plistlib.dumps(info))
run("xcrun","swiftc","-O","-target",f"{platform.machine()}-apple-macos14.0",root/"scripts"/"platform.swift","-o",bins/"subtake-platform")
run("xcrun","swiftc","-O","-target",f"{platform.machine()}-apple-macos14.0",root/"scripts"/"companion.swift","-o",bins/"subtake-companion")
run("xcrun","swiftc","-O","-target",f"{platform.machine()}-apple-macos14.0",root/"scripts"/"ScreenCaptureKitRecorder.swift","-o",bins/"recordly-screencapturekit-helper")
arch="arm64" if platform.machine()=="arm64" else "x64"
helpers=repo/"electron"/"native"/"bin"/f"darwin-{arch}"
for name in ["recordly-native-cursor-monitor","recordly-system-cursors","whisper-cli"]:
    if (helpers/name).is_file():shutil.copy2(helpers/name,bins/name)
for source,dest in [(repo/"public"/"wallpapers",resources/"public"/"wallpapers"),(repo/"src"/"assets"/"cursors",resources/"src"/"assets"/"cursors")]:
    shutil.copytree(source,dest,dirs_exist_ok=True)
shutil.copytree(root/"assets/wallpaper-thumbnails",resources/"assets/wallpaper-thumbnails",dirs_exist_ok=True)
copy_ui_assets(root, resources)
print(f"Verified {verify_ui_assets(root, resources)} packaged GPUI icon/branding resources")
for name in ["ffmpeg","ffprobe"]:
    executable=os.environ.get("SUBTAKE_"+name.upper()) or shutil.which(name)
    if not executable:raise RuntimeError(f"Install {name} or set SUBTAKE_{name.upper()}")
    shutil.copy2(executable,bins/name)

# Copy the full Homebrew dylib dependency closure and relocate it inside the bundle.
# No copied executable may depend on the developer's Homebrew prefix afterwards.
seen=set()
def bundle_libraries(path):
    if path in seen:return
    seen.add(path)
    listing=subprocess.check_output(["otool","-L",str(path)],text=True)
    for line in listing.splitlines()[1:]:
        dep=line.strip().split(" (",1)[0]
        if not dep.startswith(("/opt/homebrew/","/usr/local/")):continue
        target=bins/Path(dep).name
        if target!=path:
            if not target.exists():shutil.copy2(dep,target)
            bundle_libraries(target)
        run("install_name_tool","-change",dep,("@executable_path/../Resources/bin/" if path.parent==macos else "@loader_path/")+target.name,path)
    if path.suffix==".dylib":run("install_name_tool","-id","@loader_path/"+path.name,path)
bundle_libraries(macos/"SubTake")
for executable in list(bins.iterdir()):
    if executable.is_file():bundle_libraries(executable)
for executable in bins.iterdir():
    if executable.is_file():run("codesign","--force","--sign",args.identity,executable)
run("codesign","--force","--sign",args.identity,app)
run("codesign","--verify","--deep","--strict",app)
previous=Path(staging.name)/"previous.app"
if destination.exists(): destination.rename(previous)
try: app.rename(destination)
except Exception:
    if previous.exists(): previous.rename(destination)
    raise
staging.cleanup()
print(destination)
