"""Retain immutable runtime artifacts before releasing the shared compilation lock."""
import errno
import fcntl
import hashlib
import os
import pathlib
import shutil
import stat


def snapshot(debug, artifacts, destination, *, reflink=False):
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
            before = os.fstat(original.fileno())
            mode = before.st_mode
            if not stat.S_ISREG(mode):
                raise ValueError("Runtime artifact is not a regular file")
            destination_fd = os.open(target, os.O_RDWR | os.O_CREAT | os.O_EXCL |
                                     os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
            with os.fdopen(destination_fd, "w+b") as copied:
                created = os.fstat(copied.fileno())
                if (created.st_dev, created.st_ino) == (before.st_dev, before.st_ino):
                    raise ValueError("Runtime snapshot must have a distinct inode")
                cloned = False
                if reflink:
                    try:
                        # Linux FICLONE creates independent CoW extents, never a
                        # hardlink to a mutable Cargo output.
                        fcntl.ioctl(copied.fileno(), 0x40049409, original.fileno())
                        cloned = True
                    except OSError as error:
                        if error.errno not in (errno.EXDEV, errno.EOPNOTSUPP,
                                             errno.ENOTTY, errno.EINVAL, errno.ENOSYS):
                            raise
                        # Reserve the complete fallback before copying. If the
                        # filesystem cannot reserve it, retain the failed output
                        # and refuse; free-space estimates are insufficient.
                        os.ftruncate(copied.fileno(), 0)
                        if before.st_size:
                            os.posix_fallocate(copied.fileno(), 0, before.st_size)
                if not cloned:
                    shutil.copyfileobj(original, copied, length=1024 * 1024)
                copied.flush()
                after = os.fstat(original.fileno())
                identity = lambda item: (item.st_dev, item.st_ino, item.st_size,
                                         item.st_mtime_ns, item.st_ctime_ns, item.st_mode)
                if identity(before) != identity(after):
                    raise ValueError("Runtime source changed during snapshot")
                if os.fstat(copied.fileno()).st_size != before.st_size:
                    raise ValueError("Runtime snapshot size differs from its source")
                os.fchmod(copied.fileno(), 0o500 if mode & 0o111 else 0o400)
                copied.seek(0)
                digest = hashlib.file_digest(copied, "sha256").hexdigest()
                if identity(before) != identity(os.fstat(original.fileno())):
                    raise ValueError("Runtime source changed during snapshot hashing")
                current = target.lstat()
                if (current.st_dev, current.st_ino) != (created.st_dev, created.st_ino):
                    raise ValueError("Runtime snapshot path was replaced")
        record = {"path": str(relative), "sha256": digest, "bytes": before.st_size}
        if reflink:
            record["copyMode"] = "reflink" if cloned else "reserved-copy"
        records.append(record)
    return records
