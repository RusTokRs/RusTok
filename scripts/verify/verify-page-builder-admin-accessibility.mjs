#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : process.cwd();

const files = {
  assets: "crates/rustok-page-builder/admin/src/editor/asset_section.rs",
  styles: "crates/rustok-page-builder/admin/src/editor/style_section.rs",
  properties: "crates/rustok-page-builder/admin/src/editor/properties_section.rs",
  responsive: "crates/rustok-page-builder/admin/src/editor/responsive_styles.rs",
  traits: "crates/rustok-page-builder/admin/src/editor/trait_panel.rs",
  pages: "crates/rustok-page-builder/admin/src/editor/page_manager.rs",
  paletteLayers: "crates/rustok-page-builder/admin/src/editor/palette_layers.rs",
  toolbar: "crates/rustok-page-builder/admin/src/editor/toolbar.rs",
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
  '<label class="grid gap-1 text-sm">',
  'aria-label=add_asset_accessible_label',
  'let select_accessible_label = format!("{select_label}: {accessible_name}");',
  'let remove_accessible_label = format!("{remove_label}: {accessible_name}");',
  'aria-label=select_accessible_label',
  'aria-label=remove_accessible_label',
]) requireMarker("assets", marker, "asset controls");
for (const stale of ["placeholder=asset_id_label", "placeholder=asset_url_label"]) {
  forbidMarker("assets", stale, "asset controls");
}

for (const marker of [
  '<span class="font-medium">{property_label}</span>',
  '<span class="font-medium">{value_label}</span>',
]) requireMarker("styles", marker, "style controls");

for (const marker of [
  'aria-label=tag_label',
  'aria-label=tag_apply_accessible_label',
  '<span class="font-medium">{content_label}</span>',
  'aria-label=content_apply_accessible_label',
  'aria-label=content_clear_accessible_label',
  'aria-label=attribute_name_label.clone()',
  'aria-label=attribute_value_label.clone()',
  'aria-label=attribute_apply_accessible_label',
  'let clear_accessible_label = format!("{clear_label}: {name}");',
]) requireMarker("properties", marker, "property controls");
forbidMarker(
  "properties",
  '<label class="block text-sm font-medium">{tag_label}</label>',
  "property controls",
);

for (const marker of [
  '<span class="font-medium">{breakpoint_label}</span>',
  '<span class="font-medium">{property_label}</span>',
  '<span class="font-medium">{value_label}</span>',
]) requireMarker("responsive", marker, "responsive controls");

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
  'aria-pressed=active.to_string()',
  'aria-label=new_page_name_accessible_label',
  '<span class="font-medium">{name_label.clone()}</span>',
  '<span class="font-medium">{id_label}</span>',
]) requireMarker("pages", marker, "page controls");
forbidMarker(
  "pages",
  '<label class="text-sm font-medium">{name_label.clone()}</label>',
  "page controls",
);

for (const marker of [
  'let insert_accessible_label = format!("{add_label}: {block_accessible_name}");',
  'let drag_accessible_label = format!("{drag_label}: {block_accessible_name}");',
  'aria-label=insert_accessible_label',
  'aria-label=drag_accessible_label',
  'aria-pressed=active.to_string()',
]) requireMarker("paletteLayers", marker, "palette/layer controls");

for (const marker of [
  'role="toolbar"',
  'aria-label="Page builder actions"',
  'aria-live="polite"',
]) requireMarker("toolbar", marker, "toolbar accessibility baseline");

if (failures.length > 0) {
  console.error("Page Builder admin accessibility source verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Page Builder admin accessibility source verified.");
