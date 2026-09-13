"""Enforce the shared-control boundary without treating editor graphics as widgets."""
from pathlib import Path
import re
root=Path(__file__).resolve().parents[1]
errors=[]
for path in (root/'ui').glob('*.slint'):
    text=path.read_text()
    for match in re.finditer(r'\b(?:ComboBox|LineEdit|TextInput|CheckBox|Slider|Button|ScrollView|ProgressIndicator)\s*\{',text):
        errors.append(f'{path.relative_to(root)}: use an app primitive, not {match.group(0)}')
    for match in re.finditer(r'\b(?:ToolButton|IconButton|Dropdown|FineSlider|ValueInput)\s*\{[^{}]*?\bheight:\s*(\d+)px',text):
        if int(match.group(1))!=40: errors.append(f'{path.relative_to(root)}: controls must use the shared 40px height')
    if re.search(r'accessible-role\s*:\s*button\s*;',text):
        errors.append(f'{path.relative_to(root)}: use ButtonSurface/ChoiceTile instead of an inline button')
    if re.search(r'\b(?:quiet|primary)\s*:\s*(?:true|false)',text):
        errors.append(f'{path.relative_to(root)}: use a named button variant')
assert not errors,'\n'.join(errors)
print('Shared primitive boundary passed: no raw controls or inline buttons in screens.')
