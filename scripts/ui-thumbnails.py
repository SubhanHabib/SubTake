"""Generate small UI thumbnails from the existing bundled wallpaper assets (macOS build helper)."""
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
import subprocess
root=Path(__file__).resolve().parents[1]
source=root/'legacy-electron/public/wallpapers';destination=root/'assets/wallpaper-thumbnails';destination.mkdir(parents=True,exist_ok=True)
def make(path):
    target=destination/path.relative_to(source);target.parent.mkdir(parents=True,exist_ok=True)
    subprocess.run(['sips','-Z','240',str(path),'--out',str(target)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
paths=[p for p in source.rglob('*') if p.suffix.lower() in ['.png','.jpg','.jpeg']]
with ThreadPoolExecutor(max_workers=4) as executor:list(executor.map(make,paths))
print(f'Generated {len(paths)} wallpaper thumbnails')
