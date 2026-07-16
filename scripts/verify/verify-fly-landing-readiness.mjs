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

requireText("crates/fly/src/lib.rs", "mod landing_readiness;");
requireText("crates/fly/src/lib.rs", "pub use landing_readiness::*;");
requireText("crates/fly/src/lib.rs", "mod action;");
requireText("crates/fly/src/lib.rs", "mod audit;");
requireText(
  "crates/fly/src/runtime_validation.rs",
  "validate_component_actions(document)",
);
requireText(
  "crates/fly/src/runtime_gate.rs",
  "pub readiness: Option<LandingReadinessPolicy>",
);
requireText(
  "crates/fly/src/runtime_gate.rs",
  "runtime_publish_readiness_rejected",
);
requireText(
  "crates/rustok-page-builder/admin/src/editor/runtime.rs",
  "if matches!(&intent, UiIntent::RequestSave)",
);
rejectText(
  "crates/rustok-page-builder/admin/src/editor/runtime.rs",
  "UiIntent::Execute(_) | UiIntent::Undo | UiIntent::Redo | UiIntent::RequestSave",
);
requireText(
  "crates/rustok-page-builder/admin/src/editor/runtime_publish_gate.rs",
  "Landing readiness:",
);
requireText(
  "crates/rustok-page-builder/admin/src/editor/mod.rs",
  "mod audit_panel;",
);
requireText(
  "crates/rustok-page-builder/admin/src/editor/modular_canvas.rs",
  "<AuditPanel runtime=audit_runtime />",
);

console.log("Fly landing readiness publish-only wiring verified.");
