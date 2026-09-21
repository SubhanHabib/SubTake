"""HyperFrames adapter: editable source first, movie only on explicit export."""
import hashlib
import html
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import time
import urllib.request

HERE = Path(__file__).resolve().parent
CLI = HERE/'node_modules/.bin/hyperframes'

def command(args, cwd, timeout=180):
    env = dict(os.environ, HYPERFRAMES_SKIP_SKILLS='1', DO_NOT_TRACK='1')
    # GUI-launched applications may inherit only the minimal macOS PATH.
    env['PATH'] = '/opt/homebrew/bin:/usr/local/bin:' + env.get('PATH', '/usr/bin:/bin')
    result = subprocess.run([str(CLI), *map(str,args)], cwd=cwd, env=env,
                            capture_output=True, text=True, timeout=timeout)
    if result.returncode:
        raise ValueError((result.stderr + '\n' + result.stdout)[-6000:])
    return result.stdout

def digest(project):
    # Bind export approval to authored source and assets, excluding Studio caches.
    checksum = hashlib.sha256()
    for path in sorted(project.rglob('*')):
        relative = path.relative_to(project)
        if not path.is_file() or any(p.startswith('.') or p in {'node_modules','renders','snapshots'} for p in relative.parts):
            continue
        checksum.update(str(relative).encode())
        with path.open('rb') as stream:
            while chunk := stream.read(1024*1024): checksum.update(chunk)
    return checksum.hexdigest()

def preview(project):
    result = command(['preview', project, '--background', '--no-open', '--json'], project)
    # Current CLI may print notices before the lifecycle JSON.
    decoder = json.JSONDecoder(); payload = None
    for index, char in enumerate(result):
        if char == '{':
            try:
                candidate, _ = decoder.raw_decode(result[index:])
                if isinstance(candidate, dict) and ('url' in candidate or 'server' in candidate or 'result' in candidate): payload = candidate; break
            except ValueError: pass
    if payload is None:
        raise ValueError('HyperFrames did not return a preview URL: '+result[-2000:])
    url = payload.get('result',{}).get('studioUrl') or payload.get('url') or payload.get('server',{}).get('url')
    if not isinstance(url,str) or not url.startswith(('http://localhost:', 'http://127.0.0.1:')):
        raise ValueError('Unexpected HyperFrames preview URL: '+str(payload))
    urllib.request.urlopen(url.split('#')[0], timeout=10).close()
    if '#project/' not in url: url += '#project/'+project.name
    return url

def player(project):
    receipt=project/'.subtake-player.json'
    if receipt.exists():
        saved=json.loads(receipt.read_text())
        try:
            body=urllib.request.urlopen(saved['url'],timeout=2).read().decode()
            if '<hyperframes-player ' in body: return saved['url']
        except (OSError,ValueError): pass
    with socket.socket() as sock:
        sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
    env=dict(os.environ,DO_NOT_TRACK='1')
    env['PATH']='/opt/homebrew/bin:/usr/local/bin:'+env.get('PATH','/usr/bin:/bin')
    with (project/'.subtake-player.log').open('a') as log:
        process=subprocess.Popen([str(CLI),'play',str(project),'--port',str(port),'--no-open'],
                                 cwd=project,env=env,stdout=log,stderr=log,start_new_session=True)
    url=f'http://127.0.0.1:{port}'
    for _ in range(100):
        if process.poll() is not None: raise ValueError('HyperFrames player failed; see .subtake-player.log')
        try:
            body=urllib.request.urlopen(url,timeout=.3).read().decode()
            if '<hyperframes-player ' in body:
                receipt.write_text(json.dumps(dict(url=url,pid=process.pid)))
                return url
        except OSError: pass
        time.sleep(.1)
    process.terminate()
    raise ValueError('HyperFrames player did not start')

