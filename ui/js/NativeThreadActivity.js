.pragma library
// Native nightly 4301 label clocks; sorting and reply clocks remain separate.
function toSortableTimestamp(iso                    )                {
  if (!iso) return null;
  const ms = Date.parse(iso);
  return Number.isFinite(ms) ? ms : null;
}
function resolveSettledThreadTimestamp(thread                             )                {
  if (thread.settledAt != null && toSortableTimestamp(thread.settledAt) !== null) {
    return thread.settledAt;
  }
  return resolveThreadActivityTimestamp(thread);
}
function resolveThreadActivityTimestamp(
  thread                                                                                 ,
)                {
  let latest                = null;
  let latestMs = Number.NEGATIVE_INFINITY;
  for (const candidate of [
    thread.latestUserMessageAt,
    thread.latestRun?.requestedAt,
    thread.latestRun?.startedAt,
    thread.latestRun?.completedAt,
  ]) {
    const parsed = toSortableTimestamp(candidate ?? undefined);
    if (candidate != null && parsed !== null && parsed > latestMs) {
      latest = candidate;
      latestMs = parsed;
    }
  }
  if (latest !== null) return latest;
  return toSortableTimestamp(thread.updatedAt) === null ? null : thread.updatedAt;
}
