#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const registerPath = "docs/security/csp-next-style-exceptions.json";
const sourceRoots = ["apps/next-admin", "apps/next-frontend"];
const excludedDirectories = new Set([
  ".git",
  ".next",
  "coverage",
  "dist",
  "node_modules",
  "out",
]);
const maxRegisteredFiles = 3;
const maxStyleAttributeSites = 47;
const maxRuntimeStyleElements = 1;
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

function collectNextSourceFiles(relativeRoot) {
  const absoluteRoot = path.join(repoRoot, relativeRoot);
  if (!fs.existsSync(absoluteRoot)) return [];

  const files = [];
  const stack = [absoluteRoot];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      if (entry.isDirectory() && excludedDirectories.has(entry.name)) continue;
      const absolute = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(absolute);
      } else if (entry.isFile() && /\.(?:jsx|tsx)$/.test(entry.name)) {
        files.push(normalized(path.relative(repoRoot, absolute)));
      }
    }
  }
  return files;
}

function occurrenceCount(source, expression) {
  return [...source.matchAll(expression)].length;
}

function styleAttributeCount(source) {
  return occurrenceCount(source, /\bstyle\s*=\s*\{/g);
}

function runtimeStyleElementCount(source) {
  return occurrenceCount(source, /<style(?:\s|>)/g);
}

function domStyleWriteCount(source) {
  return occurrenceCount(source, /\.style(?:\.|\s*=)/g);
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
    if (source.includes(marker)) failures.push(`${file}: forbidden marker ${marker}`);
  }
  return source;
}

for (const [fixture, expectedStyleAttributes, expectedStyleElements, expectedDomWrites] of [
  ["const style = {};", 0, 0, 0],
  ["<div style={{ width: 10 }} />", 1, 0, 0],
  ["<div style={computedStyle} />", 1, 0, 0],
  ["<style>{css}</style>", 0, 1, 0],
  ["node.style.width = '1px';", 0, 0, 1],
]) {
  const actual = [
    styleAttributeCount(fixture),
    runtimeStyleElementCount(fixture),
    domStyleWriteCount(fixture),
  ];
  const expected = [expectedStyleAttributes, expectedStyleElements, expectedDomWrites];
  if (actual.some((value, index) => value !== expected[index])) {
    failures.push(
      `Next style parser fixture ${JSON.stringify(fixture)} expected ${expected.join("/")}, found ${actual.join("/")}`,
    );
  }
}

if (!exists(registerPath)) {
  console.error(`Next CSP style exception register is missing: ${registerPath}`);
  process.exit(1);
}

let register;
try {
  register = JSON.parse(read(registerPath));
} catch (error) {
  console.error(`Next CSP style exception register is invalid JSON: ${error.message}`);
  process.exit(1);
}

