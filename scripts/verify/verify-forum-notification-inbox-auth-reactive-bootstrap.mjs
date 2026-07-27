#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}
function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}
function rejectText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}
function between(source, start, end, label) {
  const from = source.indexOf(start);
  const to = source.indexOf(end, from + start.length);
  if (from < 0 || to < 0 || to <= from) {
    failures.push(`${label}: bounded section is missing`);
    return "";
  }
  return source.slice(from, to);
}

const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-notification-inbox-auth-reactive-bootstrap.json") ||
    "{}",
);
const transport = read(contract.storefront_transport_file ?? "");
const ui = read(contract.storefront_ui_file ?? "");
const auth = read(contract.auth_context_file ?? "");
const proof = read(contract.storefront_proof ?? "");
const note = read(contract.owner_note ?? "");
const canonical = read(contract.canonical_plan ?? "");
const local = read(contract.notifications_local_plan ?? "");
const owner = read(contract.notifications_owner_readme ?? "");
const live = read(contract.notifications_live_contract ?? "");
const storefront = read(contract.storefront_readme ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AO" ||
  contract.upstream_task !== "FORUM-20AN"
) {
  failures.push("auth-reactive bootstrap contract must identify FORUM-20AO after FORUM-20AN");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("auth-reactive bootstrap contract must not claim unexecuted evidence");
}
for (const key of [
  "reactive_auth_session_source",
  "token_change_refetch",
  "tenant_change_refetch",
  "sign_out_refetch",
  "manual_refresh_nonce_preserved",
  "single_transport_context_snapshot",
  "selected_read_facade_reused",
  "unread_and_summary_context_match",
  "cross_scope_feedback_cleared",
  "resource_driven_no_polling",
  "navigation_reactivity_preserved",
  "owner_port_unchanged",
  "native_read_path_preserved",
  "graphql_read_path_preserved",
  "no_transport_fallback",
]) {
  if (contract.composition?.[key] !== true) failures.push(`contract must record ${key}`);
}
for (const key of [
  "access_token_rendered",
  "tenant_identity_rendered",
  "local_storage",
  "shadow_inbox",
  "background_polling",
]) {
  if (contract.composition?.[key] !== false) failures.push(`contract must keep ${key} false`);
}

for (const marker of [
  "pub fn current_notification_storefront_transport_context",
  "use_context::<AuthContext>()",
  "AuthContext::get_token",
  "AuthContext::get_tenant",
  "execute_selected_transport",
]) {
  requireText(transport, marker, `transport is missing ${marker}`);
}
rejectText(transport, "fallback_failed", "transport selection must not add fallback");

for (const marker of [
  "let transport_context =",
  "Memo::new(move |_| current_notification_storefront_transport_context())",
  "Effect::new(move |_|",
  "let _ = transport_context.get();",
  "set_refresh_feedback.set(None);",
  "Resource::new_blocking",
  "move || (refresh_nonce.get(), transport_context.get())",
  "move |(_, context)| async move { load_inbox_snapshot(context).await }",
  "set_refresh_nonce.update",
  "on_refresh.run(feedback)",
]) {
  requireText(ui, marker, `storefront UI is missing ${marker}`);
}

for (const marker of [
  "pub session: RwSignal<Option<AuthSession>>",
  "self.session.get().map(|s| s.token)",
  "self.session.get().map(|s| s.tenant)",
  "self.session.set(None)",
]) {
  requireText(auth, marker, `AuthContext is missing ${marker}`);
}

const helper = between(
  ui,
  "async fn load_inbox_snapshot(",
  "fn item_state_label",
  "bootstrap helper",
);
for (const marker of [
  "context: NotificationStorefrontTransportContext",
  "load_notification_unread_count_selected(context.clone())",
  "load_notification_group_summaries_selected(context,",
]) {
  requireText(helper, marker, `bootstrap helper is missing ${marker}`);
}
rejectText(
  helper,
  "load_notification_unread_count().await?",
  "bootstrap must not re-resolve context for unread count",
);
rejectText(
  helper,
  "current_notification_storefront_transport_context()",
  "bootstrap future must reuse its source context",
);
for (const forbidden of [
  "use_interval_fn",
  "set_interval",
  "localStorage",
  "gloo_storage",
  "data-access-token",
  "data-tenant-id",
  "data-recipient-id",
]) {
  rejectText(ui, forbidden, `auth-reactive bootstrap must not add ${forbidden}`);
}

for (const marker of [
  "bootstrap_source_tracks_reactive_auth_transport_context",
  "bootstrap_reuses_one_exact_context_snapshot_and_clears_scope_feedback",
  "auth_reactivity_adds_no_polling_storage_or_identity_rendering",
]) {
  requireText(proof, marker, `source proof is missing ${marker}`);
}
for (const marker of [
  "# FORUM-20AO auth-reactive grouped inbox bootstrap",
  "source-ready / unvalidated",
  "Sign-in, sign-out, token refresh, or tenant change",
  "does not re-resolve credentials",
  "not run by the implementation agent",
]) {
  requireText(note, marker, `owner note is missing ${marker}`);
}
for (const marker of [
  "FORUM-20A-AO provide",
  "### Delivered in `FORUM-20AO`",
  "tenant-wide scheduled reconciliation",
]) {
  requireText(canonical, marker, `canonical plan is missing ${marker}`);
}
for (const marker of [
  "### `FORUM-20AO`",
  "manual refresh nonce and the reactive transport context",
]) {
  requireText(local, marker, `local plan is missing ${marker}`);
}
for (const marker of [
  "automatically reloads its bootstrap",
  "same resolved transport context",
]) {
  requireText(owner, marker, `owner README is missing ${marker}`);
  requireText(live, marker, `live contract is missing ${marker}`);
}
for (const marker of [
  "automatically reloads the grouped bootstrap",
  "current_notification_storefront_transport_context",
]) {
  requireText(storefront, marker, `storefront README is missing ${marker}`);
}

if (
  upstream.task !== "FORUM-20AN" ||
  !upstream.not_delivered?.includes(
    "auth-reactive automatic grouped inbox bootstrap refresh without explicit resource refresh",
  )
) {
  failures.push("FORUM-20AO must close the FORUM-20AN auth-reactive bootstrap residual");
}

if (failures.length > 0) {
  console.error("Forum notification auth-reactive bootstrap verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum notification auth-reactive bootstrap contract is source-ready.");
