"""Exercise the native binary with generated media; never captures the user's desktop."""
import json, math, os, subprocess, sys, struct
from pathlib import Path

root = Path(__file__).resolve().parents[1]
out = root / "test-output"
out.mkdir(exist_ok=True)
binary = Path(os.environ.get("SUBTAKE_TEST_BINARY", root / "target" / "debug" / "subtake-native"))
ffmpeg = os.environ.get("SUBTAKE_FFMPEG", "/opt/homebrew/bin/ffmpeg")
ffprobe = os.environ.get("SUBTAKE_FFPROBE", "/opt/homebrew/bin/ffprobe")
def run(*args):
    result = subprocess.run([str(x) for x in args], capture_output=True, text=True, timeout=180)
    if result.returncode:
        raise RuntimeError(f"{args[0]} exited {result.returncode}\n{result.stdout}\n{result.stderr}")
    return result.stdout

video = out / "source with spaces.mp4"
run(ffmpeg, "-v", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=30:duration=4", "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000:duration=4", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest", video)
audio = out / "music.wav"
run(ffmpeg, "-v", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=880:sample_rate=48000:duration=4", audio)
p = {"version": 2, "videoPath": str(video), "projectId": "native-smoke", "editor": {
    "wallpaper": "#25304f", "padding": {"left": 20, "right": 20, "top": 20, "bottom": 20}, "borderRadius": 8,
    "cropRegion": {"x": 0, "y": 0, "width": 1, "height": 1}, "shadowIntensity": 0.3,
    "trimRegions": [{"id":"trim", "startMs": 1000, "endMs": 1500}],
    "speedRegions": [{"id":"speed", "startMs": 2000, "endMs": 3000, "speed":2}],
    "zoomRegions": [{"id":"zoom", "startMs": 500, "endMs": 2500, "depth": 2, "focus":{"cx":0.6,"cy":0.4}}],
    "annotationRegions": [{"id":"title", "startMs":0,"endMs":3500,"type":"text","textContent":"Native SubTake", "position":{"x":12,"y":10},"size":{"width":60,"height":20}, "style":{"fontSize":70,"color":"#ffffff","fontFamily":"Helvetica","backgroundColor":"#182034"},"zIndex":1}],
    "audioRegions":[{"id":"music","startMs":500,"endMs":3500,"audioPath":str(audio),"volume":0.2}],
    "autoCaptions":[{"id":"caption","startMs":500,"endMs":3500,"text":"One renderer · preview and export"}],
    "autoCaptionSettings":{"enabled":True,"fontSize":38,"maxWidth":90,"maxRows":2,"bottomOffset":4,"textColor":"#ffffff","backgroundOpacity":0.8},
    "nativeCaptionSidecars":True, "futureSetting":{"keep":True}
}}
# Exercise separate capture tracks, camera composition, SVG cursors and a looping background.
for suffix, frequency in [("system.m4a",440),("mic.m4a",660)]:
    run(ffmpeg,"-v","error","-y","-f","lavfi","-i",f"sine=frequency={frequency}:sample_rate=48000:duration=4",video.with_suffix("."+suffix))
webcam=out/"camera.mp4"
run(ffmpeg,"-v","error","-y","-f","lavfi","-i","testsrc=size=320x240:rate=30:duration=4","-c:v","libx264","-pix_fmt","yuv420p",webcam)
background=out/"background.mp4"
run(ffmpeg,"-v","error","-y","-f","lavfi","-i","color=c=0x25304f:size=320x180:rate=30:duration=2","-c:v","libx264","-pix_fmt","yuv420p",background)
samples=[{"timeMs":i*1000/120,"cx":0.3+0.3*math.sin(i/70),"cy":0.5+0.2*math.cos(i/50),"cursorType":"arrow" if i<240 else "pointer","interactionType":"click" if i in [60,250] else "move"} for i in range(480)]
Path(str(video)+".cursor.json").write_text(json.dumps({"samples":samples}))
p["editor"].update({"wallpaper":str(background),"showCursor":True,"cursorStyle":"tahoe","cursorSize":3,"cursorClickEffect":"echo","cursorClickEffectDurationMs":600,
    "webcam":{"enabled":True,"sourcePath":str(webcam),"mirror":True,"width":28,"height":24,"roundness":75,"position":"bottom-right","margin":24},
    "defaultSourceAudioTrackSettings":{"system":{"volume":0.4},"mic":{"volume":0.3}},
})
p["editor"]["annotationRegions"].append({"id":"blur","startMs":0,"endMs":3500,"type":"blur","position":{"x":20,"y":58},"size":{"width":20,"height":15},"blurIntensity":30,"zIndex":0})
project = out / "fixture.recordly"
project.write_text(json.dumps(p,indent=2))
run(binary, "validate", project)
run(binary, "render", project, out/"preview.png", "0.8", "640", "360")
run(binary, "export", project, out/"export.mp4", "640", "360", "30")
run(binary, "export", project, out/"export.gif", "320", "180", "10")
assert "00:00:02,000 --> 00:00:02,500" in (out/"export.srt").read_text()
assert (out/"export.vtt").read_text().startswith("WEBVTT\n\n")
# Each cursor asset family must rasterize, and direct seeking must be repeatable.
for style in ["tahoe","macos","windows11","dot","figma"]:
    p["editor"]["cursorStyle"]=style
    variant=out/f"cursor-{style}.recordly";variant.write_text(json.dumps(p))
    run(binary,"render",variant,out/f"cursor-{style}.png","0.8","640","360")
p["editor"]["cursorStyle"]="tahoe"
data = json.loads(run(ffprobe,"-v","error","-count_frames","-show_streams","-show_format","-of","json",out/"export.mp4"))
v = next(s for s in data["streams"] if s["codec_type"]=="video")
a = next(s for s in data["streams"] if s["codec_type"]=="audio")
assert v["width"]==640 and v["height"]==360
assert int(v["nb_read_frames"])==90, v
assert abs(float(v["duration"])-3)<0.04, v
assert abs(float(a["duration"])-3)<0.04, a
gif = json.loads(run(ffprobe,"-v","error","-count_frames","-show_streams","-of","json",out/"export.gif"))["streams"][0]
assert int(gif["nb_read_frames"])==30, gif
def amplitudes(start):
    pcm=subprocess.run([ffmpeg,"-v","error","-ss",str(start),"-i",str(out/"export.mp4"),"-t","0.15","-f","f32le","-ac","1","-ar","48000","pipe:1"],capture_output=True,check=True).stdout
    samples=[v[0] for v in struct.iter_unpack("<f",pcm)]
    result={}
    for hz in [440,660,880]:
        real=sum(v*math.cos(2*math.pi*hz*i/48000) for i,v in enumerate(samples))
        imag=sum(v*math.sin(2*math.pi*hz*i/48000) for i,v in enumerate(samples))
        result[hz]=2*math.hypot(real,imag)/len(samples)
    return result
before=amplitudes(0.2);during=amplitudes(0.7)
assert abs(before[440]/before[660]-4/3)<0.15, before
assert before[880]<0.005, before
assert abs(during[880]/during[660]-2/3)<0.15, during
report={"video_frames":v["nb_read_frames"],"video_duration":v["duration"],"audio_duration":a["duration"],"gif_frames":gif["nb_read_frames"],"resolution":[v["width"],v["height"]],"source":"generated test pattern and sine tones", "parity_with_electron":"math fixtures verified; pixel parity not established", "audio_amplitudes_before_music":before,"audio_amplitudes_with_music":during, "features":["trim","speed","camera spring","cursor SVG families","video background","webcam","blur annotation","captions","system/mic sidecars","additional audio","SRT/VTT sidecars"]}
(out/"smoke-results.json").write_text(json.dumps(report,indent=2))
print(json.dumps(report,indent=2))
