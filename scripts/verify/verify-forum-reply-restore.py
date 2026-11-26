#!/usr/bin/env python3
"""Verify reply parent admission and archived topic solution snapshot replay SQL.

SQLite: python scripts/verify/verify-forum-reply-restore.py
PostgreSQL migration-47 matrix: set RUSTOK_FORUM_TEST_DATABASE_URL, install
pg8000 in a test virtualenv, then add --postgres. Uses a temporary schema.

Minimal extracted SQL fixtures, not Rust owners, MigrationTrait execution,
full migrations or concurrency. The service matrices live in
crates/modules/rustok-forum/tests/logic_review_remediation_sqlite.rs.
"""

import argparse
from pathlib import Path
import re
import sqlite3
import unittest
from uuid import uuid4

from lib.forum_sql_verification import postgres_connection


class ReplyRestoreGuardTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.addCleanup(self.db.close)
        self.db.executescript("""
            CREATE TABLE forum_topics (
                id INTEGER, tenant_id INTEGER, status TEXT, deleted_at TEXT
            );
            CREATE TABLE forum_replies (
                id INTEGER, tenant_id INTEGER, topic_id INTEGER, status TEXT, deleted_at TEXT
            );
            CREATE TABLE forum_solutions (tenant_id INTEGER, topic_id INTEGER, reply_id INTEGER);
            INSERT INTO forum_topics VALUES (1, 1, 'archived', NULL);
            INSERT INTO forum_replies VALUES (2, 1, 1, 'deleted', '2026-11-26');
        """)
        migration = Path(__file__).resolve().parents[2] / (
            "crates/modules/rustok-forum/src/migrations/"
            "m20260803_000016_add_forum_topic_merge_solution_policy.rs"
        )
        match = re.search(
            r"CREATE TRIGGER IF NOT EXISTS forum_10_topic_solution_target_insert\b.*?END;",
            migration.read_text(), re.S,
        )
        self.assertIsNotNone(match, "the actual solution-target guard must exist")
        self.db.executescript(match.group(0))

    def restore_source(self):
        self.db.execute("UPDATE forum_replies SET status = 'approved', deleted_at = NULL WHERE id = 2")

    def replay_solution(self):
        self.db.execute("INSERT INTO forum_solutions VALUES (1, 1, 2)")

    def test_pre_fix_accepted_restore_fails_under_archived_parent_and_rolls_back(self):
        with self.assertRaisesRegex(sqlite3.IntegrityError, "active topic and approved reply"):
            with self.db:
                self.restore_source()
                self.replay_solution()
        self.assertEqual(self.db.execute("SELECT status, deleted_at FROM forum_replies").fetchone(),
                         ("deleted", "2026-11-26"))
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM forum_solutions").fetchone(), (0,))

    def test_pre_fix_ordinary_restore_can_bypass_the_solution_guard(self):
        # This is a reproduction of the defect, not the new owner contract.
        with self.db:
            self.restore_source()
        self.assertEqual(self.db.execute("SELECT status, deleted_at FROM forum_replies").fetchone(),
                         ("approved", None))
        self.assertEqual(self.db.execute("SELECT status FROM forum_topics").fetchone(), ("archived",))

    def test_explicit_reopen_admits_solution_replay_without_weakening_guard(self):
        with self.db:
            self.db.execute("UPDATE forum_topics SET status = 'open' WHERE id = 1")
            self.restore_source()
            self.replay_solution()
        self.assertEqual(self.db.execute("SELECT * FROM forum_solutions").fetchall(), [(1, 1, 2)])
        with self.db:
            self.db.execute("UPDATE forum_topics SET status = 'archived' WHERE id = 1")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "active topic and approved reply"):
            self.replay_solution()


