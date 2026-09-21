"""Static GPUI ownership checks; does not certify rendering or input behavior."""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
CONTROLS = r"(?:Button|Dropdown|Slider|TextInput)"
FACTORIES = r"(?:button|toggle|panel|row|column)"
# A bare number where a design token belongs.
NUMBER = re.compile(r"-?\d+(?:\.\d*)?")


def violations(path, text):
    errors = []
    if re.search(r"\b(?:slint|i_slint_core)\s*::|include_modules\s*!", text):
        errors.append("active Rust source references the retired Slint runtime")
    shared = path.parts[:2] in (("crates", "ui"), ("crates", "theme"))
    if shared and re.search(r"\b(?:subtake_native|crate::(?:app|project|editing|ui_state|ui_runtime))\b", text):
        errors.append("shared presentation crate depends on application state")
    if path.parts[:2] != ("crates", "ui"):
        if re.search(r"\b(?:struct|enum)\s+" + CONTROLS + r"\b", text):
            errors.append("generic control implementation belongs in crates/ui")
        if re.search(r"\bfn\s+" + FACTORIES + r"\s*\(", text):
            errors.append("shared control/layout factory belongs in crates/ui")
    if path.parts[:2] != ("crates", "theme"):
        if re.search(r"\b(?:struct|enum)\s+(?:Palette|ThemeRegistry)\b", text):
            errors.append("theme definition belongs in crates/theme")
        # The type and icon scales are closed: every size in the interface
        # comes from a token, so a bare number here is a step nobody else
        # uses. Only numeric literals are flagged — passing a size through is
        # how the primitives themselves are written. `examples/` is exempt:
        # the font probe deliberately renders off-scale specimens.
        if path.parts[0] != "examples":
            for call, scale in ((r"text_size\(px\(([^)]*)\)\)", "Theme::FONT_*"),
                                (r"glyph_size\(([^)]*)\)", "Theme::ICON_SIZE*"),
                                (r"icon_sized\(\s*[^,]+,\s*([^,]+),", "Theme::ICON_SIZE*")):
                for literal in re.findall(call, text):
                    if NUMBER.fullmatch(literal.strip()):
                        errors.append(
                            f"size {literal.strip()} is off the {scale.split('_')[0]} scale (use {scale})")
    return [f"{path}: {message}" for message in errors]


def check(root):
    errors = []
    required = ["crates/theme/src/lib.rs", "crates/ui/src/lib.rs",
                "src/ui_state.rs", "src/ui_runtime.rs", "src/gpui_views.rs"]
    for name in required:
        if not (root / name).is_file():
            errors.append(f"{name}: required GPUI boundary source missing")
    files = sorted({p for folder in ("src", "examples", "crates/theme/src", "crates/ui/src")
                    for p in (root / folder).rglob("*.rs")})
    for path in files:
        errors.extend(violations(path.relative_to(root), path.read_text()))
    # Theme/UI crates must stay independent of the host and sibling subsystems.
    for name, allowed in (("theme", {"gpui"}), ("ui", {"gpui", "subtake-theme"})):
        manifest = root / "crates" / name / "Cargo.toml"
        if not manifest.is_file():
            errors.append(f"{manifest.relative_to(root)}: missing manifest")
            continue
        section = ""
        for line in manifest.read_text().splitlines():
            line = line.strip()
            if line.startswith("["):
                section = line
            elif "dependencies" in section and "=" in line and not line.startswith("#"):
                dependency = line.split("=", 1)[0].strip().strip('"')
                if dependency not in allowed:
                    errors.append(f"{manifest.relative_to(root)}: review presentation dependency {dependency}")
    for name in ("Cargo.toml", "Cargo.lock", "build.rs"):
        path = root / name
        if path.exists() and re.search(r"(?i)\b(?:i[-_]slint[-_]core|slint(?:[-_]build)?)\b", path.read_text()):
            errors.append(f"{name}: retired Slint dependency/build reference")
    views = root / "src/gpui_views.rs"
    if views.exists():
        text = views.read_text()
        for dependency in ("subtake_ui", "subtake_theme"):
            if not re.search(r"\b" + dependency + r"\b", text):
                errors.append(f"src/gpui_views.rs: must consume {dependency}")
    return files, errors


if __name__ == "__main__":
    files, errors = check(ROOT)
    if errors:
        raise SystemExit("\n".join(errors))
    print(f"GPUI static boundary passed: {len(files)} Rust sources; shared controls/theme ownership and dependency checks. No rendering or input validation.")
