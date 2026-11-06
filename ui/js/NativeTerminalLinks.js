.pragma library
// Original T3 Code c55db3e9c72fdf584f4b54192fb39beb88667b59.
function create(nativeMatches) {
const MIN_SCROLLBAR_THUMB_HEIGHT=18;
function isMacPlatform(platform        )          {
  return /mac|iphone|ipad|ipod/i.test(platform);
}



























const URL_PATTERN = {"source":"https?:\\/\\/[^\\x{0009}-\\x{000d}\\x{0020}\\x{00a0}\\x{1680}\\x{2000}-\\x{200a}\\x{2028}\\x{2029}\\x{202f}\\x{205f}\\x{3000}\\x{feff}\"'`<>]+","flags":"giu"};
// The default PowerShell prompt delimits its unquoted path with `PS ` and `>`.
const POWERSHELL_PROMPT_PATH_PATTERN = /^PS ((?:[A-Za-z]:[\\/]|\\\\|\/)[^>\r\n]*)>/u;
// Bare paths start only at a segment boundary: hover detection runs this on
// every frame, so separator-free output must not be rescanned at every suffix.
// For the same reason `]` needs its `[`: trimming a long run of `]` is quadratic.
// Parentheses count only before a slash, so `src/foo.ts(3,10)` stays a compiler location.
const FILE_PATH_PATTERN = {"source":"(?:~[\\\\/]|\\.{1,2}[\\\\/]|\\/|[A-Za-z]:[\\\\/]|\\\\\\\\)[^\\x{0009}-\\x{000d}\\x{0020}\\x{00a0}\\x{1680}\\x{2000}-\\x{200a}\\x{2028}\\x{2029}\\x{202f}\\x{205f}\\x{3000}\\x{feff}\"'`<>]+|(?<![\\p{L}\\p{M}\\p{N}._-])[\\p{L}\\p{M}\\p{N}._-]+(?:[\\\\/](?:[\\p{L}\\p{M}\\p{N}._$+@-]|\\[{1,2}[\\p{L}\\p{M}\\p{N}._-]+\\]{1,2}|\\([\\p{L}\\p{M}\\p{N}._-]+\\)(?=[\\\\/]))+)+(?::\\d+){0,2}","flags":"gu"};
const QUOTED_FILE_PATH_PATTERN = {"source":"([\"'`])(?<path>(?:~[\\\\/]|\\.{1,2}[\\\\/]|\\/|[A-Za-z]:[\\\\/]|\\\\\\\\|[\\p{L}\\p{M}\\p{N}._ -]+[\\\\/])[^\\r\\n]*?|[\\p{L}\\p{M}\\p{N}._ -]+\\.[\\p{L}\\p{M}\\p{N}_-]+(?::\\d+){0,2})\\1(?<position>(?::\\d+){0,2})","flags":"gu"};
const TRAILING_PUNCTUATION_PATTERN = /[.,;!?]+$/;
// Paths also drop a trailing colon: compilers end `file:line:col:` with one.
const TRAILING_PATH_PUNCTUATION_PATTERN = /[.,;:!?]+$/;
// A question mark is legal in filenames; only exclude query-bearing request targets.
const REQUEST_ROUTE_PATTERN = {"source":"(?:^|[\\x{0009}-\\x{000d}\\x{0020}\\x{00a0}\\x{1680}\\x{2000}-\\x{200a}\\x{2028}\\x{2029}\\x{202f}\\x{205f}\\x{3000}\\x{feff}\"'`])(?:GET|HEAD|POST|PUT|PATCH|DELETE|OPTIONS|CONNECT|TRACE|proxying)[\\x{0009}-\\x{000d}\\x{0020}\\x{00a0}\\x{1680}\\x{2000}-\\x{200a}\\x{2028}\\x{2029}\\x{202f}\\x{205f}\\x{3000}\\x{feff}]+[\"'`]?(?:\\/|[A-Za-z0-9_-]+\\/)[^\\x{0009}-\\x{000d}\\x{0020}\\x{00a0}\\x{1680}\\x{2000}-\\x{200a}\\x{2028}\\x{2029}\\x{202f}\\x{205f}\\x{3000}\\x{feff}?]*\\?[^\\x{0009}-\\x{000d}\\x{0020}\\x{00a0}\\x{1680}\\x{2000}-\\x{200a}\\x{2028}\\x{2029}\\x{202f}\\x{205f}\\x{3000}\\x{feff}]*","flags":"g"};
// The tail of `redis://host/0` or `file:///a.mjs` is not a path. One-letter schemes stay drives,
// a diff's `+` or `-` may lead, and the lookbehind keeps runs from being rescanned at every suffix.
const SCHEME_URL_PATTERN = {"source":"(?<![A-Za-z\\d+.-])[+-]?[A-Za-z][A-Za-z\\d+.-]+:\\/\\/[^\\x{0009}-\\x{000d}\\x{0020}\\x{00a0}\\x{1680}\\x{2000}-\\x{200a}\\x{2028}\\x{2029}\\x{202f}\\x{205f}\\x{3000}\\x{feff}\"'`<>]+","flags":"gu"};

function trimClosingDelimiters(value        , kind                  )         {
  let output = value.replace(
    kind === "path" ? TRAILING_PATH_PUNCTUATION_PATTERN : TRAILING_PUNCTUATION_PATTERN,
    "",
  );
  if (output.length === 0) return output;

  const trimUnbalanced = (open        , close        ) => {
    while (output.endsWith(close)) {
      const opens = output.split(open).length - 1;
      const closes = output.split(close).length - 1;
      if (opens >= closes) return;
      output = output.slice(0, -1);
    }
  };

  trimUnbalanced("(", ")");
  trimUnbalanced("[", "]");
  trimUnbalanced("{", "}");
  return output;
}

function overlaps(a                                , b                                )          {
  return a.start < b.end && b.start < a.end;
}

function collectMatches(
  line        ,
  kind                  ,
  pattern        ,
  existing                     ,
)                      {
  const matches                      = [];
  pattern.lastIndex = 0;

  for (const rawMatch of nativeMatches(line, pattern.source, pattern.flags)) {
    const quotedPath = rawMatch.groups?.path;
    const raw =
      quotedPath === undefined ? rawMatch[0] : quotedPath + (rawMatch.groups?.position ?? "");
    const start = rawMatch.index + (quotedPath === undefined ? 0 : 1);
    if (start < 0 || raw.length === 0) continue;

    const trimmed = quotedPath === undefined ? trimClosingDelimiters(raw, kind) : raw;
    if (trimmed.length === 0) continue;
    if (kind === "path" && isTerminalUrl(trimmed)) continue;

    const candidate                    = {
      kind,
      text: trimmed,
      start,
      // A position outside quotes spans the closing quote in the terminal buffer.
      end: start + trimmed.length + (rawMatch.groups?.position ? 1 : 0),
    };

    const collides = [...existing, ...matches].some((other) => overlaps(candidate, other));
    if (collides) continue;

    matches.push(candidate);
  }

  return matches;
}

function extractTerminalLinks(line        )                      {
  const promptPath = POWERSHELL_PROMPT_PATH_PATTERN.exec(line)?.[1];
  const promptMatches                      = promptPath
    ? [{ kind: "path", text: promptPath, start: 3, end: 3 + promptPath.length }]
    : [];
  const nonPathRanges = Array.from(
    [...nativeMatches(line, REQUEST_ROUTE_PATTERN.source, REQUEST_ROUTE_PATTERN.flags), ...nativeMatches(line, SCHEME_URL_PATTERN.source, SCHEME_URL_PATTERN.flags)],
    (match) => ({ start: match.index, end: match.index + match[0].length }),
  );
  const urlMatches = collectMatches(line, "url", URL_PATTERN, promptMatches);
  const quotedMatches = collectMatches(line, "path", QUOTED_FILE_PATH_PATTERN, [
    ...promptMatches,
    ...urlMatches,
  ]);
  const pathMatches = collectMatches(line, "path", FILE_PATH_PATTERN, [
    ...promptMatches,
    ...urlMatches,
    ...quotedMatches,
  ]);
  return [...promptMatches, ...urlMatches, ...quotedMatches, ...pathMatches]
    .filter(
      (match) => match.kind === "url" || !nonPathRanges.some((range) => overlaps(match, range)),
    )
    .slice().sort((a, b) => a.start - b.start);
}

function isTerminalUrl(value        )          {
  return /^https?:\/\//iu.test(value);
}

function collectWrappedTerminalLinkLine(
  bufferLineNumber        ,
  getLine                                                                        ,
)                                 {
  const anchorLine = getLine(bufferLineNumber - 1);
  if (!anchorLine) return null;

  let startBufferLineNumber = bufferLineNumber;
  let startLine = anchorLine;

  while (startBufferLineNumber > 1 && startLine.isWrapped) {
    const previousLine = getLine(startBufferLineNumber - 2);
    if (!previousLine) return null;
    startBufferLineNumber -= 1;
    startLine = previousLine;
  }

  const segments                                   = [];
  let nextStartIndex = 0;
  let currentBufferLineNumber = startBufferLineNumber;

  while (true) {
    const currentLine = getLine(currentBufferLineNumber - 1);
    if (!currentLine) break;

    const nextLine = getLine(currentBufferLineNumber);
    const hasWrappedContinuation = nextLine?.isWrapped === true;
    const text = currentLine.translateToString(!hasWrappedContinuation);

    segments.push({
      bufferLineNumber: currentBufferLineNumber,
      text,
      startIndex: nextStartIndex,
      endIndex: nextStartIndex + text.length,
    });
    nextStartIndex += text.length;

    if (!hasWrappedContinuation) break;
    currentBufferLineNumber += 1;
  }

  return {
    text: segments.map((segment) => segment.text).join(""),
    segments,
  };
}

function isTerminalLinkActivation(
  event                                         ,
  platform = typeof navigator === "undefined" ? "" : navigator.platform,
)          {
  if (platform.length === 0) return false;
  return isMacPlatform(platform)
    ? event.metaKey && !event.ctrlKey
    : event.ctrlKey && !event.metaKey;
}

const GHOSTTY_CELL_WIDE = {
  narrow: 0,
  wide: 1,
  spacerTail: 2,
  spacerHead: 3,
}         ;
function terminalScrollbarGeometry(
  state                  ,
  trackHeight        ,
)                                   {
  const total = Math.max(0, state.total);
  const len = Math.max(0, Math.min(state.len, total));
  const maxOffset = Math.max(0, total - len);
  if (trackHeight <= 0 || len <= 0 || maxOffset === 0) return null;
  const thumbHeight = Math.min(
    trackHeight,
    Math.max(MIN_SCROLLBAR_THUMB_HEIGHT, (trackHeight * len) / total),
  );
  const travel = Math.max(0, trackHeight - thumbHeight);
  const offset = Math.max(0, Math.min(state.offset, maxOffset));
  return {
    thumbHeight,
    thumbTop: travel * (offset / maxOffset),
    maxOffset,
  };
}
function terminalScrollbarOffsetAtPointer(
  state                  ,
  trackHeight        ,
  pointerY        ,
  pointerOffset        ,
)         {
  const geometry = terminalScrollbarGeometry(state, trackHeight);
  if (geometry === null) return 0;
  const travel = Math.max(0, trackHeight - geometry.thumbHeight);
  if (travel === 0) return 0;
  const thumbTop = Math.max(0, Math.min(pointerY - pointerOffset, travel));
  return Math.round((thumbTop / travel) * geometry.maxOffset);
}
function terminalCellText(cell             )         {
  if (cell.wide === GHOSTTY_CELL_WIDE.spacerTail || cell.wide === GHOSTTY_CELL_WIDE.spacerHead) {
    return "";
  }
  return cell.text || " ";
}
function terminalRowText(row                                    , trimRight         )         {
  const text = row.cells.map(terminalCellText).join("");
  return trimRight ? text.replace(/[\u0009-\u000d\u0020\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000\ufeff]+$/, "") : text;
}
function terminalColumnOffset(row                                    , column        )         {
  let offset = 0;
  for (let cellIndex = 0; cellIndex < column; cellIndex += 1) {
    const cell = row.cells[cellIndex];
    offset += cell ? terminalCellText(cell).length : 1;
  }
  return offset;
}
function terminalColumnAtOffset(row                                    , offset        )         {
  for (let column = 0; column < row.cells.length; column += 1) {
    const nextOffset = terminalColumnOffset(row, column + 1);
    if (offset < nextOffset) return column;
  }
  return Math.max(0, row.cells.length - 1);
}
function terminalLinkAtPositionWithRange(
  rows                            ,
  rowIndex        ,
  column        ,
)                               {
  const wrappedLine = collectWrappedTerminalLinkLine(rowIndex + 1, (index) => {
    const row = rows[index];
    if (!row) return null;
    return {
      isWrapped: row.isWrapContinuation,
      translateToString: (trimRight = false) => terminalRowText(row, trimRight),
    };
  });
  if (!wrappedLine) return null;
  // A wrapped line starting above the viewport would produce a truncated, incorrect link.
  const firstSegment = wrappedLine.segments[0];
  if (firstSegment && rows[firstSegment.bufferLineNumber - 1]?.isWrapContinuation) {
    return null;
  }
  const segment = wrappedLine.segments.find((value) => value.bufferLineNumber === rowIndex + 1);
  const row = rows[rowIndex];
  if (!segment || !row) return null;
  const lastSegment = wrappedLine.segments.slice(-1)[0];
  const lastRow = lastSegment ? rows[lastSegment.bufferLineNumber - 1] : undefined;
  const continuesBelowViewport = lastRow !== undefined && lastRow.wrapsToNext;
  const hitColumn = row.cells[column]?.wide === GHOSTTY_CELL_WIDE.spacerTail ? column - 1 : column;
  const offset = segment.startIndex + terminalColumnOffset(row, hitColumn);
  for (const match of extractTerminalLinks(wrappedLine.text)) {
    if (offset >= match.start && offset < match.end) {
      if (match.end === wrappedLine.text.length && continuesBelowViewport) return null;
      const startSegment = wrappedLine.segments.find(
        (value) => match.start >= value.startIndex && match.start < value.endIndex,
      );
      const endSegment = wrappedLine.segments.find(
        (value) => match.end - 1 >= value.startIndex && match.end - 1 < value.endIndex,
      );
      const startRow = startSegment ? rows[startSegment.bufferLineNumber - 1] : undefined;
      const endRow = endSegment ? rows[endSegment.bufferLineNumber - 1] : undefined;
      if (!startSegment || !endSegment || !startRow || !endRow) return null;
      const endColumn = terminalColumnAtOffset(endRow, match.end - 1 - endSegment.startIndex);
      return {
        text: match.text,
        range: {
          start: {
            x: terminalColumnAtOffset(startRow, match.start - startSegment.startIndex),
            y: startSegment.bufferLineNumber - 1,
          },
          end: {
            x:
              endRow.cells[endColumn + 1]?.wide === GHOSTTY_CELL_WIDE.spacerTail
                ? endColumn + 1
                : endColumn,
            y: endSegment.bufferLineNumber - 1,
          },
        },
      };
    }
  }
  return null;
}
const POSITION_SUFFIX_CAPTURE_PATTERN = /:(\d+)(?::(\d+))?$/;
const POSITION_HASH_PATTERN = /^#L(\d+)(?:C(\d+))?$/i;
function isWindowsDrivePath(value        )          {
  return /^[a-zA-Z]:([/\\]|$)/.test(value);
}
function isUncPath(value        )          {
  return value.startsWith("\\\\");
}
function isWindowsAbsolutePath(value        )          {
  return isUncPath(value) || isWindowsDrivePath(value);
}
function isAbsolutePath(value        )          {
  return value.startsWith("/") || isWindowsAbsolutePath(value);
}
function isNetworkPath(value        )          {
  return /^[\\/]{2}/.test(value);
}
function splitFilePathPosition(path        , hash = "")                   {
  const suffixMatch = path.match(POSITION_SUFFIX_CAPTURE_PATTERN);
  const match = suffixMatch ?? hash.match(POSITION_HASH_PATTERN);
  if (!match?.[1]) return { path };

  const line = Number.parseInt(match[1], 10);
  const column = match[2] === undefined ? undefined : Number.parseInt(match[2], 10);
  return Object.assign({path: suffixMatch ? path.slice(0, -suffixMatch[0].length) : path}, line > 0 ? {line} : {}, column !== undefined && column > 0 ? {column} : {});
}
function formatFilePathPosition(position                  )         {
  if (!position.line) return position.path;
  return `${position.path}:${position.line}${position.column ? `:${position.column}` : ""}`;
}
function isWindowsPathStyle(value        )          {
  return isWindowsAbsolutePath(value) || /[A-Za-z]:\\/.test(value);
}
function joinPath(base        , next        , separator            )         {
  const cleanBase = base.replace(/[\\/]+$/, "");
  if (separator === "\\") {
    return `${cleanBase}\\${next.split("/").join("\\")}`;
  }
  return `${cleanBase}/${next.replace(/^\/+/, "")}`;
}
function inferHomeFromCwd(cwd        )                     {
  const posixUser = cwd.match(/^\/Users\/([^/]+)/);
  if (posixUser?.[1]) {
    return `/Users/${posixUser[1]}`;
  }

  const posixHome = cwd.match(/^\/home\/([^/]+)/);
  if (posixHome?.[1]) {
    return `/home/${posixHome[1]}`;
  }

  const windowsUser = cwd.match(/^([A-Za-z]:[\\/]Users[\\/][^\\/]+)/i);
  if (windowsUser?.[1]) {
    return windowsUser[1];
  }

  return undefined;
}
function resolvePathLinkTarget(rawPath        , cwd        )         {
  const position = splitFilePathPosition(rawPath);
  const { path } = position;

  let resolvedPath = path;
  if (path.startsWith("~/") || (isWindowsPathStyle(cwd) && path.startsWith("~\\"))) {
    const home = inferHomeFromCwd(cwd);
    if (home) {
      const separator             = isWindowsPathStyle(home) ? "\\" : "/";
      resolvedPath = joinPath(home, path.slice(2), separator);
    }
  } else if (!isAbsolutePath(path)) {
    const separator             = isWindowsPathStyle(cwd) ? "\\" : "/";
    resolvedPath = joinPath(cwd, path, separator);
  }

  return formatFilePathPosition(Object.assign({}, position, {path: resolvedPath}));
}
return {extractTerminalLinks:extractTerminalLinks,isTerminalUrl:isTerminalUrl,collectWrappedTerminalLinkLine:collectWrappedTerminalLinkLine,isTerminalLinkActivation:isTerminalLinkActivation,terminalLinkAtPositionWithRange:terminalLinkAtPositionWithRange,resolvePathLinkTarget:resolvePathLinkTarget,isNetworkPath:isNetworkPath,terminalScrollbarGeometry:terminalScrollbarGeometry,terminalScrollbarOffsetAtPointer:terminalScrollbarOffsetAtPointer};
}