if (register.schema_version !== 1) {
  failures.push(`${registerPath}: schema_version must be 1`);
}
requireNonEmpty(register.policy?.directive, `${registerPath}: policy.directive`);
requireNonEmpty(
  register.policy?.runtime_style_directive,
  `${registerPath}: policy.runtime_style_directive`,
);
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
    failures.push(`${registerPath}: Next style exception review expired on ${register.policy.review_by}`);
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

  if (
    !Number.isInteger(entry.expected_style_attribute_occurrences) ||
    entry.expected_style_attribute_occurrences < 0
  ) {
    failures.push(`${label}.expected_style_attribute_occurrences must be a non-negative integer`);
  }
  if (
    !Number.isInteger(entry.expected_runtime_style_element_occurrences) ||
    entry.expected_runtime_style_element_occurrences < 0
  ) {
    failures.push(
      `${label}.expected_runtime_style_element_occurrences must be a non-negative integer`,
    );
  }
  if (
    entry.expected_style_attribute_occurrences === 0 &&
    entry.expected_runtime_style_element_occurrences === 0
  ) {
    failures.push(`${label}: entry must register at least one observed CSP style site`);
  }

  if (typeof entry.path === "string") {
    const allowedPath = sourceRoots.some((root) => entry.path.startsWith(`${root}/`));
    if (!allowedPath || !/\.(?:jsx|tsx)$/.test(entry.path)) {
      failures.push(`${label}.path must be a JSX/TSX file under a reviewed Next source root`);
    }
  }
  if (registered.has(entry.path)) {
    failures.push(`${registerPath}: duplicate exception path ${entry.path}`);
  } else {
    registered.set(entry.path, entry);
  }

  if (typeof entry.path !== "string" || !exists(entry.path)) {
    failures.push(`${entry.path}: registered Next style source file does not exist`);
    continue;
  }
  const source = read(entry.path);
  const actualStyleAttributes = styleAttributeCount(source);
  const actualStyleElements = runtimeStyleElementCount(source);
  if (actualStyleAttributes !== entry.expected_style_attribute_occurrences) {
    failures.push(
      `${entry.path}: expected exactly ${entry.expected_style_attribute_occurrences} JSX style prop(s), found ${actualStyleAttributes}`,
    );
  }
  if (actualStyleElements !== entry.expected_runtime_style_element_occurrences) {
    failures.push(
      `${entry.path}: expected exactly ${entry.expected_runtime_style_element_occurrences} runtime style element(s), found ${actualStyleElements}`,
    );
  }
}

const nextFiles = [...new Set(sourceRoots.flatMap(collectNextSourceFiles))].sort();
const observed = new Map();
let totalStyleAttributes = 0;
let totalStyleElements = 0;
let totalDomStyleWrites = 0;
for (const relativePath of nextFiles) {
  const source = read(relativePath);
  const styleAttributes = styleAttributeCount(source);
  const styleElements = runtimeStyleElementCount(source);
  const domStyleWrites = domStyleWriteCount(source);
  totalStyleAttributes += styleAttributes;
  totalStyleElements += styleElements;
  totalDomStyleWrites += domStyleWrites;
  if (styleAttributes > 0 || styleElements > 0) {
    observed.set(relativePath, { styleAttributes, styleElements });
  }
  if (domStyleWrites > 0) {
    failures.push(
      `${relativePath}: found ${domStyleWrites} direct DOM style write(s); use a reviewed class, attribute, or adapter`,
    );
  }
}

for (const [relativePath, counts] of observed) {
  if (!registered.has(relativePath)) {
    failures.push(
      `${relativePath}: found ${counts.styleAttributes} JSX style prop(s) and ${counts.styleElements} runtime style element(s) without a register entry`,
    );
  }
}
for (const relativePath of registered.keys()) {
  if (!observed.has(relativePath)) {
    failures.push(`${relativePath}: stale Next style exception entry has no observed CSP style site`);
  }
}

if (totalStyleAttributes > maxStyleAttributeSites) {
  failures.push(
    `${registerPath}: observed ${totalStyleAttributes} JSX style props exceeds ratchet ${maxStyleAttributeSites}`,
  );
}
if (totalStyleElements > maxRuntimeStyleElements) {
  failures.push(
    `${registerPath}: observed ${totalStyleElements} runtime style elements exceeds ratchet ${maxRuntimeStyleElements}`,
  );
}
if (totalDomStyleWrites !== 0) {
  failures.push(`${registerPath}: direct DOM style write ratchet is zero, found ${totalDomStyleWrites}`);
}

