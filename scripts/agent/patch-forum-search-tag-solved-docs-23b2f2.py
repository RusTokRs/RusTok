from pathlib import Path

FORUM_PLAN = Path("crates/rustok-forum/docs/implementation-plan.md")
SEARCH_PLAN = Path("crates/rustok-search/docs/implementation-plan.md")


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement, found {count}\n{old[:200]}")
    path.write_text(text.replace(old, new, 1))


replace_once(
    FORUM_PLAN,
    "- solved topics match the presence or absence of `solution_reply_id`; replies match\n  the exact current projected `is_solution` boolean;",
    "- solved topics require either a valid UUID string or explicit null in\n  `solution_reply_id`; replies require the exact current projected `is_solution`\n  boolean, and malformed tag or solved projections fail closed;",
)

replace_once(
    FORUM_PLAN,
    "No database migration, manual backfill, Search query shape, Forum projection\nshape, dependency, public DTO or `Cargo.lock` change is required by\n`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2`. The Search-owned Product payload gains the",
    "No database migration, manual backfill, Search query shape, dependency, public\nDTO or `Cargo.lock` change is required by\n`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2`. `FORUM-23B2F2` extends the\nForum reply projection payload with parent-topic `topic_tags`; legacy reply rows\nrequire reindex before positive tag matches and fail closed until repaired. The\nSearch-owned Product payload gains the",
)

replace_once(
    SEARCH_PLAN,
    "replies use Forum-projected parent `payload.topic_tags`. Solved topics are derived\nfrom `solution_reply_id`, while replies use the exact current `is_solution` marker.",
    "replies use Forum-projected parent `payload.topic_tags`. Solved topics require a\nvalid UUID or explicit null in `solution_reply_id`, while replies use the exact\ncurrent boolean `is_solution` marker; malformed projected values fail closed.",
)

print("FORUM-23B2F2 final documentation contract synchronized.")
