# FORUM-20AI grouped notification storefront UI

Status: **source-ready / unvalidated**

## Scope

This slice composes the native authenticated-user inbox adapter from `FORUM-20AH`
into the module-owned Leptos storefront view.

`NotificationsView` now renders:

- exact unread count from the owner count endpoint;
- bounded authorized notification group summaries;
- one expanded group with bounded exact-group item pages;
- fresh open authorization before browser navigation;
- bounded group mark-read, mark-unread, and archive commands;
- explicit loading, empty, unavailable, paging-error, item-error, and action-error states.

The reusable `NotificationUnreadBadge` component is exported from the package root.
Global storefront navigation or header composition remains a separate application gate.

## Authoritative state

The UI does not derive unread totals from summary pages and does not optimistically
change group or unread counts after a command. A successful command collapses the
expanded group and increments the SSR resource refresh nonce. The next rendered state
comes from a new exact unread-count read and a new first group-summary page.

Group and item pagination append only rows returned by the native owner adapter. Pure
state helpers deduplicate group keys and notification IDs while retaining the owner
cursor and `has_more` result.

## Bounded commands

Each group action submits one owner page with a maximum of 64 eligible rows and a fresh
UUID idempotency key. The UI does not loop automatically over an unbounded group. When
the owner reports `has_more`, the result message asks the caller to repeat the action
after the authoritative refresh.

## Concurrency and navigation

Only one group is expanded at a time. Item requests carry an in-memory request nonce;
closing or replacing an expansion invalidates older responses, including the case where
the same group is opened again before its earlier request finishes.

An item button calls the native open-authorization endpoint. Browser navigation happens
only for an `allowed` decision carrying the current owner-provided internal route.
Unavailable decisions remain on the inbox page.

## Presentation and storage

Notification titles and bodies use bounded string values from `title`, `topic_title`,
`subject`, `body`, `message`, or `summary`, then fall back to the semantic notification
and template keys. Values are rendered as text, not HTML.

All paging and interaction state is in memory. The view reads no local storage, writes no
shadow inbox, creates no delivery attempt, and bypasses no owner authorization service.

## Evidence

- view: `storefront/src/ui/leptos.rs`;
- page state: `storefront/src/core.rs`;
- state evidence: `storefront/tests/grouped_ui_state.rs`;
- upstream transport evidence: `storefront/tests/native_transport_contract.rs`;
- machine contract:
  `rustok-forum/contracts/forum-notification-inbox-grouped-storefront-ui.json`;
- static verifier:
  `scripts/verify/verify-forum-notification-inbox-grouped-storefront-ui.mjs`.

## Validation status

Tests, Cargo commands, formatting commands, verifier execution, workflows, and CI were
not run by the implementation agent, per maintainer instruction.

## Remaining work

Global navigation/header badge composition, GraphQL exposure, scheduled reconciliation,
payload redaction, channel delivery, delivery-time authorization, host trust/channel
facts, non-public descriptor materialization, search/SEO migration, and PostgreSQL
runtime evidence remain separate gates.

The canonical Forum ledger, Notifications owner-local implementation ledger, and large
Notifications owner/live README files still require safe synchronization through
`FORUM-20AI`. This slice records that pending state instead of replacing those large
files wholesale while unrelated work may be landing.
