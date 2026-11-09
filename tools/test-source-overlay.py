"""Isolated source preparation with a synthetic builder in private container RAM."""
import json
import os
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest import mock
import native_voice_bundle as voice

MODULE = runpy.run_path(str(voice.ROOT / 'tools/prepare-source-overlay'))
OVERLAY = MODULE['OVERLAY']


class SourceTests(unittest.TestCase):
    def setUp(self):
        self.assertTrue(Path('/run/.containerenv').exists())
        self.root = Path(tempfile.mkdtemp(prefix='overlay-build-', dir='/dev/shm'))
        self.repo = self.root / 'installed'
        self.repo.mkdir(mode=0o700)
        OVERLAY['git'](self.repo, 'init', '-q', '-b', 'main')
        OVERLAY['git'](self.repo, 'config', 'user.name', 'Fixture User')
        OVERLAY['git'](self.repo, 'config', 'user.email', 'fixture@localhost')
        (self.repo / '.gitignore').write_text('/bin/active-client\n/bin/omarchy-t3-client\n')
        (self.repo / 'source.txt').write_text('old source\n')
        (self.repo / 'bin').mkdir(mode=0o700)
        (self.repo / 'bin/build-client').write_text('#!/bin/sh\nexit 1\n')
        (self.repo / 'bin/build-client').chmod(0o755)
        for name in MODULE['GENERATED_TRACKED']:
            path = self.repo / name
            path.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
            path.write_text('Original license fixture\n')
        OVERLAY['git'](self.repo, 'add', '--', '.')
        OVERLAY['git'](self.repo, 'commit', '-qm', 'Old isolated build fixture')
        self.old = OVERLAY['head'](self.repo)
        (self.repo / 'source.txt').write_text('new source\n')
        OVERLAY['git'](self.repo, 'commit', '-qam', 'Expected isolated build fixture')
        self.expected = OVERLAY['head'](self.repo)
        OVERLAY['git'](self.repo, 'checkout', '-qb', 'installed', self.old)
        (self.repo / 'bin/active-client').write_text('Active application bytes\n')
        self.destination = self.root / 'prepared'
        self.calls = []

    def build(self, command, destination, environment):
        self.calls.append(dict(command=command, destination=destination, environment=environment))
        self.assertEqual(OVERLAY['head'](self.repo), self.old)
        self.assertEqual(OVERLAY['head'](destination), self.expected)
        for name in OVERLAY['ASSETS']:
            path = destination / name
            path.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
            path.write_bytes(('synthetic:' + name).encode())
            path.chmod(0o644 if name in OVERLAY['TEXT_ASSETS'] else 0o755)

    def prepare(self, builder=None):
        return MODULE['prepare_source'](self.repo, self.expected, self.destination, builder or self.build)

    def assert_active_unchanged(self):
        self.assertEqual(OVERLAY['head'](self.repo), self.old)
        self.assertEqual((self.repo / 'source.txt').read_text(), 'old source\n')
        self.assertEqual((self.repo / 'bin/active-client').read_text(), 'Active application bytes\n')
        self.assertFalse((self.repo / 'bin/omarchy-t3-client').exists())

    def test_complete_build_isolated_from_active_assets_and_reuses_explicit_target(self):
        target = self.root / 'existing-target'
        target.mkdir(mode=0o700)
        with mock.patch.dict(os.environ, {'CARGO_TARGET_DIR': str(target), 'GIT_DIR': '/not-an-input'}):
            result = self.prepare()
        self.assertEqual(result['assets'], len(OVERLAY['ASSETS']))
        self.assertEqual(result['cargoTarget'], str(target))
        self.assertIsNone(result['bundle'])
        self.assertFalse((self.destination / 'bin/active-client').exists())
        self.assertEqual((self.destination / 'COMMIT').read_text().strip(), self.expected)
        self.assertEqual(len(self.calls), 1)
        self.assertNotIn('GIT_DIR', self.calls[0]['environment'])
        self.assertEqual(self.calls[0]['environment']['GIT_CONFIG_VALUE_0'], '/dev/null')
        self.assert_active_unchanged()

    def test_build_failure_retains_partial_stage_and_active_installation(self):
        def failing(command, destination, environment):
            (destination / 'bin/omarchy-t3-client').write_bytes(b'partial builder output')
            raise RuntimeError('fixture_build_failed')
        with self.assertRaisesRegex(RuntimeError, 'fixture_build_failed'):
            self.prepare(failing)
        self.assertEqual((self.destination / 'bin/omarchy-t3-client').read_bytes(), b'partial builder output')
        self.assert_active_unchanged()

    def test_incomplete_asset_group_is_refused_before_installation(self):
        def incomplete(*arguments):
            self.build(*arguments)
            (self.destination / 'bin/libt3-browser.so').unlink()
        with self.assertRaises(FileNotFoundError):
            self.prepare(incomplete)
        self.assert_active_unchanged()

    def test_unexpected_tracked_source_edit_is_refused(self):
        def changed(*arguments):
            self.build(*arguments)
            (self.destination / 'source.txt').write_text('Unexpected build edit\n')
        with self.assertRaisesRegex(ValueError, 'Unexpected tracked source'):
            self.prepare(changed)
        self.assert_active_unchanged()

    def test_original_license_outputs_can_be_regenerated_for_selected_toolchain(self):
        def licenses(*arguments):
            self.build(*arguments)
            for name in MODULE['GENERATED_TRACKED']:
                (self.destination / name).write_text('Regenerated fixture license\n')
        result = self.prepare(licenses)
        self.assertEqual(set(result['generatedLicenseOutputs']), MODULE['GENERATED_TRACKED'])
        self.assert_active_unchanged()

    def test_index_mutation_is_refused(self):
        def staged(*arguments):
            self.build(*arguments)
            (self.destination / 'source.txt').write_text('Unexpected staged source\n')
            OVERLAY['git'](self.destination, 'add', '--', 'source.txt')
        with self.assertRaisesRegex(ValueError, 'index changed'):
            self.prepare(staged)
        self.assert_active_unchanged()

    def test_revision_mutation_is_refused(self):
        def changed(*arguments):
            self.build(*arguments)
            OVERLAY['git'](self.destination, 'checkout', '--detach', self.old)
        with self.assertRaisesRegex(ValueError, 'revision changed'):
            self.prepare(changed)
        self.assert_active_unchanged()


if __name__ == '__main__':
    unittest.main()
