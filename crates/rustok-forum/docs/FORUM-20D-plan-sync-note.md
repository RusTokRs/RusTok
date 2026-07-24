# FORUM-20D canonical plan synchronization note

This temporary execution note exists only because the connected GitHub Contents API requires a complete replacement of the approximately 2,000-line canonical implementation plan. The FORUM-20D source slice was prepared while `main` continued moving, so replacing that shared file through the connector would risk overwriting unrelated concurrent plan edits.

The canonical target remains `crates/rustok-forum/docs/implementation-plan.md`. A conflict-safe local update must:

- extend the `FORUM-20` ledger entry through FORUM-20D;
- record FORUM-20C storefront category composition;
- record FORUM-20D topic/reply owner exact and paginated read composition;
- remove those delivered read paths from remaining scope;
- add `topic_authenticated_visibility_sqlite`, `topic_reply_owner_visibility_sqlite`, and `verify-forum-owner-read-visibility.mjs` to the verification section;
- preserve `FORUM-20` as `in_progress` for roles, trust, memberships, explicit allow/deny, write audiences, category reads, cross-consumer authorization, scoped bulk commands, and runtime evidence.

Delete this note in the same conflict-safe commit that updates the canonical plan. It is not a second roadmap and does not supersede the canonical plan.
