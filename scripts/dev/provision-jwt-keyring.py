#!/usr/bin/env python3
"""Explicit, one-time RSA JWT keyring provisioning for isolated POSIX dev/test.

Never invoked by host startup. Does not provision a MAC root, rotate an
enrolled generation, or modify any database/authority/witness state.
"""
from __future__ import annotations

import argparse
import json
import os
import stat
import subprocess
import tempfile
import uuid
from pathlib import Path

PURPOSES = ("access", "password_reset", "email_verification", "invite")
REPO_ROOT = Path(__file__).resolve().parents[2]


def generate_ring(purpose: str) -> dict:
    private = subprocess.run(
        ["openssl", "genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    ).stdout
    public = subprocess.run(
        ["openssl", "pkey", "-pubout"],
        input=private,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    ).stdout
    return {
        "algorithm": "RS256",
        "keyring": {"active_kid": f"{purpose}-{uuid.uuid4().hex}"},
        "rsa_private_key_pem": private.decode("ascii"),
        "rsa_public_key_pem": public.decode("ascii"),
    }


def provision(output: Path) -> None:
    if os.name != "posix":
        raise ValueError("Provisioning requires POSIX private-file permissions")
    parent = output.parent.resolve(strict=True)
    output = parent / output.name
    if output.is_relative_to(REPO_ROOT):
        raise ValueError("Keep private keyring material outside the repository")
    info = parent.stat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) & 0o077:
        raise ValueError("Output directory must be owned by the caller and private (0700)")
    if output.exists() or output.is_symlink():
        raise FileExistsError("Refusing to replace an existing keyring")

    document = {"generation": 1}
    for purpose in PURPOSES:
        document[purpose] = generate_ring(purpose)

    # Publish only a complete private file. Hard-link admission is atomic and
    # refuses replacement, including a concurrent provisioner or symlink.
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w", encoding="utf-8", dir=parent, prefix=".jwt-", delete=False
        ) as stream:
            temporary = Path(stream.name)
            os.fchmod(stream.fileno(), 0o600)
            json.dump(document, stream, indent=2)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.link(temporary, output)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)

    # File fsync alone does not make the published directory entry durable.
    # Never remove or regenerate a published keyring after an uncertain sync.
    try:
        descriptor = os.open(parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    except OSError as error:
        raise OSError(
            "Keyring was published but directory durability could not be confirmed; "
            "inspect and preserve the existing file before retrying"
        ) from error


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="New JSON file in a private directory outside the checkout")
    args = parser.parse_args()
    try:
        provision(args.output)
    except subprocess.CalledProcessError:
        parser.exit(1, "OpenSSL RSA key generation failed; no keyring was published\n")
    except (OSError, ValueError) as error:
        parser.exit(1, f"JWT keyring provisioning failed: {error}\n")
    print("Created private generation-1 RSA keyring for isolated development/test")


if __name__ == "__main__":
    main()
