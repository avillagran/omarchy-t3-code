#!/usr/bin/env python3
"""Private row UI transport feed; all clock and row logic runs in production QML."""
import json
import sys
from pathlib import Path

if not Path("/run/.containerenv").is_file():
    raise SystemExit("This fixture requires rootless Podman")

if len(sys.argv) < 2 or sys.argv[1] != "watch":
    print("{}", flush=True)
    raise SystemExit(0)
print(json.dumps({"type":"ui-state","data":{"windowMode":"floating","threadOrder":"reply","sidebarGrouped":False,"threadRowSize":"compact"}}), flush=True)
for line in sys.stdin:
    # The fixture controls state explicitly. Never contact accounts, files or a host service.
    json.loads(line)