requireMarkers("apps/admin/index.html", [
  '<meta name="color-scheme" content="light dark" />',
  'document.documentElement.classList.toggle("dark", theme === "dark")',
  'document.documentElement.classList.remove("dark")',
]);
forbidMarkers("apps/admin/index.html", ["document.documentElement.style", " style="]);
requireMarkers("apps/admin/input.css", [
  ":root {",
  "color-scheme: light;",
  ".dark {",
  "color-scheme: dark;",
]);
requireMarkers("apps/server/src/middleware/security_headers.rs", [
  "style-src-attr 'unsafe-inline'",
  "style-src-attr 'none'",
  "content-security-policy-report-only",
]);
requireMarkers("apps/next-admin/src/shared/ui/forms/form-textarea.tsx", [
  "const TEXTAREA_RESIZE_CLASSES",
  "NonNullable<TextareaConfig['resize']>",
  "className={TEXTAREA_RESIZE_CLASSES[resize]}",
]);
forbidMarkers("apps/next-admin/src/shared/ui/forms/form-textarea.tsx", [
  "style={{ resize }}",
]);
requireMarkers("apps/next-admin/src/features/overview/components/bar-graph-skeleton.tsx", [
  "const BAR_HEIGHT_CLASSES = [",
  "BAR_HEIGHT_CLASSES.map",
  "className={`w-full ${heightClass}`}",
]);
forbidMarkers("apps/next-admin/src/features/overview/components/bar-graph-skeleton.tsx", [
  "Math.random",
  "style=",
]);
requireMarkers("apps/next-admin/src/shared/ui/shadcn/sonner.tsx", [
  "type RusTokToasterProps = Omit<ToasterProps, 'style'>",
  "[--normal-bg:var(--popover)]",
  "[--normal-text:var(--popover-foreground)]",
  "[--normal-border:var(--border)]",
]);
forbidMarkers("apps/next-admin/src/shared/ui/shadcn/sonner.tsx", ["style="]);
requireMarkers("apps/next-admin/src/shared/ui/shadcn/progress.tsx", [
  "const progressValue = Math.min(100, Math.max(0, numericValue))",
  "viewBox='0 0 100 2'",
  "width={progressValue}",
]);
forbidMarkers("apps/next-admin/src/shared/ui/shadcn/progress.tsx", [
  "style=",
  "translateX",
]);
requireMarkers("apps/next-admin/src/widgets/data-table/data-table-skeleton.tsx", [
  "<TableHead key={columnIndex}>",
  "<TableCell key={columnIndex}>",
]);
forbidMarkers("apps/next-admin/src/widgets/data-table/data-table-skeleton.tsx", [
  "cellWidths",
  "shrinkZero",
  "style=",
]);
requireMarkers("apps/next-admin/src/shared/ui/shadcn/infobar.tsx", [
  "type InfobarProviderProps = Omit<React.ComponentProps<'div'>, 'style'>",
  "[--infobar-width:22rem]",
  "[--infobar-width-icon:3rem]",
  "data-pathname-changing={isPathnameChanging}",
  "group-data-[pathname-changing=true]:duration-0",
  "max-w-[70%]",
]);
forbidMarkers("apps/next-admin/src/shared/ui/shadcn/infobar.tsx", [
  "style=",
  "Math.random",
  "INFOBAR_WIDTH",
  "--infobar-transition-duration",
]);
requireMarkers("apps/next-admin/src/shared/ui/shadcn/sidebar.tsx", [
  "type SidebarProviderProps = Omit<React.ComponentProps<'div'>, 'style'>",
  "[--sidebar-width:16rem]",
  "[--sidebar-width-icon:3rem]",
  "w-[18rem]",
  "max-w-[70%]",
]);
forbidMarkers("apps/next-admin/src/shared/ui/shadcn/sidebar.tsx", [
  "style=",
  "Math.random",
  "SIDEBAR_WIDTH",
]);
requireMarkers("apps/next-admin/src/shared/ui/shadcn/chart.tsx", [
  "<style",
  "dangerouslySetInnerHTML",
  "--color-${configKey}",
]);

if (failures.length > 0) {
  console.error("Next CSP style boundary verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  `✔ ${totalStyleAttributes} JSX style prop(s) across ${observed.size} registered Next file(s); ${totalStyleElements} runtime style element(s); direct DOM style writes ${totalDomStyleWrites}; ratchet ${maxStyleAttributeSites}/${maxRegisteredFiles}/${maxRuntimeStyleElements}; review due ${register.policy.review_by}`,
);
