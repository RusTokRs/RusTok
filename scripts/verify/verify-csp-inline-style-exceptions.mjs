#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const registerPath = "docs/security/csp-inline-style-attribute-exceptions.json";
const maxRegisteredSites = 0;
const maxRegisteredFiles = 0;
const failures = [];

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function exists(relativePath) {
  return fs.existsSync(path.join(repoRoot, relativePath));
}

function normalized(relativePath) {
  return relativePath.split(path.sep).join("/");
}

function collectRustFiles(relativeRoot) {
  const absoluteRoot = path.join(repoRoot, relativeRoot);
  if (!fs.existsSync(absoluteRoot)) return [];

  const files = [];
  const stack = [absoluteRoot];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const absolute = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(absolute);
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
        files.push(normalized(path.relative(repoRoot, absolute)));
      }
    }
  }
  return files;
}

function isRustHostedUiPath(relativePath) {
  return (
    relativePath.startsWith("apps/admin/src/") ||
    relativePath.startsWith("apps/storefront/src/") ||
    relativePath.includes("/admin/src/") ||
    relativePath.includes("/storefront/src/")
  );
}

function styleAttributeCount(source) {
  let count = 0;
  for (const line of source.split(/\r?\n/)) {
    // Multiline Leptos attributes conventionally start their own line. Rust bindings such as
    // `let style = ...` do not match because `style` is not the first token on the line.
    if (/^\s*style\s*=/.test(line)) {
      count += 1;
      continue;
    }
    // Also cover compact tags such as `<div class="..." style="...">` without treating
    // identifiers like `accent_style =` or ordinary Rust assignments as HTML attributes.
    count += [...line.matchAll(/<[^>\n]*\sstyle\s*=/g)].length;
  }
  return count;
}

for (const [fixture, expected] of [
  ["let style = String::new();", 0],
  ["let accent_style = format!(\"background:{}\", color);", 0],
  ["    style=move || runtime.style()", 1],
  ["    style = progress_width", 1],
  ['<div class="meter" style="width:50%"></div>', 1],
]) {
  const actual = styleAttributeCount(fixture);
  if (actual !== expected) {
    failures.push(
      `inline-style parser fixture ${JSON.stringify(fixture)} expected ${expected}, found ${actual}`,
    );
  }
}

function requireNonEmpty(value, label) {
  if (typeof value !== "string" || value.trim().length === 0) {
    failures.push(`${label} must be a non-empty string`);
  }
}

function requireMarkers(file, markers) {
  const source = read(file);
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${file}: missing required marker ${marker}`);
  }
  return source;
}

function forbidMarkers(file, markers) {
  const source = read(file);
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${file}: forbidden legacy marker ${marker}`);
  }
  return source;
}

if (!exists(registerPath)) {
  console.error(`Inline-style exception register is missing: ${registerPath}`);
  process.exit(1);
}

let register;
try {
  register = JSON.parse(read(registerPath));
} catch (error) {
  console.error(`Inline-style exception register is invalid JSON: ${error.message}`);
  process.exit(1);
}

if (register.schema_version !== 1) {
  failures.push(`${registerPath}: schema_version must be 1`);
}
requireNonEmpty(register.policy?.directive, `${registerPath}: policy.directive`);
requireNonEmpty(register.policy?.owner, `${registerPath}: policy.owner`);
requireNonEmpty(register.policy?.approved_on, `${registerPath}: policy.approved_on`);
requireNonEmpty(register.policy?.review_by, `${registerPath}: policy.review_by`);
requireNonEmpty(register.policy?.exit_criteria, `${registerPath}: policy.exit_criteria`);

const reviewBy = Date.parse(`${register.policy?.review_by}T23:59:59Z`);
if (!Number.isFinite(reviewBy)) {
  failures.push(`${registerPath}: policy.review_by must be an ISO date`);
} else {
  const verificationDate = process.env.VERIFICATION_DATE
    ? Date.parse(`${process.env.VERIFICATION_DATE}T00:00:00Z`)
    : Date.now();
  if (!Number.isFinite(verificationDate)) {
    failures.push("VERIFICATION_DATE must be an ISO date when provided");
  } else if (verificationDate > reviewBy) {
    failures.push(
      `${registerPath}: inline-style exception review expired on ${register.policy.review_by}`,
    );
  }
}

