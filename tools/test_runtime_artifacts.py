"""Artifact snapshots survive later Cargo replacement without following fixture links."""
import errno
import os
import pathlib
import unittest
from unittest import mock
from runtime_artifacts import snapshot
from contextlib import contextmanager
import tempfile

@contextmanager
def retained_directory(*, prefix):
    if not pathlib.Path('/run/.containerenv').is_file():
        raise SystemExit('Private Podman is required')
    yield tempfile.mkdtemp(prefix=prefix, dir=os.environ.get("O3_RUNTIME_ARTIFACT_FIXTURES"))


class ArtifactTests(unittest.TestCase):
    def test_snapshot_is_independent_and_includes_embedded_cli_and_libraries(self):
        with retained_directory(prefix='runtime-artifacts-') as directory:
            root = pathlib.Path(directory)
            debug = root / 'debug'
            deps = debug / 'deps'
            deps.mkdir(parents=True)
            test = deps / 'test-binary'
            test.write_bytes(b'old-test')
            test.chmod(0o700)
            cli = debug / 'server'
            cli.write_bytes(b'old-cli')
            cli.chmod(0o700)
            library = deps / 'libfixture.so'
            library.write_bytes(b'old-library')
            runtime = root / 'snapshot'
            records = snapshot(debug, [test], runtime)
            test.write_bytes(b'new-test')
            cli.write_bytes(b'new-cli')
            library.write_bytes(b'new-library')
            self.assertEqual((runtime / 'deps/test-binary').read_bytes(), b'old-test')
            self.assertEqual((runtime / 'server').read_bytes(), b'old-cli')
            self.assertEqual((runtime / 'deps/libfixture.so').read_bytes(), b'old-library')
            self.assertEqual(len(records), 3)
            self.assertTrue(all(len(item['sha256']) == 64 for item in records))

    def test_optional_reflink_is_a_distinct_independent_inode(self):
        with retained_directory(prefix='runtime-artifacts-reflink-') as directory:
            root = pathlib.Path(directory)
            debug = root / 'debug'
            debug.mkdir()
            source = debug / 'fixture'
            source.write_bytes(b'original' * 8192)
            source.chmod(0o700)
            records = snapshot(debug, [source], root / 'snapshot', reflink=True)
            copied = root / 'snapshot/fixture'
            self.assertNotEqual(source.stat().st_ino, copied.stat().st_ino)
            if os.environ.get('O3_REQUIRE_FICLONE') == '1':
                self.assertEqual(records[0]['copyMode'], 'reflink')
            source.write_bytes(b'changed')
            self.assertEqual(copied.read_bytes(), b'original' * 8192)
            replacement = debug / 'replacement'
            replacement.write_bytes(b'replaced')
            replacement.replace(source)
            self.assertEqual(copied.read_bytes(), b'original' * 8192)
            self.assertEqual(copied.stat().st_mode & 0o777, 0o500)

    def test_reflink_fallback_reserves_before_copy_and_refuses_enospc(self):
        with retained_directory(prefix='runtime-artifacts-reserve-') as directory:
            root = pathlib.Path(directory)
            debug = root / 'debug'
            debug.mkdir()
            source = debug / 'fixture'
            source.write_bytes(b'owned')
            with mock.patch('runtime_artifacts.fcntl.ioctl', side_effect=OSError(errno.EXDEV, 'fixture')):
                with mock.patch('runtime_artifacts.os.posix_fallocate', wraps=os.posix_fallocate) as reserve:
                    records = snapshot(debug, [source], root / 'reserved', reflink=True)
                    reserve.assert_called_once()
                    self.assertEqual(records[0]['copyMode'], 'reserved-copy')
                with mock.patch('runtime_artifacts.os.posix_fallocate', side_effect=OSError(errno.ENOSPC, 'fixture')):
                    with mock.patch('runtime_artifacts.shutil.copyfileobj') as copy:
                        with self.assertRaises(OSError):
                            snapshot(debug, [source], root / 'refused', reflink=True)
                        copy.assert_not_called()
            self.assertEqual(source.read_bytes(), b'owned')

    def test_source_mutation_during_copy_is_refused(self):
        with retained_directory(prefix='runtime-artifacts-mutation-') as directory:
            root = pathlib.Path(directory)
            debug = root / 'debug'
            debug.mkdir()
            source = debug / 'fixture'
            source.write_bytes(b'owned')
            import shutil
            actual_copy = shutil.copyfileobj
            def changed(original, copied, length):
                actual_copy(original, copied, length)
                source.write_bytes(b'changed')
            with mock.patch('runtime_artifacts.shutil.copyfileobj', side_effect=changed):
                with self.assertRaises(ValueError):
                    snapshot(debug, [source], root / 'refused')

    def test_replaced_destination_cannot_redirect_chmod_or_hash(self):
        with retained_directory(prefix='runtime-artifacts-path-') as directory:
            root = pathlib.Path(directory)
            debug = root / 'debug'
            debug.mkdir()
            source = debug / 'fixture'
            source.write_bytes(b'owned-source')
            canary = root / 'owned-canary'
            canary.write_bytes(b'owned-canary')
            canary.chmod(0o600)
            import shutil
            actual_copy = shutil.copyfileobj
            def replaced(original, copied, length):
                actual_copy(original, copied, length)
                target = root / 'snapshot/fixture'
                target.rename(root / 'retained-original')
                target.symlink_to(canary)
            with mock.patch('runtime_artifacts.shutil.copyfileobj', side_effect=replaced):
                with self.assertRaises(ValueError):
                    snapshot(debug, [source], root / 'snapshot')
            self.assertEqual(canary.stat().st_mode & 0o777, 0o600)
            self.assertEqual(canary.read_bytes(), b'owned-canary')

    def test_owned_link_and_external_artifact_are_rejected(self):
        with retained_directory(prefix='runtime-artifacts-reject-') as directory:
            root = pathlib.Path(directory)
            debug = root / 'debug'
            debug.mkdir()
            owned = root / 'owned'
            owned.write_bytes(b'kept')
            link = debug / 'link'
            link.symlink_to(owned)
            with self.assertRaises(ValueError):
                snapshot(debug, [link], root / 'link-snapshot')
            with self.assertRaises(ValueError):
                snapshot(debug, [owned], root / 'outside-snapshot')
            self.assertEqual(owned.read_bytes(), b'kept')


if __name__ == '__main__':
    unittest.main()
