"""Bounded native voice package operations; no discovery or runtime activation."""
import hashlib
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import stat
import tarfile
import uuid

ROOT = Path(__file__).resolve().parent.parent
RUNTIME = 'electron-44.10.1-linux-x64'
ADDON = 'electron-private-fd.node'
RECEIPT = 'build-metadata.json'
LICENSE = 'LICENSE.o3-code'
PREFIX = 'bin/native-voice/'
loader = importlib.machinery.SourceFileLoader('voice_build_metadata',
    str(ROOT / 'tools/prepare-native-voice-build-metadata'))
spec = importlib.util.spec_from_loader(loader.name, loader)
metadata_tool = importlib.util.module_from_spec(spec)
loader.exec_module(metadata_tool)


def manifest():
    value = json.loads((ROOT / 'ui/voice/electron-runtime-44.10.1-linux-x64.json').read_text())
    metadata_tool.validate_manifest(value)
    return value


def digest(data):
    return hashlib.sha256(data).hexdigest()


def identity(metadata, immutable=True, single_link=True):
    if (not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.geteuid()
            or single_link and metadata.st_nlink != 1 or immutable and metadata.st_mode & 0o222):
        raise ValueError('Expected an owned single-link regular file without write bits')
    return (metadata.st_dev, metadata.st_ino, metadata.st_size, metadata.st_mode,
        metadata.st_mtime_ns, metadata.st_ctime_ns)


def canonical(path):
    path = Path(path)
    if not path.is_absolute() or path.resolve(strict=True) != path:
        raise ValueError('An absolute canonical path without symlinks is required')
    return path


def directory(path, private=False):
    path = canonical(path)
    metadata = path.lstat()
    if (not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != os.geteuid()
            or metadata.st_mode & (0o077 if private else 0o022)):
        raise ValueError('Invalid package directory ownership or permissions')
    return path


def regular(path, pin=None, immutable=True, single_link=True):
    """Retain and check one nofollow descriptor; callers own the returned file."""
    path = canonical(path)
    before = identity(path.lstat(), immutable, single_link)
    fd = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
    file = os.fdopen(fd, 'rb')
    try:
        if identity(os.fstat(fd), immutable, single_link) != before:
            raise ValueError('Package input changed while opening')
        h = hashlib.file_digest(file, 'sha256').hexdigest()
        if pin and (h != pin['sha256'] or before[2] != pin['bytes']):
            raise ValueError('Package member digest or size mismatch')
        if pin and 'executable' in pin and bool(before[3] & 0o111) != pin['executable']:
            raise ValueError('Package input executable mode differs from its pin')
        if identity(os.fstat(fd), immutable, single_link) != before or identity(path.lstat(), immutable, single_link) != before:
            raise ValueError('Package input changed while hashing')
        file.seek(0)
        return file, before, h
    except Exception:
        file.close()
        raise


def stable(file, path, before, immutable=True, single_link=True):
    if (identity(os.fstat(file.fileno()), immutable, single_link) != before
            or identity(Path(path).lstat(), immutable, single_link) != before
            or Path(path).resolve(strict=True) != Path(path)):
        raise ValueError('Package member changed during transfer')


def runtime_rows(runtime):
    runtime = directory(runtime)
    files = manifest()['files']
    expected = {row['path'] for row in files}
    expected_dirs = {str(parent) for name in expected for parent in PurePosixPath(name).parents
        if str(parent) != '.'}
    actual, actual_dirs = set(), set()
    for current, children, names in os.walk(runtime, followlinks=False):
        current = Path(current)
        directory(current)
        for child in children:
            item = current / child
            directory(item)
            actual_dirs.add(str(item.relative_to(runtime)))
        actual.update(str((current / name).relative_to(runtime)) for name in names)
    if actual != expected or actual_dirs != expected_dirs:
        raise ValueError('Native voice runtime must contain exactly the 72 pinned members')
    return files


