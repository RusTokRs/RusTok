# FORUM-21O native Leptos admin topic-merge transport

## Status

`source_ready_maintainer_execution_pending`

FORUM-21O closes the Leptos transport gap left by FORUM-21N. The module-owned
merge page now selects a direct authenticated native server-function path in
SSR/hydrate builds while retaining the existing GraphQL adapter for CSR and
headless/default builds.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-merge-native-admin.json
```

## Selected transport, no fallback

`admin/src/transport.rs` uses
`rustok_ui_transport::execute_selected_transport` and chooses exactly one path:

- `ssr` and `hydrate`: native server functions;
- `csr` and default/headless: GraphQL;
- a failure on the selected path is returned to the UI and never causes an
  implicit retry through the other transport.

This preserves deterministic transport ownership and avoids duplicate merge
commands with the same operation identity crossing two transport paths.

## Native owner composition

`admin/src/transport/topic_merge_native_server_adapter.rs` owns two server
functions:

```text
/api/fn/forum/topic-merge-candidates
/api/fn/forum/topic-merge
```

The candidate endpoint accepts only locale. It extracts `AuthContext` and
`TenantContext`, requires exact auth/routed tenant agreement plus
`forum_topics:list`, obtains the database and `TransactionalEventBus` from the
server-only `HostRuntimeContext`, and calls
`TopicService::list_with_locale_fallback` with the existing 100-topic bound.

The merge endpoint accepts only the framework-neutral
`ForumTopicMergeCommand`. It derives tenant and actor from server context,
requires `forum_topics:manage`, parses the bounded UUID identities, and calls
the existing `ForumTopicMergeService`. Ordinary and explicit accepted-solution
resolution paths return the same immutable owner receipt used by GraphQL.

No native DTO accepts an access token, tenant ID, actor ID, permission snapshot,
database handle or event-bus handle.

## GraphQL parity

`admin/src/transport/topic_merge_graphql_adapter.rs` remains unchanged and
continues to own:

- the bounded candidate query;
- `mergeForumTopic`;
- `mergeForumTopicResolvingSolution`.

CSR/headless builds therefore retain the existing external transport contract.
The native path is direct owner composition rather than GraphQL wrapped inside
a server function.

## Host feature propagation

`apps/admin/Cargo.toml` forwards all three Forum admin profiles:

```text
rustok-forum-admin/csr
rustok-forum-admin/hydrate
rustok-forum-admin/ssr
```

The package itself exposes matching features and enables its server-only owner,
outbox, UUID and `leptos_axum` dependencies only for `ssr`.

## Compatibility

FORUM-21O changes no Forum owner method, GraphQL schema, REST route, merge
receipt, semantic event, migration, canonical source resolution or Next-admin
composition. The FORUM-21N UI policy, retry identity and accepted-solution
selection remain unchanged.

## Remaining FORUM-21 scope

The canonical task remains `planned`. Remaining work includes:

- maintainer SQLite/PostgreSQL, mounted-browser and transport execution evidence;
- idempotent split-selected-replies workflow;
- idempotent reply-branch fork workflow;
- bounded reply-range movement;
- final localized canonical aliases and route tombstones under FORUM-24.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-native-admin.mjs
node scripts/verify/verify-forum-topic-merge-admin-ui.mjs
cargo check -p rustok-forum-admin --features ssr --all-targets
cargo check -p rustok-forum-admin --features hydrate --target wasm32-unknown-unknown
cargo check -p rustok-forum-admin --features csr --target wasm32-unknown-unknown
cargo check -p rustok-admin --no-default-features --features ssr
cargo check -p rustok-admin --no-default-features --features hydrate --target wasm32-unknown-unknown
```

No command above was run by the implementation agent, per maintainer request.
