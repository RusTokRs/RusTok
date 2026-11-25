"""Regression coverage for explicit development RSA provisioning."""
from __future__ import annotations

import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "dev" / "provision-jwt-keyring.py"


@unittest.skipUnless(os.name == "posix" and shutil.which("openssl"), "requires POSIX and OpenSSL")
class ProvisionJwtKeyringTests(unittest.TestCase):
    def test_four_independent_pairs_are_private_and_never_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "keyring.json"
            command = [sys.executable, str(SCRIPT), "--output", str(output)]
            subprocess.run(command, check=True, capture_output=True)
            original = output.read_bytes()
            document = json.loads(original)
            self.assertEqual(document.pop("generation"), 1)
            self.assertEqual(set(document), {"access", "password_reset", "email_verification", "invite"})
            self.assertEqual(stat.S_IMODE(output.stat().st_mode), 0o600)
            kids, public_keys = set(), set()
            for ring in document.values():
                self.assertEqual(ring["algorithm"], "RS256")
                kids.add(ring["keyring"]["active_kid"])
                public = ring["rsa_public_key_pem"].encode("ascii")
                private = ring["rsa_private_key_pem"].encode("ascii")
                derived = subprocess.run(
                    ["openssl", "pkey", "-pubout"],
                    input=private,
                    check=True,
                    capture_output=True,
                ).stdout
                self.assertEqual(derived, public)
                public_keys.add(public)
            self.assertEqual(len(kids), 4)
            self.assertEqual(len(public_keys), 4)
            rejected = subprocess.run(command, capture_output=True)
            self.assertNotEqual(rejected.returncode, 0)
            self.assertTrue(output.read_bytes() == original, "Existing keyring changed")

    def test_insecure_directory_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory)
            parent.chmod(0o755)
            output = parent / "keyring.json"
            rejected = subprocess.run(
                [sys.executable, str(SCRIPT), "--output", str(output)],
                capture_output=True,
            )
            self.assertNotEqual(rejected.returncode, 0)
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