def admit_runtime(runtime):
    rows = runtime_rows(runtime)
    for row in rows:
        file, _, _ = regular(runtime / row['path'], row)
        file.close()
    return rows


def copy_member(source, target, pin=None, executable=False, immutable=True, single_link=True):
    if immutable and not single_link:
        raise ValueError('Admitted immutable artifacts must always have one link')
    file, before, h = regular(source, pin, immutable, single_link)
    try:
        with target.open('xb') as output:
            os.chmod(target, 0o600)
            while data := file.read(1024 * 1024):
                output.write(data)
            output.flush()
            os.fsync(output.fileno())
        stable(file, source, before, immutable, single_link)
        os.chmod(target, 0o500 if executable else 0o400)
    finally:
        file.close()
    return {'sha256': h, 'bytes': before[2]}


def expected_files(addon):
    files = {RUNTIME + '/' + row['path']: row for row in manifest()['files']}
    files[ADDON] = {**addon, 'executable': True}
    license_bytes = (ROOT / 'LICENSE').read_bytes()
    files[LICENSE] = {'sha256': digest(license_bytes), 'bytes': len(license_bytes), 'executable': False}
    return files


def stage(runtime, addon, destination):
    """Copy only enumerated application files into a brand-new private bundle."""
    runtime = directory(runtime)
    rows = runtime_rows(runtime)
    receipt = metadata_tool.prepare(addon, ROOT)
    destination = Path(destination)
    directory(destination.parent)
    destination.mkdir(mode=0o700)
    target_runtime = destination / RUNTIME
    target_runtime.mkdir(mode=0o700)
    for row in rows:
        target = target_runtime / row['path']
        target.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
        copy_member(runtime / row['path'], target, row, row['executable'])
    copy_member(addon, destination / ADDON, receipt['addon'], True)
    copy_member(ROOT / 'LICENSE', destination / LICENSE, immutable=False)
    metadata_bytes = (json.dumps(receipt, indent=2, sort_keys=True) + '\n').encode()
    with (destination / RECEIPT).open('xb') as output:
        output.write(metadata_bytes)
    (destination / RECEIPT).chmod(0o400)
    verify(destination)
    return receipt


def verify(bundle):
    bundle = directory(bundle, private=True)
    if {path.name for path in bundle.iterdir()} != {RUNTIME, ADDON, RECEIPT, LICENSE}:
        raise ValueError('Unexpected native voice bundle members')
    receipt_file, before, _ = regular(bundle / RECEIPT)
    try:
        if before[2] > 65536:
            raise ValueError('Oversized native voice build metadata')
        receipt = json.load(receipt_file)
        stable(receipt_file, bundle / RECEIPT, before)
    finally:
        receipt_file.close()
    actual = metadata_tool.prepare(bundle / ADDON, ROOT)
    if receipt != actual:
        raise ValueError('Native voice build metadata does not match these package bytes')
    runtime_rows(bundle / RUNTIME)
    for name, pin in expected_files(actual['addon']).items():
        file, _, _ = regular(bundle / name, pin)
        file.close()
        if bool((bundle / name).lstat().st_mode & 0o111) != pin['executable']:
            raise ValueError('Invalid native voice member executable mode')
    return actual


def compiled(receipt):
    return {'schema': 1, 'target': 'linux-x64', 'runtimeVersion': '44.10.1',
        'addonSha256': receipt['addon']['sha256'], 'addonBytes': receipt['addon']['bytes']}


