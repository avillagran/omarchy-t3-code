"""Validate build metadata using small owned files; never load native code."""
import copy
import hashlib
import json
import os
from pathlib import Path
import runpy
import stat
import struct
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
TOOL = runpy.run_path(str(ROOT / "tools/prepare-native-voice-build-metadata"))


def elf(*, machine=62):
    data = bytearray(64)
    data[:7] = b"\x7fELF\x02\x01\x01"
    struct.pack_into("<HHI", data, 16, 3, machine, 1)
    struct.pack_into("<H", data, 52, 64)
    return bytes(data)


class BuildMetadataTests(unittest.TestCase):
    def setUp(self):
        self.assertTrue(Path('/run/.containerenv').exists())
        # Retain every tiny fixture in private RAM until container exit. Link
        # fixtures point only to siblings within this newly allocated directory.
        self.root = Path(tempfile.mkdtemp(prefix="voice-build-metadata-", dir='/dev/shm'))
        self.addon = self.root / "electron-private-fd.node"
        self.addon.write_bytes(elf())
        self.addon.chmod(0o400)

    def test_owned_readonly_addon_produces_compile_time_environment_as_json(self):
        actual = TOOL["prepare"](self.addon, ROOT)
        self.assertEqual(actual["target"], "linux-x64")
        self.assertEqual(actual["addon"], {
            "sha256": hashlib.sha256(elf()).hexdigest(), "bytes": 64})
        self.assertEqual(json.loads(json.dumps(actual))["environment"], {
            "O3_NATIVE_VOICE_BUNDLED_ADDON_SHA256": hashlib.sha256(elf()).hexdigest(),
            "O3_NATIVE_VOICE_BUNDLED_ADDON_BYTES": "64",
        })
        self.assertEqual(self.addon.read_bytes(), elf())
        self.assertEqual(stat.S_IMODE(self.addon.stat().st_mode), 0o400)
        self.assertEqual(list(self.root.iterdir()), [self.addon])

    def test_writable_addon_is_rejected(self):
        for mode in (0o600, 0o420, 0o402):
            self.addon.chmod(mode)
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                TOOL["addon_metadata"](self.addon)

    def test_relative_paths_symlinks_and_directory_aliases_are_rejected(self):
        link = self.root / "linked.node"
        link.symlink_to(self.addon.name)
        alias = self.root / "alias"
        alias.symlink_to(".", target_is_directory=True)
        for path in (Path("relative.node"), link, alias / self.addon.name):
            with self.subTest(path=path), self.assertRaises(ValueError):
                TOOL["addon_metadata"](path)
        self.assertEqual(self.addon.read_bytes(), elf())

    def test_hardlinked_files_and_directories_are_rejected(self):
        linked = self.root / "hardlinked.node"
        os.link(self.addon, linked)
        for path in (self.addon, linked, self.root):
            with self.subTest(path=path), self.assertRaises(ValueError):
                TOOL["addon_metadata"](path)

    def test_foreign_owner_and_out_of_bounds_sizes_are_rejected(self):
        actual = self.addon.stat()
        fields = {key: getattr(actual, key) for key in (
            "st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_nlink",
            "st_size", "st_mtime_ns", "st_ctime_ns")}
        for overrides in ({"st_uid": actual.st_uid + 1}, {"st_size": 0},
                          {"st_size": TOOL["MAX_ADDON_BYTES"] + 1}):
            with self.subTest(overrides=overrides), self.assertRaises(ValueError):
                TOOL["_identity"](SimpleNamespace(**{**fields, **overrides}))

    def test_wrong_architecture_executable_and_malformed_headers_are_rejected(self):
        wrong_type = bytearray(elf())
        struct.pack_into("<H", wrong_type, 16, 2)
        wrong_endian = bytearray(elf())
        wrong_endian[5] = 2
        wrong_header_size = bytearray(elf())
        struct.pack_into("<H", wrong_header_size, 52, 0)
        for data in (b"", b"not ELF", elf()[:63], elf(machine=183),
                     wrong_type, wrong_endian, wrong_header_size):
            with self.subTest(data=bytes(data)), self.assertRaises(ValueError):
                TOOL["validate_elf"](data)

    def test_in_place_mutation_during_read_is_rejected(self):
        actual_read = os.read
        changed = False

        def read_and_mutate(descriptor, size):
            nonlocal changed
            data = actual_read(descriptor, size)
            if not changed:
                changed = True
                self.addon.chmod(0o600)
                self.addon.write_bytes(elf() + b"changed")
                self.addon.chmod(0o400)
            return data

        with mock.patch.object(os, "read", side_effect=read_and_mutate):
            with self.assertRaises(ValueError):
                TOOL["addon_metadata"](self.addon)

    def test_path_replacement_during_read_is_rejected(self):
        actual_read = os.read
        changed = False

        def read_and_replace(descriptor, size):
            nonlocal changed
            data = actual_read(descriptor, size)
            if not changed:
                changed = True
                self.addon.rename(self.root / "retained-original.node")
                self.addon.write_bytes(elf() + b"replacement")
                self.addon.chmod(0o400)
            return data

        with mock.patch.object(os, "read", side_effect=read_and_replace):
            with self.assertRaises(ValueError):
                TOOL["addon_metadata"](self.addon)

    def test_manifest_rejects_wrong_target_duplicate_paths_and_malformed_pins(self):
        source = json.loads((ROOT / "ui/voice/electron-runtime-44.10.1-linux-x64.json").read_text())
        TOOL["validate_manifest"](source)
        for key, value in (("version", "44.10.2"), ("platform", "linux-arm64"),
                           ("archiveSha256", "0" * 64), ("files", source["files"][:-1])):
            changed = copy.deepcopy(source)
            changed[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                TOOL["validate_manifest"](changed)
        for key, value in (("path", source["files"][1]["path"]), ("path", "../outside"),
                           ("path", "/outside"), ("path", "a//b"),
                           ("path", "a\\b"), ("bytes", True),
                           ("sha256", "A" * 64), ("executable", "true")):
            changed = copy.deepcopy(source)
            changed["files"][0][key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                TOOL["validate_manifest"](changed)


if __name__ == "__main__":
    unittest.main()
