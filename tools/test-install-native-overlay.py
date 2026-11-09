"""Joint overlay/source transactions in retained private RAM; no real installer."""
import hashlib
import io
import json
import os
from pathlib import Path
import runpy
import struct
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock
import native_voice_bundle as voice

MODULE = runpy.run_path(str(voice.ROOT / 'tools/install-native-overlay'))


class OverlayTests(unittest.TestCase):
    def setUp(self):
        self.assertTrue(Path('/run/.containerenv').exists())
        self.root = Path(tempfile.mkdtemp(prefix='overlay-source-', dir='/dev/shm'))
        self.handles = []
        self.repo = self.root / 'repo'
        self.repo.mkdir(mode=0o700)
        self.raw_git('init', '-q', '-b', 'main')
        self.raw_git('config', 'user.name', 'Fixture User')
        self.raw_git('config', 'user.email', 'fixture@localhost')
        (self.repo / '.gitignore').write_text('bin/\nadded.txt\nremoved.txt\n*.ignored\n')
        (self.repo / 'README.md').write_text('Old fixture source\n')
        (self.repo / 'removed.txt').write_text('Old tracked file\n')
        (self.repo / 'source-link').symlink_to('README.md')
        (self.repo / 'dir-to-file').mkdir(mode=0o700)
        (self.repo / 'dir-to-file/known.txt').write_text('Old known child\n')
        (self.repo / 'file-to-dir').write_text('Old regular file\n')
        self.raw_git('add', '-f', '--', '.gitignore', 'README.md', 'removed.txt',
            'source-link', 'dir-to-file/known.txt', 'file-to-dir')
        self.raw_git('commit', '-qm', 'Old fixture source')
        self.old = self.raw_git('rev-parse', 'HEAD').stdout.decode().strip()
        (self.repo / 'README.md').write_text('New fixture source\n')
        (self.repo / 'added.txt').write_text('New tracked file\n')
        self.raw_git('rm', '-q', '--', 'removed.txt', 'dir-to-file/known.txt', 'file-to-dir')
        if (self.repo / 'dir-to-file').exists():
            (self.repo / 'dir-to-file').rmdir()
        (self.repo / 'dir-to-file').write_text('New regular file\n')
        (self.repo / 'file-to-dir').mkdir(mode=0o700)
        (self.repo / 'file-to-dir/known.txt').write_text('New known child\n')
        (self.repo / 'source-link').unlink()
        (self.repo / 'source-link').symlink_to('added.txt')
        self.raw_git('add', '-f', '--', 'README.md', 'added.txt', 'source-link',
            'dir-to-file', 'file-to-dir/known.txt')
        self.raw_git('commit', '-qm', 'Expected fixture source')
        self.expected = self.raw_git('rev-parse', 'HEAD').stdout.decode().strip()
        self.raw_git('checkout', '-qb', 'installed', self.old)
        runtime = self.root / 'runtime'
        runtime.mkdir(mode=0o700)
        rows = []
        for index in range(72):
            name = 'electron' if index == 0 else 'member-' + str(index)
            data = ('fixture-' + str(index)).encode()
            path = runtime / name
            path.write_bytes(data)
            path.chmod(0o500 if index == 0 else 0o400)
            rows.append(dict(path=name, sha256=hashlib.sha256(data).hexdigest(),
                bytes=len(data), executable=index == 0))
        self.manifest_patch = mock.patch.object(voice, 'manifest', return_value={'files': rows})
        self.manifest_patch.start()
        addon = self.root / voice.ADDON
        data = bytearray(64)
        data[:7] = b'\x7fELF\x02\x01\x01'
        struct.pack_into('<HHI', data, 16, 3, 62, 1)
        struct.pack_into('<H', data, 52, 64)
        addon.write_bytes(data + b'old')
        addon.chmod(0o500)
        self.old_bundle = self.root / 'old-bundle'
        self.old_receipt = voice.stage(runtime, addon, self.old_bundle)
        (self.repo / 'bin').mkdir(mode=0o700)
        voice.activate(self.old_bundle, self.repo / 'bin/native-voice')
        addon.chmod(0o600)
        addon.write_bytes(data + b'new')
        addon.chmod(0o500)
        self.bundle = self.root / 'new-bundle'
        self.new_receipt = voice.stage(runtime, addon, self.bundle)
        self.stage = self.root / 'stage'
        self.stage.mkdir(mode=0o700)
        (self.stage / 'COMMIT').write_text(self.expected + '\n')
        (self.stage / 'BUILD-INFO').write_text('version=fixture\n')
        self.old_bytes = {}
        self.new_bytes = {}
        for name in MODULE['ASSETS']:
            old_data = ('old:' + name).encode()
            new_data = ('new:' + name).encode()
            if name == 'bin/omarchy-t3-client':
                old_data = json.dumps(voice.compiled(self.old_receipt)).encode()
                new_data = json.dumps(voice.compiled(self.new_receipt)).encode()
            for parent, value in [(self.repo, old_data), (self.stage, new_data)]:
                path = parent / name
                path.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
                path.write_bytes(value)
                path.chmod(0o644 if name in MODULE['TEXT_ASSETS'] else 0o755)
            self.old_bytes[name] = old_data
            self.new_bytes[name] = new_data

    def tearDown(self):
        for handle in self.handles:
            handle.close()
        self.manifest_patch.stop()

    def raw_git(self, *arguments):
        environment = {key: value for key, value in os.environ.items() if not key.startswith('GIT_')}
        environment.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL='/dev/null')
        return subprocess.run(['git', '-C', str(self.repo), *arguments], env=environment,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=20)

    def inspect(self, path, verify=False):
        value = json.loads(path.read_text())
        if verify:
            self.assertEqual(value, voice.compiled(voice.verify(path.parent / 'native-voice')))
        return value

    def install(self, checkpoint=None, inspector=None):
        return MODULE['install_overlay'](self.repo, self.expected, self.stage, self.bundle,
            inspector or self.inspect, checkpoint)

    def assert_old(self):
        self.assertEqual(MODULE['head'](self.repo), self.old)
        self.assertEqual(MODULE['branch'](self.repo), 'refs/heads/installed')
        self.assertEqual((self.repo / 'README.md').read_text(), 'Old fixture source\n')
        self.assertEqual((self.repo / 'removed.txt').read_text(), 'Old tracked file\n')
        self.assertFalse((self.repo / 'added.txt').exists())
        self.assertEqual(os.readlink(self.repo / 'source-link'), 'README.md')
        for name, data in self.old_bytes.items():
            self.assertEqual((self.repo / name).read_bytes(), data, name)
        self.assertEqual(voice.verify(self.repo / 'bin/native-voice'), self.old_receipt)

    def test_success_advances_complete_overlay_and_source_without_running_hooks(self):
        marker = self.root / 'hook-ran'
        hooks = self.root / 'hooks'
        hooks.mkdir(mode=0o700)
        for hook in [hooks / 'post-merge', self.root / 'fsmonitor']:
            hook.write_text('#!/usr/bin/python3\nfrom pathlib import Path\nPath(' + repr(str(marker)) + ').write_text("unexpected")\n')
            hook.chmod(0o755)
        self.raw_git('config', 'core.hooksPath', str(hooks))
        self.raw_git('config', 'core.fsmonitor', str(self.root / 'fsmonitor'))
        user = self.repo / 'user-untracked.txt'
        user.write_text('User data\n')
        running = (self.repo / 'bin/omarchy-t3-client').open('rb')
        self.handles.append(running)
        result = self.install()
        self.assertEqual(MODULE['head'](self.repo), self.expected)
        self.assertEqual(result['assets'], len(MODULE['ASSETS']))
        self.assertEqual(os.fstat(running.fileno()).st_nlink, 0)
        for name, data in self.new_bytes.items():
            self.assertEqual((self.repo / name).read_bytes(), data)
            self.assertEqual((Path(result['transaction']) / 'old' / name).read_bytes(), self.old_bytes[name])
        self.assertEqual(user.read_text(), 'User data\n')
        self.assertFalse(marker.exists())

    def test_staging_copy_failure_changes_no_active_asset_or_source(self):
        actual = voice.copy_member
        def copy(source, destination, *args, **kwargs):
            if source == self.stage / 'bin/libt3-browser.so':
                destination.write_bytes(b'partial staging bytes')
                raise OSError('fixture_copy_failed')
            return actual(source, destination, *args, **kwargs)
        with mock.patch.object(voice, 'copy_member', copy):
            with self.assertRaisesRegex(OSError, 'fixture_copy_failed'):
                self.install()
        self.assert_old()

    def test_failure_after_source_advance_restores_all_libraries_client_bundle_source(self):
        observed = []
        def checkpoint(phase, _plan):
            if phase == 'assets-installed':
                handle = (self.repo / 'bin/omarchy-t3-client').open('rb')
                self.handles.append(handle)
                observed.append(handle)
            if phase == 'source-advanced':
                raise RuntimeError('fixture_after_source_advance')
        with self.assertRaisesRegex(RuntimeError, 'fixture_after_source_advance'):
            self.install(checkpoint=checkpoint)
        self.assert_old()
        self.assertEqual(os.fstat(observed[0].fileno()).st_nlink, 0)

    def test_post_admission_failure_restores_complete_previous_revision(self):
        def inspector(path, verify=False):
            result = self.inspect(path, verify)
            if verify:
                self.assertEqual(MODULE['head'](self.repo), self.expected)
                raise RuntimeError('fixture_post_admission')
            return result
        with self.assertRaisesRegex(RuntimeError, 'fixture_post_admission'):
            self.install(inspector=inspector)
        self.assert_old()

    def test_ignored_new_source_collision_blocks_before_mutation(self):
        path = self.repo / 'added.txt'
        path.write_text('Ignored user data\n')
        with self.assertRaises(MODULE['FenceChanged']):
            self.install()
        self.assertEqual(path.read_text(), 'Ignored user data\n')
        self.assertEqual(MODULE['head'](self.repo), self.old)
        for name, data in self.old_bytes.items():
            self.assertEqual((self.repo / name).read_bytes(), data)

    def test_ignored_collision_at_deleted_tracked_path_refuses_destructive_rollback(self):
        def checkpoint(phase, _plan):
            if phase == 'source-advanced':
                (self.repo / 'removed.txt').write_text('New ignored user data\n')
                raise RuntimeError('fixture_user_collision')
        with self.assertRaises(MODULE['RollbackUnknown']):
            self.install(checkpoint=checkpoint)
        self.assertEqual((self.repo / 'removed.txt').read_text(), 'New ignored user data\n')
        self.assertEqual(MODULE['head'](self.repo), self.expected)
        for name, data in self.new_bytes.items():
            self.assertEqual((self.repo / name).read_bytes(), data)

    def test_tracked_user_edit_is_preserved_without_reset(self):
        def checkpoint(phase, _plan):
            if phase == 'source-advanced':
                (self.repo / 'README.md').write_text('User edited this source\n')
                raise RuntimeError('fixture_user_edit')
        with self.assertRaises(MODULE['RollbackUnknown']):
            self.install(checkpoint=checkpoint)
        self.assertEqual((self.repo / 'README.md').read_text(), 'User edited this source\n')
        self.assertEqual(MODULE['head'](self.repo), self.expected)

    def test_ignored_directory_child_blocks_forward_file_replacement(self):
        path = self.repo / 'dir-to-file/user.ignored'
        path.write_text('User directory data\n')
        with self.assertRaises(MODULE['FenceChanged']):
            self.install()
        self.assertEqual(path.read_text(), 'User directory data\n')
        self.assertEqual(MODULE['head'](self.repo), self.old)

    def test_ignored_directory_child_blocks_rollback_to_previous_file(self):
        def checkpoint(phase, _plan):
            if phase == 'source-advanced':
                (self.repo / 'file-to-dir/user.ignored').write_text('User new directory data\n')
                raise RuntimeError('fixture_directory_collision')
        with self.assertRaises(MODULE['RollbackUnknown']):
            self.install(checkpoint=checkpoint)
        self.assertEqual((self.repo / 'file-to-dir/user.ignored').read_text(), 'User new directory data\n')
        self.assertEqual(MODULE['head'](self.repo), self.expected)

    def test_first_install_failure_restores_absence_and_deleted_worker_marker(self):
        for name in MODULE['ASSETS']:
            (self.repo / name).unlink()
        (self.repo / 'bin/native-voice').rename(self.root / 'unused-old-bundle')
        running = []
        def checkpoint(phase, _plan):
            if phase == 'source-advanced':
                handle = (self.repo / 'bin/omarchy-t3-client').open('rb')
                self.handles.append(handle)
                running.append(handle)
                raise RuntimeError('fixture_first_install_admission')
        with self.assertRaisesRegex(RuntimeError, 'fixture_first_install_admission'):
            self.install(checkpoint=checkpoint)
        self.assertEqual(MODULE['head'](self.repo), self.old)
        self.assertEqual(os.fstat(running[0].fileno()).st_nlink, 0)
        for name in MODULE['ASSETS']:
            self.assertFalse((self.repo / name).exists())
        self.assertFalse((self.repo / 'bin/native-voice').exists())

    def test_unknown_installed_library_edit_blocks_entire_rollback(self):
        def checkpoint(phase, _plan):
            if phase == 'source-advanced':
                (self.repo / 'bin/libt3-browser.so').write_bytes(b'user-library-edit')
                raise RuntimeError('fixture_asset_edit')
        with self.assertRaises(MODULE['RollbackUnknown']):
            self.install(checkpoint=checkpoint)
        self.assertEqual(MODULE['head'](self.repo), self.expected)
        self.assertEqual((self.repo / 'bin/libt3-browser.so').read_bytes(), b'user-library-edit')
        self.assertEqual((self.repo / 'bin/omarchy-t3-client').read_bytes(), self.new_bytes['bin/omarchy-t3-client'])
        self.assertEqual(voice.verify(self.repo / 'bin/native-voice'), self.new_receipt)

    def test_byte_identical_source_replacement_blocks_rollback(self):
        def checkpoint(phase, _plan):
            if phase == 'source-advanced':
                replacement = self.root / 'replacement-source'
                replacement.write_bytes((self.repo / 'README.md').read_bytes())
                replacement.chmod((self.repo / 'README.md').stat().st_mode & 0o777)
                replacement.replace(self.repo / 'README.md')
                raise RuntimeError('fixture_source_identity')
        with self.assertRaises(MODULE['RollbackUnknown']):
            self.install(checkpoint=checkpoint)
        self.assertEqual(MODULE['head'](self.repo), self.expected)
        self.assertEqual((self.repo / 'README.md').read_text(), 'New fixture source\n')

    def archive(self, name, invalid=None):
        path = self.root / name
        with tarfile.open(path, 'w:gz') as archive:
            for member in MODULE['PACKAGE']['BASE']:
                data = (self.stage / member).read_bytes()
                info = tarfile.TarInfo(member)
                info.size = len(data)
                if invalid == 'link' and member == 'bin/libt3-document.so':
                    info.type = tarfile.SYMTYPE
                    info.linkname = str(self.root / 'private-fixture-target')
                    info.size = 0
                    archive.addfile(info)
                else:
                    archive.addfile(info, io.BytesIO(data))
                if invalid == 'duplicate' and member == 'bin/omarchy-t3-client':
                    archive.addfile(info, io.BytesIO(data))
        return path

    def test_fixed_overlay_extraction_produces_complete_installable_stage(self):
        destination = self.root / 'extracted'
        result = MODULE['extract_overlay'](self.archive('valid.tar.gz'), destination, self.expected)
        self.assertEqual(result['assets'], len(MODULE['ASSETS']))
        for name, data in self.new_bytes.items():
            self.assertEqual((destination / name).read_bytes(), data)
        MODULE['install_overlay'](self.repo, self.expected, destination, self.bundle, self.inspect)
        self.assertEqual(MODULE['head'](self.repo), self.expected)

    def test_fixed_overlay_extraction_rejects_archive_links_and_duplicates(self):
        for invalid in ['link', 'duplicate']:
            with self.subTest(invalid=invalid):
                destination = self.root / ('rejected-' + invalid)
                with self.assertRaises(MODULE['FenceChanged']):
                    MODULE['extract_overlay'](self.archive(invalid + '.tar.gz', invalid), destination, self.expected)
                self.assertFalse(destination.exists())
                self.assertFalse((self.root / 'private-fixture-target').exists())

    def test_staged_commit_link_is_not_followed(self):
        commit = self.stage / 'COMMIT'
        commit.rename(self.root / 'private-commit')
        commit.symlink_to(self.root / 'private-commit')
        with self.assertRaises(ValueError):
            self.install()
        self.assert_old()


if __name__ == '__main__':
    unittest.main()
