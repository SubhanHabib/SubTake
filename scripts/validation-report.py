"""Collect historical pre-GPUI evidence only; never certify the migrated UI."""
import hashlib,json,pathlib,re,subprocess,datetime
root=pathlib.Path(__file__).resolve().parents[1];out=root/'test-output'
if (root/'src/ui_runtime.rs').exists():
    raise SystemExit('Legacy validation aggregation is retired for GPUI: its logs and JSON inputs predate the migration. Follow docs/GPUI-MIGRATION.md and collect fresh evidence; docs/validation.json will not be overwritten.')
def read(name):return json.loads((out/name).read_text())
def command(*args):return subprocess.check_output(args,cwd=root,text=True).strip()
tests=(out/'tests-final.log').read_text();counts=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed',tests)
assert counts and all(int(failed)==0 for _,failed in counts) and 'FAILED' not in tests and not re.search(r'^error:',tests,re.M),'Test log is not successful'
files=sorted([*root.glob('src/*.rs'),*root.glob('ui/**/*.slint'),*root.glob('examples/*.rs'),*root.glob('scripts/*.c'),*root.glob('scripts/*.m'),*root.glob('scripts/*.swift'),*root.glob('scripts/*.py'),*root.glob('scripts/*.mjs'),root/'Cargo.toml',root/'Cargo.lock',root/'build.rs',root/'assets/localization.json',*root.glob('assets/icons/*'),*root.glob('assets/wallpaper-thumbnails/**/*')])
files=[p for p in files if p.is_file()]
hashes={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
app=root/'dist/SubTake.app/Contents/MacOS/SubTake'
subprocess.run(['codesign','--verify','--deep','--strict',str(root/'dist/SubTake.app')],check=True)
paths=[app,*list((root/'dist/SubTake.app/Contents/Resources/bin').iterdir())];bad=[]
for p in paths:
    if p.is_file():
        for line in command('otool','-L',str(p)).splitlines()[1:]:
            dep=line.strip().split(' (')[0]
            if dep.startswith(('/opt/homebrew/','/usr/local/')):bad.append({'binary':str(p),'dependency':dep})
assert not bad,bad
corpus=json.loads((root/'tests/reference-fixtures.json').read_text())
report={
 'recorded_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
 'reference_commit':command('git','rev-parse','HEAD'),
 'host':{'model':command('sysctl','-n','machdep.cpu.brand_string'),'memory_bytes':int(command('sysctl','-n','hw.memsize')),'macos':command('sw_vers','-productVersion'),'macos_build':command('sw_vers','-buildVersion'),'architecture':command('uname','-m')},
 'tests':{'passed':sum(int(n) for n,_ in counts),'failed':0,'reference_cases':{k:len(v) for k,v in corpus.items()}},
 'ui_redesign':json.loads((root/'docs/ui-validation.json').read_text()),
 'ui_primitives':json.loads((root/'docs/primitives-validation.json').read_text()),
 'ui_smoke':{name:('UI_SMOKE_PASSED' in (out/name).read_text()) for name in ['ui-final.log','ui-minimum-final.log','ui-french-final.log','ui-crop-final.log']},
 'media_smoke':read('smoke-results.json'),
 'videotoolbox':read('videotoolbox-results.json'),
 'audio_device_smoke':{'status':'passed' if '1 passed; 0 failed' in (out/'audio-device-final.log').read_text() else 'failed','signal':'silence','checks':['real default output device','audio-master clock progression','bounded cancellation without jumping to end']},
 'transcription':{'source':'generated speech, microphone sidecar, no embedded audio','model':'Whisper Small','cues':read('transcription-final.json')},
 'benchmarks':{name:read(name) for name in ['benchmark-native-540p.json','benchmark-native-1080p.json','benchmark-native-4k-source.json']},
 'prior_subprocess_benchmark':read('benchmark-540p.json'),
 'package':{'main_sha256':hashlib.sha256(app.read_bytes()).hexdigest(),'disk_usage':command('du','-sh',str(root/'dist/SubTake.app')).split()[0],'signature':'ad-hoc, deep/strict verification passed','bundled_binaries_and_libraries_checked':len(paths),'external_homebrew_dependencies':bad},
 'source_sha256':hashes,
 'screen_capture_helper':read('window-capture-ui-fixes.json'),
 'recorder_lifecycle':json.loads((root/'docs/launcher-validation.json').read_text()),
 'previous_capture_lifecycle':json.loads((root/'docs/capture-validation-before-root-move.json').read_text()),
 'not_validated':['real microphone/camera/system-audio recording integration','long-session A/V drift and playback device changes','full visual parity against Electron','macOS 14, stable macOS, Intel Mac','Windows or Linux build/runtime','Developer ID signing, notarization, public distribution/updater'],
}
idle=out/'idle-final.json'
if idle.exists():report['idle_process_sample']=read('idle-final.json')
assert all(report['ui_smoke'].values()),'A UI smoke log did not pass'
(root/'docs/validation.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({k:report[k] for k in ['recorded_at','host','tests','ui_smoke','package']},indent=2))
