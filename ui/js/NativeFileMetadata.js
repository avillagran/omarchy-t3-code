.pragma library
// Pinned native metadata query and path functions, 32a0531748c44b25072d7495ce0b9bd7e9450a87.
function create(){
const projectEnvironment={fileMetadata:function(value){return value;}};
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
function isRootPath(value        )          {
  // Bare C: means the current directory on C, not its root; require the drive separator.
  return value === "/" || value === "\\" || /^[a-zA-Z]:[/\\]$/.test(value);
}
function trimTrailingPathSeparators(value        )         {
  if (value.length === 0 || isRootPath(value)) {
    return value;
  }
  const trimmed = value.startsWith("/")
    ? value.replace(/\/+$/g, "")
    : value.replace(/[\\/]+$/g, "");
  if (trimmed.length === 0) {
    return value;
  }
  return /^[a-zA-Z]:$/.test(trimmed) ? `${trimmed}\\` : trimmed;
}
function normalizeProjectPathForDispatch(value        )         {
  return trimTrailingPathSeparators(value.trim());
}
function normalizeProjectPathForComparison(value        )         {
  const normalized = normalizeProjectPathForDispatch(value);
  if (isWindowsDrivePath(normalized) || isUncPath(normalized)) {
    return normalized.split("/").join("\\").toLowerCase();
  }
  return normalized;
}
function resolveWorkspaceFilePath(path        , workspaceRoot        )         {
  if (isAbsolutePath(path) || path.startsWith("~/") || path.startsWith("~\\")) return path;
  const base = workspaceRoot.replace(/[/\\]+$/, "");
  return isWindowsAbsolutePath(workspaceRoot)
    ? `${base}\\${path.split("/").join("\\")}`
    : `${base}/${path}`;
}
const POSITION_SUFFIX_CAPTURE_PATTERN = /:(\d+)(?::(\d+))?$/;
const POSITION_HASH_PATTERN = /^#L(\d+)(?:C(\d+))?$/i;
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

  return formatFilePathPosition(Object.assign({},position,{path:resolvedPath}));
}
function getFileMetadataQueryAtom(
  environmentId               ,
  path        ,
  cwd         ,
  threadRef                         ,
) {
  const hostPath = splitFilePathPosition(path).path;
  const targetPath = normalizeProjectPathForComparison(
    hostPath.startsWith("~/") || !cwd ? hostPath : resolvePathLinkTarget(hostPath, cwd),
  );
  const absolute = isAbsolutePath(targetPath) || targetPath.startsWith("~/");
  return (
    // Servers stat what they are asked about, and a network path leaks the
    // Windows user's NTLM credentials to its host before any chip is clicked.
    absolute && !isNetworkPath(targetPath) && targetPath.length <= 512
      ? projectEnvironment.fileMetadata({
          environmentId,
          input: Object.assign({path:targetPath},threadRef?.environmentId===environmentId?{threadId:threadRef.threadId}:{}),
        })
      : null
  );
}
return {query:getFileMetadataQueryAtom,normalize:normalizeProjectPathForComparison,resolveLink:resolvePathLinkTarget,resolveWorkspace:resolveWorkspaceFilePath,split:splitFilePathPosition};
}