const entries = Array.isArray(register.exceptions) ? register.exceptions : [];
if (!Array.isArray(register.exceptions)) {
  failures.push(`${registerPath}: exceptions must be an array`);
}
if (entries.length > maxRegisteredFiles) {
  failures.push(
    `${registerPath}: exception file count ${entries.length} exceeds ratchet ${maxRegisteredFiles}`,
  );
}

const registered = new Map();
for (const [index, entry] of entries.entries()) {
  const label = `${registerPath}: exceptions[${index}]`;
  requireNonEmpty(entry.path, `${label}.path`);
  requireNonEmpty(entry.owner, `${label}.owner`);
  requireNonEmpty(entry.reason, `${label}.reason`);
  requireNonEmpty(entry.constraints, `${label}.constraints`);
  requireNonEmpty(entry.exit_criteria, `${label}.exit_criteria`);

  if (!Number.isInteger(entry.expected_occurrences) || entry.expected_occurrences <= 0) {
    failures.push(`${label}.expected_occurrences must be a positive integer`);
  }
  if (registered.has(entry.path)) {
    failures.push(`${registerPath}: duplicate exception path ${entry.path}`);
  } else {
    registered.set(entry.path, entry);
  }

  if (!exists(entry.path)) {
    failures.push(`${entry.path}: registered inline-style source file does not exist`);
    continue;
  }
  const source = read(entry.path);
  const count = styleAttributeCount(source);
  if (count !== entry.expected_occurrences) {
    failures.push(
      `${entry.path}: expected exactly ${entry.expected_occurrences} style attribute source site(s), found ${count}`,
    );
  }

  const markers = Array.isArray(entry.required_markers) ? entry.required_markers : [];
  if (markers.length === 0) {
    failures.push(`${label}.required_markers must contain at least one source marker`);
  }
  for (const marker of markers) {
    requireNonEmpty(marker, `${label}.required_markers`);
    if (typeof marker === "string" && !source.includes(marker)) {
      failures.push(`${entry.path}: missing required inline-style constraint marker ${marker}`);
    }
  }

  const evidence = Array.isArray(entry.required_evidence) ? entry.required_evidence : [];
  for (const [evidenceIndex, item] of evidence.entries()) {
    const evidenceLabel = `${label}.required_evidence[${evidenceIndex}]`;
    requireNonEmpty(item?.path, `${evidenceLabel}.path`);
    requireNonEmpty(item?.marker, `${evidenceLabel}.marker`);
    if (typeof item?.path !== "string" || !exists(item.path)) {
      failures.push(`${evidenceLabel}: evidence file does not exist`);
      continue;
    }
    if (typeof item?.marker === "string" && !read(item.path).includes(item.marker)) {
      failures.push(`${item.path}: missing required evidence marker ${item.marker}`);
    }
  }
}

const rustFiles = [
  ...collectRustFiles("apps/admin/src"),
  ...collectRustFiles("apps/storefront/src"),
  ...collectRustFiles("crates").filter(isRustHostedUiPath),
];
const uniqueRustFiles = [...new Set(rustFiles)].sort();
const observed = new Map();
for (const relativePath of uniqueRustFiles) {
  const count = styleAttributeCount(read(relativePath));
  if (count > 0) observed.set(relativePath, count);
}

for (const [relativePath, count] of observed) {
  if (!registered.has(relativePath)) {
    failures.push(
      `${relativePath}: found ${count} unregistered inline style attribute source site(s)`,
    );
  }
}
for (const relativePath of registered.keys()) {
  if (!observed.has(relativePath)) {
    failures.push(`${relativePath}: stale exception entry has no inline style attribute source site`);
  }
}

const totalSites = [...observed.values()].reduce((sum, value) => sum + value, 0);
if (totalSites > maxRegisteredSites) {
  failures.push(
    `${registerPath}: observed ${totalSites} inline-style sites exceeds ratchet ${maxRegisteredSites}`,
  );
}

const legacyCanvas = "crates/rustok-page-builder/admin/src/editor/admin_canvas.rs";
if (exists(legacyCanvas)) {
  failures.push(`${legacyCanvas}: dead legacy canvas must not be restored`);
}

