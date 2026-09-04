import fs from "node:fs";

const serverCargo = fs.readFileSync("apps/server/Cargo.toml", "utf8");
const dispatcher = fs.readFileSync(
  "apps/server/src/services/module_event_dispatcher.rs",
  "utf8",
);
const profiles = fs.readFileSync(
  "apps/server/tests/moderation_composition_profiles.rs",
  "utf8",
);
const forumCargo = fs.readFileSync("crates/rustok-forum/Cargo.toml", "utf8");
const forumLib = fs.readFileSync("crates/rustok-forum/src/lib.rs", "utf8");

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

function forbidText(source, needle, message) {
  if (source.includes(needle)) throw new Error(message);
}

requireText(
  serverCargo,
  'mod-moderation = ["dep:rustok-moderation", "rustok-distribution/mod-moderation"]',
  "server must select Moderation owner explicitly through mod-moderation",
);
requireText(
  serverCargo,
  'mod-forum     = ["dep:rustok-forum", "mod-content", "mod-taxonomy", "rustok-content-orchestration/mod-forum", "rustok-distribution/mod-forum"]',
  "Forum server feature must remain independent from Moderation owner selection",
);
const defaultFeatures = serverCargo.match(/default = \[([\s\S]*?)\]\r?\nredis-cache =/u)?.[1];
if (!defaultFeatures) {
  throw new Error("server default feature block could not be located");
}
forbidText(
  defaultFeatures,
  '"mod-moderation"',
  "Moderation owner must remain outside server default features",
);

requireText(
  dispatcher,
  '#[cfg(feature = "mod-moderation")]',
  "host must gate Moderation materialization on explicit owner selection",
);
requireText(
  dispatcher,
  'registry.contains("moderation")',
  "selected Moderation feature must verify the owner module is registered",
);
requireText(
  dispatcher,
  "Moderation feature is selected but ModerationModule is missing from ModuleRegistry",
  "missing Moderation owner must fail host composition",
);
requireText(
  dispatcher,
  "rustok_moderation::materialize_moderation_subject_adapter_registry",
  "host must materialize the neutral Moderation adapter registry",
);
requireText(
  dispatcher,
  "moderation subject adapter materialization failed",
  "factory build/registry failures must remain startup errors",
);
requireText(
  dispatcher,
  "extensions.apply_to_host_runtime(rustok_api::HostRuntimeContext::new(db.clone()))",
  "Moderation factories must materialize only after HostRuntimeContext exists",
);

for (const marker of [
  "forum_without_moderation_keeps_forum_host_composition_available",
  "moderation_without_forum_materializes_an_empty_subject_registry",
  "forum_with_moderation_materializes_topic_and_reply_adapters",
  "selected_moderation_feature_fails_when_owner_module_is_missing",
  "subjects.is_empty()",
  "subjects.len(), 2",
  'subjects.contains("forum", rustok_moderation::ModerationSubjectKind::ForumTopic)',
  'subjects.contains("forum", rustok_moderation::ModerationSubjectKind::ForumPost)',
]) {
  requireText(profiles, marker, `Moderation composition profile source is missing ${marker}`);
}

requireText(
  forumCargo,
  'rustok-moderation-api = { path = "../rustok-moderation-api" }',
  "Forum must keep only the neutral Moderation API dependency",
);
forbidText(
  forumCargo,
  "rustok-moderation =",
  "Forum must not depend on the Moderation owner crate",
);
requireText(
  forumLib,
  "ForumModerationSubjectAdapterFactory::topic()",
  "Forum must continue registering the topic adapter factory",
);
requireText(
  forumLib,
  "ForumModerationSubjectAdapterFactory::reply()",
  "Forum must continue registering the reply adapter factory",
);
requireText(
  forumLib,
  '&["content", "taxonomy"]',
  "Forum module dependencies must remain owner-neutral and exclude Moderation",
);

for (const forbidden of [
  "moderation_cases",
  "moderation_reports",
  "moderation_appeals",
  "ReactionBar",
  "reactionSnapshot",
  "applyReaction",
]) {
  forbidText(
    profiles,
    forbidden,
    `Moderation host composition profiles must not absorb unrelated owner logic: ${forbidden}`,
  );
}

console.log("Moderation host adapter materialization source guard passed");
