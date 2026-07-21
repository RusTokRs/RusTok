#!/usr/bin/env node

import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const verifier = path.join(scriptDir, "verify-notifications-foundation.mjs");
const fixtureRoot = mkdtempSync(path.join(tmpdir(), "rustok-notifications-foundation-"));

function write(relativePath, content) {
  const target = path.join(fixtureRoot, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content, "utf8");
}

function run() {
  return spawnSync(process.execPath, [verifier], {
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: fixtureRoot },
    encoding: "utf8",
  });
}

const validModel = `
  const MAX_NOTIFICATION_TEMPLATE_FIELDS: usize = 32;
  const MAX_NOTIFICATION_TEMPLATE_DATA_BYTES: usize = 4096;
  const MAX_NOTIFICATION_AUDIENCE_PAGE_SIZE: usize = 256;
  enum Error { DuplicateAudienceRecipient, InvalidSourceRevision }
  impl<'de> Deserialize<'de> for NotificationSourceEventRef {}
  impl NotificationSourceEventRef { pub fn source_revision(&self) {} }
  impl<'de> Deserialize<'de> for NotificationAudiencePage {}
  impl NotificationAudiencePage { pub fn recipients(&self) {} }
  enum NotificationOpenAuthorization { Unavailable }
`;

const files = {
  "crates/rustok-notifications-api/src/lib.rs": `
    pub struct NotificationSourceSlug;
    pub struct NotificationTypeKey;
    pub struct NotificationTemplateKey;
    pub struct NotificationTargetRoute;
  `,
  "crates/rustok-notifications-api/src/keys.rs": `
    const SOURCE_SLUG_MAX_BYTES: usize = 64;
    const SEMANTIC_KEY_MAX_BYTES: usize = 96;
    const AUDIENCE_CURSOR_MAX_BYTES: usize = 512;
    const TARGET_ROUTE_MAX_BYTES: usize = 512;
    enum NotificationKeyError { InvalidRoute }
    fn safe(segment: &str) -> bool { segment != "." && segment != ".." }
  `,
  "crates/rustok-notifications-api/src/model.rs": validModel,
  "crates/rustok-notifications-api/src/provider.rs": `
    trait NotificationSourceProvider {
      fn describe_event();
      fn resolve_audience();
      fn authorize_target_open();
    }
    struct NotificationSourceRegistry;
    fn register_notification_source_provider() {}
    fn notification_source_registry_from_extensions() {}
    enum Error { DuplicateSource }
  `,
  "crates/rustok-notifications/src/lib.rs": `
    fn dependencies() -> &'static [&'static str] { &["outbox"] }
    fn register() { ensure_notification_source_registry(); }
  `,
  "crates/rustok-notifications/src/service.rs": `
    let registry = value.unwrap_or_else(|| Arc::new(NotificationSourceRegistry::default()));
  `,
  "crates/rustok-notifications/rustok-module.toml": `
    [module]
    slug = "notifications"
    [provides.admin_ui]
    leptos_crate = "rustok-notifications-admin"
    [provides.storefront_ui]
    leptos_crate = "rustok-notifications-storefront"
  `,
  "crates/rustok-notifications/admin/src/core.rs": "struct NotificationsAdminStatus;",
  "crates/rustok-notifications/admin/src/transport.rs": "fn load() { NotificationsAdminStatus::foundation(); }",
  "crates/rustok-notifications/admin/src/ui/leptos.rs": "fn NotificationsAdmin() {}",
  "crates/rustok-notifications/storefront/src/core.rs": "struct State { unread_count: Option<u32> } const STATE: State = State { unread_count: None };",
  "crates/rustok-notifications/storefront/src/transport.rs": "fn load() { NotificationStorefrontState::foundation(); }",
  "crates/rustok-notifications/storefront/src/ui/leptos.rs": "fn NotificationsView() {}",
  "crates/rustok-forum/docs/implementation-plan.md": "### Delivered in `NOTIFY-00A`",
};

try {
  for (const [relativePath, content] of Object.entries(files)) write(relativePath, content);

  const baseline = run();
  if (baseline.status !== 0) {
    throw new Error(`valid fixture failed:\n${baseline.stdout}\n${baseline.stderr}`);
  }

  write(
    "crates/demo/src/lib.rs",
    "use rustok_notifications::NotificationsService;",
  );
  const ownerImport = run();
  if (ownerImport.status === 0 || !ownerImport.stderr.includes("producer imports")) {
    throw new Error(`owner-import fixture did not fail correctly:\n${ownerImport.stdout}\n${ownerImport.stderr}`);
  }
  rmSync(path.join(fixtureRoot, "crates/demo"), { recursive: true, force: true });

  write(
    "crates/rustok-notifications/storefront/src/transport.rs",
    "fn load() { NotificationStorefrontState::foundation(); let unread = Some(1); }",
  );
  const shadowUnread = run();
  if (shadowUnread.status === 0 || !shadowUnread.stderr.includes("shadow unread state")) {
    throw new Error(`shadow-unread fixture did not fail correctly:\n${shadowUnread.stdout}\n${shadowUnread.stderr}`);
  }
  write(
    "crates/rustok-notifications/storefront/src/transport.rs",
    "fn load() { NotificationStorefrontState::foundation(); }",
  );

  write(
    "crates/rustok-notifications-api/src/model.rs",
    `${validModel}\nstruct Event { pub source_revision: u64 }`,
  );
  const publicRevision = run();
  if (publicRevision.status === 0 || !publicRevision.stderr.includes("source revision must remain")) {
    throw new Error(`public-revision fixture did not fail correctly:\n${publicRevision.stdout}\n${publicRevision.stderr}`);
  }

  write(
    "crates/rustok-notifications-api/src/model.rs",
    `${validModel}\nstruct Page { pub recipients: Vec<u64> }`,
  );
  const publicRecipients = run();
  if (publicRecipients.status === 0 || !publicRecipients.stderr.includes("audience recipients must remain")) {
    throw new Error(`public-recipients fixture did not fail correctly:\n${publicRecipients.stdout}\n${publicRecipients.stderr}`);
  }

  console.log("notifications foundation verifier fixtures passed");
} finally {
  rmSync(fixtureRoot, { recursive: true, force: true });
}
