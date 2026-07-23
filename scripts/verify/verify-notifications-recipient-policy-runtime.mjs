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

function requireOrder(source, markers, message) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0 || index <= previous) {
      failures.push(`${message}: missing or out-of-order marker ${marker}`);
      return;
    }
    previous = index;
  }
}

function reject(source, pattern, message) {
  if (pattern.test(source)) failures.push(message);
}

const contractPath =
  "crates/rustok-notifications/contracts/notifications-recipient-policy-runtime.json";
const contract = JSON.parse(read(contractPath) || "{}");
const profilePort = read(contract.profile_owner_port?.contract ?? "");
const relationPorts = read(contract.relation_owner_ports?.contract ?? "");
const adapter = read(contract.server_composition?.adapter ?? "");
const composition = read(contract.server_composition?.composition_root ?? "");
const services = read("apps/server/src/services/mod.rs");

if (contract.slice !== "NOTIFY-07B") {
  failures.push("runtime contract must identify NOTIFY-07B");
}
if (contract.relation_owner_ports?.permissive_default_forbidden !== true) {
  failures.push("block/mute owner ports must not have a permissive default");
}
if (contract.worker_gate?.candidate_worker_default_enabled !== false) {
  failures.push("candidate worker must remain disabled by default");
}
if (contract.worker_gate?.requires_relation_ports_ready !== true) {
  failures.push("candidate worker readiness must require both relation ports");
}
if (contract.worker_gate?.readiness_separate_from_enablement !== true) {
  failures.push("relation readiness must remain separate from worker enablement");
}

for (const marker of [
  "pub trait ProfilePrivacyReadPort",
  "pub struct ProfilePrivacyRuntime",
  "context.require_policy(PortCallPolicy::read())",
  "ProfileStatus::Active",
  "ProfileVisibility::FollowersOnly | ProfileVisibility::Private",
]) {
  requireText(profilePort, marker, `Profiles privacy owner port is missing ${marker}`);
}

for (const marker of [
  "pub trait NotificationBlockReadPort",
  "pub trait NotificationMuteReadPort",
  "pub struct NotificationBlockReadRuntime",
  "pub struct NotificationMuteReadRuntime",
  "pub struct NotificationRecipientPolicyRuntime",
  "relation_ports_ready",
  "candidate_worker_enabled",
  "candidate_worker_ready",
  "self.relation_ports_ready && self.candidate_worker_enabled",
]) {
  requireText(relationPorts, marker, `recipient policy runtime contract is missing ${marker}`);
}

for (const marker of [
  "ServerNotificationRecipientPolicy",
  "ProfilePrivacyRuntime",
  "NotificationBlockReadRuntime",
  "NotificationMuteReadRuntime",
  "NotificationRecipientPolicyError::retryable",
  "NotificationRecipientSuppression::ProfileRestricted",
  "NotificationRecipientSuppression::Blocked",
  "NotificationRecipientSuppression::Muted",
  "with_candidate_worker_enabled(candidate_worker_enabled_from_environment())",
]) {
  requireText(adapter, marker, `server recipient policy adapter is missing ${marker}`);
}
requireOrder(
  adapter,
  [
    "evaluate_profile_privacy",
    "blocks_notification",
    "mutes_notification",
    "NotificationRecipientPolicyDecision::Allow",
  ],
  "recipient policy must evaluate profile, block, and mute owners before allowing delivery",
);
reject(
  adapter,
  /profile::Entity|notification_preferences|user_blocks|user_mutes/,
  "server recipient policy must use owner ports instead of private tables",
);
reject(
  adapter,
  /AllowAll|Permissive|unwrap_or\(false\)|unwrap_or_default\(\)/,
  "recipient policy must not convert missing/error relation state into allow",
);

for (const marker of [
  "ServerNotificationRecipientPolicy::compose",
  "NotificationRecipientPolicyRuntime",
  "extensions.insert(policy)",
]) {
  requireText(composition, marker, `server composition root is missing ${marker}`);
}
requireText(
  services,
  '#[cfg(all(feature = "mod-notifications", feature = "mod-profiles"))]',
  "server policy module must require both Notifications and Profiles capabilities",
);

if (failures.length > 0) {
  console.error("Notifications recipient policy runtime verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Notifications recipient policy runtime boundary verified.");
