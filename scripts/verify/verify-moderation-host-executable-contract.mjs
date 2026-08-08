import fs from "node:fs";

const profiles = fs.readFileSync(
  "apps/server/tests/moderation_composition_profiles.rs",
  "utf8",
);
const failure = fs.readFileSync(
  "apps/server/tests/moderation_factory_failure_composition.rs",
  "utf8",
);
const host = fs.readFileSync(
  "apps/server/src/services/module_event_dispatcher.rs",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-moderation/docs/host-composition-executable-contract.md",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  "moderation_without_forum_materializes_an_empty_subject_registry",
  "assert!(subjects.is_empty())",
  "forum_with_moderation_materializes_topic_and_reply_adapters",
  "assert_eq!(subjects.len(), 2)",
  'subjects.contains("forum", rustok_moderation::ModerationSubjectKind::ForumTopic)',
  'subjects.contains("forum", rustok_moderation::ModerationSubjectKind::ForumPost)',
  "selected_moderation_feature_fails_when_owner_module_is_missing",
  "Moderation feature is selected but ModerationModule is missing from ModuleRegistry",
  "build_shared_runtime_extensions_with_host_providers",
]) {
  requireText(profiles, marker, `Moderation composition profile evidence is missing ${marker}`);
}

for (const marker of [
  "FailingModerationFactory",
  "FailingModerationProducerModule",
  "register_moderation_subject_adapter_factory",
  "ModerationSubjectAdapterBuildError::InvalidConfiguration",
  "selected_moderation_host_fails_closed_when_subject_factory_build_fails",
  "build_shared_runtime_extensions_with_host_providers",
  "moderation subject adapter materialization failed",
  "broken_moderation_producer/forum_post",
  "moderation subject adapter configuration is invalid",
]) {
  requireText(failure, marker, `Moderation factory-failure evidence is missing ${marker}`);
}

for (const marker of [
  "Moderation feature is selected but ModerationModule is missing from ModuleRegistry",
  "materialize_moderation_subject_adapter_registry",
  "moderation subject adapter materialization failed",
]) {
  requireText(host, marker, `Server Moderation composition boundary is missing ${marker}`);
}

for (const marker of [
  "Retained composition matrix",
  "Producer factory failure",
  "broken_moderation_producer/forum_post",
  "Factory build failure therefore remains a startup failure",
  "--no-default-features --features mod-moderation",
]) {
  requireText(docs, marker, `Moderation executable composition handoff is missing ${marker}`);
}

console.log("Moderation executable host-composition source guard passed");
