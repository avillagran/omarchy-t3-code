#!/usr/bin/env python3
"""Erase only the pinned original helper's known TypeScript syntax."""
import argparse
import hashlib
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("native_root", type=Path)
parser.add_argument("client_root", type=Path)
args = parser.parse_args()
source = (args.native_root / "apps/web/src/browser/BrowserProfileReporter.ts").read_text()
assert hashlib.sha256(source.encode()).hexdigest() == "ec8dbd094cf6bff0eb545478e57ed8b3255f80d13193a9b9141e967505112045"
helper = source[source.index("export function takeDueProfileReports("):]
replacements = {
    "export function takeDueProfileReports(\n  reported: ReportedProfiles,\n  targets: ReadonlyMap<EnvironmentId, EnvironmentServerConfig>,\n  input: ProfileReport,\n): EnvironmentId[] {": "function takeDueProfileReports(reported, targets, input) {",
    "const due: EnvironmentId[] = [];": "const due = [];",
    "function sameReportInput(left: ProfileReport, right: ProfileReport): boolean {": "function sameReportInput(left, right) {",
}
for before, after in replacements.items():
    assert helper.count(before) == 1, before
    helper = helper.replace(before, after)
for forbidden in ("EnvironmentId", "ProfileReport,", ": boolean", "export function"):
    assert forbidden not in helper, forbidden
output = args.client_root / "ui/js/NativeBrowserProfileReporting.js"
existing = output.read_text()
prefix = existing[:existing.index("function takeDueProfileReports(")]
consumer = existing[existing.index("// The singleton owns reporting"):]
output.write_text(prefix + helper + "\n" + consumer)
