#!/usr/bin/env node

import { readFileSync } from "node:fs";

const files = {
  cargo: readFileSync("crates/rustok-social-graph/Cargo.toml", "utf8"),
  lib: readFileSync("crates/rustok-social-graph/src/lib.rs", "utf8"),
  adapter: readFileSync("crates/rustok-social-graph/src/index.rs", "utf8"),
  api: readFileSync("crates/rustok-social-graph/CRATE_API.md", "utf8"),
  socialPlan: readFileSync(
    "crates/rustok-social-graph/docs/implementation-plan.md",
    "utf8",
  ),
  profilesPlan: readFileSync(
    "crates/rustok-profiles/docs/implementation-plan.md",
    "utf8",
  ),
};

const failures = [];

function requireText(name, source, text) {
  if (!source.includes(text)) {
    failures.push(`${name} is missing required marker: ${text}`);
  }
}

function forbidText(name, source, text) {
  if (source.includes(text)) {
    failures.push(`${name} contains forbidden marker: ${text}`);
  }
}

requireText(
  "Cargo.toml",
  files.cargo,
  'rustok-index = { workspace = true, optional = true }',
);
requireText("Cargo.toml", files.cargo, 'index = ["dep:rustok-index"]');
requireText("lib.rs", files.lib, '#[cfg(feature = "index")]');
requireText("lib.rs", files.lib, "pub mod index;");

for (const marker of [
  "SocialGraphRelationEvent",
  "event.validate()?",
  "IndexMutation::Upsert",
  "IndexMutation::Delete",
  "source_version",
  "relation_id",
  "tenant_id.is_nil()",
  "event_id.is_nil()",
  "LocaleMode::None",
]) {
  requireText("index adapter", files.adapter, marker);
}

for (const forbidden of [
  "sea_orm",
  "DatabaseConnection",
  "SocialGraphService",
  "social_graph_relations",
  "ProfilePresentationService",
  "acknowledge(",
  "PersistentContractConsumerGroup",
]) {
  forbidText("index adapter", files.adapter, forbidden);
}

requireText("CRATE_API.md", files.api, "acknowledges only after");
requireText("CRATE_API.md", files.api, "that result is committed");
requireText(
  "Social Graph plan",
  files.socialPlan,
  "Delivered first approved Index consumer contract",
);
requireText(
  "Social Graph plan",
  files.socialPlan,
  "Profiles privacy continues to use authoritative owner ports",
);
requireText(
  "Profiles plan",
  files.profilesPlan,
  "must not replace audience-bound profile authorization",
);
requireText(
  "Profiles plan",
  files.profilesPlan,
  "node scripts/verify/verify-social-graph-index-consumer.mjs",
);

if (failures.length > 0) {
  console.error("Social Graph Index consumer boundary verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Social Graph Index consumer boundary verification passed: sealed event conversion, monotonic source versions, tombstones, optional dependency, durable result-first acknowledgement guidance, and authoritative owner-port privacy are locked.",
);
