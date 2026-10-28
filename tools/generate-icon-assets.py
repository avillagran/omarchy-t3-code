#!/usr/bin/env python3
"""Embed the checked-in local SVGs for cached, palette-aware Qt rendering."""
import json
from pathlib import Path
import xml.etree.ElementTree as ET
root=Path(__file__).resolve().parents[1]
icons={}
for path in sorted((root/'ui/assets/icons').glob('*.svg')):
    svg=path.read_text().strip()
    ET.fromstring(svg)
    icons[path.stem]=svg
# Only the generated line is replaced: icons added by hand below it are kept.
target=root/'ui/js/IconAssets.js'
generated='var icons = '+json.dumps(icons,separators=(',',':'))+';'
lines=target.read_text().split('\n') if target.exists() else ['.pragma library','','function svg(name) { return icons[name] || icons["server"] }','']
index=next((i for i,l in enumerate(lines) if l.startswith('var icons = ')),None)
if index is None: lines.insert(1,generated)
else: lines[index]=generated
target.write_text('\n'.join(lines))
print(f'{len(icons)} local SVGs validated')
