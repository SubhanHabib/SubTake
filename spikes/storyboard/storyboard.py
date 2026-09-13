#!/usr/bin/env python3
"""Agent-driven storyboard experiment; Python standard library plus bundled SubTake tools."""
import argparse
import contextlib
import copy
import fcntl
import hashlib
import http.server
import json
import math
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys
import threading
import time
import urllib.parse
import urllib.request
import uuid

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
APP = ROOT / 'dist/SubTake.app'
BINARY = APP / 'Contents/MacOS/SubTake'
BINS = APP / 'Contents/Resources/bin'
IMAGE_EXT = {'.png', '.jpg', '.jpeg', '.webp'}

def run(args):
    result = subprocess.run([str(x) for x in args], capture_output=True, text=True)
    if result.returncode:
        raise ValueError(result.stderr[-2000:] or result.stdout[-2000:])
    return result.stdout

def read(path):
    return json.loads(path.read_text())

def write(path, data):
    temp = path.with_name(path.name + '.tmp-' + uuid.uuid4().hex)
    temp.write_text(json.dumps(data, indent=2) + '\n')
    os.replace(temp, path)

@contextlib.contextmanager
def locked(work):
    with (work / '.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        yield

def state(work):
    return read(work / 'storyboard.json')

def probe(path):
    return json.loads(run([BINS/'ffprobe', '-v', 'error', '-show_streams', '-show_format', '-of', 'json', path]))

def init(work, brief, assets):
    work.mkdir(parents=True, exist_ok=True)
    if any(work.iterdir()):
        raise ValueError('Use an empty workspace; existing media and projects are never overwritten by intake')
    (work/'assets').mkdir(exist_ok=True)
    (work/'posters').mkdir(exist_ok=True)
    inventory = []
    for index, source in enumerate(assets):
        source = source.resolve(strict=True)
        if source.suffix.lower() not in IMAGE_EXT | {'.mp4', '.mov', '.mkv', '.webm', '.m4v'}:
            raise ValueError('Unsupported media: ' + source.name)
        identifier = 'asset-' + str(index+1)
        target = work/'assets'/(identifier + source.suffix.lower())
        shutil.copy2(source, target)
        info = probe(target)
        video = next(s for s in info['streams'] if s['codec_type']=='video')
        still = source.suffix.lower() in IMAGE_EXT
        poster = work/'posters'/(identifier+'.jpg')
        run([BINS/'ffmpeg','-v','error','-y','-i',target,'-frames:v','1','-vf','scale=960:540:force_original_aspect_ratio=decrease,pad=960:540:(ow-iw)/2:(oh-ih)/2:color=0x171c35',poster])
        inventory.append(dict(id=identifier,name=source.name,path=str(target.relative_to(work)),poster=str(poster.relative_to(work)),
                              kind='image' if still else 'video',width=video['width'],height=video['height'],
                              duration=None if still else float(info['format']['duration'])))
    result=dict(version=1,revision=0,brief=brief.read_text(),assets=inventory,
                plan=dict(title='Untitled video',message='',scenes=[]),approved_revision=None,feedback=[],build=None,job=None)
    write(work/'storyboard.json',result)
    return result

def number(value, low, high, name):
    if isinstance(value,bool) or not isinstance(value,(int,float)) or not math.isfinite(value) or not low <= value <= high:
        raise ValueError(name+' is out of range')
    return value

def validate(plan, inventory):
    if not isinstance(plan,dict) or not isinstance(plan.get('scenes'),list):
        raise ValueError('Plan needs scenes')
    if not 1 <= len(plan['scenes']) <= 12:
        raise ValueError('Use 1–12 scenes')
    for key in ('title','message'):
        if not isinstance(plan.get(key),str) or not plan[key].strip() or len(plan[key])>500:
            raise ValueError('Plan needs a short '+key)
    assets={a['id']:a for a in inventory}; ids=set(); total=0
    for scene in plan['scenes']:
        sid=scene.get('id')
        if not isinstance(sid,str) or not sid or len(sid)>80 or sid in ids:
            raise ValueError('Scene IDs must be unique, non-empty strings')
        ids.add(sid)
        if scene.get('asset_id') not in assets:
            raise ValueError('Scene references unknown media')
        for key in ('title','purpose'):
            if not isinstance(scene.get(key),str) or not scene[key].strip() or len(scene[key])>1000:
                raise ValueError('Scene needs '+key)
        for key in ('caption','narration'):
            if not isinstance(scene.get(key,''),str) or len(scene.get(key,''))>2000:
                raise ValueError('Invalid '+key)
        start=number(scene.get('start',0),0,86400,'start')
        duration=number(scene.get('duration'),1,20,'duration'); total+=duration
        asset=assets[scene['asset_id']]
        if asset['kind']=='video' and start+duration > asset['duration']+0.02:
            raise ValueError('Scene extends beyond source video')
        if asset['kind']=='image' and start!=0:
            raise ValueError('Still images must start at zero')
        if scene.get('zoom') is not None:
            z=scene['zoom'];number(z.get('depth'),1,3,'zoom depth')
            for axis in ('cx','cy'): number(z.get(axis),0,1,'zoom '+axis)
    if total>120: raise ValueError('Spike is limited to 120 seconds')
    return copy.deepcopy(plan)

def update_plan(work, plan, revision):
    with locked(work):
        data=state(work)
        if revision!=data['revision']: raise ValueError('Revision conflict; re-read context before editing')
        if data.get('job',{} ) and data['job'].get('status')=='running': raise ValueError('Wait for the current build')
        data['plan']=validate(plan,data['assets']);data['revision']+=1;data['approved_revision']=None
        write(work/'storyboard.json',data)
        return data

def review(work, revision, action, text=''):
    with locked(work):
        data=state(work)
        if revision!=data['revision']: raise ValueError('Revision changed; refresh and review again')
        if action=='approve':
            validate(data['plan'],data['assets']);data['approved_revision']=revision
        elif action=='feedback':
            if not isinstance(text,str) or not text.strip() or len(text)>10000: raise ValueError('Enter feedback')
            data['feedback'].append(dict(revision=revision,text=text,time=time.time()))
            data['approved_revision']=None
        else: raise ValueError('Unknown review action')
        write(work/'storyboard.json',data)
        return data

def build(work):
    with locked(work):
        data=state(work)
        if data['approved_revision']!=data['revision']: raise ValueError('Review and approve this revision on the board before building')
        if data.get('job') and data['job']['status']=='running': raise ValueError('Build already running')
        plan=validate(data['plan'],data['assets']); revision=data['revision']
        generation='generation-'+str(revision)+'-'+uuid.uuid4().hex[:8]
        output=work/'builds'/generation;output.mkdir(parents=True)
        data['job']=dict(status='running',message='Preparing scene footage',generation=generation)
        write(work/'storyboard.json',data)
    try:
        assets={a['id']:a for a in data['assets']}; parts=[]; total=0; mapping=[]; captions=[]; zooms=[]; clips=[]
        for index,scene in enumerate(plan['scenes']):
            asset=assets[scene['asset_id']]; target=output/f'part-{index:02d}.mp4'
            command=[BINS/'ffmpeg','-v','error','-y']
            command+=['-loop','1'] if asset['kind']=='image' else ['-ss',str(scene.get('start',0))]
            command+=['-i',work/asset['path'],'-t',str(scene['duration']),'-an','-vf',
                      'scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2:color=0x171c35,setsar=1,fps=30',
                      '-c:v','libx264','-preset','fast','-crf','20','-pix_fmt','yuv420p',target]
            run(command);parts.append(target)
            begin=round(total*1000);total+=scene['duration'];end=round(total*1000)
            clips.append(dict(id=scene['id'],startMs=begin,endMs=end))
            if scene.get('caption'): captions.append(dict(id='caption-'+scene['id'],startMs=begin,endMs=end,text=scene['caption']))
            if scene.get('zoom'):
                z=scene['zoom'];zooms.append(dict(id='zoom-'+scene['id'],startMs=begin+200,endMs=end-200,depth=z['depth'],focus=dict(cx=z['cx'],cy=z['cy']),mode='manual'))
            mapping.append(dict(scene_id=scene['id'],asset_id=scene['asset_id'],source_start=scene.get('start',0),output_start=begin/1000,output_end=end/1000))
        # Only generated simple basenames enter the concat manifest.
        (output/'parts.txt').write_text(''.join("file '"+p.name+"'\n" for p in parts))
        base=output/'assembly.mp4';run([BINS/'ffmpeg','-v','error','-y','-f','concat','-safe','1','-i',output/'parts.txt','-c','copy',base])
        project=dict(version=2,videoPath=str(base),editor=dict(
            wallpaper='#171c35',padding=8,borderRadius=3,shadowIntensity=0.2,aspectRatio='16:9',
            zoomRegions=zooms,clipRegions=clips,trimRegions=[],speedRegions=[],annotationRegions=[],audioRegions=[],autoCaptions=captions,
            autoCaptionSettings=dict(enabled=True,fontSize=28,bottomOffset=4,maxWidth=80,maxRows=1,textColor='#ffffff',backgroundOpacity=0.85),
            showCursor=False,connectZooms=False),projectId=uuid.uuid4().hex,
            subtakeStoryboard=dict(revision=revision,workspace=str(work),scenes=mapping))
        native=output/'draft.recordly';write(native,project);run([BINARY,'validate',native])
        for index,m in enumerate(mapping):
            run([BINARY,'render',native,output/f'scene-{index:02d}.png',str((m['output_start']+m['output_end'])/2),960,540])
        with locked(work):
            latest=state(work);latest['job']['message']='Rendering native preview';write(work/'storyboard.json',latest)
        run([BINARY,'export',native,output/'preview.mp4',960,540,30])
        report=dict(revision=revision,generation=generation,project=str(native.relative_to(work)),preview=str((output/'preview.mp4').relative_to(work)),
                    posters=[str((output/f'scene-{i:02d}.png').relative_to(work)) for i in range(len(mapping))],
                    duration=total,scene_mapping=mapping,binary_sha256=hashlib.sha256(BINARY.read_bytes()).hexdigest())
        write(output/'report.json',report)
        with locked(work):
            latest=state(work);latest['build']=report;latest['job']=dict(status='complete',message='Draft ready');write(work/'storyboard.json',latest)
        return report
    except Exception as error:
        with locked(work):
            latest=state(work);latest['job']=dict(status='failed',message=str(error));write(work/'storyboard.json',latest)
        raise

def serve(work):
    token=secrets.token_urlsafe(32)
    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self,*args): pass
        def reply(self, data, status=200, mime='application/json'):
            body=json.dumps(data).encode() if mime=='application/json' else data
            self.send_response(status);self.send_header('Content-Type',mime);self.send_header('Content-Length',str(len(body)))
            self.send_header('Cache-Control','no-store');self.send_header('X-Content-Type-Options','nosniff');self.end_headers();self.wfile.write(body)
        def do_GET(self):
            parsed=urllib.parse.urlparse(self.path)
            if parsed.path=='/': return self.reply((HERE/'board.html').read_bytes(),mime='text/html; charset=utf-8')
            if not secrets.compare_digest(urllib.parse.parse_qs(parsed.query).get('token',[''])[0],token):
                return self.reply({'error':'Unauthorised'},403)
            if parsed.path=='/api/state': return self.reply(state(work))
            if parsed.path=='/media':
                requested=urllib.parse.parse_qs(parsed.query).get('path',[''])[0]
                data=state(work);allowed={a['poster'] for a in data['assets']}
                if data.get('build'): allowed.update(data['build']['posters']);allowed.add(data['build']['preview'])
                if requested not in allowed: return self.reply({'error':'Unknown media'},404)
                path=(work/requested).resolve()
                if not path.is_relative_to(work): return self.reply({'error':'Invalid path'},403)
                mime='video/mp4' if path.suffix=='.mp4' else 'image/png' if path.suffix=='.png' else 'image/jpeg'
                return self.reply(path.read_bytes(),mime=mime)
            self.reply({'error':'Not found'},404)
        def do_POST(self):
            if not secrets.compare_digest(self.headers.get('Authorization',''),'Bearer '+token): return self.reply({'error':'Unauthorised'},403)
            origin=self.headers.get('Origin')
            if origin and origin!=f'http://127.0.0.1:{self.server.server_port}': return self.reply({'error':'Invalid origin'},403)
            try:
                length=int(self.headers.get('Content-Length','0'))
                if not 0<length<=1024*1024: raise ValueError('Invalid request size')
                data=json.loads(self.rfile.read(length))
                if self.path=='/api/plan': result=update_plan(work,data['plan'],data['revision'])
                elif self.path=='/api/review': result=review(work,data['revision'],data['action'],data.get('text',''))
                elif self.path=='/api/build':
                    current=state(work)
                    if data['revision']!=current['revision'] or current['approved_revision']!=current['revision']: raise ValueError('Approve the current revision first')
                    threading.Thread(target=background_build,args=(work,),daemon=True).start();result={'started':True}
                elif self.path=='/api/open':
                    current=state(work)
                    if not current.get('build'): raise ValueError('Build a draft first')
                    subprocess.Popen(['open','-a',str(APP),str(work/current['build']['project'])]);result={'opened':True}
                else: raise ValueError('Unknown action')
                self.reply(result)
            except Exception as e: self.reply({'error':str(e)},400)
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler)
    url=f'http://127.0.0.1:{server.server_port}/?token={token}'
    write(work/'.server.json',dict(url=url,pid=os.getpid()))
    print(url,flush=True)
    server.serve_forever()