class TopicSolutionReplayTests(unittest.TestCase):
    postgres = False
    migrations = Path(__file__).resolve().parents[2] / "crates/modules/rustok-forum/src/migrations"

    def execute(self, sql):
        cursor = self.db.cursor()
        cursor.execute(sql)
        return cursor

    def script(self, sql):
        if self.postgres:
            self.execute(sql)
        else:
            self.db.executescript(sql)

    def constant(self, name):
        source = (self.migrations / "m20261126_000047_allow_topic_solution_snapshot_replay.rs").read_text()
        match = re.search(rf'const {name}: &str = r#"(.*?)"#;', source, re.S)
        self.assertIsNotNone(match, f"actual migration constant must exist: {name}")
        return match.group(1)

    def install(self, replay):
        # Instantiate the actual Rust migration's constant SQL templates. This
        # is extracted SQL evidence, not execution of MigrationTrait::up/down.
        admission = ""
        if replay:
            predicate = self.constant("SNAPSHOT_REPLAY")
            admission = f"OR (TG_OP = 'INSERT' AND {predicate})" if self.postgres else f"OR {predicate}"
        target = self.constant("TARGET_QUERY").replace("{archived_replay}", admission)
        validator = self.constant("POSTGRES_VALIDATOR" if self.postgres else "SQLITE_VALIDATOR")
        self.script(validator.replace("{target_query}", target))

    def setUp(self):
        self.db = postgres_connection() if self.postgres else sqlite3.connect(":memory:", isolation_level=None)
        self.addCleanup(self.db.close)
        if self.postgres:
            import pg8000.dbapi
            self.guard_error = pg8000.dbapi.DatabaseError
            schema = f"forum_solution_replay_{uuid4().hex}"
            self.execute(f'CREATE SCHEMA "{schema}"')
            self.addCleanup(lambda: self.execute(f'DROP SCHEMA "{schema}" CASCADE'))
            self.execute(f'SET search_path = "{schema}"')
        else:
            self.guard_error = sqlite3.IntegrityError
        self.script("""
            CREATE TABLE forum_topics (id INTEGER, tenant_id INTEGER, status TEXT, deleted_at TEXT);
            CREATE TABLE forum_replies (id INTEGER, tenant_id INTEGER, topic_id INTEGER, status TEXT, deleted_at TEXT);
            CREATE TABLE forum_solutions (tenant_id INTEGER, topic_id INTEGER, reply_id INTEGER,
                PRIMARY KEY (tenant_id, topic_id));
            CREATE TABLE forum_topic_delete_snapshots (
                tenant_id INTEGER, topic_id INTEGER, previous_status TEXT, solution_reply_id INTEGER
            );
            CREATE TABLE forum_topic_reply_delete_snapshots (
                tenant_id INTEGER, topic_id INTEGER, reply_id INTEGER, previous_status TEXT
            );
            INSERT INTO forum_topics VALUES (1, 1, 'archived', NULL);
            INSERT INTO forum_replies VALUES (2, 1, 1, 'approved', NULL), (3, 1, 1, 'approved', NULL);
            INSERT INTO forum_topic_delete_snapshots VALUES (1, 1, 'archived', 2);
            INSERT INTO forum_topic_reply_delete_snapshots VALUES (1, 1, 2, 'approved');
        """)
        baseline = (self.migrations / "m20260803_000016_add_forum_topic_merge_solution_policy.rs").read_text()
        if self.postgres:
            function = re.search(r"CREATE OR REPLACE FUNCTION forum_validate_topic_solution_target\(\).*?\$\$ LANGUAGE plpgsql;", baseline, re.S)
            self.assertIsNotNone(function)
            self.script(function.group(0))
            self.script("""CREATE TRIGGER forum_10_topic_solution_target
                BEFORE INSERT OR UPDATE OF tenant_id, topic_id, reply_id ON forum_solutions
                FOR EACH ROW EXECUTE FUNCTION forum_validate_topic_solution_target();""")
        else:
            for operation in ("insert", "update"):
                trigger = re.search(rf"CREATE TRIGGER IF NOT EXISTS forum_10_topic_solution_target_{operation}\b.*?END;", baseline, re.S)
                self.assertIsNotNone(trigger)
                self.script(trigger.group(0))

    def replay(self):
        self.execute("INSERT INTO forum_solutions VALUES (1, 1, 2)")

    def rejects_replay(self):
        with self.assertRaisesRegex(self.guard_error, "active topic and approved reply"):
            self.replay()

    def test_old_guard_reproduces_archived_topic_snapshot_restore_failure(self):
        self.rejects_replay()
        self.assertEqual(self.execute("SELECT COUNT(*) FROM forum_solutions").fetchone()[0], 0)

    def test_snapshot_target_replays_and_consumption_closes_admission(self):
        self.install(True)
        self.replay()
        self.assertEqual(tuple(self.execute("SELECT * FROM forum_solutions").fetchone()), (1, 1, 2))
        self.assertEqual(self.execute("SELECT status FROM forum_topics").fetchone()[0], "archived")
        self.execute("DELETE FROM forum_solutions")
        self.execute("DELETE FROM forum_topic_reply_delete_snapshots")
        self.execute("DELETE FROM forum_topic_delete_snapshots")
        self.rejects_replay()

    def test_snapshot_and_source_mismatches_fail_closed(self):
        self.install(True)
        mutations = [
            "DELETE FROM forum_topic_delete_snapshots",
            "DELETE FROM forum_topic_reply_delete_snapshots",
            "UPDATE forum_topic_delete_snapshots SET tenant_id = 9",
            "UPDATE forum_topic_delete_snapshots SET topic_id = 9",
            "UPDATE forum_topic_delete_snapshots SET solution_reply_id = 3",
            "UPDATE forum_topic_delete_snapshots SET previous_status = 'open'",
            "UPDATE forum_topic_reply_delete_snapshots SET tenant_id = 9",
            "UPDATE forum_topic_reply_delete_snapshots SET topic_id = 9",
            "UPDATE forum_topic_reply_delete_snapshots SET reply_id = 3",
            "UPDATE forum_topic_reply_delete_snapshots SET previous_status = 'pending'",
            "UPDATE forum_replies SET tenant_id = 9 WHERE id = 2",
            "UPDATE forum_replies SET topic_id = 9 WHERE id = 2",
            "UPDATE forum_replies SET status = 'pending' WHERE id = 2",
            "UPDATE forum_replies SET deleted_at = '2026-11-26' WHERE id = 2",
            "UPDATE forum_topics SET deleted_at = '2026-11-26'",
        ]
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                self.execute("BEGIN")
                try:
                    self.execute(mutation)
                    self.rejects_replay()
                finally:
                    self.execute("ROLLBACK")

    def test_snapshot_does_not_admit_another_reply(self):
        self.install(True)
        with self.assertRaisesRegex(self.guard_error, "active topic and approved reply"):
            self.execute("INSERT INTO forum_solutions VALUES (1, 1, 3)")

    def test_archived_target_update_remains_forbidden_even_with_snapshot(self):
        self.install(True)
        self.replay()
        for assignment in ["reply_id = reply_id", "reply_id = 3", "tenant_id = 9", "topic_id = 9"]:
            with self.subTest(assignment=assignment):
                with self.assertRaisesRegex(self.guard_error, "active topic and approved reply"):
                    self.execute(f"UPDATE forum_solutions SET {assignment}")

    def test_ordinary_live_solution_inserts_still_work_without_snapshots(self):
        self.install(True)
        self.execute("DELETE FROM forum_topic_delete_snapshots")
        self.execute("DELETE FROM forum_topic_reply_delete_snapshots")
        for status in ("open", "closed"):
            with self.subTest(status=status):
                self.execute(f"UPDATE forum_topics SET status = '{status}'")
                self.replay()
                self.execute("UPDATE forum_solutions SET reply_id = 3")
                self.execute("DELETE FROM forum_solutions")

    def test_down_restores_strict_guard_and_up_reopens_only_snapshot_admission(self):
        self.install(True)
        self.replay()
        self.install(False)
        # Down must not rewrite a valid archived relation already restored.
        self.assertEqual(self.execute("SELECT reply_id FROM forum_solutions").fetchone()[0], 2)
        self.execute("DELETE FROM forum_solutions")
        self.rejects_replay()
        self.install(True)
        self.replay()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--postgres", action="store_true", help="run the migration-47 SQL matrix on PostgreSQL")
    args = parser.parse_args()
    TopicSolutionReplayTests.postgres = args.postgres
    unittest.main(argv=[__file__], defaultTest="TopicSolutionReplayTests" if args.postgres else None, verbosity=2)
