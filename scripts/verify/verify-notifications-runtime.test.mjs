#!/usr/bin/env node

import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const verifier = path.join(scriptDir, "verify-notifications-runtime.mjs");
const fixtureRoot = mkdtempSync(path.join(tmpdir(), "rustok-notifications-runtime-"));

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

const validForumSource = `
  use rustok_notifications_api::NotificationOpenAuthorization;
  fn source(request: Request, event: Event, sequence_no: i64, limit: usize) {
    let _ = "forum.topic.created";
    forum_domain_event::Entity::find();
    SequenceNo.eq(sequence_no);
    TenantId.eq(event.tenant_id());
    TopicStatus::Open;
    forum_topic_channel_access::Entity::find();
    NotifyNewTopics.eq(true);
    ForumSubscriptionLevel::Muted;
    request.bounded_limit();
    query.limit((limit + 1) as u64);
    NotificationAudienceCursor::new("cursor");
    NotificationOpenAuthorization::Unavailable;
    let _ = "/modules/forum?category={}&topic={}";
    let _ = Internal { retryable: true };
  }
`;

const files = {
  "modules.toml": `
    [modules]
    notifications = { crate = "rustok-notifications", source = "path", path = "crates/rustok-notifications", depends_on = ["outbox"] }
    [settings]
    default_enabled = ["content"]
  `,
  "crates/rustok-distribution/Cargo.toml": `
    [features]
    mod-notifications = ["dep:rustok-notifications"]
  `,
  "crates/rustok-distribution/src/lib.rs": `
    registry.register(rustok_notifications::NotificationsModule);
  `,
  "apps/server/Cargo.toml": `
    default = ["mod-notifications"]
    mod-notifications = ["dep:rustok-notifications", "rustok-distribution/mod-notifications"]
  `,
  "apps/server/src/services/module_event_dispatcher.rs": `
    extensions.apply_to_host_runtime(host);
    materialize_notification_source_registry(&mut extensions, &host);
  `,
  "apps/admin/Cargo.toml": `rustok-notifications-admin = { path = "../../crates/rustok-notifications/admin" }`,
  "apps/storefront/Cargo.toml": `rustok-notifications-storefront = { path = "../../crates/rustok-notifications/storefront" }`,
  "crates/rustok-notifications-api/Cargo.toml": `server = ["dep:rustok-api"]`,
  "crates/rustok-notifications-api/src/provider.rs": `
    trait NotificationSourceProviderFactory {}
    struct NotificationSourceFactoryRegistry;
    fn register_notification_source_provider_factory() {}
    fn materialize_notification_source_registry() {}
    enum Error { FactorySourceMismatch, FactoryBuild }
  `,
  "crates/rustok-notifications-api/src/keys.rs": `fn safe_route_query() {}`,
  "crates/rustok-forum/Cargo.toml": `
    [dependencies]
    rustok-notifications-api.workspace = true
    [dev-dependencies]
    rustok-notifications.workspace = true
  `,
  "crates/rustok-forum/src/lib.rs": `fn register() { register_notification_source_provider_factory(); }`,
  "crates/rustok-forum/src/notification_source.rs": validForumSource,
  "crates/rustok-forum/tests/notification_source_sqlite.rs": `
    // notifications owner is absent
    use rustok_notifications::NotificationsModule;
    materialize_notification_source_registry();
    let request = Request { limit: 1 };
    // cross-tenant authorization should fail closed
    db.execute_unprepared("DROP TABLE forum_domain_events");
    let error = Internal { retryable: true };
  `,
  "crates/rustok-notifications/docs/implementation-plan.md": `
    NOTIFY-00 remains \`in_progress\` until maintainer-run verification.
    ### Delivered in \`NOTIFY-00B\`
  `,
};

try {
  for (const [relativePath, content] of Object.entries(files)) write(relativePath, content);

  const baseline = run();
  if (baseline.status !== 0) {
    throw new Error(`valid fixture failed:\n${baseline.stdout}\n${baseline.stderr}`);
  }

  write(
    "modules.toml",
    `[modules]\nnotifications = { crate = "rustok-notifications" }\n[settings]\ndefault_enabled = ["notifications"]`,
  );
  const defaultEnabled = run();
  if (defaultEnabled.status === 0 || !defaultEnabled.stderr.includes("tenant-disabled by default")) {
    throw new Error(`default-enabled fixture did not fail correctly:\n${defaultEnabled.stdout}\n${defaultEnabled.stderr}`);
  }
  write("modules.toml", files["modules.toml"]);

  write(
    "crates/rustok-forum/src/notification_source.rs",
    `${validForumSource}\nuse rustok_notifications::NotificationsService;`,
  );
  const ownerImport = run();
  if (ownerImport.status === 0 || !ownerImport.stderr.includes("imports the notifications owner")) {
    throw new Error(`owner-import fixture did not fail correctly:\n${ownerImport.stdout}\n${ownerImport.stderr}`);
  }
  write("crates/rustok-forum/src/notification_source.rs", validForumSource);

  write(
    "crates/rustok-forum/src/notification_source.rs",
    validForumSource.replace("forum_topic_channel_access::Entity::find();", ""),
  );
  const channelBypass = run();
  if (channelBypass.status === 0 || !channelBypass.stderr.includes("forum_topic_channel_access")) {
    throw new Error(`channel-bypass fixture did not fail correctly:\n${channelBypass.stdout}\n${channelBypass.stderr}`);
  }

  console.log("notifications runtime verifier fixtures passed");
} finally {
  rmSync(fixtureRoot, { recursive: true, force: true });
}
