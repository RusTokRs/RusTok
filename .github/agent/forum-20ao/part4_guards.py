from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    source = target.read_text()
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one anchor, found {count}")
    target.write_text(source.replace(old, new, 1))


replace_once(
    "crates/rustok-notifications/storefront/tests/grouped_graphql_contract.rs",
    '        "load_notification_unread_count().await?",',
    '        "load_notification_unread_count_selected",',
)

replace_once(
    "scripts/verify/verify-forum-notification-inbox-grouped-storefront-ui.mjs",
    '''  "load_inbox_snapshot().await",
  "load_notification_unread_count().await?",''',
    '''  "load_inbox_snapshot(context).await",
  "load_notification_unread_count_selected",''',
)

replace_once(
    "scripts/verify/verify-forum-notification-inbox-grouped-graphql.mjs",
    '  "load_notification_unread_count().await?",',
    '  "load_notification_unread_count_selected",',
)

replace_once(
    "scripts/verify/verify-forum-notification-inbox-group-state-graphql.mjs",
    '''for (const marker of [
  "FORUM-20A-AN provide",
  "### Delivered in `FORUM-20AN`",
  "auth-reactive automatic grouped-inbox bootstrap refresh",
]) {
  requireText(canonical, marker, `canonical plan is missing ${marker}`);
}
for (const marker of ["### `FORUM-20AN`", "GraphQL CSR/headless path without fallback"]) {
  requireText(local, marker, `local plan is missing ${marker}`);
}
for (const marker of [
  "notificationInboxApplyGroupState",
  "typed actions and explicit",
  "auth-reactive automatic grouped bootstrap refresh",
]) {
  requireText(ownerReadme, marker, `owner README is missing ${marker}`);
}
for (const marker of [
  "GraphQL group-state mutations now delegate",
  "auth-reactive automatic grouped bootstrap refresh",
]) {
  requireText(live, marker, `live contract is missing ${marker}`);
}''',
    '''for (const marker of [
  "FORUM-20A-AO provide",
  "### Delivered in `FORUM-20AN`",
  "### Delivered in `FORUM-20AO`",
]) {
  requireText(canonical, marker, `canonical plan is missing ${marker}`);
}
for (const marker of [
  "### `FORUM-20AN`",
  "### `FORUM-20AO`",
  "GraphQL CSR/headless path without fallback",
]) {
  requireText(local, marker, `local plan is missing ${marker}`);
}
for (const marker of [
  "notificationInboxApplyGroupState",
  "typed actions and explicit",
  "automatically reloads its bootstrap",
]) {
  requireText(ownerReadme, marker, `owner README is missing ${marker}`);
}
for (const marker of [
  "GraphQL group-state mutations now delegate",
  "automatically reloads its bootstrap",
]) {
  requireText(live, marker, `live contract is missing ${marker}`);
}''',
)

plan_sync = "scripts/verify/verify-forum-notification-plan-sync.mjs"
replace_once(
    plan_sync,
    '''for (const marker of [
  "FORUM-20A-AL provide",
  "### Delivered in `FORUM-20H` through `FORUM-20Q`",
  "### Delivered in `FORUM-20R` through `FORUM-20AF`",
  "### Delivered in `FORUM-20AG` through `FORUM-20AL`",
  "### Delivered in `FORUM-20AM`",
  "GraphQL group-state mutations",
  "PostgreSQL concurrency",
]) {''',
    '''for (const marker of [
  "FORUM-20A-AO provide",
  "### Delivered in `FORUM-20H` through `FORUM-20Q`",
  "### Delivered in `FORUM-20R` through `FORUM-20AF`",
  "### Delivered in `FORUM-20AG` through `FORUM-20AL`",
  "### Delivered in `FORUM-20AM`",
  "### Delivered in `FORUM-20AN`",
  "### Delivered in `FORUM-20AO`",
  "PostgreSQL concurrency",
]) {''',
)

replace_once(
    plan_sync,
    '''for (const marker of [
  "### `FORUM-20AB`",
  "### `FORUM-20AC / FORUM-20AD`",
  "### `FORUM-20AE / FORUM-20AF`",
  "### `FORUM-20AG / FORUM-20AH`",
  "### `FORUM-20AI / FORUM-20AJ`",
  "### `FORUM-20AK / FORUM-20AL`",
  "### `FORUM-20AM`",
  "GraphQL group-state writes remain on the native path",
]) {''',
    '''for (const marker of [
  "### `FORUM-20AB`",
  "### `FORUM-20AC / FORUM-20AD`",
  "### `FORUM-20AE / FORUM-20AF`",
  "### `FORUM-20AG / FORUM-20AH`",
  "### `FORUM-20AI / FORUM-20AJ`",
  "### `FORUM-20AK / FORUM-20AL`",
  "### `FORUM-20AM`",
  "### `FORUM-20AN`",
  "### `FORUM-20AO`",
]) {''',
)

replace_once(
    plan_sync,
    '''for (const marker of [
  "NotificationInboxStorefrontPort",
  "feature-gated Notifications GraphQL query root",
  "### 14. Authenticated storefront transport and grouped UI",
  "GraphQL group-state mutations",
]) {
  requireText(owner, marker, `Notifications owner README is missing ${marker}`);
}
for (const marker of [
  "authenticated native/GraphQL storefront reads",
  "### Authenticated storefront ports, transports, and UI",
  "GraphQL group-state mutations",
]) {
  requireText(live, marker, `Notifications live contract is missing ${marker}`);
}''',
    '''for (const marker of [
  "NotificationInboxStorefrontPort",
  "### 14. Authenticated storefront transport and grouped UI",
  "GraphQL now",
  "automatically reloads its bootstrap",
]) {
  requireText(owner, marker, `Notifications owner README is missing ${marker}`);
}
for (const marker of [
  "### Authenticated storefront ports, transports, and UI",
  "GraphQL group-state mutations now delegate",
  "automatically reloads its bootstrap",
]) {
  requireText(live, marker, `Notifications live contract is missing ${marker}`);
}''',
)

replace_once(
    plan_sync,
    '''console.log("Forum and Notifications plans are synchronized through FORUM-20AL.");''',
    '''console.log("Historical FORUM-20AM synchronization remains valid through downstream FORUM-20AO.");''',
)
