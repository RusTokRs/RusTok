from pathlib import Path
import re

def read(path):
    return Path(path).read_text()

def write(path, text):
    Path(path).write_text(text)

def replace_once(text, old, new, path):
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one occurrence, found {count}: {old[:100]!r}")
    return text.replace(old, new, 1)

path = "crates/rustok-forum/src/migrations/mod.rs"
text = read(path)
text = replace_once(text, "mod m20260804_000023_advance_forum_reply_range_move_positions;\n", "mod m20260804_000023_advance_forum_reply_range_move_positions;\nmod m20260805_000024_add_forum_topic_route_aliases;\n", path)
text = replace_once(text, "        Box::new(m20260804_000023_advance_forum_reply_range_move_positions::Migration),\n", "        Box::new(m20260804_000023_advance_forum_reply_range_move_positions::Migration),\n        Box::new(m20260805_000024_add_forum_topic_route_aliases::Migration),\n", path)
write(path, text)

path = "crates/rustok-forum/src/services/mod.rs"
text = read(path)
text = replace_once(text, "mod topic_canonical_resolution;\n", "mod topic_canonical_resolution;\nmod topic_route;\n", path)
anchor = """pub use topic_canonical_resolution::{
    ForumTopicCanonicalResolution, MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS,
};
"""
addition = anchor + """pub use topic_route::{
    FORUM_TOPIC_ROUTE_SHORT_ID_LEN, ForumTopicRouteDescriptor, ForumTopicRouteDisposition,
    ForumTopicRouteResolution, ForumTopicRouteService, MAX_FORUM_TOPIC_ROUTE_ALIAS_REASON_LEN,
    MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN, MAX_FORUM_TOPIC_ROUTE_SLUG_LEN,
};
"""
text = replace_once(text, anchor, addition, path)
write(path, text)

path = "crates/rustok-forum/src/lib.rs"
text = read(path)
anchor = "pub use state_machine::{ReplyStatus, TopicStatus};\n"
addition = """pub use services::{
    FORUM_TOPIC_ROUTE_SHORT_ID_LEN, ForumTopicRouteDescriptor, ForumTopicRouteDisposition,
    ForumTopicRouteResolution, ForumTopicRouteService, MAX_FORUM_TOPIC_ROUTE_ALIAS_REASON_LEN,
    MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN, MAX_FORUM_TOPIC_ROUTE_SLUG_LEN,
};
""" + anchor
text = replace_once(text, anchor, addition, path)
write(path, text)

path = "crates/rustok-forum/src/error.rs"
text = read(path)
anchor = """    #[error("Forum topic canonical resolution is inconsistent: {0}")]
    TopicCanonicalResolutionConflict(Uuid),

"""
addition = anchor + """    #[error("Forum topic route was not found")]
    TopicRouteNotFound,

    #[error("Forum topic route resolution is inconsistent")]
    TopicRouteResolutionConflict,

"""
text = replace_once(text, anchor, addition, path)
anchor = """            Self::TopicCanonicalResolutionConflict(_) => {
                "FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT"
            }
"""
addition = anchor + """            Self::TopicRouteNotFound => "FORUM_TOPIC_ROUTE_NOT_FOUND",
            Self::TopicRouteResolutionConflict => {
                "FORUM_TOPIC_ROUTE_RESOLUTION_CONFLICT"
            }
"""
text = replace_once(text, anchor, addition, path)
write(path, text)

path = "crates/rustok-forum/src/controllers/mod.rs"
text = read(path)
text = replace_once(text, "        | ForumError::SolutionNotFound(_) => {\n", "        | ForumError::SolutionNotFound(_)\n        | ForumError::TopicRouteNotFound => {\n", path)
text = replace_once(text, "        | ForumError::TopicCanonicalResolutionConflict(_) => HttpError::new(\n", "        | ForumError::TopicCanonicalResolutionConflict(_)\n        | ForumError::TopicRouteResolutionConflict => HttpError::new(\n", path)
write(path, text)

