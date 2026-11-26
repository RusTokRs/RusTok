#!/usr/bin/env python3
"""Execute the current Forum topic-repair SQL with its actual lifecycle guards.

SQLite: python scripts/verify/verify-forum-topic-repair.py
PostgreSQL: set RUSTOK_FORUM_TEST_DATABASE_URL, install pg8000 in a test
virtualenv, then add --postgres. Uses a temporary schema, never existing tables.

This exercises SQL extracted from Rust, not Rust migration execution, owner
services, the full migration chain, concurrency or the mounted application.
"""

import argparse
from pathlib import Path
import re
import sqlite3
import unittest
from uuid import uuid4

from lib.forum_sql_verification import postgres_connection

MIGRATIONS = Path(__file__).resolve().parents[2] / "crates/modules/rustok-forum/src/migrations"
BASELINE = "m20260713_000009_add_forum_soft_delete_revisions"
RESTORE = "m20260921_000034_add_forum_topic_restore_snapshots.rs"
REPAIR = "m20261126_000046_reconcile_reply_publication_counters.rs"


def extract(pattern, source):
    match = re.search(pattern, source, re.S)
    if match is None:
        raise AssertionError(f"SQL extraction failed: {pattern}")
    return match.group(0)


class TopicRepairTests(unittest.TestCase):
    postgres = False

    def execute(self, sql):
        if self.postgres:
            cursor = self.db.cursor()
            cursor.execute(sql)
            return cursor
        return self.db.executescript(sql)

    def rows(self, sql):
        cursor = self.db.cursor()
        cursor.execute(sql)
        return [tuple(row) for row in cursor.fetchall()]

    def setUp(self):
        self.db = postgres_connection() if self.postgres else sqlite3.connect(":memory:")
        self.addCleanup(self.db.close)
        if self.postgres:
            schema = f"forum_topic_repair_{uuid4().hex}"
            self.execute(f'CREATE SCHEMA "{schema}"; SET search_path = "{schema}"')
            self.addCleanup(lambda: self.execute(f'DROP SCHEMA "{schema}" CASCADE'))
        self.execute("""
            CREATE TABLE forum_topics (
                id INTEGER PRIMARY KEY, tenant_id INTEGER, category_id INTEGER, author_id INTEGER,
                status TEXT, metadata TEXT DEFAULT '{}', is_pinned BOOLEAN DEFAULT FALSE,
                is_locked BOOLEAN DEFAULT FALSE, reply_count INTEGER DEFAULT 0,
                last_reply_at TIMESTAMP, deleted_at TIMESTAMP,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE forum_replies (
                id INTEGER PRIMARY KEY, tenant_id INTEGER, topic_id INTEGER, status TEXT,
                deleted_at TIMESTAMP, created_at TIMESTAMP
            );
            CREATE TABLE forum_solutions (
                tenant_id INTEGER, topic_id INTEGER, reply_id INTEGER,
                PRIMARY KEY (tenant_id, topic_id)
            );
            CREATE TABLE forum_topic_delete_snapshots (
                tenant_id INTEGER, topic_id INTEGER, previous_status TEXT, previous_is_locked BOOLEAN
            );
            CREATE TABLE forum_topic_translations (
                tenant_id INTEGER, topic_id INTEGER, locale TEXT, title TEXT, slug TEXT, body TEXT
            );
            CREATE TABLE forum_topic_revisions (
                tenant_id INTEGER, topic_id INTEGER, locale TEXT, title TEXT, slug TEXT,
                body TEXT, metadata TEXT, revision_reason TEXT
            );
            INSERT INTO forum_topics (id, tenant_id, status, reply_count, last_reply_at) VALUES
                (1, 1, 'open', 2, '2020-01-03'), (2, 1, 'archived', 9, '2020-01-03');
            INSERT INTO forum_topics (id, tenant_id, status, deleted_at) VALUES
                (3, 1, 'archived', '2020-01-04');
            INSERT INTO forum_replies (id, tenant_id, topic_id, status, created_at) VALUES
                (1, 1, 1, 'pending', '2020-01-03'), (2, 1, 1, 'approved', '2020-01-02'),
                (3, 1, 2, 'approved', '2020-01-01');
        """)
        restore = (MIGRATIONS / RESTORE).read_text()
        if self.postgres:
            baseline = (MIGRATIONS / BASELINE / "postgres_up.rs").read_text()
            for name, source in [("forum_guard_deleted_topic_update", restore),
                                 ("forum_enforce_topic_public_reply_count", baseline)]:
                self.execute(extract(rf"CREATE OR REPLACE FUNCTION {name}\(\).*?\$\$ LANGUAGE plpgsql;", source))
            self.execute("""
                CREATE TRIGGER forum_02_topics_deleted_guard BEFORE UPDATE ON forum_topics
                FOR EACH ROW EXECUTE FUNCTION forum_guard_deleted_topic_update();
                CREATE TRIGGER forum_90_topics_public_reply_count BEFORE UPDATE OF reply_count ON forum_topics
                FOR EACH ROW EXECUTE FUNCTION forum_enforce_topic_public_reply_count();
            """)
        else:
            baseline = (MIGRATIONS / BASELINE / "sqlite_counters.rs").read_text()
            for name, source in [("forum_topics_deleted_update_guard", restore),
                                 ("forum_topics_public_reply_count_update", baseline)]:
                sql = extract(rf'CREATE TRIGGER {name}\b.*?END(?="#)', source)
                self.execute(sql)
        source = (MIGRATIONS / REPAIR).read_text()
        self.repair = extract(r'UPDATE forum_topics SET reply_count.*?(?=",)', source)
        self.solution_repair = extract(r'DELETE FROM forum_solutions.*?(?=",)', source)
        self.tombstone = self.rows("SELECT * FROM forum_topics WHERE id = 3")

    def test_repairs_live_and_archived_topics_idempotently_without_touching_tombstones(self):
        for _ in range(2):
            self.execute(self.repair)
            self.assertEqual(self.rows("SELECT id,reply_count FROM forum_topics ORDER BY id"), [(1, 1), (2, 1), (3, 0)])
            self.assertEqual(self.rows("SELECT * FROM forum_topics WHERE id = 3"), self.tombstone)
            self.assertEqual(self.rows("SELECT id,status FROM forum_replies ORDER BY id"), [(1, "pending"), (2, "approved"), (3, "approved")])
            self.assertEqual(self.rows("SELECT COUNT(*) FROM forum_topic_revisions"), [(0,)])
            # last_reply_at must follow the approved row, not the newer pending row.
            value = self.rows("SELECT last_reply_at FROM forum_topics WHERE id = 1")[0][0]
            self.assertTrue(str(value).startswith("2020-01-02"), value)

    def test_repair_removes_only_nonpublic_solution_relations_and_is_idempotent(self):
        for index, status in enumerate(("pending", "hidden", "rejected", "flagged", "approved"), 10):
            deleted = "'2020-01-03'" if status == "approved" else "NULL"
            self.execute(f"INSERT INTO forum_topics (id,tenant_id,status) VALUES ({index},1,'open')")
            self.execute(f"INSERT INTO forum_replies (id,tenant_id,topic_id,status,deleted_at) VALUES ({index},1,{index},'{status}',{deleted})")
            self.execute(f"INSERT INTO forum_solutions VALUES (1,{index},{index})")
        # Both open and archived approved targets must survive. No topic/body,
        # source status, delete snapshot or revision is rewritten by cleanup.
        self.execute("INSERT INTO forum_solutions VALUES (1,1,2), (1,2,3)")
        sources = self.rows("SELECT * FROM forum_replies ORDER BY id")
        topics = self.rows("SELECT * FROM forum_topics ORDER BY id")
        for _ in range(2):
            self.execute(self.solution_repair)
            self.assertEqual(self.rows("SELECT * FROM forum_solutions ORDER BY topic_id"), [(1,1,2), (1,2,3)])
            self.assertEqual(self.rows("SELECT * FROM forum_replies ORDER BY id"), sources)
            self.assertEqual(self.rows("SELECT * FROM forum_topics ORDER BY id"), topics)
            self.assertEqual(self.rows("SELECT COUNT(*) FROM forum_topic_revisions"), [(0,)])
            self.assertEqual(self.rows("SELECT COUNT(*) FROM forum_topic_delete_snapshots"), [(0,)])

    def test_unscoped_repair_reproduces_immutable_tombstone_failure(self):
        with self.assertRaisesRegex(Exception, "deleted forum topic is immutable"):
            self.execute("UPDATE forum_topics SET reply_count = reply_count")

    def test_repair_preserves_the_deleted_topic_guard(self):
        self.execute(self.repair)
        with self.assertRaisesRegex(Exception, "deleted forum topic is immutable"):
            self.execute("UPDATE forum_topics SET updated_at = CURRENT_TIMESTAMP WHERE id = 3")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--postgres", action="store_true")
    args = parser.parse_args()
    TopicRepairTests.postgres = args.postgres
    unittest.main(argv=[__file__], verbosity=2)
