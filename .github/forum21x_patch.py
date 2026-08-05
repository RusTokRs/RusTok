from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_once(path: str, old: str, new: str) -> None:
    file_path = ROOT / path
    text = file_path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one occurrence, found {count}: {old!r}")
    file_path.write_text(text.replace(old, new, 1), encoding="utf-8")


def add_locale(path: str, entries: dict[str, str]) -> None:
    file_path = ROOT / path
    data = json.loads(file_path.read_text(encoding="utf-8"))
    overlap = sorted(set(entries).intersection(data))
    if overlap:
        raise SystemExit(f"{path}: locale keys already exist: {overlap}")
    data.update(entries)
    file_path.write_text(
        json.dumps(data, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


replace_once(
    "apps/next-admin/packages/forum/src/index.ts",
    "export { ForumTopicMerge } from './components/forum-topic-merge';\nexport { ForumTopicSplit } from './components/forum-topic-split';\nexport * from './api/forum';\nexport * from './core/topic-fork';",
    "export { ForumTopicMerge } from './components/forum-topic-merge';\nexport { ForumTopicReplyRange } from './components/forum-topic-reply-range';\nexport { ForumTopicSplit } from './components/forum-topic-split';\nexport * from './api/forum';\nexport * from './api/topic-reply-range';\nexport * from './core/topic-fork';",
)
replace_once(
    "apps/next-admin/packages/forum/src/index.ts",
    "export * from './core/topic-merge';\nexport * from './core/topic-split';",
    "export * from './core/topic-merge';\nexport * from './core/topic-reply-range';\nexport * from './core/topic-split';",
)

replace_once(
    "apps/next-admin/packages/forum/src/nav.ts",
    "    {\n      title: 'Split Topic',\n      url: '/dashboard/forum/split',",
    "    {\n      title: 'Move Reply Range',\n      url: '/dashboard/forum/reply-range',\n      icon: 'messageSquare',\n      moduleSlug: 'forum',\n      access: { permission: 'forum_topics:manage' }\n    },\n    {\n      title: 'Split Topic',\n      url: '/dashboard/forum/split',",
)

replace_once(
    "crates/rustok-forum/admin/src/lib.rs",
    "mod topic_merge_model;\nmod topic_split_model;",
    "mod topic_merge_model;\nmod topic_reply_range_model;\nmod topic_split_model;",
)
replace_once(
    "crates/rustok-forum/admin/src/ui/mod.rs",
    "mod topic_merge;\nmod topic_split;",
    "mod topic_merge;\nmod topic_reply_range;\nmod topic_split;",
)
replace_once(
    "crates/rustok-forum/admin/src/ui/root.rs",
    "use super::topic_merge::ForumTopicMergeAdmin;\nuse super::topic_split::ForumTopicSplitAdmin;",
    "use super::topic_merge::ForumTopicMergeAdmin;\nuse super::topic_reply_range::ForumTopicReplyRangeAdmin;\nuse super::topic_split::ForumTopicSplitAdmin;",
)
replace_once(
    "crates/rustok-forum/admin/src/ui/root.rs",
    "    if route_context.subpath_matches(\"fork\") {",
    "    if route_context.subpath_matches(\"reply-range\") {\n        view! { <ForumTopicReplyRangeAdmin /> }.into_any()\n    } else if route_context.subpath_matches(\"fork\") {",
)

replace_once(
    "crates/rustok-forum/admin/src/transport.rs",
    "mod topic_merge_native_server_adapter;\nmod topic_split_graphql_adapter;",
    "mod topic_merge_native_server_adapter;\nmod topic_reply_range_graphql_adapter;\nmod topic_split_graphql_adapter;",
)
replace_once(
    "crates/rustok-forum/admin/src/transport.rs",
    "use crate::topic_merge_model::{\n    ForumTopicMergeCandidate, ForumTopicMergeCommand, ForumTopicMergeReceipt,\n};\nuse crate::topic_split_model::{",
    "use crate::topic_merge_model::{\n    ForumTopicMergeCandidate, ForumTopicMergeCommand, ForumTopicMergeReceipt,\n};\nuse crate::topic_reply_range_model::{\n    ForumReplyRangeMoveCandidate, ForumReplyRangeMoveCommand, ForumReplyRangeMoveReceipt,\n};\nuse crate::topic_split_model::{",
)
replace_once(
    "crates/rustok-forum/admin/src/transport.rs",
    "pub async fn fetch_topic_split_candidates(\n",
    "pub async fn fetch_reply_range_move_candidates(\n    token: Option<String>,\n    tenant_slug: Option<String>,\n    locale: String,\n) -> Result<Vec<ForumReplyRangeMoveCandidate>, ApiError> {\n    topic_reply_range_graphql_adapter::fetch_candidates(token, tenant_slug, locale).await\n}\n\npub async fn move_reply_range(\n    token: Option<String>,\n    tenant_slug: Option<String>,\n    command: ForumReplyRangeMoveCommand,\n) -> Result<ForumReplyRangeMoveReceipt, ApiError> {\n    topic_reply_range_graphql_adapter::move_reply_range(token, tenant_slug, command).await\n}\n\npub async fn fetch_topic_split_candidates(\n",
)
replace_once(
    "crates/rustok-forum/admin/src/transport.rs",
    "    #[test]\n    fn topic_split_uses_the_manager_graphql_transport_without_fallback() {",
    "    #[test]\n    fn reply_range_move_uses_the_manager_graphql_transport_without_fallback() {\n        for operation in [\"fetch_reply_range_move_candidates\", \"move_reply_range\"] {\n            let source = function_source(operation);\n            assert!(source.contains(\"topic_reply_range_graphql_adapter::\"));\n            assert!(!source.contains(\"native_server_adapter\"));\n            assert!(!source.contains(\"execute_selected_transport\"));\n            assert!(!source.contains(\"fallback\"));\n        }\n    }\n\n    #[test]\n    fn topic_split_uses_the_manager_graphql_transport_without_fallback() {",
)

add_locale(
    "crates/rustok-forum/admin/locales/en.json",
    {
        "forum.replyRange.title": "Move reply range",
        "forum.replyRange.subtitle": "Move one inclusive owner-position range into an existing topic.",
        "forum.replyRange.source": "Source topic",
        "forum.replyRange.target": "Target topic",
        "forum.replyRange.choose": "Choose a topic",
        "forum.replyRange.start": "Inclusive start position",
        "forum.replyRange.end": "Inclusive end position",
        "forum.replyRange.reason": "Reason",
        "forum.replyRange.warning": "Use canonical owner positions, not row numbers. The owner validates occupied endpoints, bounds, parent edges, ACL, solutions and counters.",
        "forum.replyRange.operation": "Retry identity",
        "forum.replyRange.retryHint": "Exact retries keep this identity. Editing source, target, either endpoint or reason rotates it.",
        "forum.replyRange.submit": "Move reply range",
        "forum.replyRange.pending": "Moving…",
        "forum.replyRange.complete": "Move committed",
    },
)
add_locale(
    "crates/rustok-forum/admin/locales/ru.json",
    {
        "forum.replyRange.title": "Перенос диапазона ответов",
        "forum.replyRange.subtitle": "Перенесите один включительный диапазон owner-позиций в существующую тему.",
        "forum.replyRange.source": "Исходная тема",
        "forum.replyRange.target": "Целевая тема",
        "forum.replyRange.choose": "Выберите тему",
        "forum.replyRange.start": "Начальная позиция включительно",
        "forum.replyRange.end": "Конечная позиция включительно",
        "forum.replyRange.reason": "Причина",
        "forum.replyRange.warning": "Используйте канонические owner-позиции, а не номера строк. Owner проверяет занятость границ, лимиты, родительские связи, ACL, решения и счётчики.",
        "forum.replyRange.operation": "Идентификатор повтора",
        "forum.replyRange.retryHint": "Точный повтор сохраняет идентификатор. Изменение источника, цели, границы или причины создаёт новый.",
        "forum.replyRange.submit": "Перенести диапазон",
        "forum.replyRange.pending": "Перенос…",
        "forum.replyRange.complete": "Перенос зафиксирован",
    },
)

replace_once(
    "crates/rustok-forum/admin/README.md",
    "- Presents category, topic, reply-preview, merge and selected-reply split workflows as module-owned Forum pages.",
    "- Presents category, topic, reply-preview, merge, selected-reply split, reply-branch fork and reply-range move workflows as module-owned Forum pages.",
)
replace_once(
    "crates/rustok-forum/admin/README.md",
    "- `ForumAdmin` — root route dispatcher for ordinary Forum admin pages plus `/modules/forum/merge` and `/modules/forum/split`.",
    "- `ForumAdmin` — root route dispatcher for ordinary Forum admin pages plus `/modules/forum/merge`, `/modules/forum/split`, `/modules/forum/fork` and `/modules/forum/reply-range`.",
)
replace_once(
    "crates/rustok-forum/admin/README.md",
    "- `admin/src/topic_split_model.rs` owns split command/receipt validation and the paired operation/target retry identities without Leptos imports.\n- `admin/src/transport.rs` is the only UI-facing transport facade, selects exactly one merge transport per compile profile, and exposes the split manager GraphQL adapter without fallback.",
    "- `admin/src/topic_split_model.rs` owns split command/receipt validation and the paired operation/target retry identities without Leptos imports.\n- `admin/src/topic_reply_range_model.rs` owns exact endpoint/reason validation and the operation retry identity without Leptos imports.\n- `admin/src/transport.rs` is the only UI-facing transport facade, selects exactly one merge transport per compile profile, and exposes split, fork and reply-range manager GraphQL adapters without fallback.",
)
replace_once(
    "crates/rustok-forum/admin/README.md",
    "- `admin/src/transport/topic_split_graphql_adapter.rs` composes `splitForumTopicReplies` and bounded candidate/reply reads without owner policy.\n- `admin/src/ui/root.rs` performs route-only composition.\n- `admin/src/ui/topic_merge.rs` and `admin/src/ui/topic_split.rs` are thin Leptos render/effect adapters.",
    "- `admin/src/transport/topic_split_graphql_adapter.rs` composes `splitForumTopicReplies` and bounded candidate/reply reads without owner policy.\n- `admin/src/transport/topic_reply_range_graphql_adapter.rs` composes `moveForumTopicReplyRange` without inferring owner positions or reading audit state.\n- `admin/src/ui/root.rs` performs route-only composition.\n- `admin/src/ui/topic_merge.rs`, `admin/src/ui/topic_split.rs`, `admin/src/ui/topic_fork.rs` and `admin/src/ui/topic_reply_range.rs` are thin Leptos render/effect adapters.",
)
replace_once(
    "crates/rustok-forum/admin/README.md",
    "- Mounted under `/modules/forum` with child pages for topics, categories, merge and split.",
    "- Mounted under `/modules/forum` with child pages for topics, categories, merge, split, fork and reply-range movement.",
)
replace_once(
    "crates/rustok-forum/admin/README.md",
    "- See [FORUM-21V selected-reply split admin composition](../docs/forum-21v-topic-split-admin-ui.md).",
    "- See [FORUM-21V selected-reply split admin composition](../docs/forum-21v-topic-split-admin-ui.md).\n- See [FORUM-21W reply-branch fork admin composition](../docs/forum-21w-topic-fork-admin-ui.md).\n- See [FORUM-21X reply-range move admin composition](../docs/forum-21x-reply-range-move-admin-ui.md).",
)

replace_once(
    "crates/rustok-forum/docs/README.md",
    "- FORUM-21T exposes `moveForumTopicReplyRange` as a routed-tenant, `forum_topics:manage` GraphQL adapter over the unchanged reply-range owner and immutable receipt; admin composition remains follow-up scope;",
    "- FORUM-21T exposes `moveForumTopicReplyRange` as a routed-tenant, `forum_topics:manage` GraphQL adapter over the unchanged reply-range owner and immutable receipt; FORUM-21X provides its Leptos and Next-admin composition without inferring canonical positions from visible row order;",
)
replace_once(
    "crates/rustok-forum/docs/README.md",
    "- [`forum-21w-topic-fork-admin-ui.md`](./forum-21w-topic-fork-admin-ui.md) — FORUM-21W manager fork workflow composition for Leptos and Next-admin.",
    "- [`forum-21w-topic-fork-admin-ui.md`](./forum-21w-topic-fork-admin-ui.md) — FORUM-21W manager fork workflow composition for Leptos and Next-admin.\n- [FORUM-21X reply-range move admin composition](./forum-21x-reply-range-move-admin-ui.md)",
)

plan = "crates/rustok-forum/docs/implementation-plan.md"
replace_once(
    plan,
    "the FORUM-21N/V/W\ntopic merge, split and fork workflows.",
    "the FORUM-21N/V/W/X\ntopic merge, split, fork and reply-range workflows.",
)
replace_once(
    plan,
    "| `FORUM-21` | `planned` | FORUM-21A-W provide move, merge, split, fork and reply-range owners, manager GraphQL transports, and split/fork admin composition; reply-range admin composition, runtime evidence and FORUM-24 aliases remain. |",
    "| `FORUM-21` | `planned` | FORUM-21A-X provide move, merge, split, fork and reply-range owners, manager GraphQL transports, and split/fork/reply-range admin composition; runtime evidence and FORUM-24 aliases remain. |",
)
replace_once(plan, "### Delivered through `FORUM-21W`", "### Delivered through `FORUM-21X`")
replace_once(
    plan,
    "- FORUM-21W composes the reply-branch fork command in the module-owned Leptos\n  and Next-admin surfaces. Both retain operation and target-topic UUIDs for an\n  exact retry, rotate both when the source, root or target shape changes, require\n  the root to be present in the bounded visible reply page and display the\n  immutable owner receipt without discovering descendants or adding copy policy;",
    "- FORUM-21W composes the reply-branch fork command in the module-owned Leptos\n  and Next-admin surfaces. Both retain operation and target-topic UUIDs for an\n  exact retry, rotate both when the source, root or target shape changes, require\n  the root to be present in the bounded visible reply page and display the\n  immutable owner receipt without discovering descendants or adding copy policy;\n- FORUM-21X composes the bounded reply-range move command in the module-owned\n  Leptos and Next-admin surfaces. Both retain one operation UUID for an exact\n  retry, rotate it when source, target, endpoint or reason changes, accept exact\n  canonical owner positions instead of inferring positions from visible row\n  order, and display the immutable owner receipt without adding movement policy;",
)
replace_once(
    plan,
    "FORUM-21V adds Leptos and\nNext-admin split composition only. FORUM-21W adds Leptos and Next-admin fork\ncomposition only: neither slice adds a migration, native split/fork transport,\nowner, GraphQL, receipt or semantic-event change.",
    "FORUM-21V adds Leptos and\nNext-admin split composition only. FORUM-21W adds Leptos and Next-admin fork\ncomposition only. FORUM-21X adds Leptos and Next-admin reply-range composition\nonly: none of the three slices adds a migration, native split/fork/reply-range\ntransport, owner, GraphQL, receipt or semantic-event change.",
)
replace_once(
    plan,
    "- public admin composition for the reply-range workflow without\n  transport-local movement policy;\n",
    "",
)
replace_once(
    plan,
    "node scripts/verify/verify-forum-topic-fork-admin-ui.mjs\nnpm run verify:forum:admin-boundary",
    "node scripts/verify/verify-forum-topic-fork-admin-ui.mjs\nnode scripts/verify/verify-forum-reply-range-move-admin-ui.mjs\nnpm run verify:forum:admin-boundary",
)
replace_once(
    plan,
    "cargo test -p rustok-forum-admin topic_fork_model -- --nocapture\ncargo check -p rustok-forum-admin --all-targets",
    "cargo test -p rustok-forum-admin topic_fork_model -- --nocapture\ncargo test -p rustok-forum-admin topic_reply_range_model -- --nocapture\ncargo check -p rustok-forum-admin --all-targets",
)
replace_once(
    plan,
    "The FORUM-21A through FORUM-21W source and contract records do not claim",
    "The FORUM-21A through FORUM-21X source and contract records do not claim",
)

print("FORUM-21X source composition applied")
