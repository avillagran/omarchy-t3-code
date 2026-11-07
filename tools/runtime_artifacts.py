"""Retain immutable runtime artifacts before releasing the shared compilation lock."""
import hashlib
import os
import pathlib
import shutil
import stat


def snapshot(debug, artifacts, destination):
    """Copy selected Cargo executables and runtime libraries, never directory trees."""
    debug = pathlib.Path(debug)
    destination = pathlib.Path(destination)
    destination.mkdir(mode=0o700)
    candidates = set(pathlib.Path(path) for path in artifacts)
    # Integration binaries embed CARGO_BIN_EXE paths to top-level executables.
    candidates.update(path for path in debug.iterdir()
                      if stat.S_ISREG(path.lstat().st_mode) and path.lstat().st_mode & 0o111)
    candidates.update((debug / "deps").glob("*.so"))
    records = []
    for source in sorted(candidates):
        relative = source.relative_to(debug)
        if ".." in relative.parts or source.is_symlink():
            raise ValueError("Runtime artifact is not an owned regular file")
        target = destination / relative
        target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        source_fd = os.open(source, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
        with os.fdopen(source_fd, "rb") as original:
            mode = os.fstat(original.fileno()).st_mode
            if not stat.S_ISREG(mode):
                raise ValueError("Runtime artifact is not a regular file")
            with target.open("xb") as copied:
                shutil.copyfileobj(original, copied, length=1024 * 1024)
        target.chmod(0o500 if mode & 0o111 else 0o400)
        with target.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        records.append({"path": str(relative), "sha256": digest, "bytes": target.stat().st_size})
    return records
