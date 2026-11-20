# SPDX-License-Identifier: Apache-2.0
"""Tests for the download retries in scripts/fetch_specs.py.

Transient upstream failures (HTTP 5xx and 429, connection errors, timeouts) are retried; other
HTTP errors and checksum mismatches are not. Run with: python -m unittest discover -s scripts/tests
"""
import hashlib
import io
import sys
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import fetch_specs  # noqa: E402

DATA = b"spec file contents"
SHA = hashlib.sha256(DATA).hexdigest()
URL = "https://example.org/spec.pdf"


def http_error(code):
    return urllib.error.HTTPError(URL, code, "error", {}, None)


def response():
    return io.BytesIO(DATA)


class Retries(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.dest = Path(self.dir.name) / "spec.pdf"
        self.sleep = mock.patch.object(fetch_specs.time, "sleep").start()
        self.addCleanup(mock.patch.stopall)
        self.addCleanup(self.dir.cleanup)

    def urlopen(self, *results):
        return mock.patch.object(fetch_specs.urllib.request, "urlopen", side_effect=list(results))

    def test_server_errors_are_retried(self):
        # The failure seen in CI: Cloudflare 525 and 521 from omg.org.
        with self.urlopen(http_error(525), http_error(521), response()) as opened:
            fetch_specs.download(URL, self.dest, SHA)
        self.assertEqual(opened.call_count, 3)
        self.assertEqual(self.dest.read_bytes(), DATA)
        self.assertEqual(self.sleep.call_count, 2)

    def test_connection_errors_and_timeouts_are_retried(self):
        with self.urlopen(urllib.error.URLError("reset"), TimeoutError("slow"), response()):
            fetch_specs.download(URL, self.dest, SHA)
        self.assertEqual(self.dest.read_bytes(), DATA)

    def test_rate_limiting_is_retried(self):
        with self.urlopen(http_error(429), response()):
            fetch_specs.download(URL, self.dest, SHA)
        self.assertEqual(self.dest.read_bytes(), DATA)

    def test_client_errors_are_not_retried(self):
        with self.urlopen(http_error(404), response()) as opened, self.assertRaises(urllib.error.HTTPError):
            fetch_specs.download(URL, self.dest, SHA)
        self.assertEqual(opened.call_count, 1)

    def test_attempts_are_limited(self):
        failures = [http_error(503)] * fetch_specs.ATTEMPTS
        with self.urlopen(*failures) as opened, self.assertRaises(urllib.error.HTTPError):
            fetch_specs.download(URL, self.dest, SHA)
        self.assertEqual(opened.call_count, fetch_specs.ATTEMPTS)
        self.assertFalse(self.dest.exists())
        self.assertEqual(list(self.dest.parent.iterdir()), [], "no partial download is left behind")

    def test_checksum_mismatch_is_not_retried(self):
        with self.urlopen(response(), response()) as opened, self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
            fetch_specs.download(URL, self.dest, "0" * 64)
        self.assertEqual(opened.call_count, 1)
        self.assertFalse(self.dest.exists())


if __name__ == "__main__":
    unittest.main()
