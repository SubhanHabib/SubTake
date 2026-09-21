"""Check controller callbacks, native snapshots and visible UI labels; not hit targets."""
import datetime, hashlib, json, os, pathlib, subprocess
root=pathlib.Path(__file__).resolve().parents[1];out=root/'test-output';app=root/'dist/SubTake.app/Contents/MacOS/SubTake'
cases=[('final','Frame','1360x880','en'),('minimum-final','Frame','980x680','en'),('french-final','Cursor','980x680','fr'),('crop-final','Crop','980x680','en'),('background-final','Wallpapers','1360x880','en'),('export-final','Export','980x680','en'),('presets-final','Presets','980x680','en'),('captions-final','Captions','980x680','en'),('settings-final','Preferences','980x680','en'),('recording-final','Recording','980x680','en'),('empty-final','Recording','1360x880','en')]
prefs=pathlib.Path.home()/'Library/Application Support/com.SubTake.SubTake-Native/preferences.json';before=prefs.read_bytes() if prefs.exists() else None
results=[]
for name,panel,size,language in cases:
    log=out/f'gpui-ui-{name}.log';snapshot=out/f'gpui-editor-{name}.png'
    snapshot.unlink(missing_ok=True)
    env=dict(os.environ,PATH='/usr/bin:/bin',SUBTAKE_UI_SNAPSHOT=str(snapshot),SUBTAKE_UI_PANEL=panel,SUBTAKE_UI_SIZE=size,SUBTAKE_UI_LANGUAGE=language)
    if panel=='Recording': env['SUBTAKE_UI_EMPTY']='1'
    with log.open('w') as file:subprocess.run([str(app),str(out/'fixture.recordly')],env=env,stdout=file,stderr=subprocess.STDOUT,check=True,timeout=60)
    assert 'UI_SMOKE_PASSED: controller callbacks and native snapshot; pointer hit testing not exercised' in log.read_text(),log
    assert snapshot.is_file() and snapshot.stat().st_size > 0, snapshot
    ocr_path=out/f'gpui-ui-{name}-ocr.json'
    ocr=subprocess.run(['swift',str(root/'scripts/verify-ui-text.swift'),str(snapshot),'--language',language],capture_output=True,text=True,timeout=120)
    ocr_path.write_text(ocr.stdout)
    if ocr.returncode:
        raise RuntimeError(f'Visible UI text check failed for {snapshot}: {ocr.stderr.strip()}; evidence: {ocr_path}')
    ocr_result=json.loads(ocr.stdout)
    results.append(dict(panel=panel,size=size,language=language,status='passed',log=str(log.relative_to(root)),snapshot=str(snapshot.relative_to(root)),visible_ui_text=ocr_result,ocr_report=str(ocr_path.relative_to(root))))
    print(panel,size,language,'passed',flush=True)
assert before==(prefs.read_bytes() if prefs.exists() else None),'User preferences changed during smoke tests'
report=dict(ui_backend='gpui',coverage='controller callbacks, native snapshots and Vision OCR of editor labels; no pointer/keyboard/gesture coverage',recorded_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),binary_sha256=hashlib.sha256(app.read_bytes()).hexdigest(),cases=results,user_preferences_unchanged=True,checks=['controller callbacks select the intended panel; no pointer hit testing','recording discovery clears busy on success, failure, empty result and cancellation','refresh preserves selected source','Record opens configuration without starting capture','empty recording layout','editing and caption/history commands','selector preserves nondefault stored value','linked and independent padding with undo','crop preview and undo','folder search','wallpaper thumbnail readiness','snapshot of the selected panel','Vision OCR finds at least 3 distinct localized editor labels in header/rail; video fixture text excluded'])
(root/'docs/gpui-ui-validation.json').write_text(json.dumps(report,indent=2)+'\n')
print(f'All {len(cases)} packaged UI checks passed; preferences unchanged',flush=True)
