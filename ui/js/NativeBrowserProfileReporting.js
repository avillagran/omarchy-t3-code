/*
MIT License

Copyright (c) 2026 T3 Tools Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/
// Whole pinned helper; only TypeScript annotations/exports are removed.
// T3 83a3674ffd84ffb33de768210c806cb1ac53fd80, BrowserProfileReporter.ts.
function takeDueProfileReports(reported, targets, input) {
  for (const environmentId of reported.keys()) {
    if (!targets.has(environmentId)) reported.delete(environmentId);
  }
  const due = [];
  for (const [environmentId, config] of targets) {
    const last = reported.get(environmentId);
    if (last && last.config === config && sameReportInput(last.input, input)) continue;
    reported.set(environmentId, { config, input });
    due.push(environmentId);
  }
  return due;
}

function sameReportInput(left, right) {
  return (
    left.profiles === right.profiles &&
    left.defaultProfileId === right.defaultProfileId &&
    left.recordingFrameRate === right.recordingFrameRate
  );
}

// The singleton owns reporting independently of preview-panel mounting. Only
// successful decoded preference reads hydrate it; the initial UI object does not.
function settings(backend, value, profilesReplaced) {
  if (!value || !Array.isArray(value.browserProfiles) ||
      typeof value.browserDefaultProfileId !== "string" ||
      [30, 60].indexOf(value.browserRecordingFrameRate) < 0) return;
  var old = backend.profileReportInput;
  // IPC snapshots clone arrays. Preserve the decoded settings field's identity
  // until that field actually changes, rather than treating each sidebar poll
  // as a settings edit.
  var profiles = !profilesReplaced && old && JSON.stringify(old.profiles) === JSON.stringify(value.browserProfiles)
    ? old.profiles : value.browserProfiles;
  backend.profileReportInput = {
    profiles: profiles,
    defaultProfileId: value.browserDefaultProfileId,
    recordingFrameRate: value.browserRecordingFrameRate
  };
  pump(backend);
}
function target(backend, value) {
  var targets = backend.profileReportTargets;
  if (!value.operable) {
    var old = targets.get(value.environment);
    if (value.removed || !value.sessionId || old && old.sessionId === value.sessionId)
      targets.delete(value.environment);
  } else {
    var old = targets.get(value.environment);
    if (!old || old.configId !== value.configId || old.sessionId !== value.sessionId)
      targets.set(value.environment, {configId:value.configId, sessionId:value.sessionId});
  }
  pump(backend);
}
function session(backend, value) {
  if (value.write !== true || value.state !== "closed") return;
  var old = backend.profileReportTargets.get(value.environment);
  if (old && old.sessionId === value.sessionId) {
    backend.profileReportTargets.delete(value.environment);
    pump(backend);
  }
}
function pump(backend) {
  if (!backend.profileReportInput || !backend.previewWorkerReady) return;
  var targets = backend.profileReportTargets;
  var due = takeDueProfileReports(backend.profileReports, targets, backend.profileReportInput);
  for (var i = 0; i < due.length; i++) {
    var environment = due[i], target = targets.get(environment);
    backend.send({op:"browser-profile-report", environment:environment,
      sessionId:target.sessionId, configId:target.configId, input:backend.profileReportInput});
  }
}
function lost(backend) {
  backend.profileReportTargets = new Map();
  backend.profileReports = new Map();
  backend.profileReportInput = null;
}
