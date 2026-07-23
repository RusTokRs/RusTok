import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const files = {
  apiCargo: "crates/rustok-moderation-api/Cargo.toml",
  apiModel: "crates/rustok-moderation-api/src/model.rs",
  apiProvider: "crates/rustok-moderation-api/src/provider.rs",
  ownerCargo: "crates/rustok-moderation/Cargo.toml",
  ownerDomain: "crates/rustok-moderation/src/domain.rs",
  ownerPorts: "crates/rustok-moderation/src/ports.rs",
  decide: "crates/rustok-moderation/src/commands/case_decide.rs",
  entity: "crates/rustok-moderation/src/entities/moderation_decision_effect.rs",
  migration: "crates/rustok-moderation/src/migrations/m20260723_000003_create_moderation_decision_effects.rs",
};

for (const relative of Object.values(files)) {
  if (!fs.existsSync(path.join(root, relative))) {
    failures.push(`missing moderation API artifact: ${relative}`);
  }
}

const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) {
      failures.push(`${relative}: missing marker ${JSON.stringify(marker)}`);
    }
  }
};

if (failures.length === 0) {
  const apiCargo = read(files.apiCargo);
  for (const forbidden of ["sea-orm", "sea-orm-migration", "rustok-moderation ="]) {
    if (apiCargo.includes(forbidden)) {
      failures.push(`${files.apiCargo}: neutral API contains forbidden owner dependency ${JSON.stringify(forbidden)}`);
    }
  }

  requireMarkers(files.apiModel, [
    "ModerationSubjectKind",
    "ModerationScopeRef",
    "ModerationDecisionEffect",
    "MODERATION_DECISION_EFFECT_SCHEMA_V1",
    "SuspendSubject",
    "effective_until",
    "MAX_MODERATION_EFFECT_CAPABILITIES",
    "validate_for_decision_kind",
    "ApplyModerationDecisionCommand",
    "ModerationDecisionApplication",
  ]);
  requireMarkers(files.apiProvider, [
    "ModerationSubjectCommandPort",
    "ModerationSubjectAdapterKey",
    "ModerationSubjectAdapterRegistry",
    "ModerationSubjectAdapterFactoryRegistry",
    "DuplicateAdapter",
    "DuplicateFactory",
    "FactoryKeyMismatch",
    "materialize_moderation_subject_adapter_registry",
  ]);
  requireMarkers(files.ownerCargo, ['rustok-moderation-api = { path = "../rustok-moderation-api" }']);
  requireMarkers(files.ownerDomain, [
    "pub use rustok_moderation_api",
    "pub effect: ModerationDecisionEffect",
    "pub effect: Option<ModerationDecisionEffect>",
  ]);
  requireMarkers(files.ownerPorts, ["pub use rustok_moderation_api::ModerationSubjectCommandPort"]);
  if (read(files.ownerPorts).includes("pub trait ModerationSubjectCommandPort")) {
    failures.push(`${files.ownerPorts}: owner crate redeclares the neutral subject port`);
  }
  requireMarkers(files.decide, [
    "validate_for_decision_kind",
    '"version": 2',
    '"effect": &command.effect',
    "moderation_decision_effect::ActiveModel",
    "effect_schema_version",
    "map_decision(decision, Some(command.effect))",
  ]);
  requireMarkers(files.entity, [
    'table_name = "moderation_decision_effects"',
    "schema_version",
    "effect_kind",
    "effect_payload",
  ]);
  requireMarkers(files.migration, [
    "ModerationDecisionEffects::Table",
    "ux_moderation_decisions_tenant_id",
    "fk_moderation_decision_effects_tenant_decision",
    "schema_version >= 1",
  ]);
}

if (failures.length > 0) {
  console.error("Moderation neutral API boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Moderation neutral API, typed effect, registry, owner bridge, hash binding, and persistence source checks passed.");
