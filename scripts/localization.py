"""Reuse the legacy product's translations without a browser or JavaScript runtime."""
import json,re
from pathlib import Path
root=Path(__file__).resolve().parents[1]
source=root/'legacy-electron/src/i18n/locales'
locales=['en','es','fr','de','it','nl','ko','pt-BR','zh-CN','zh-TW']
def flatten(value,prefix=''):
    if isinstance(value,str):return {prefix:value}
    out={}
    if isinstance(value,dict):
        for k,v in value.items():out.update(flatten(v,prefix+'.'+k))
    return out
def normalize(s):return s.strip().removesuffix('...').removesuffix('…').lower()
catalog={locale:{} for locale in locales}
for base in sorted((source/'en').glob('*.json')):
    english=flatten(json.loads(base.read_text()))
    for locale in locales:
        path=source/locale/base.name
        translated=flatten(json.loads(path.read_text())) if path.exists() else {}
        for key,text in english.items():
            if '{{' not in text:catalog[locale].setdefault(normalize(text),translated.get(key,text))
aliases={'open':'open video','transcribe':'generate captions','frame':'background','captions':'auto captions','source':'source video','audio':'audio','font family':'font','show webcam':'webcam','speech language (auto, en, fr, …)':'language','appearance presets':'presets','word text':'text content'}
for values in catalog.values():
    for native,parent in aliases.items():
        if native not in values and parent in values:values[native]=values[parent]
(root/'assets').mkdir(exist_ok=True)
(root/'assets/localization.json').write_text(json.dumps(catalog,ensure_ascii=False,separators=(',',':'))+'\n')
print(f'Generated {len(locales)} locale catalogs with {len(catalog["en"])} source strings each.')
