"""Copy and verify GPUI's runtime assets without using the source-tree loader."""
from pathlib import Path
import shutil

UI_ASSET_DIRECTORIES = ("icons", "branding")
REQUIRED_UI_ASSETS = (
    "assets/branding/menu-bar.svg",
    "assets/branding/SubTake.icns",
    "assets/icons/Record-fill.svg",
)


def copy_ui_assets(root, resources):
    for name in UI_ASSET_DIRECTORIES:
        shutil.copytree(root / "assets" / name, resources / "assets" / name,
                        dirs_exist_ok=True)


def verify_ui_assets(root, resources, *, development=False):
    """Require every icon/brand file, preserving bytes and bundle-relative paths.

    Development may use the source assets symlink; packages must be independent
    regular files inside Resources. Never resolve missing files through the app's
    fallback asset loader.
    """
    resources = Path(resources)
    resolved_resources = resources.resolve()
    checked = 0
    for name in UI_ASSET_DIRECTORIES:
        source_directory = root / "assets" / name
        sources = sorted(p for p in source_directory.rglob("*") if p.is_file())
        if not sources:
            raise RuntimeError(f"Missing or empty GPUI asset source: {source_directory}")
        for source in sources:
            relative = source.relative_to(root)
            target = resources / relative
            if not target.is_file() or target.stat().st_size == 0:
                raise RuntimeError(f"Missing or empty GPUI bundle resource: {target}")
            if not development:
                if resolved_resources not in target.resolve().parents:
                    raise RuntimeError(f"GPUI resource escapes the bundle: {target}")
                if any(p.is_symlink() for p in (target, *target.parents)
                       if p != resources and resources in p.parents):
                    raise RuntimeError(f"Packaged GPUI resource must not be a symlink: {target}")
            if source.read_bytes() != target.read_bytes():
                raise RuntimeError(f"GPUI bundle resource differs from canonical source: {target}")
            checked += 1
    for relative in REQUIRED_UI_ASSETS:
        if not (resources / relative).is_file():
            raise RuntimeError(f"Required GPUI asset missing: {relative}")
    return checked
