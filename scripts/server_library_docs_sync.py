#!/usr/bin/env python3
"""Sync/check snapshots of server core library documentation references."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import sys
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path

WARNING_DAYS = 30
FAIL_DAYS = 60
TIMEOUT_SECONDS = 20

ROOT = Path(__file__).resolve().parents[1]
SNAPSHOT_DIR = ROOT / "apps/server/docs/upstream-libraries"
VERSION_FILE = SNAPSHOT_DIR / "VERSION"
INDEX_FILE = SNAPSHOT_DIR / "README.md"
CARGO_LOCK = ROOT / "Cargo.lock"


@dataclass(frozen=True)
class Library:
    slug: str
    crate: str
    rustdoc_path: str


LIBRARIES: tuple[Library, ...] = (
    Library("axum", "axum", "axum"),
    Library("sea-orm", "sea-orm", "sea_orm"),
    Library("async-graphql", "async-graphql", "async_graphql"),
    Library("tokio", "tokio", "tokio"),
    Library("serde", "serde", "serde"),
    Library("tracing", "tracing", "tracing"),
    Library("utoipa", "utoipa", "utoipa"),
)


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def semver_key(version: str) -> tuple[int, int, int, str]:
    main, *suffix = version.split("-", 1)
    parts = [int(p) for p in main.split(".")]
    while len(parts) < 3:
        parts.append(0)
    pre = suffix[0] if suffix else ""
    return (parts[0], parts[1], parts[2], pre)


def parse_cargo_lock_versions() -> dict[str, str]:
    if not CARGO_LOCK.exists():
        raise RuntimeError(f"Cargo.lock not found: {CARGO_LOCK}")

    text = CARGO_LOCK.read_text(encoding="utf-8")
    pattern = re.compile(r'\[\[package\]\]\nname = "(?P<name>[^"]+)"\nversion = "(?P<version>[^"]+)"', re.MULTILINE)

    versions: dict[str, list[str]] = {}
    for match in pattern.finditer(text):
        name = match.group("name")
        version = match.group("version")
        versions.setdefault(name, []).append(version)

    result: dict[str, str] = {}
    for lib in LIBRARIES:
        candidates = versions.get(lib.crate, [])
        if not candidates:
            raise RuntimeError(f"Crate {lib.crate} was not found in Cargo.lock")
        result[lib.crate] = sorted(candidates, key=semver_key)[-1]
    return result


def try_download(url: str) -> tuple[bool, str]:
    req = urllib.request.Request(url, headers={"User-Agent": "RusTok-docs-sync/1.0"})
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT_SECONDS) as response:  # nosec: B310
            return True, response.read().decode("utf-8", errors="replace")
    except urllib.error.URLError as exc:
        return False, str(exc)


def sync_library(lib: Library, version: str, download_html: bool) -> dict[str, str]:
    docs_rs_crate = f"https://docs.rs/crate/{lib.crate}/{version}"
    docs_rs_rustdoc = f"https://docs.rs/{lib.crate}/{version}/{lib.rustdoc_path}/"

    lib_dir = SNAPSHOT_DIR / lib.slug
    lib_dir.mkdir(parents=True, exist_ok=True)

    metadata = {
        "crate": lib.crate,
        "version": version,
        "docs_rs_crate": docs_rs_crate,
        "docs_rs_rustdoc": docs_rs_rustdoc,
    }

    if download_html:
        ok_crate, crate_body = try_download(docs_rs_crate)
        ok_rustdoc, rustdoc_body = try_download(docs_rs_rustdoc)

        metadata["downloaded"] = ok_crate and ok_rustdoc
        if ok_crate:
            write_text(lib_dir / "docsrs-crate.html", crate_body)
        if ok_rustdoc:
            write_text(lib_dir / "docsrs-rustdoc.html", rustdoc_body)

        if not (ok_crate and ok_rustdoc):
            write_text(
                lib_dir / "DOWNLOAD_STATUS.txt",
                "Unable to download docs.rs pages in this environment.\n"
                f"crate_page={crate_body if not ok_crate else 'ok'}\n"
                f"rustdoc_page={rustdoc_body if not ok_rustdoc else 'ok'}\n",
            )

    write_text(lib_dir / "metadata.json", json.dumps(metadata, indent=2, ensure_ascii=False) + "\n")

    return {
        "slug": lib.slug,
        "crate": lib.crate,
        "version": version,
        "docs_rs_crate": docs_rs_crate,
        "docs_rs_rustdoc": docs_rs_rustdoc,
    }


def write_version(snapshot_date: dt.date, download_html: bool) -> None:
    mode = "docsrs-html" if download_html else "docsrs-links"
    content = "\n".join(
        [
            "# Server library upstream docs snapshot metadata",
            f"snapshot_date={snapshot_date.isoformat()}",
            f"source={mode}",
            "",
        ]
    )
    write_text(VERSION_FILE, content)


def write_index(snapshot_date: dt.date, records: list[dict[str, str]], download_html: bool) -> None:
    rows = "\n".join(
        f"| `{row['crate']}` | `{row['version']}` | [crate]({row['docs_rs_crate']}) | [rustdoc]({row['docs_rs_rustdoc']}) | `apps/server/docs/upstream-libraries/{row['slug']}/metadata.json` |"
        for row in records
    )

    mode_note = (
        "Снапшот включает HTML-копии `docsrs-*.html` (если сеть доступна)."
        if download_html
        else "Снапшот содержит актуальные версии и прямые ссылки на docs.rs без скачивания HTML."
    )

    content = f"""# Upstream snapshots for server core libraries

