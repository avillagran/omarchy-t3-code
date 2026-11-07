#!/usr/bin/env python3
"""Inert transport boundary for the permission modal's incidental singleton."""
import json
import sys

for line in sys.stdin:
    try:
        value = json.loads(line)
    except ValueError:
        continue
    if value.get('op') == 'quit':
        break
