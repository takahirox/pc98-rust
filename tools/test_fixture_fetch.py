"""Offline checks for checksum-pinned fixture downloads and mirror failures."""
import contextlib
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

import prepare_dos_apps as fixtures


class FixtureFetchTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.dest = Path(directory.name)
        self.path = self.dest / 'fixture.zip'
        self.data = b'checksum-pinned archive'
        self.primary = 'https://primary.invalid/fixture.zip'
        self.mirror = 'https://mirror.invalid/fixture.zip'
        self.enterContext(patch.object(fixtures, 'DEST', self.dest))
        self.network = self.enterContext(patch.object(fixtures.urllib.request, 'urlopen'))
        self.enterContext(contextlib.redirect_stderr(io.StringIO()))

    def fetch(self):
        return fixtures.fetch('fixture.zip', self.primary, fixtures.sha(self.data),
                              fallback_urls=(self.mirror,))

    def test_valid_primary_does_not_contact_mirror(self):
        self.network.return_value = io.BytesIO(self.data)
        self.assertEqual(self.fetch(), self.data)
        self.network.assert_called_once_with(self.primary, timeout=60)

    def test_http_failure_uses_verified_mirror(self):
        self.network.side_effect = [
            HTTPError(self.primary, 503, 'Service Unavailable', {}, None),
            io.BytesIO(self.data),
        ]
        self.assertEqual(self.fetch(), self.data)
        self.assertEqual(self.path.read_bytes(), self.data)
        self.assertEqual(self.network.call_args_list,
                         [unittest.mock.call(self.primary, timeout=60),
                          unittest.mock.call(self.mirror, timeout=60)])

    def test_bad_download_is_never_cached_and_tries_mirror(self):
        for bad in (b'wrong archive', b'x' * (8 * 1024 * 1024 + 1)):
            with self.subTest(size=len(bad)):
                def response(url, **kwargs):
                    if url == self.primary:
                        return io.BytesIO(bad)
                    self.assertEqual(url, self.mirror)
                    self.assertFalse(self.path.exists())
                    return io.BytesIO(self.data)

                self.network.side_effect = response
                self.assertEqual(self.fetch(), self.data)
                self.assertEqual(self.path.read_bytes(), self.data)
                self.path.unlink()

    def test_exhausted_network_failures_leave_no_cache(self):
        self.network.side_effect = OSError('unavailable')
        with self.assertRaises(OSError):
            self.fetch()
        self.assertEqual(self.network.call_count, 2)
        self.assertFalse(self.path.exists())

    def test_mirror_must_match_original_pin(self):
        self.network.side_effect = [OSError('unavailable'), io.BytesIO(b'wrong')]
        with self.assertRaisesRegex(ValueError, 'checksum or size mismatch'):
            self.fetch()
        self.assertFalse(self.path.exists())

    def test_valid_cache_works_offline(self):
        self.path.write_bytes(self.data)
        self.assertEqual(self.fetch(), self.data)
        self.network.assert_not_called()

    def test_corrupt_cache_fails_without_network_or_overwrite(self):
        self.path.write_bytes(b'corrupt')
        with self.assertRaisesRegex(ValueError, 'checksum or size mismatch'):
            self.fetch()
        self.network.assert_not_called()
        self.assertEqual(self.path.read_bytes(), b'corrupt')


if __name__ == '__main__':
    unittest.main()
