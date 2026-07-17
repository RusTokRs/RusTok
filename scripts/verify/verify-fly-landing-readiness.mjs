import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();

function source(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), "utf8");
}

function requireText(relativePath, text) {
  const contents = source(relativePath);
  if (!contents.includes(text)) {
    throw new Error(`${relativePath} must contain ${JSON.stringify(text)}`);
  }
}

function rejectText(relativePath, text) {
  const contents = source(relativePath);
  if (contents.includes(text)) {
    throw new Error(`${relativePath} must not contain ${JSON.stringify(text)}`);
  }
}

function requireOrder(relativePath, fragments) {
  const contents = source(relativePath);
  let previous = -1;
  for (const fragment of fragments) {
    const index = contents.indexOf(fragment, previous + 1);
    if (index < 0) {
      throw new Error(`${relativePath} must contain ${JSON.stringify(fragment)}`);
    }
    if (index <= previous) {
      throw new Error(
        `${relativePath} must keep ${JSON.stringify(fragments)} in runtime order`,
      );
    }
    previous = index;
  }
}

requireText("crates/fly/src/lib.rs", "mod landing_readiness;");
requireText("crates/fly/src/lib.rs", "pub use landing_readiness::*;");
requireText("crates/fly/src/lib.rs", "mod action;");
requireText("crates/fly/src/lib.rs", "mod audit;");
requireText(
  "crates/fly/src/runtime_validation.rs",
  "validate_component_actions(document)",
);
requireText(
  "crates/fly/src/runtime_pipeline.rs",
  "pub materialized_forms: usize",
);
requireOrder("crates/fly/src/runtime_pipeline.rs", [
  "materialize_bindings(&localized_document, &effective_context)",
  "materialize_runtime(&bound_document, &effective_context)",
  "materialize_internal_page_links(&dynamic_document, &effective_context)",
  "materialize_component_actions(&linked_document, &effective_context)",
]);
requireText(
  "crates/fly/src/runtime_pipeline.rs",
  "runtime_binding_can_supply_action_before_native_materialization",
);
requireText(
  "crates/fly/src/runtime_render.rs",
  "pub native_actions: usize",
);
requireText(
  "crates/fly/src/runtime_scenario_render.rs",
  "pub materialized_forms: usize",
);
requireText(
  "crates/fly/src/runtime_scenario_render.rs",
  "pub unresolved_actions: usize",
);
requireText(
  "crates/fly/src/landing_readiness/evaluate.rs",
  "evaluate_landing_readiness_with_context",
);
requireText(
  "crates/fly/src/landing_readiness/evaluate.rs",
  "landing_translation_locale_missing",
);
requireText(
  "crates/fly/src/landing_readiness/evaluate.rs",
  "materialize_structural_document",
);
requireText(
  "crates/fly/src/landing_readiness/evaluate.rs",
  "publish_materialization_failure",
);
requireText(
  "crates/fly/src/landing_readiness/evaluate.rs",
  '"runtime_action_unresolved"',
);
requireText(
  "crates/fly/src/runtime_gate.rs",
  "pub readiness: Option<LandingReadinessPolicy>",
);
requireText(
  "crates/fly/src/runtime_gate.rs",
  "evaluate_landing_readiness_with_context",
);
requireText(
  "crates/fly/src/runtime_gate.rs",
  "runtime_publish_readiness_rejected",
);
requireText(
  "crates/fly/src/page_metadata.rs",
  "localized_metadata_exposes_preview_and_round_trips_losslessly",
);
requireText(
  "crates/rustok-page-builder/admin/src/editor/runtime.rs",
  "pub runtime_context_configured: RwSignal<bool>",
);
requireText(
  "crates/rustok-page-builder/admin/src/editor/runtime.rs",
  "configured.then_some(&context)",
);
requireText(
  "crates/rustok-page-builder/admin/src/editor/runtime.rs",
  "if matches!(&intent, UiIntent::RequestSave)",
);
rejectText(
  "crates/rustok-page-builder/admin/src/editor/runtime.rs",
  "UiIntent::Execute(_) | UiIntent::Undo | UiIntent::Redo | UiIntent::RequestSave",
);
rejectText(
  "crates/rustok-page-builder/admin/src/editor/runtime.rs",
  "Runtime publish gate rejected save",
);
requireText(
  "crates/rustok-page-builder/admin/src/editor/runtime_publish_gate.rs",
  "page_builder.runtimePublishGate.readiness",
);
requireText(
  "crates/rustok-page-builder/admin/src/editor/audit_panel.rs",
  "page_builder.audit.errors",
);
requireText(
  "crates/rustok-page-builder/admin/src/editor/mod.rs",
  "mod audit_panel;",
);
requireText(
  "crates/rustok-page-builder/admin/src/editor/modular_canvas.rs",
  "<AuditPanel runtime=audit_runtime />",
);
for (const localePath of [
  "crates/rustok-page-builder/admin/locales/en.json",
  "crates/rustok-page-builder/admin/locales/ru.json",
]) {
  requireText(localePath, '"runtimePublishGate"');
  requireText(localePath, '"readinessCategory"');
  requireText(localePath, '"audit"');
  requireText(localePath, '"checkRuntimeGate"');
}
requireText(
  "crates/rustok-page-builder/admin/locales/en.json",
  "Block publish when required translations are missing",
);
requireText(
  "crates/rustok-page-builder/admin/locales/ru.json",
  "Блокировать публикацию при отсутствии обязательных переводов",
);
rejectText(
  "crates/rustok-page-builder/admin/locales/en.json",
  "Block save and publish",
);
rejectText(
  "crates/rustok-page-builder/admin/locales/ru.json",
  "Блокировать сохранение и публикацию",
);

console.log("Fly landing readiness publish-only wiring verified.");