path = "crates/rustok-forum/docs/README.md"
text = read(path)
text = replace_once(text, "- resolve selected merged-source topic IDs through the immutable merge receipt ledger;\n", "- resolve selected merged-source topic IDs through the immutable merge receipt ledger;\n- own deterministic localized topic route identity plus immutable redirect/tombstone history;\n", path)
text = replace_once(text, "- FORUM-21V composes `splitForumTopicReplies` in the module-owned Leptos and Next-admin surfaces with stable retry/target identities and no transport-local movement policy.\n", "- FORUM-21V composes `splitForumTopicReplies` in the module-owned Leptos and Next-admin surfaces with stable retry/target identities and no transport-local movement policy.\n- FORUM-24A adds `ForumTopicRouteService`, a twelve-hex topic identity, exact-locale canonical descriptors and an append-only redirect/tombstone ledger; host mounting and owner write composition remain follow-up scope.\n", path)
text = replace_once(text, "- [FORUM-21K topic merge GraphQL transport](./forum-21k-topic-merge-graphql-transport.md)\n", "- [FORUM-21K topic merge GraphQL transport](./forum-21k-topic-merge-graphql-transport.md)\n- [FORUM-24A topic route identity owner](./forum-24a-topic-route-identity-owner.md)\n", path)
write(path, text)

path = "crates/rustok-forum/docs/implementation-plan.md"
text = read(path)
text, count = re.subn(r"\| `FORUM-21` \| `planned` \| FORUM-21A-X provide .*? \|", "| `FORUM-21` | `planned` | FORUM-21A-X provide move, merge, split, fork and reply-range owners, manager GraphQL transports, and split/fork/reply-range admin composition; retained owner/transport runtime evidence remains, while localized route identity proceeds under FORUM-24. |", text, count=1)
if count != 1:
    raise SystemExit(f"{path}: FORUM-21 ledger replacement count {count}")
text, count = re.subn(r"\| `FORUM-24` \| `planned` \| .*? \|", "| `FORUM-24` | `planned` | FORUM-24A adds deterministic exact-locale topic route identity and an immutable redirect/tombstone ledger; owner write composition, category routes, storefront mounts, hreflang/SEO policy and runtime evidence remain. |", text, count=1)
if count != 1:
    raise SystemExit(f"{path}: FORUM-24 ledger replacement count {count}")
start = text.index("## `FORUM-24`")
end = text.index("## `FORUM-25`", start)
section = text[start:end]
if "### Delivered in FORUM-24A" not in section:
    scope = section.index("### Scope")
    paragraph_start = section.index("\n\n", scope) + 2
    paragraph_end = section.index("\n\n", paragraph_start)
    delivered = """
### Delivered in FORUM-24A

- `ForumTopicRouteService` owns `/{locale}/forum/t/{short_id}/{slug}` descriptors,
  where `short_id` is the first 48 bits of the topic UUID in lowercase hex and
  the readable slug is not identity;
- current route lookup reads at most two candidates and fails closed on a
  short-identity collision instead of choosing by slug;
- existing bounded merge canonical resolution is reused so an archived merge
  source redirects to the terminal retained topic;
- PostgreSQL and SQLite `forum_topic_route_aliases` provide one append-only
  redirect/gone ledger keyed by tenant, locale, short identity and slug;
- redirects store target topic plus locale and recompute the latest target slug;
- route identity is transport-neutral and does not bypass topic visibility,
  channel, moderation or SEO publication authorization.

Verification sources:

```bash
node scripts/verify/verify-forum-topic-route-identity-owner.mjs
cargo test -p rustok-forum services::topic_route::tests -- --nocapture
cargo test -p rustok-forum --test topic_route_identity_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.
"""
    section = section[:paragraph_end] + "\n\n" + delivered.strip() + section[paragraph_end:]
    text = text[:start] + section + text[end:]
write(path, text)
