.pragma library
// What a screen reader hears when the open thread's turn changes state.
// Port of T3 Code apps/web/src/components/chat/threadActivityAnnouncement.ts
// (eb1510d24); MIT, licenses/T3-Code-MIT.txt. Object spread becomes Object.assign.

var ACTIVE_RUN_STATUSES = ["preparing", "starting", "running", "waiting"];
var STARTING_RUN_STATUSES = ["queued", "preparing", "starting"];

function runOutcomeText(status) {
  switch (status) {
    case "completed":
      return "Done";
    case "failed":
      return "Failed";
    case "interrupted":
    case "cancelled":
      return "Stopped";
    default:
      return null;
  }
}

// previous: the memory returned last time for this thread, or null for a silent baseline.
// next: {status: "working"|"approval"|"input"|"idle", requestId, runId, runStatus}.
function nextThreadAnnouncement(previous, next) {
  var idle = next.status === "idle";
  // A run still starting up while the thread looks idle (a queued message,
  // a resume) is the next turn's, not a leftover from the last one.
  var idleRunId =
    idle && STARTING_RUN_STATUSES.indexOf(next.runStatus) < 0
      ? next.runId
      : previous ? previous.idleRunId : null;
  // A run doing new work; the run left over from the last turn doesn't count.
  var newRunId =
    !idle && ACTIVE_RUN_STATUSES.indexOf(next.runStatus) >= 0 && next.runId !== idleRunId
      ? next.runId
      : null;

  if (previous === null) {
    return {
      memory: {
        snapshot: next,
        turnRunId: idle ? null : newRunId !== null ? newRunId : "unknown",
        settledRunId: runOutcomeText(next.runStatus) === null ? null : next.runId,
        idleRunId: idleRunId,
      },
      announcement: null,
    };
  }
  var memory = Object.assign({}, previous, { snapshot: next, idleRunId: idleRunId });
  var turnRunId = previous.turnRunId;

  if (!idle) {
    // Sending can look idle for a moment before the server reports the run,
    // so a turn is tied to its run as soon as one appears, and only another
    // run taking over (a queued message starting) is a new turn.
    var turnStarted =
      turnRunId === null ||
      (newRunId !== null && turnRunId !== "unknown" && newRunId !== turnRunId);
    var nextTurnRunId =
      turnStarted || turnRunId === "unknown" ? (newRunId !== null ? newRunId : "unknown") : turnRunId;
    var announcement =
      next.status === "approval"
        ? previous.snapshot.status === "approval" && previous.snapshot.requestId === next.requestId
          ? null
          : "Needs approval"
        : next.status === "input"
          ? previous.snapshot.status === "input" && previous.snapshot.requestId === next.requestId
            ? null
            : "Needs your input"
          : turnStarted
            ? "Working"
            : null;
    return { memory: Object.assign(memory, { turnRunId: nextTurnRunId }), announcement: announcement };
  }

  // Work can stop a moment before the run settles; the ending waits for the run.
  var outcome = runOutcomeText(next.runStatus);
  if (turnRunId === null) {
    // A run that settled unseen is history too, or the next failed send would claim it.
    var settledRunId = outcome === null ? previous.settledRunId : next.runId;
    return { memory: Object.assign(memory, { settledRunId: settledRunId }), announcement: null };
  }
  if (next.runStatus === "rolled_back") {
    return { memory: Object.assign(memory, { turnRunId: null, settledRunId: next.runId }), announcement: null };
  }
  if (outcome === null) return { memory: memory, announcement: null };
  // Work stopped without a new run, e.g. a send that failed; the retry is a new turn.
  if (next.runId === previous.settledRunId) {
    return { memory: Object.assign(memory, { turnRunId: null }), announcement: null };
  }
  // The last turn's run settling while this turn waits for its own run is not this turn's ending.
  if (turnRunId === "unknown" && next.runId === previous.idleRunId) {
    return { memory: Object.assign(memory, { settledRunId: next.runId }), announcement: null };
  }
  return {
    memory: Object.assign(memory, { turnRunId: null, settledRunId: next.runId }),
    announcement: outcome,
  };
}