Этот каталог фиксирует **свежие ссылки на документацию** по ключевым библиотекам сервера.

- Источник версий: `Cargo.lock`
- Дата snapshot: `{snapshot_date.isoformat()}`
- Обновление: `make docs-sync-server-libs`
- Проверка свежести: `make docs-check-server-libs`
- Режим: `{mode_note}`

## Текущие версии и ссылки

| Crate | Version (`Cargo.lock`) | Docs.rs crate page | Rustdoc index | Local metadata |
|---|---:|---|---|---|
{rows}

Для попытки скачать HTML-копии docs.rs используйте:

```bash
python3 scripts/server_library_docs_sync.py sync --download-html
```
"""
    write_text(INDEX_FILE, content)


def parse_snapshot_date() -> dt.date:
    if not VERSION_FILE.exists():
        raise RuntimeError(f"Missing snapshot metadata file: {VERSION_FILE}")

    raw = VERSION_FILE.read_text(encoding="utf-8")
    for line in raw.splitlines():
        line = line.strip()
        if line.startswith("snapshot_date="):
            _, value = line.split("=", 1)
            return dt.date.fromisoformat(value.strip())
    raise RuntimeError(f"snapshot_date key not found in {VERSION_FILE}")


def check_snapshot() -> int:
    snapshot_date = parse_snapshot_date()
    today = dt.date.today()
    age_days = (today - snapshot_date).days

    missing: list[str] = []
    for lib in LIBRARIES:
        path = SNAPSHOT_DIR / lib.slug / "metadata.json"
        if not path.exists():
            missing.append(str(path.relative_to(ROOT)))

    if missing:
        print("::error::Missing server library snapshot files:")
        for item in missing:
            print(f" - {item}")
        return 1

    print(f"Server library docs snapshot date: {snapshot_date.isoformat()}")
    print(f"Snapshot age: {age_days} day(s)")

    if age_days > FAIL_DAYS:
        print(
            f"::error::Server library docs snapshot is older than {FAIL_DAYS} days. "
            "Run `make docs-sync-server-libs`."
        )
        return 1

    if age_days > WARNING_DAYS:
        print(
            f"::warning::Server library docs snapshot is older than {WARNING_DAYS} days. "
            "Please refresh with `make docs-sync-server-libs`."
        )

    return 0


def sync_snapshot(download_html: bool) -> int:
    snapshot_date = dt.date.today()
    versions = parse_cargo_lock_versions()

    records: list[dict[str, str]] = []
    for lib in LIBRARIES:
        records.append(sync_library(lib, versions[lib.crate], download_html))

    write_version(snapshot_date, download_html)
    write_index(snapshot_date, records, download_html)

    print(f"Done. Snapshot saved to {SNAPSHOT_DIR.relative_to(ROOT)}")
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    sync_parser = sub.add_parser("sync")
    sync_parser.add_argument("--download-html", action="store_true")

    sub.add_parser("check")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.command == "sync":
        return sync_snapshot(args.download_html)
    return check_snapshot()


if __name__ == "__main__":
    sys.exit(main())
