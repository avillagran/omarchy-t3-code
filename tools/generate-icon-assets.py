#!/usr/bin/env python3
"""Embed the checked-in local SVGs for cached, palette-aware Qt rendering."""
import json
from pathlib import Path
import xml.etree.ElementTree as ET
root=Path(__file__).resolve().parents[1]
icons={}
for path in sorted((root/'assets/icons').glob('*.svg')):
    svg=path.read_text().strip()
    ET.fromstring(svg)
    icons[path.stem]=svg
(root/'IconAssets.js').write_text('.pragma library\nvar icons = '+json.dumps(icons,separators=(',',':'))+';\nfunction svg(name) { return icons[name] || icons["server"] }\n')
print(f'{len(icons)} local SVGs validated')
