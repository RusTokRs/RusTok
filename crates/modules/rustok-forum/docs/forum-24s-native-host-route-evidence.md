# FORUM-24S registered native-host route evidence

Status: **executable SQLite source / maintainer execution pending**

## Scope

FORUM-24S adds an executable registered-host harness for the native Forum category and topic route-decision server functions:

```text
crates/rustok-forum/storefront/tests/native_host_route_decision_sqlite.rs
```

The machine-readable contract is:

```text
crates/rustok-forum/contracts/forum-native-host-route-evidence.json
```

This slice does not add or change route behavior. It proves that the existing native adapters are discoverable through the same Axum and Leptos server-function dispatch used by a host application.

## Registered host composition

The harness constructs an Axum router with:

```text
/api/fn/{*fn_name}
```

and dispatches requests through:

```rust
leptos_axum::handle_server_fns_with_context
```

`HostRuntimeContext` is provided to the server-function owner context. Tenant identity is supplied only through `TenantContextExtension`; it is not accepted from the route-decision form body.

The exercised endpoints are:

```text
/api/fn/forum/storefront-category-route
/api/fn/forum/storefront-topic-route
```

The requests are anonymous and omit a channel context. Channel enablement and authenticated audience facts remain covered by their existing owner and transport contracts and are not reimplemented in this harness.

## Owner fixtures

The SQLite fixture applies the real Taxonomy and Forum migrations. It creates and renames the route-bearing entities through existing Forum commands:

- `CategoryService::create`;
- `CategoryService::update`;
- `TopicService::create`;
- `TopicService::rename_slug`.

No route alias row is inserted directly. The historical category and topic slugs are therefore created by the same owner transactions used by production writes.

## Evidence cases

The registered category endpoint must return:

- `canonical` for `/en/forum/c/platform-engineering`;
- `redirect` from the historical `platform` slug to that path;
- an absent optional result for a missing category.

The registered topic endpoint must return:

- `canonical` for `/en/forum/t/{short_id}/registered-native-host`;
- `redirect` from the historical `native-host-route` slug to that path;
- an absent optional result for a missing short-id and slug pair.

The harness reads all decisions through HTTP requests to the registered server-function endpoints. It does not invoke `ForumCategoryRouteService::resolve` or `ForumTopicRouteService::resolve` directly.

## Preserved boundaries

FORUM-24S changes no production runtime code, route owner, visibility policy, channel policy, GraphQL contract, storefront DTO, storage schema, event schema or migration.

It does not claim:

- execution of the SQLite harness;
- shared storefront document-response evidence;
- browser navigation evidence;
- execution of the FORUM-24R PostgreSQL reindex harness;
- deployment reindex completion.

## Verification handoff

Maintainers can run:

```bash
node scripts/verify/verify-forum-native-host-route-evidence.mjs
cargo test -p rustok-forum-storefront --features ssr --test native_host_route_decision_sqlite -- --nocapture
```

No tests, Node verifiers, Cargo commands, formatting, SQLite execution, mounted storefront requests, browser scenarios, workflows or CI were run while preparing this slice.

## Remaining FORUM-24 evidence

- execute the FORUM-24R PostgreSQL reindex harness and the target-environment reindex;
- execute this registered native-host SQLite harness;
- retain shared storefront HTTP response evidence for canonical and redirect routes;
- retain browser navigation evidence for category, topic and reply Search destinations;
- reconcile the canonical FORUM-24 ledger after maintainer execution.

`crates/rustok-forum/docs/implementation-plan.md` remains the only authoritative Forum roadmap. Its FORUM-24 ledger is stale relative to the merged source slices. This bounded evidence document does not create a second roadmap or claim canonical ledger synchronization.
