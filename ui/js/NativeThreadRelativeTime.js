.pragma library
// Native 4301 relative label; the mounted owner provides the shared minute snapshot.
const MINUTE_MS = 60_000;
const HOUR_MS = 3_600_000;
const DAY_MS = 86_400_000;
function parseTimestampDate(isoDate        )              {
  const date = new Date(isoDate);
  return Number.isNaN(date.getTime()) ? null : date;
}
function relativeTimeUnitMs(elapsedMs        )         {
  if (elapsedMs < HOUR_MS) return MINUTE_MS;
  if (elapsedMs < DAY_MS) return HOUR_MS;
  return DAY_MS;
}
function relativeTimeElapsedMs(isoDate        , nowMs         = Date.now())                {
  const date = parseTimestampDate(isoDate);
  if (!date) return null;
  const elapsedMs = Math.max(0, nowMs - date.getTime());
  const unitMs = relativeTimeUnitMs(elapsedMs);
  return Math.floor(elapsedMs / unitMs) * unitMs;
}
function formatRelativeElapsed(elapsedMs        )                    {
  const seconds = Math.floor(elapsedMs / 1000);
  if (seconds < 60) return { value: "just now", suffix: null };
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return { value: `${minutes}m`, suffix: "ago" };
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return { value: `${hours}h`, suffix: "ago" };
  const days = Math.floor(hours / 24);
  return { value: `${days}d`, suffix: "ago" };
}
function formatRelativeTime(
  isoDate        ,
  nowMs         = Date.now(),
)                           {
  const elapsedMs = relativeTimeElapsedMs(isoDate, nowMs);
  return elapsedMs === null ? null : formatRelativeElapsed(elapsedMs);
}
function formatRelativeTimeLabel(isoDate        , nowMs         = Date.now()) {
  const relative = formatRelativeTime(isoDate, nowMs);
  if (!relative) return "";
  return relative.suffix ? `${relative.value} ${relative.suffix}` : relative.value;
}
function compactSidebarTimeLabel(label        )         {
  if (label === "just now") return "now";
  return label.endsWith(" ago") ? label.slice(0, -4) : label;
}
function relativeLabel(props,nowMinute) {

  return props.timestamp === null
    ? null
    : compactSidebarTimeLabel(
        formatRelativeTimeLabel(props.timestamp, Date.parse(`${nowMinute}:00.000Z`)),
      );
}