def generate(work, data, output):
    if not CLI.exists(): raise ValueError('Install the pinned spike dependencies with npm ci --prefix spikes/storyboard')
    project = output/'composition'
    command(['init', project, '--non-interactive', '--example', 'blank'], output)
    (project/'assets').mkdir(exist_ok=True)
    shutil.copy2(HERE/'node_modules/gsap/dist/gsap.min.js', project/'assets/gsap.min.js')
    assets = {a['id']:a for a in data['assets']}
    elements = []; mapping = []; posters = []; cursor = 0
    for index, scene in enumerate(data['plan']['scenes']):
        if scene.get('zoom'): raise ValueError('HyperFrames spike does not yet translate native zoom regions; remove the zoom from this plan')
        asset = assets[scene['asset_id']]
        source = (work/asset['path']).resolve()
        if not source.is_relative_to(work): raise ValueError('Asset escapes workspace')
        dest = project/'assets'/source.name
        shutil.copy2(source, dest)
        timing = f'data-start="{cursor}" data-duration="{scene["duration"]}" data-track-index="0"'
        if asset['kind']=='video':
            elements.append(f'<video id="footage-{index}" class="clip footage" src="assets/{html.escape(dest.name,quote=True)}" {timing} data-media-start="{scene.get("start",0)}" muted playsinline></video>')
        else:
            elements.append(f'<img id="footage-{index}" class="clip footage" src="assets/{html.escape(dest.name,quote=True)}" {timing} alt="">')
        elements.append(f'<h1 id="caption-{index}" class="clip caption" data-start="{cursor}" data-duration="{scene["duration"]}" data-track-index="1">{html.escape(scene.get("caption") or scene["title"])}</h1>')
        mapping.append(dict(scene_id=scene['id'],asset_id=asset['id'],source_start=scene.get('start',0),output_start=cursor,output_end=cursor+scene['duration']))
        poster=output/f'scene-{index:02d}.jpg'
        ffmpeg=HERE.parents[1]/'dist/SubTake.app/Contents/Resources/bin/ffmpeg'
        args=[str(ffmpeg),'-v','error','-y']
        if asset['kind']=='video': args += ['-ss',str(scene.get('start',0)+scene['duration']/2)]
        args += ['-i',str(source),'-frames:v','1','-vf','scale=960:540:force_original_aspect_ratio=decrease,pad=960:540:(ow-iw)/2:(oh-ih)/2',str(poster)]
        subprocess.run(args,check=True,capture_output=True,timeout=30)
        posters.append(str(poster.relative_to(work))); cursor += scene['duration']
    (project/'index.html').write_text('''<!doctype html><html lang="en"><head><meta charset="utf-8"><title>SubTake feature story</title><script src="assets/gsap.min.js"></script><style>
    *{box-sizing:border-box}body{margin:0;background:#f5f3ef;color:#252525;font-family:Arial,sans-serif}#root{position:relative;width:1280px;height:720px;overflow:hidden;background:#f5f3ef}.footage{position:absolute;left:40px;top:30px;width:1200px;height:594px;object-fit:contain;background:#e8e5df;border-radius:18px}.caption{position:absolute;left:48px;bottom:24px;width:1184px;height:54px;display:flex;align-items:center;gap:18px}.number{font-size:18px;color:#58564f}h1{font-size:30px;margin:0;font-weight:600}
    </style></head><body><div id="root" data-composition-id="main" data-width="1280" data-height="720" data-duration="'''+str(cursor)+'">'+''.join(elements)+'''</div><script>window.__timelines["main"]=gsap.timeline({paused:true});</script></body></html>''')
    (project/'BRIEF.md').write_text('# SubTake agent video integration test\n\n'+data['brief']+'\n\nSilent technical spike using real captured UI. Story approved in chat. Live review before final export.\n')
    # Full engine check catches missing media, timing, runtime and layout problems.
    checked = command(['check', project], project, timeout=300)
    (output/'check.log').write_text(checked)
    url = preview(project)
    return dict(engine='hyperframes',version='0.8.55',revision=data['revision'],generation=output.name,
                project=str(project.relative_to(work)),preview_url=url,player_url=player(project),posters=posters,
                duration=cursor,scene_mapping=mapping,source_sha256=digest(project))

def export(work, report, expected_hash):
    project = work/report['project']
    if digest(project) != expected_hash: raise ValueError('Composition changed; review the latest preview before exporting')
    # Render an immutable copy so later Studio edits cannot change the approved output.
    target = work/'exports'/('export-'+str(time.time_ns()))
    frozen = target/'composition'
    shutil.copytree(project, frozen, ignore=shutil.ignore_patterns('.hyperframes','node_modules','renders','snapshots'))
    if digest(frozen) != expected_hash: raise ValueError('Composition changed during snapshot; review again')
    command(['check', frozen], frozen, timeout=300)
    output = target/'video.mp4'
    command(['render', frozen, '--quality','draft','--output',output, '--workers','2'], frozen, timeout=900)
    if not output.exists() or output.stat().st_size == 0: raise ValueError('HyperFrames produced no movie')
    return str(output.relative_to(work))
