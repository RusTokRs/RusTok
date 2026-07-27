from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    source = target.read_text()
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one anchor, found {count}")
    target.write_text(source.replace(old, new, 1))


canonical = "crates/rustok-forum/docs/implementation-plan.md"
local = "crates/rustok-notifications/docs/implementation-plan.md"

replace_once(
    canonical,
    '''| `FORUM-20` | `in_progress` | FORUM-20A-AN provide inherited and richer category/topic visibility, recipient-aware Forum notification authorization, the Notifications inbox/group owner plane, authenticated storefront ports, native and GraphQL read/open/write transport parity, grouped UI and navigation. FORUM-20AM synchronizes the ledgers; FORUM-20AN adds GraphQL group-state commands. Write audiences, remaining trust/channel facts, search/index/SEO/deep-link migration, scheduled reconciliation/redaction, delivery transports and PostgreSQL cross-consumer evidence remain. |''',
    '''| `FORUM-20` | `in_progress` | FORUM-20A-AO provide inherited and richer category/topic visibility, recipient-aware Forum notification authorization, the Notifications inbox/group owner plane, authenticated storefront ports, native and GraphQL read/open/write transport parity, grouped UI and navigation. FORUM-20AM synchronizes the ledgers; FORUM-20AN adds GraphQL group-state commands; FORUM-20AO adds auth-reactive grouped bootstrap refresh. Write audiences, remaining trust/channel facts, search/index/SEO/deep-link migration, scheduled reconciliation/redaction, delivery transports and PostgreSQL cross-consumer evidence remain. |''',
)

replace_once(
    canonical,
    '''### Compatibility and degraded mode

The nullable public/authenticated category floor''',
    '''### Delivered in `FORUM-20AO`

- expose one current storefront transport-context resolver that reads the reactive auth
  session token and tenant signals without placing owner identity in request DTOs;
- key the grouped bootstrap resource by both the existing manual refresh nonce and the
  current transport context, so sign-in, sign-out, token refresh, and tenant changes refetch;
- pass one resolved context snapshot through exact unread-count and first bounded summary-page
  reads instead of re-resolving credentials between owner calls;
- clear prior mutation feedback when the auth scope changes while preserving explicit
  post-command refresh, compile-profile transport selection, and no-fallback behavior.

### Compatibility and degraded mode

The nullable public/authenticated category floor''',
)

replace_once(
    canonical,
    '''- add visibility-scoped category/all-read commands over an exact bounded policy scope;
- add auth-reactive automatic grouped-inbox bootstrap refresh without requiring an
  explicit resource refresh;
- add tenant-wide scheduled reconciliation, payload redaction, channel enqueue/transports,''',
    '''- add visibility-scoped category/all-read commands over an exact bounded policy scope;
- add tenant-wide scheduled reconciliation, payload redaction, channel enqueue/transports,''',
)

replace_once(
    canonical,
    '''node scripts/verify/verify-forum-notification-inbox-group-state-graphql.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs''',
    '''node scripts/verify/verify-forum-notification-inbox-group-state-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-auth-reactive-bootstrap.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs''',
)

replace_once(
    local,
    '''The authenticated storefront plane is also delivered. `NotificationInboxStorefrontPort`
derives tenant and recipient scope from `PortContext`, native Leptos server functions serve
SSR/hydrate, while GraphQL serves CSR/headless grouped reads, fresh open authorization,
and bounded group-state writes; no transport fallback is permitted. The module-owned grouped inbox UI pages owner results,
uses stale-response guards, refreshes authoritatively after writes, and navigates only after
an `Allowed` open decision. A generic manifest-driven header action exposes the localized
Notifications route and exact unread badge. Automatic auth-change bootstrap refresh,
tenant-wide scheduling/redaction, delivery transports, and PostgreSQL cross-consumer evidence
remain open.''',
    '''The authenticated storefront plane is also delivered. `NotificationInboxStorefrontPort`
derives tenant and recipient scope from `PortContext`, native Leptos server functions serve
SSR/hydrate, while GraphQL serves CSR/headless grouped reads, fresh open authorization,
and bounded group-state writes; no transport fallback is permitted. The module-owned grouped
inbox UI pages owner results, uses stale-response guards, refreshes authoritatively after
writes, navigates only after an `Allowed` open decision, and automatically reloads its
bootstrap when the reactive auth token or tenant changes. A generic manifest-driven header
action exposes the localized Notifications route and exact unread badge. Tenant-wide
scheduling/redaction, delivery transports, and PostgreSQL cross-consumer evidence remain open.''',
)