def background_build(work):
    try: build(work)
    except Exception as error: print(str(error),file=sys.stderr)

def launch(work):
    state(work)  # Refuse a missing or invalid workspace before changing the launcher target.
    (HERE/'workspaces').mkdir(exist_ok=True)
    write(HERE/'workspaces/active.json', {'workspace':str(work)})
    config=work/'.server.json'
    if config.exists():
        saved=read(config);url=saved['url']
        try:
            urllib.request.urlopen(url.replace('/?','/api/state?'),timeout=1).read()
            subprocess.Popen(['open',url]);return {'url':url}
        except Exception: pass
    with (work/'server.log').open('a') as log:
        process=subprocess.Popen([sys.executable,str(__file__),'serve',str(work)],stdout=log,stderr=log,start_new_session=True)
    for _ in range(100):
        if config.exists() and read(config)['pid']==process.pid:
            url=read(config)['url'];subprocess.Popen(['open',url]);return {'url':url}
        if process.poll() is not None: raise ValueError('Board server failed; see server.log')
        time.sleep(.05)
    raise ValueError('Board server did not start')

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command',choices=['init','context','plan','build','serve','launch','open'])
    parser.add_argument('workspace',nargs='?',type=Path)
    parser.add_argument('--brief',type=Path);parser.add_argument('--asset',type=Path,action='append',default=[])
    parser.add_argument('--file',type=Path);parser.add_argument('--revision',type=int)
    args=parser.parse_args()
    active=HERE/'workspaces/active.json'
    work=(args.workspace or (Path(read(active)['workspace']) if active.exists() else HERE/'workspaces/demo')).resolve()
    if args.command=='init':
        if not args.brief or not args.asset: parser.error('init requires --brief and --asset')
        result=init(work,args.brief,args.asset)
    elif args.command=='context': result=state(work)
    elif args.command=='plan':
        if not args.file or args.revision is None: parser.error('plan requires --file and --revision')
        result=update_plan(work,read(args.file),args.revision)
    elif args.command=='build': result=build(work)
    elif args.command=='serve': return serve(work)
    elif args.command=='launch': result=launch(work)
    else:
        data=state(work)
        if not data.get('build'): raise ValueError('Build a draft first')
        subprocess.Popen(['open','-a',str(APP),str(work/data['build']['project'])]);result=data['build']
    print(json.dumps(result,indent=2))

if __name__=='__main__':
    try: main()
    except Exception as error:
        print(json.dumps({'error':str(error)}),file=sys.stderr);sys.exit(1)
