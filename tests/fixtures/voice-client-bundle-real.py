"""Actual client/bundle transaction in private RAM; no installer, audio or display."""
import hashlib
import json
import os
from pathlib import Path
import runpy
import sys
import tempfile

sys.path.insert(0, '/repo/tools')
import native_voice_bundle as voice

CLIENT_SHA = 'db084d705ee1792b87ee54cf861acc379f2b620c348b9ae851ae071e6e914e6f'
CLIENT_BYTES = 215727792
ADDON_SHA = '6cbce632628b75d8e214d76a0f84fdc543071a9e8f3b0406746079a08aa1ccc2'
ADDON_BYTES = 16408
assert Path('/run/.containerenv').exists()
assert Path('/sys/fs/cgroup/memory.max').exists(), 'Container memory limit must be observable'
memory_limit = int(Path('/sys/fs/cgroup/memory.max').read_text().strip())
assert 3 * 1024**3 <= memory_limit <= 4 * 1024**3
ram = Path('/ram')
ram_info = os.statvfs(ram)
assert ram_info.f_blocks * ram_info.f_frsize <= 2 * 1024**3
assert ram_info.f_bavail * ram_info.f_frsize >= 1600 * 1024**2
root = Path(tempfile.mkdtemp(prefix='voice-real-pair-', dir=ram))
os.umask(0o077)


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def fd_digest(descriptor):
    os.lseek(descriptor, 0, os.SEEK_SET)
    with os.fdopen(os.dup(descriptor), 'rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def inode(path):
    metadata = path.stat()
    return [metadata.st_dev, metadata.st_ino]


class PostAdmissionFault(Exception):
    pass


proof = {'complete': False, 'memoryLimitBytes': memory_limit,
    'ramLimitBytes': ram_info.f_blocks * ram_info.f_frsize,
    'persistedRuntimeCopies': 0, 'scope': 'Actual install-client helper only in private tmpfs. '
        'Real compiled client/default admission and unchanged runtime/addon. '
        'No install.sh, source checkout, other libraries, hardware, provider or app worker acceptance.'}
try:
    client = Path('/inputs/client')
    runtime = Path('/inputs/runtime')
    addon = Path('/inputs/addon.node')
    assert client.stat().st_size == CLIENT_BYTES and digest(client) == CLIENT_SHA
    assert addon.stat().st_nlink == 1 and addon.stat().st_size == ADDON_BYTES and digest(addon) == ADDON_SHA
    voice.admit_runtime(runtime)
    source = voice.metadata_tool.prepare(addon, voice.ROOT)
    expected = voice.compiled(source)
    tool = runpy.run_path('/repo/tools/package-native-voice')
    install = tool['install_client']
    metadata = tool['client_metadata']
    assert metadata(client) == expected
    binary_dir = root / 'bin'
    binary_dir.mkdir(mode=0o700)
    destination = binary_dir / 'omarchy-t3-client'
    first_bundle = root / 'first-bundle'
    voice.stage(runtime, addon, first_bundle)
    first = install(client, first_bundle, destination, cargo_output=True)
    assert metadata(destination, verify=True) == expected
    assert digest(destination) == CLIENT_SHA and destination.stat().st_nlink == 1
    original_bundle = inode(binary_dir / 'native-voice')
    original_members = {str(path.relative_to(binary_dir / 'native-voice')): inode(path)
        for path in (binary_dir / 'native-voice' / voice.RUNTIME).rglob('*') if path.is_file()}
    old_descriptor = os.open(destination, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW)
    proof['firstInstall'] = {'defaultAdmission': expected, 'clientSha256': digest(destination),
        'retiredClient': first['retiredClient'], 'retiredBundle': first['retiredBundle']}
    second_bundle = root / 'second-bundle'
    voice.stage(runtime, addon, second_bundle)
    observations = []
    new_descriptors = []

    def inspected(path, verify=False):
        actual = metadata(path, verify=verify)
        assert actual == expected
        observations.append({'verify': verify, 'actual': actual})
        if verify:
            assert inode(binary_dir / 'native-voice') != original_bundle
            new_descriptors.append(os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW))
            raise PostAdmissionFault('Explicit fixture fault after successful real default admission')
        return actual

    install.__globals__['client_metadata'] = inspected
    try:
        install(client, second_bundle, destination, cargo_output=True)
        raise AssertionError('Expected the fixture post-admission fault')
    except PostAdmissionFault:
        pass
    finally:
        install.__globals__['client_metadata'] = metadata
    assert metadata(destination, verify=True) == expected
    assert digest(destination) == CLIENT_SHA
    assert inode(binary_dir / 'native-voice') == original_bundle
    for name, identity in original_members.items():
        assert inode(binary_dir / 'native-voice' / name) == identity
    assert os.fstat(old_descriptor).st_nlink == 0
    assert len(new_descriptors) == 1 and os.fstat(new_descriptors[0]).st_nlink == 0
    assert fd_digest(old_descriptor) == CLIENT_SHA
    assert fd_digest(new_descriptors[0]) == CLIENT_SHA
    assert voice.verify(binary_dir / 'native-voice') == source
    failed = list(binary_dir.glob('.native-voice-transaction-*/failed-bundle'))
    assert len(failed) == 1 and voice.verify(failed[0]) == source
    voice.admit_runtime(runtime)
    assert digest(client) == CLIENT_SHA and digest(addon) == ADDON_SHA
    used = os.statvfs(ram)
    proof.update({'complete': True, 'rollback': {'oldClientSha256': CLIENT_SHA,
        'defaultAdmission': expected, 'bundleDirectoryRestored': True,
        'runtimeMemberInodesRestored': len(original_members),
        'oldRunningInodeLinks': os.fstat(old_descriptor).st_nlink,
        'failedNewRunningInodeLinks': os.fstat(new_descriptors[0]).st_nlink,
        'realInspections': observations},
        'runtimeCopiesInPrivateRam': 2,
        'ramUsedBytesBeforeExit': (used.f_blocks - used.f_bfree) * used.f_frsize,
        'sourceClientLinks': client.stat().st_nlink,
        'sourceAddonLinks': addon.stat().st_nlink,
        'retainedRamUntilContainerExit': True})
    peak = Path('/sys/fs/cgroup/memory.peak')
    if peak.exists():
        proof['containerMemoryPeakBytes'] = int(peak.read_text().strip())
    print(json.dumps(proof), flush=True)
except BaseException as error:
    proof['failure'] = type(error).__name__ + ': ' + str(error)
    print(json.dumps(proof), flush=True)
    raise
