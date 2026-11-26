"""Optional PostgreSQL connection for isolated Forum SQL-only verification."""

import os
from pathlib import Path
from urllib.parse import parse_qs, unquote, urlparse


def postgres_connection():
    import pg8000.dbapi  # Optional test-only driver; no production dependency.

    url = urlparse(os.environ["RUSTOK_FORUM_TEST_DATABASE_URL"])
    if url.scheme not in ("postgres", "postgresql"):
        raise ValueError("RUSTOK_FORUM_TEST_DATABASE_URL must be a PostgreSQL URL")
    host = parse_qs(url.query).get("host", [url.hostname or "localhost"])[0]
    options = dict(user=unquote(url.username or "postgres"), database=unquote(url.path.lstrip("/")),
                   port=url.port or 5432, timeout=10)
    if url.password:
        options["password"] = unquote(url.password)
    if host.startswith("/"):
        options["unix_sock"] = str(Path(host) / f".s.PGSQL.{options['port']}")
    else:
        options["host"] = host
    connection = pg8000.dbapi.connect(**options)
    connection.autocommit = True
    return connection

