"""Small package fixtures only; the real Electron graph is never copied here."""
import io
import json
import os
from pathlib import Path
import runpy
import stat
import struct
import tarfile
import tempfile
import unittest
from unittest import mock
import native_voice_bundle as voice


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.assertTrue(Path('/run/.containerenv').exists())
        # Private RAM fixtures are retained until container exit, without a
        # finalizer or recursive filesystem removal.
        self.root = Path(tempfile.mkdtemp(prefix='voice-package-', dir='/dev/shm'))
        self.opened_handles = []
        self.runtime = self.root / 'runtime'
        self.runtime.mkdir(mode=0o700)
        rows = []
        for index in range(72):
            name = 'electron' if index == 0 else f'member-{index}.bin'
            data = f'fixture-{index}'.encode()
            path = self.runtime / name
            path.write_bytes(data)
            path.chmod(0o500 if index == 0 else 0o400)
            rows.append({'path': name, 'bytes': len(data), 'sha256': voice.digest(data),
                'executable': index == 0})
        self.pin = {'files': rows}
        self.manifest_patch = mock.patch.object(voice, 'manifest', return_value=self.pin)
        self.manifest_patch.start()
        data = bytearray(64)
        data[:7] = b'\x7fELF\x02\x01\x01'
        struct.pack_into('<HHI', data, 16, 3, 62, 1)
        struct.pack_into('<H', data, 52, 64)
        self.addon = self.root / voice.ADDON
        self.addon.write_bytes(data)
        self.addon.chmod(0o500)

    def tearDown(self):
        for handle in self.opened_handles:
            handle.close()
        self.manifest_patch.stop()

    def bundle(self, name='bundle'):
        bundle = self.root / name
        receipt = voice.stage(self.runtime, self.addon, bundle)
        return bundle, receipt

    def archive(self, bundle, extra=None, omit=None, corrupt=None):
        path = self.root / ('archive-' + os.urandom(6).hex() + '.tar.gz')
        with tarfile.open(path, 'w:gz') as archive:
            if omit or corrupt:
                receipt = voice.verify(bundle)
                names = list(voice.expected_files(receipt['addon'])) + [voice.RECEIPT]
                for name in names:
                    if name == omit:
                        continue
                    data = (bundle / name).read_bytes()
                    if name == corrupt:
                        data = b'X' * len(data)
                    member = tarfile.TarInfo(voice.PREFIX + name)
                    member.size = len(data)
                    archive.addfile(member, io.BytesIO(data))
            else:
                voice.add_bundle(archive, bundle)
            if extra is not None:
                archive.addfile(extra, io.BytesIO(b''))
        return path

    def test_stage_archive_extract_preserves_exact_bytes_modes_and_licenses(self):
        bundle, receipt = self.bundle()
        self.assertEqual(voice.verify(bundle), receipt)
        extracted = self.root / 'extracted'
        self.assertEqual(voice.extract(self.archive(bundle), extracted), receipt)
        self.assertEqual(voice.verify(extracted), receipt)
        for name, pin in voice.expected_files(receipt['addon']).items():
            self.assertEqual((bundle / name).read_bytes(), (extracted / name).read_bytes())
            self.assertEqual(stat.S_IMODE((extracted / name).stat().st_mode),
                0o500 if pin['executable'] else 0o400)

    def test_runtime_extra_file_and_fixture_symlink_are_rejected(self):
        extra = self.runtime / 'extra'
        extra.write_bytes(b'fixture')
        extra.chmod(0o400)
        with self.assertRaises(ValueError):
            self.bundle()
        extra.unlink()
        member = self.runtime / 'member-1.bin'
        member.unlink()
        member.symlink_to('member-2.bin')
        with self.assertRaises(ValueError):
            self.bundle('linked')

    def test_archive_link_duplicate_traversal_and_extra_member_are_rejected(self):
        bundle, _ = self.bundle()
        for name, kind in [('../escape', tarfile.REGTYPE),
                ('extra', tarfile.REGTYPE), (voice.ADDON, tarfile.REGTYPE),
                ('link', tarfile.SYMTYPE), ('hardlink', tarfile.LNKTYPE)]:
            with self.subTest(name=name, kind=kind):
                entry = tarfile.TarInfo(voice.PREFIX + name)
                entry.type = kind
                entry.linkname = 'fixture-target'
                target = self.root / ('rejected-' + os.urandom(6).hex())
                with self.assertRaises(ValueError):
                    voice.extract(self.archive(bundle, extra=entry), target)
                self.assertFalse((self.root / 'escape').exists())

    def test_missing_or_corrupt_archive_member_cannot_be_admitted(self):
        bundle, _ = self.bundle()
        for changes in [{'omit': voice.RUNTIME + '/electron'},
                {'corrupt': voice.RUNTIME + '/member-1.bin'}]:
            target = self.root / ('rejected-' + os.urandom(6).hex())
            with self.assertRaises(ValueError):
                voice.extract(self.archive(bundle, **changes), target)

    def test_receipt_alone_cannot_change_addon_identity(self):
        bundle, receipt = self.bundle()
        file = bundle / voice.RECEIPT
        file.chmod(0o600)
        receipt['addon']['sha256'] = '0' * 64
        file.write_text(json.dumps(receipt))
        file.chmod(0o400)
        with self.assertRaises(ValueError):
            voice.verify(bundle)

    def test_matching_receipt_cannot_enable_an_unbundled_client(self):
        bundle, receipt = self.bundle()
        tool = runpy.run_path(str(voice.ROOT / 'tools/package-native-voice'))
        overlay = tool['overlay']
        empty = self.root / 'overlay'
        empty.mkdir(mode=0o700)
        output = self.root / 'not-created.tar.gz'
        with mock.patch.dict(overlay.__globals__, {'client_metadata': lambda _: None}):
            with self.assertRaises(ValueError):
                overlay(empty, bundle, output)
        with mock.patch.dict(overlay.__globals__,
                {'client_metadata': lambda _: voice.compiled(receipt)}):
            with self.assertRaises(ValueError):
                overlay(empty, None, output)
        self.assertFalse(output.exists())

    def test_activation_retains_old_directory_and_never_adopts_destination_link(self):
        first, _ = self.bundle()
        destination = self.root / 'native-voice'
        self.assertIsNone(voice.activate(first, destination))
        second, _ = self.bundle('second')
        retired = voice.activate(second, destination)
        self.assertTrue(retired.is_dir())
        self.assertEqual(voice.verify(retired), voice.verify(destination))
        link_parent = self.root / 'linked'
        link_parent.mkdir(mode=0o700)
        (link_parent / 'native-voice').symlink_to(destination, target_is_directory=True)
        third, _ = self.bundle('third')
        with self.assertRaises(ValueError):
            voice.activate(third, link_parent / 'native-voice')
        voice.verify(destination)

    def test_cross_filesystem_activation_stages_before_replacement(self):
        parent = Path(tempfile.mkdtemp(prefix='voice-package-cross-', dir='/tmp'))
        self.assertNotEqual(parent.stat().st_dev, self.root.stat().st_dev)
        bundle, receipt = self.bundle()
        destination = parent / 'native-voice'
        voice.activate(bundle, destination)
        self.assertEqual(voice.verify(destination), receipt)
        self.assertTrue(bundle.is_dir())

    def test_empty_overlay_keeps_optional_voice_absent(self):
        archive = self.root / 'empty.tar.gz'
        with tarfile.open(archive, 'w:gz'):
            pass
        destination = self.root / 'absent'
        self.assertIsNone(voice.extract(archive, destination))
        self.assertFalse(destination.exists())

    def test_mutable_cargo_output_links_never_relax_runtime_admission(self):
        source = self.root / 'cargo-output'
        source.write_bytes(b'fixture build output')
        source.chmod(0o755)
        link = self.root / 'cargo-deps-output'
        os.link(source, link)
        target = self.root / 'copied-client'
        with self.assertRaises(ValueError):
            voice.copy_member(source, target, immutable=False)
        voice.copy_member(source, target, immutable=False, single_link=False)
        self.assertEqual(target.stat().st_nlink, 1)
        self.assertEqual(target.read_bytes(), source.read_bytes())
        with self.assertRaises(ValueError):
            voice.copy_member(source, self.root / 'not-runtime', single_link=False)

    def client_pair(self):
        installed = self.root / 'installed'
        installed.mkdir(mode=0o700)
        old_bundle, old_receipt = self.bundle('old-bundle')
        voice.activate(old_bundle, installed / 'native-voice')
        old_client = installed / 'omarchy-t3-client'
        old_bytes = json.dumps(voice.compiled(old_receipt)).encode()
        old_client.write_bytes(old_bytes)
        old_client.chmod(0o755)
        self.addon.chmod(0o600)
        self.addon.write_bytes(self.addon.read_bytes() + b'new-fixture-version')
        self.addon.chmod(0o500)
        new_bundle, new_receipt = self.bundle('new-bundle')
        new_client = self.root / 'new-client-input'
        new_client.write_text(json.dumps(voice.compiled(new_receipt)))
        new_client.chmod(0o755)
        return installed, old_client, old_bytes, old_receipt, new_bundle, new_client, new_receipt

    def test_client_bundle_transaction_restores_pair_after_post_admission_failure(self):
        installed, old_client, old_bytes, old_receipt, bundle, client, new_receipt = self.client_pair()
        function = runpy.run_path(str(voice.ROOT / 'tools/package-native-voice'))['install_client']
        opened = []
        new_bytes = client.read_bytes()
        def inspect(path, verify=False):
            value = json.loads(path.read_text())
            if verify:
                self.assertEqual(value, voice.compiled(new_receipt))
                self.assertEqual(voice.verify(installed / 'native-voice'), new_receipt)
                running = path.open('rb')
                opened.append(running)
                self.opened_handles.append(running)
                raise RuntimeError('fixture_post_admission_failed')
            return value
        with mock.patch.dict(function.__globals__, {'client_metadata': inspect}):
            with self.assertRaisesRegex(RuntimeError, 'fixture_post_admission_failed'):
                function(client, bundle, old_client)
        self.assertEqual(old_client.read_bytes(), old_bytes)
        self.assertEqual(voice.verify(installed / 'native-voice'), old_receipt)
        transactions = list(installed.glob('.native-voice-transaction-*'))
        self.assertEqual(len(transactions), 1)
        self.assertEqual(client.read_bytes(), new_bytes)
        self.assertEqual(os.fstat(opened[0].fileno()).st_nlink, 0)
        self.assertEqual(opened[0].read(), new_bytes)
        self.assertEqual(voice.verify(transactions[0] / 'failed-bundle'), new_receipt)

    def test_client_bundle_transaction_restores_bundle_when_client_replace_fails(self):
        installed, old_client, old_bytes, old_receipt, bundle, client, _ = self.client_pair()
        function = runpy.run_path(str(voice.ROOT / 'tools/package-native-voice'))['install_client']
        actual_replace = Path.replace
        def replace(path, target):
            if path.name == 'new-client' and Path(target) == old_client:
                raise OSError('fixture_client_replace_failed')
            return actual_replace(path, target)
        with mock.patch.dict(function.__globals__,
                {'client_metadata': lambda path, verify=False: json.loads(path.read_text())}), \
                mock.patch.object(Path, 'replace', replace):
            with self.assertRaisesRegex(OSError, 'fixture_client_replace_failed'):
                function(client, bundle, old_client)
        self.assertEqual(old_client.read_bytes(), old_bytes)
        self.assertEqual(voice.verify(installed / 'native-voice'), old_receipt)

    def test_client_bundle_transaction_copy_failure_does_not_activate_either_candidate(self):
        installed, old_client, old_bytes, old_receipt, bundle, client, _ = self.client_pair()
        function = runpy.run_path(str(voice.ROOT / 'tools/package-native-voice'))['install_client']
        actual_copy = voice.copy_member
        def copy(source, target, *args, **kwargs):
            if source == old_client:
                target.write_bytes(b'partial fixture backup')
                raise OSError('fixture_client_backup_copy_failed')
            return actual_copy(source, target, *args, **kwargs)
        with mock.patch.dict(function.__globals__,
                {'client_metadata': lambda path, verify=False: json.loads(path.read_text())}), \
                mock.patch.object(voice, 'copy_member', copy):
            with self.assertRaisesRegex(OSError, 'fixture_client_backup_copy_failed'):
                function(client, bundle, old_client)
        self.assertEqual(old_client.read_bytes(), old_bytes)
        self.assertEqual(voice.verify(installed / 'native-voice'), old_receipt)

    def test_client_bundle_transaction_success_retains_backup_without_linking_running_inode(self):
        installed, old_client, old_bytes, old_receipt, bundle, client, new_receipt = self.client_pair()
        function = runpy.run_path(str(voice.ROOT / 'tools/package-native-voice'))['install_client']
        def inspect(path, verify=False):
            value = json.loads(path.read_text())
            if verify:
                self.assertEqual(voice.compiled(voice.verify(path.parent / 'native-voice')), value)
            return value
        with old_client.open('rb') as running, mock.patch.dict(function.__globals__, {'client_metadata': inspect}):
            result = function(client, bundle, old_client)
            self.assertEqual(os.fstat(running.fileno()).st_nlink, 0)
            self.assertEqual(running.read(), old_bytes)
        self.assertEqual(old_client.read_bytes(), client.read_bytes())
        self.assertEqual(old_client.stat().st_nlink, 1)
        self.assertEqual(Path(result['retiredClient']).read_bytes(), old_bytes)
        self.assertEqual(voice.verify(Path(result['retiredBundle'])), old_receipt)
        self.assertEqual(voice.verify(installed / 'native-voice'), new_receipt)


if __name__ == '__main__':
    unittest.main()