def activate(bundle, destination):
    """Replace only an admitted bundle, retaining the old directory for recovery."""
    bundle = directory(bundle, private=True)
    verify(bundle)
    destination = Path(destination)
    directory(destination.parent)
    if destination.name != 'native-voice':
        raise ValueError('Only the fixed native-voice bundle destination may be activated')
    if bundle.stat().st_dev != destination.parent.stat().st_dev:
        # Downloads may live on another filesystem. Copy the enumerated files
        # into a new sibling before any active bundle is renamed.
        local = destination.parent / ('.native-voice-stage-' + uuid.uuid4().hex)
        stage(bundle / RUNTIME, bundle / ADDON, local)
        bundle = local
    retired = None
    if destination.exists() or destination.is_symlink():
        # Old releases can have a different pin. Inspect the directory itself;
        # rename it intact without traversing, modifying or deleting its files.
        directory(destination, private=True)
        retired = destination.parent / ('.native-voice-retired-' + uuid.uuid4().hex)
        destination.rename(retired)
    try:
        bundle.rename(destination)
    except Exception:
        if retired is not None:
            retired.rename(destination)
        raise
    return retired


def add_file(archive, source, name, pin=None, executable=False, immutable=True):
    file, before, _ = regular(source, pin, immutable)
    try:
        member = tarfile.TarInfo(name)
        member.size = before[2]
        member.mode = (0o500 if executable else 0o400) if immutable else (0o755 if executable else 0o644)
        member.mtime = 0
        archive.addfile(member, file)
        stable(file, source, before, immutable)
    finally:
        file.close()


def add_bundle(archive, bundle):
    receipt = verify(bundle)
    for name, pin in expected_files(receipt['addon']).items():
        add_file(archive, bundle / name, PREFIX + name, pin, pin['executable'])
    add_file(archive, bundle / RECEIPT, PREFIX + RECEIPT)
    return receipt


def extract(archive_path, destination):
    """Extract this application subset only; never follow archive links or paths."""
    destination = Path(destination)
    directory(destination.parent, private=True)
    archive_file, before, _ = regular(archive_path, immutable=False)
    try:
        with tarfile.open(fileobj=archive_file, mode='r:gz') as archive:
            selected = {}
            count, total = 0, 0
            for member in archive:
                count += 1
                total += member.size
                if member.size < 0 or count > 256 or total > 1024 * 1024 * 1024:
                    raise ValueError('Native overlay exceeds its package bounds')
                if not member.name.startswith(PREFIX):
                    continue
                name = member.name[len(PREFIX):]
                if not name and member.isdir():
                    continue
                if member.isdir():
                    continue
                if name in selected or not member.isfile() or member.size > 512 * 1024 * 1024:
                    raise ValueError('Invalid or duplicate native voice archive member')
                selected[name] = member
            if not selected:
                return None
            metadata_member = selected.get(RECEIPT)
            if metadata_member is None or metadata_member.size > 65536:
                raise ValueError('Missing or oversized native voice archive build metadata')
            metadata_bytes = archive.extractfile(metadata_member).read()
            receipt = json.loads(metadata_bytes)
            addon = receipt.get('addon', {})
            if not metadata_tool._sha256(addon.get('sha256')) or type(addon.get('bytes')) is not int or not 0 < addon['bytes'] <= metadata_tool.MAX_ADDON_BYTES:
                raise ValueError('Invalid native voice addon archive pin')
            files = expected_files(addon)
            if set(selected) != {*files, RECEIPT}:
                raise ValueError('Unexpected or missing native voice archive members')
            destination.mkdir(mode=0o700)
            for name, member in selected.items():
                pin = files.get(name)
                if pin and member.size != pin['bytes']:
                    raise ValueError('Invalid native voice archive member size')
                target = destination / name
                target.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
                h = hashlib.sha256()
                fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
                with os.fdopen(fd, 'wb') as output, archive.extractfile(member) as source:
                    while data := source.read(1024 * 1024):
                        output.write(data)
                        h.update(data)
                    output.flush()
                    os.fsync(output.fileno())
                if pin and h.hexdigest() != pin['sha256']:
                    raise ValueError('Native voice archive member digest mismatch')
                target.chmod(0o500 if pin and pin['executable'] else 0o400)
        stable(archive_file, archive_path, before, immutable=False)
    finally:
        archive_file.close()
    verify(destination)
    return receipt