replace_once(
    local,
    '''### `FORUM-20AK / FORUM-20AL`

- GraphQL parity for unread count, bounded group summaries/items, and fresh open
  authorization;
- request DTOs cannot select tenant, recipient, or user identity;
- SSR/hydrate select native reads while CSR/headless select GraphQL with no fallback;
- group-state writes remain native-only.''',
    '''### `FORUM-20AK / FORUM-20AL`

- GraphQL parity for unread count, bounded group summaries/items, and fresh open
  authorization;
- request DTOs cannot select tenant, recipient, or user identity;
- SSR/hydrate select native reads while CSR/headless select GraphQL with no fallback;
- the later `FORUM-20AN` slice closes group-state command parity without changing these
  grouped read and open-authorization contracts.''',
)

replace_once(
    local,
    '''### `FORUM-20AN`

- GraphQL mutation parity for bounded exact-group mark-read, mark-unread, and archive;
- typed action plus bounded group/cursor/limit inputs and a required bounded idempotency key;
- authenticated context-derived tenant/recipient scope with five-second write deadline;
- selected native SSR/hydrate and GraphQL CSR/headless command paths without fallback;
- unchanged owner state service, timestamp invariants, terminal archive, and UI refresh flow.

## Remaining `NOTIFY-01`''',
    '''### `FORUM-20AN`

- GraphQL mutation parity for bounded exact-group mark-read, mark-unread, and archive;
- typed action plus bounded group/cursor/limit inputs and a required bounded idempotency key;
- authenticated context-derived tenant/recipient scope with five-second write deadline;
- selected native SSR/hydrate and GraphQL CSR/headless command paths without fallback;
- unchanged owner state service, timestamp invariants, terminal archive, and UI refresh flow.

### `FORUM-20AO`

- grouped bootstrap source combines the manual refresh nonce and the reactive transport context;
- auth token, tenant, sign-in, sign-out, and refresh-session changes trigger automatic reload;
- one context snapshot is reused for exact unread count and the first bounded summary page;
- auth-scope changes clear prior mutation feedback without polling or shadow client state.

## Remaining `NOTIFY-01`''',
)

replace_once(
    local,
    '''caller idempotency semantics. The admin package remains outside this
storefront completion claim and retains its explicit degraded state.''',
    '''caller idempotency semantics. The grouped bootstrap also tracks the reactive auth
transport context and reloads automatically when its token or tenant changes, while manual
post-command refresh remains authoritative. The admin package remains outside this storefront
completion claim and retains its explicit degraded state.''',
)

replace_once(
    local,
    '''node scripts/verify/verify-forum-notification-inbox-group-state-graphql.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs''',
    '''node scripts/verify/verify-forum-notification-inbox-group-state-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-auth-reactive-bootstrap.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs''',
)

replace_once(
    local,
    '''`FORUM-20R/20S/20T/20U/20V/20W/20X/20Y/20Z/20AA/20AB/20AC/20AD/20AE/20AF/20AG/20AH/20AI/20AJ/20AK/20AL/20AM/20AN` source and documentation slices.''',
    '''`FORUM-20R/20S/20T/20U/20V/20W/20X/20Y/20Z/20AA/20AB/20AC/20AD/20AE/20AF/20AG/20AH/20AI/20AJ/20AK/20AL/20AM/20AN/20AO` source and documentation slices.''',
)
