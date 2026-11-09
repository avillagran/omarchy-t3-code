"""Actual overlay admission and source rollback in retained container-only RAM."""
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
memory_limit = int(Path('/sys/fs/cgroup/memory.max').read_text().strip())
assert 3 * 1024**3 <= memory_limit <= 4 * 1024**3
ram = Path('/ram')
ram_info = os.statvfs(ram)
assert ram_info.f_blocks * ram_info.f_frsize <= 2 * 1024**3
assert ram_info.f_bavail * ram_info.f_frsize >= 1900 * 1024**2
root = Path(tempfile.mkdtemp(prefix='voice-real-overlay-', dir=ram))
os.umask(0o077)


def digest(path):
    source, _, value = voice.regular(path, immutable=False, single_link=False)
    source.close()
    return value


def fd_digest(descriptor):
    os.lseek(descriptor, 0, os.SEEK_SET)
    with os.fdopen(os.dup(descriptor), 'rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def inode(path):
    metadata = path.lstat()
    return [metadata.st_dev, metadata.st_ino]


class PostAdmissionFault(Exception):
    pass


proof = {'complete': False, 'memoryLimitBytes': memory_limit,
    'ramLimitBytes': ram_info.f_blocks * ram_info.f_frsize,
    'persistedRuntimeCopies': 0,
    'scope': 'Actual install-native-overlay, compiled client, default admission and runtime/addon. '
        'Private fixture Git revisions and explicit synthetic nonclient assets. '
        'No install.sh, compiler, native library launch, audio, display, provider or app worker acceptance.'}
try:
    client = Path('/inputs/client')
    runtime = Path('/inputs/runtime')
    addon = Path('/inputs/addon.node')
    assert client.stat().st_size == CLIENT_BYTES and digest(client) == CLIENT_SHA
    assert addon.stat().st_nlink == 1 and addon.stat().st_size == ADDON_BYTES and digest(addon) == ADDON_SHA
    voice.admit_runtime(runtime)
    source = voice.metadata_tool.prepare(addon, voice.ROOT)
    expected = voice.compiled(source)
    module = runpy.run_path('/repo/tools/install-native-overlay')
    install = module['install_overlay']
    metadata = module['PACKAGE']['client_metadata']
    assert metadata(client) == expected
    repo = root / 'installed'
    repo.mkdir(mode=0o700)
    git = module['git']
    git(repo, 'init', '-q', '-b', 'main')
    git(repo, 'config', 'user.name', 'Fixture User')
    git(repo, 'config', 'user.email', 'fixture@localhost')
    (repo / '.gitignore').write_text('/bin/\n')
    (repo / 'bin').mkdir(mode=0o700)
    (repo / 'bin/fixture-launcher').write_text('Tracked fixture source\n')
    (repo / 'source.txt').write_text('Initial fixture source\n')
    git(repo, 'add', '-f', '--', '.gitignore', 'source.txt', 'bin/fixture-launcher')
    git(repo, 'commit', '-qm', 'Initial fixture revision')
    initial = module['head'](repo)
    (repo / 'source.txt').write_text('First fixture source\n')
    (repo / 'first.txt').write_text('First tracked file\n')
    git(repo, 'add', '--', 'source.txt', 'first.txt')
    git(repo, 'commit', '-qm', 'First fixture overlay revision')
    first_revision = module['head'](repo)
    (repo / 'source.txt').write_text('Second fixture source\n')
    git(repo, 'rm', '-q', '--', 'first.txt')
    (repo / 'second.txt').write_text('Second tracked file\n')
    git(repo, 'add', '--', 'source.txt', 'second.txt')
    git(repo, 'commit', '-qm', 'Second fixture overlay revision')
    second_revision = module['head'](repo)
    git(repo, 'checkout', '-qb', 'installed', initial)
    stage = root / 'stage'
    stage.mkdir(mode=0o700)
    (stage / 'bin').mkdir(mode=0o700)
    voice.copy_member(client, stage / 'bin/omarchy-t3-client', executable=True,
        immutable=False, single_link=False)
    (stage / 'bin/omarchy-t3-client').chmod(0o755)
    synthetic_assets = [name for name in module['ASSETS'] if name != 'bin/omarchy-t3-client']
    def write_assets(version):
        for name in synthetic_assets:
            path = stage / name
            path.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
            path.write_text('Explicit synthetic ' + version + ' asset: ' + name + '\n')
            path.chmod(0o644 if name in module['TEXT_ASSETS'] else 0o755)
    write_assets('first')
    (stage / 'COMMIT').write_text(first_revision + '\n')
    (stage / 'BUILD-INFO').write_text('syntheticSourceRevision=' + first_revision + '\n')
    input_bundle = root / 'input-bundle'
    voice.stage(runtime, addon, input_bundle)
    first = install(repo, first_revision, stage, input_bundle)
    assert metadata(repo / 'bin/omarchy-t3-client', verify=True) == expected
    assert module['head'](repo) == first_revision
    assert (repo / 'source.txt').read_text() == 'First fixture source\n'
    assert (repo / 'first.txt').is_file() and not (repo / 'second.txt').exists()
    assert module['branch'](repo) == 'refs/heads/installed'
    asset_hashes = {name: digest(repo / name) for name in module['ASSETS']}
    assert asset_hashes == {name: digest(stage / name) for name in module['ASSETS']}
    original_bundle = inode(repo / 'bin/native-voice')
    bundle_names = [*voice.expected_files(source['addon']), voice.RECEIPT]
    member_inodes = {name: inode(repo / 'bin/native-voice' / name) for name in bundle_names}
    old_descriptor = os.open(repo / 'bin/omarchy-t3-client', os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW)
    proof['firstInstall'] = {'sourceBefore': initial, 'sourceAfter': first_revision,
        'defaultAdmission': expected, 'assetsSha256': asset_hashes,
        'clientSha256': digest(repo / 'bin/omarchy-t3-client'),
        'bundleDirectoryIdentity': original_bundle, 'bundleMembers': len(member_inodes)}
    write_assets('second')
    (stage / 'COMMIT').write_text(second_revision + '\n')
    new_hashes = {name: digest(stage / name) for name in module['ASSETS']}
    failed_descriptors = []
    observations = []
    def inspected(path, verify=False):
        actual = metadata(path, verify=verify)
        assert actual == expected
        observations.append({'verify': verify, 'actual': actual})
        if verify:
            assert module['head'](repo) == second_revision
            assert (repo / 'source.txt').read_text() == 'Second fixture source\n'
            assert not (repo / 'first.txt').exists() and (repo / 'second.txt').is_file()
            assert new_hashes == {name: digest(repo / name) for name in module['ASSETS']}
            assert inode(repo / 'bin/native-voice') != original_bundle
            failed_descriptors.append(os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW))
            raise PostAdmissionFault('Explicit fixture fault after successful real default admission')
        return actual
    try:
        install(repo, second_revision, stage, input_bundle, inspector=inspected)
        raise AssertionError('The explicit fixture post-admission fault must fire')
    except PostAdmissionFault:
        pass
    assert module['head'](repo) == first_revision
    assert module['branch'](repo) == 'refs/heads/installed'
    module['clean_tracked'](repo)
    assert (repo / 'source.txt').read_text() == 'First fixture source\n'
    assert (repo / 'first.txt').is_file() and not (repo / 'second.txt').exists()
    assert asset_hashes == {name: digest(repo / name) for name in module['ASSETS']}
    assert metadata(repo / 'bin/omarchy-t3-client', verify=True) == expected
    assert inode(repo / 'bin/native-voice') == original_bundle
    for name, identity in member_inodes.items():
        assert inode(repo / 'bin/native-voice' / name) == identity
    assert os.fstat(old_descriptor).st_nlink == 0
    assert len(failed_descriptors) == 1 and os.fstat(failed_descriptors[0]).st_nlink == 0
    assert fd_digest(old_descriptor) == CLIENT_SHA
    assert fd_digest(failed_descriptors[0]) == CLIENT_SHA
    assert voice.verify(repo / 'bin/native-voice') == source
    failed_bundles = list((repo / '.git').glob('o3-overlay-*/failed-bundle'))
    assert len(failed_bundles) == 1 and voice.verify(failed_bundles[0]) == source
    voice.admit_runtime(runtime)
    assert digest(client) == CLIENT_SHA and digest(addon) == ADDON_SHA
    used = os.statvfs(ram)
    proof.update(complete=True, syntheticAssets=synthetic_assets,
        actualNativeLibraryLaunches=0, runtimeCopiesInPrivateRam=3,
        rollback={'sourceRestored': first_revision, 'failedSource': second_revision,
            'assetsSha256': asset_hashes, 'bundleDirectoryRestored': True,
            'allBundleMemberInodesRestored': len(member_inodes),
            'runtimeMemberInodesRestored': 72,
            'oldRunningInodeLinks': os.fstat(old_descriptor).st_nlink,
            'failedNewRunningInodeLinks': os.fstat(failed_descriptors[0]).st_nlink,
            'realInspections': observations},
        ramUsedBytesBeforeExit=(used.f_blocks - used.f_bfree) * used.f_frsize,
        sourceClientLinks=client.stat().st_nlink, sourceAddonLinks=addon.stat().st_nlink,
        retainedRamUntilContainerExit=True)
    peak = Path('/sys/fs/cgroup/memory.peak')
    if peak.exists():
        proof['containerMemoryPeakBytes'] = int(peak.read_text().strip())
    print(json.dumps(proof), flush=True)
except BaseException as error:
    proof['failure'] = type(error).__name__ + ': ' + str(error)
    print(json.dumps(proof), flush=True)
    raise