requireMarkers("crates/rustok-ui-core/src/css.rs", [
  "normalize_css_hex_color",
  "css_hex_accent_class",
  "css_background_accent_class",
  "let warm_threshold = ((u16::from(red) * 3) / 4) as u8",
  "matches!(digits.len(), 3 | 4 | 6 | 8)",
  "character.is_ascii_hexdigit()",
  "bg-gradient-to-b from-sky-500 to-amber-500",
  "#fff;background:url(https://attacker.invalid/x)",
]);
requireMarkers("crates/rustok-forum/src/entities/forum_category.rs", [
  "async fn before_save",
  "ActiveValue::Set(Some(color))",
  "normalize_category_color",
  "matches!(digits.len(), 3 | 4 | 6 | 8)",
  "character.is_ascii_hexdigit()",
  "DbErr::Custom",
  "#fff;background:url(https://attacker.invalid/x)",
]);
requireMarkers("crates/rustok-forum/storefront/src/core.rs", [
  "pub accent_class: &'static str",
  "pub fn forum_storefront_accent_class",
  "css_hex_accent_class(color)",
  "accent_class: forum_storefront_accent_class",
]);
forbidMarkers("crates/rustok-forum/storefront/src/core.rs", [
  "accent_style",
  "forum_storefront_accent_style",
  "background:{value}",
]);
requireMarkers("crates/rustok-forum/storefront/src/ui/leptos.rs", [
  "card.accent_class",
  "absolute inset-y-0 left-0 w-1.5",
]);
forbidMarkers("crates/rustok-forum/storefront/src/ui/leptos.rs", [
  "style=card.accent_style",
  "card.accent_style",
]);
requireMarkers("crates/rustok-page-builder/admin/src/editor/palette_layers.rs", [
  "fn layer_indent_class",
  '0 => "pl-2"',
  '_ => "pl-[120px]"',
  "layer_indent_uses_a_bounded_class_scale",
]);
forbidMarkers("crates/rustok-page-builder/admin/src/editor/palette_layers.rs", [
  'style=format!("padding-left:',
]);
requireMarkers("crates/rustok-page-builder/admin/src/editor/isolated_canvas.rs", [
  "struct ViewportSvgGeometry",
  "fn viewport_svg_geometry",
  "data-fly-svg-viewport",
  "<foreignObject",
  "viewBox=move || viewport_geometry.get().view_box",
  "viewport_geometry_preserves_source_dimensions_and_applies_zoom",
  "struct OverlayGeometry",
  "fn overlay_geometry",
  "overlay_geometry_uses_svg_coordinates_without_css_text",
]);
forbidMarkers("crates/rustok-page-builder/admin/src/editor/isolated_canvas.rs", [
  "style=",
  "transform:scale",
  "fn overlay_style",
]);
requireMarkers("crates/rustok-page-builder/admin/src/editor/resize_handles.rs", [
  "struct SvgRectGeometry",
  "fn svg_handle_position",
  "fn resize_handle_cursor_class",
  "<circle",
  "resize_geometry_uses_svg_attributes_and_bounded_cursor_classes",
]);
forbidMarkers("crates/rustok-page-builder/admin/src/editor/resize_handles.rs", [
  "fn rect_style",
  "fn handle_style",
  "style=move ||",
]);

requireMarkers("crates/rustok-forum/admin/src/ui/leptos.rs", [
  "css_background_accent_class(vm.accent_style.as_str())",
  "absolute inset-y-0 left-0 w-1.5",
]);
forbidMarkers("crates/rustok-forum/admin/src/ui/leptos.rs", [
  "style=vm.accent_style",
]);
requireMarkers("apps/admin/src/features/modules/components/modules_list.rs", [
  "let progress_value = build.progress.clamp(0, 100)",
  "<progress",
  "max="100"",
  "value=progress_value",
]);
forbidMarkers("apps/admin/src/features/modules/components/modules_list.rs", [
  "progress_width",
  "style=progress_width",
]);

for (const [file, required] of [
  [
    "apps/server/src/middleware/security_headers.rs",
    ["style-src 'self' {nonce}", "style-src-attr 'unsafe-inline'", "style-src-attr 'none'"],
  ],
  [
    "apps/admin/src/app/security.rs",
    ["style-src 'self' {nonce}", "style-src-attr 'unsafe-inline'"],
  ],
]) {
  const source = read(file);
  for (const marker of required) {
    if (!source.includes(marker)) failures.push(`${file}: missing CSP marker ${marker}`);
  }
  if (source.includes("style-src 'self' 'unsafe-inline'")) {
    failures.push(`${file}: blanket inline style elements must remain forbidden`);
  }
}

if (failures.length > 0) {
  console.error("CSP inline-style exception verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  `✔ ${totalSites} inline style attribute source site(s) are exactly registered across ${observed.size} Rust-hosted UI file(s); ratchet ${maxRegisteredSites}/${maxRegisteredFiles}; review due ${register.policy.review_by}`,
);
