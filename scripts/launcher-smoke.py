"""Packaged recorder lifecycle checks. Optional capture uses only our generated fixture window."""
import argparse, datetime, hashlib, json, os, pathlib, subprocess
parser=argparse.ArgumentParser()
parser.add_argument('--capture-fixture',action='store_true')
args=parser.parse_args()
root=pathlib.Path(__file__).resolve().parents[1]
app=root/'dist/SubTake.app/Contents/MacOS/SubTake'
out=root/'test-output/launcher';out.mkdir(parents=True,exist_ok=True)
prefs=pathlib.Path.home()/'Library/Application Support/com.SubTake.SubTake-Native/preferences.json'
before=prefs.read_bytes() if prefs.exists() else None
cases=[]
for mode in ['idle','sources','audio','camera','countdown','more']+(['capture'] if args.capture_fixture else []):
    env=dict(os.environ,PATH='/usr/bin:/bin',SUBTAKE_LAUNCHER_SMOKE=mode,SUBTAKE_LAUNCHER_TEST_DIRECTORY=str(out))
    if mode=='capture':
        sources=json.loads(subprocess.check_output([str(root/'dist/SubTake.app/Contents/Resources/bin/subtake-platform'),'sources'],timeout=80))
        candidates=[s for s in sources if s['kind']=='window' and s['name']=='SubTake — SubTake Capture Fixture']
        assert len(candidates)==1,'Open the generated SubTake Capture Fixture window first; other windows will not be recorded'
        env['SUBTAKE_CAPTURE_SMOKE_WINDOW']=str(candidates[0]['nativeId'])
    log=out/f'{mode}.log'
    with log.open('w') as file:subprocess.run([str(app)],env=env,stdout=file,stderr=subprocess.STDOUT,check=True,timeout=65)
    assert 'LAUNCHER_SMOKE_PASSED' in log.read_text(),log
    assert 'RECORDER_NATIVE_GLASS_INSTALLED' in log.read_text(), 'Native recorder material missing: '+str(log)
    cases.append(dict(mode=mode,status='passed',log=str(log.relative_to(root))))
    print(mode,'passed',flush=True)
assert before==(prefs.read_bytes() if prefs.exists() else None),'User preferences changed during launcher checks'
report=dict(recorded_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),binary_sha256=hashlib.sha256(app.read_bytes()).hexdigest(),cases=cases,user_preferences_unchanged=True)
if args.capture_fixture:
    report['capture']=json.loads((out/'capture-lifecycle.json').read_text())
    video=report['capture']['source'];checks=[]
    for second in [.25, .75, 1.5, 2.5]:
        pixels=subprocess.check_output([str(root/'dist/SubTake.app/Contents/Resources/bin/ffmpeg'),'-v','error','-ss',str(second),'-i',video,'-frames:v','1','-vf','scale=160:100','-pix_fmt','rgb24','-f','rawvideo','pipe:1'],timeout=20)
        assert len(pixels)==160*100*3,'Missing captured frame'
        colors=list(zip(pixels[0::3],pixels[1::3],pixels[2::3]))
        blue=sum(b>g+20 and b>r+30 and r<100 for r,g,b in colors)/len(colors)
        red=sum(r>g+40 and r>b+40 for r,g,b in colors)/len(colors)
        assert blue>.65 and red<.015, f'Window capture contains covering content at {second}s: blue={blue}, red={red}'
        checks.append(dict(time=second,blue_fraction=blue,red_fraction=red))
    report['capture']['occlusion_checks']=checks
    report['capture']['helper_sha256']=hashlib.sha256((root/'dist/SubTake.app/Contents/Resources/bin/recordly-screencapturekit-helper').read_bytes()).hexdigest()
    print('Independent-window content checks passed',flush=True)
(root/'docs/launcher-validation.json').write_text(json.dumps(report,indent=2)+'\n')
