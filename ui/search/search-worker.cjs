// Appended to the compiled, source-pinned NativeSearchRuntime.cjs by the owner.
// Requests contain JSON metadata only. Native algorithms remain in that module.
'use strict';
process.title = 'o3-native-search';
const searchNative = module.exports;
const searchMaxPacket = 16 * 1024 * 1024;
const searchSubjects = new Map();
const searchRows = new Map();
let searchFrameParts = [], searchFrameBytes = 0, searchClosed = false;

function searchWrite(packet) {
  const bytes = Buffer.from(JSON.stringify(packet) + '\n');
  if (bytes.length > searchMaxPacket) throw Error('native_search_output_too_large');
  process.stdout.write(bytes);
}
function searchRequire(value, condition) {
  if (!condition) throw Error('invalid_native_search_request');
  return value;
}
function searchUpdateRows(rows) {
  searchRequire(rows, Array.isArray(rows));
  const next = new Map();
  for (const value of rows) {
    searchRequire(value, value && typeof value.key === 'string' && value.thread &&
      typeof value.thread.id === 'string' && typeof value.thread.environmentId === 'string' &&
      typeof value.thread.title === 'string' && typeof value.thread.createdAt === 'string' &&
      typeof value.thread.updatedAt === 'string');
    searchRequire(value, !next.has(value.key));
    const encoded = JSON.stringify(value);
    const prior = searchRows.get(value.key);
    next.set(value.key, prior?.encoded === encoded ? prior : {encoded, value});
  }
  searchRows.clear();
  for (const [key, value] of next) searchRows.set(key, value);
  searchSubjects.clear();
}
function searchSubject(key) {
  if (searchSubjects.has(key)) return searchSubjects.get(key);
  const row = searchRows.get(key)?.value;
  if (!row) return undefined;
  const subject = searchNative.threadSearchSubject(row.thread, row.projectTitle, row.environmentLabel);
  searchSubjects.set(key, subject);
  return subject;
}
function searchPaletteItem(item) {
  searchRequire(item, item && typeof item.value === 'string' && typeof item.title === 'string' &&
    Array.isArray(item.searchTerms) && item.searchTerms.every(term => typeof term === 'string'));
  if (item.threadKey === undefined) return item;
  const subject = searchSubject(item.threadKey);
  return subject ? {...item, threadSearch: subject} : null;
}
function searchItems(items) {
  searchRequire(items, Array.isArray(items));
  return items.map(searchPaletteItem).filter(item => item !== null);
}
function searchPalette(request, now) {
  const groups = searchRequire(request.activeGroups, Array.isArray(request.activeGroups));
  const activeGroups = groups.map(group => ({...group, items: group.value === 'recent-threads'
    ? searchItems(group.items).filter(item => !item.secondary).sort((left, right) =>
      (right.threadSearch ? searchNative.threadRecentUse(right.threadSearch) : 0) -
      (left.threadSearch ? searchNative.threadRecentUse(left.threadSearch) : 0))
        .slice(0, searchNative.RECENT_THREAD_LIMIT)
    : searchItems(group.items)}));
  const input = {
    activeGroups, query: request.query, isInSubmenu: request.isInSubmenu === true,
    projectSearchItems: searchItems(request.projectSearchItems ?? []),
    settingsSearchItems: searchItems(request.settingsSearchItems ?? []),
    threadSearchItems: searchItems(request.threadSearchItems ?? []), now,
  };
  if (request.allOptions !== undefined) {
    const options = request.allOptions;
    input.allOptions = {
      rootGroups: searchRequire(options.rootGroups, Array.isArray(options.rootGroups)).map(group =>
        ({...group, items: searchItems(group.items)})),
      nestedItems: searchItems(options.nestedItems),
    };
  }
  const normalized = searchNative.foldSearchQuery(request.query.startsWith('>')
    ? request.query.slice(1) : request.query);
  if (normalized.length > 0 && !input.isInSubmenu)
    input.activeGroups = input.activeGroups.filter(group =>
      group.value !== 'projects-search' && group.value !== 'settings-search');
  let shownOrder = null;
  if (request.messageMatches !== undefined) {
    const matches = searchRequire(request.messageMatches, Array.isArray(request.messageMatches));
    const previous = {query: request.messageQuery ?? '', matches};
    const ready = searchNative.foldSearchQuery(previous.query) === normalized ? previous : null;
    const visible = searchNative.visibleThreadSearch(normalized, ready, previous);
    const ranked = searchNative.rankMessageMatches(visible.matches, match => searchSubject(match.threadKey),
      normalized, now, {withoutTitleMatches: true});
    const byKey = new Map(ranked.map(match => [match.value, match]));
    searchRequire(byKey, byKey.size === ranked.length);
    shownOrder = searchNative.nextShownOrder(request.shownOrder ?? null, normalized,
      ranked.map(match => match.value), searchNative.PALETTE_MESSAGE_LIMIT);
    input.threadMessageItems = shownOrder.keys.slice(0, shownOrder.shown).map(key =>
      searchPaletteItem(byKey.get(key)));
  }
  const result = searchNative.filterCommandPaletteGroups(input);
  return {groups: result.map(group => ({value: group.value, label: group.label,
    items: group.items.map(item => ({value: item.value, matchHint: item.matchHint ?? null,
      find: group.value === 'threads-messages' ? searchNative.findForMessageMatch(
        {snippet: item.snippet ?? '', messageId: item.messageId}, request.messageQuery ?? request.query) : null,
      titleParts: searchNative.splitSearchText(item.title,
        searchNative.findSearchTokenRanges(item.title, normalized))}))})), shownOrder};
}
function searchDispatch(request) {
  searchRequire(request, request && typeof request.id === 'string' && request.id.length <= 128 &&
    typeof request.op === 'string');
  if (request.rows !== undefined) searchUpdateRows(request.rows);
  const now = request.now ?? Date.now();
  searchRequire(now, Number.isFinite(now));
  if (request.op === 'snapshot') return {updated: true};
  if (request.op === 'recordOpen')
    return searchNative.recordThreadOpen(request.previous ?? null, now, request.query);
  if (request.op === 'open') {
    const row = searchRows.get(request.key)?.value;
    searchRequire(row, row !== undefined);
    const send = searchNative.claimThreadOpen(row.thread.environmentId, row.thread.id, now);
    // Every actual local open changes recency, including deduplicated server sends.
    searchSubjects.clear();
    return {send, searchQuery: typeof request.query === 'string' && !request.query.startsWith('>')
      ? searchNative.pickedQuery(request.query) : undefined};
  }
  if (request.op === 'quit') {
    searchClosed = true;
    process.stdin.pause();
    return {closed: true};
  }
  searchRequire(request.query, typeof request.query === 'string');
  if (request.op === 'palette') return searchPalette(request, now);
  if (request.op === 'sidebar') {
    const query = searchNative.foldSearchQuery(request.query);
    const keys = searchRequire(request.keys, Array.isArray(request.keys));
    return {keys: searchNative.rankSearchItems(keys, (key, options) => {
      const subject = searchSubject(key);
      return subject ? searchNative.scoreThreadSearch(query, subject, now,
        {...options, picks: false}) : null;
    }).map(row => row.item)};
  }
  throw Error('invalid_native_search_request');
}
function searchFrame(bytes) {
  let envelope, request, packed = false;
  try {
    envelope = JSON.parse(bytes.toString('utf8'));
    packed = typeof envelope?.payloadJson === 'string';
    request = packed ? {...JSON.parse(envelope.payloadJson), id: envelope.id, op: envelope.op} : envelope;
    const result = searchDispatch(request);
    // An inner JSON string preserves every native UTF16 code unit across Rust's
    // UTF8 JSON representation, including lone surrogates in a transient query.
    searchWrite(packed ? {id: request.id, resultJson: JSON.stringify(result)} : {id: request.id, result});
    if (searchClosed) process.stdin.destroy();
  } catch {
    searchWrite({id: typeof envelope?.id === 'string' ? envelope.id : null,
      error: 'native_search_request_failed'});
  }
}
process.stdin.on('data', chunk => {
  if (searchClosed) return;
  let offset = 0;
  while (offset < chunk.length) {
    const newline = chunk.indexOf(10, offset);
    const end = newline === -1 ? chunk.length : newline;
    const part = chunk.subarray(offset, end);
    searchFrameBytes += part.length;
    if (searchFrameBytes > searchMaxPacket) {
      searchClosed = true;
      process.stdin.destroy();
      process.exitCode = 1;
      return;
    }
    if (part.length) searchFrameParts.push(part);
    if (newline === -1) break;
    const bytes = Buffer.concat(searchFrameParts, searchFrameBytes);
    searchFrameParts = []; searchFrameBytes = 0;
    searchFrame(bytes);
    if (searchClosed) return;
    offset = newline + 1;
  }
});
process.stdin.on('end', () => {
  if (searchFrameBytes !== 0) process.exitCode = 1;
  searchClosed = true;
});
process.stdin.on('error', () => { searchClosed = true; process.exitCode = 1; });
searchWrite({type: 'ready', protocol: 'o3-native-search-v1', electron: process.versions.electron});
