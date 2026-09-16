"""Build the macOS ICNS from the approved Porcelain PNG (macOS tools only)."""
from pathlib import Path
import subprocess
import tempfile


def build():
    root = Path(__file__).resolve().parents[1]
    source = root / "assets/branding/app-icon.png"
    output = source.with_name("SubTake.icns")
    if output.exists() and output.stat().st_mtime_ns >= max(
        source.stat().st_mtime_ns, Path(__file__).stat().st_mtime_ns
    ):
        return output
    with tempfile.TemporaryDirectory(prefix="subtake-icons-") as temp:
        iconset = Path(temp) / "SubTake.iconset"
        iconset.mkdir()
        for size in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                suffix = "@2x" if scale == 2 else ""
                subprocess.run([
                    "sips", "-z", str(size * scale), str(size * scale), str(source),
                    "--out", str(iconset / f"icon_{size}x{size}{suffix}.png"),
                ], check=True, stdout=subprocess.DEVNULL)
        subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(output)], check=True)
    return output


if __name__ == "__main__":
    print(build())
