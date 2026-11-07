"""Artifact snapshots survive later Cargo replacement without following fixture links."""
import pathlib
import unittest
from runtime_artifacts import snapshot
from contextlib import contextmanager
import tempfile

@contextmanager
def retained_directory(*, prefix):
    if not pathlib.Path('/run/.containerenv').is_file():
        raise SystemExit('Private Podman is required')
    yield tempfile.mkdtemp(prefix=prefix)


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
