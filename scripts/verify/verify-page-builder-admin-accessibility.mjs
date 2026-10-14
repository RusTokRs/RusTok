#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : process.cwd();

// The authoring panels guarded here used to live as separate page-builder
// admin files; they now live in the framework-owned Leptos workspace
// (`FlyEditorWorkspace` and its panels), so the markers bind to that owner.
const files = {
  workspace: "crates/ui/fly/leptos/src/editor.rs",
  traits: "crates/modules/rustok-page-builder/admin/src/editor/trait_panel.rs",
  toolbar: "crates/modules/rustok-page-builder/admin/src/editor/toolbar.rs",
};

const failures = [];
const source = {};

for (const [key, relativePath] of Object.entries(files)) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(absolutePath)) {
    failures.push(`${relativePath}: required accessibility source is missing`);
    source[key] = "";
    continue;
  }
  const stats = fs.lstatSync(absolutePath);
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${relativePath}: source must be a regular non-symlink file`);
    source[key] = "";
    continue;
  }
  source[key] = fs.readFileSync(absolutePath, "utf8");
}

function requireMarker(key, marker, label) {
  if (!source[key].includes(marker)) failures.push(`${label}: missing '${marker}'`);
}

function forbidMarker(key, marker, label) {
  if (source[key].includes(marker)) failures.push(`${label}: stale pattern remains '${marker}'`);
}

for (const marker of [
  '<span>{asset_id_label.clone()}</span>',
  '<span>{asset_url_label.clone()}</span>',
  'aria-label=asset_id_label',
  'aria-label=asset_url_label',
]) requireMarker("workspace", marker, "asset controls");
for (const stale of ["placeholder=asset_id_label", "placeholder=asset_url_label"]) {
  forbidMarker("workspace", stale, "asset controls");
}

for (const marker of [
  'aria-label=style_property_label',
  'aria-label=style_value_label',
]) requireMarker("workspace", marker, "style controls");

for (const marker of [
  'aria-label=tag_label',
  'aria-label=content_label',
  'aria-label=attribute_name_label',
  'aria-label=attribute_value_label',
  'aria-label=clear_accessible',
]) requireMarker("workspace", marker, "property controls");

for (const marker of [
  'attr:aria-label=breakpoint_label',
  'attr:aria-label=state_label',
  'attr:aria-label=property_label.clone()',
  'attr:aria-label=messages.style_value.clone()',
]) requireMarker("workspace", marker, "responsive controls");

for (const marker of [
  'data-fly-trait-id=schema.id.clone()',
  'aria-label=input_label.clone()',
  'aria-label=apply_accessible_label',
  'aria-label=clear_accessible_label',
]) requireMarker("traits", marker, "trait controls");
forbidMarker(
  "traits",
  '<label class="block text-sm font-medium">{schema.label.clone()}</label>',
  "trait controls",
);

for (const marker of [
  'aria-current=is_active.then_some("page")',
  'aria-label=remove_label',
  'aria-label=new_page_name_label.clone()',
]) requireMarker("workspace", marker, "page controls");

for (const marker of [
  'role="listitem"',
  'aria-label=palette_label',
  'aria-label=layers_label',
  'aria-current=is_selected.then_some("true")',
]) requireMarker("workspace", marker, "palette/layer controls");

for (const marker of [
  'role="toolbar"',
  'aria-label=',
  'aria-live="polite"',
]) requireMarker("toolbar", marker, "toolbar accessibility baseline");

if (failures.length > 0) {
  console.error("Page Builder admin accessibility source verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Page Builder admin accessibility source verified.");
