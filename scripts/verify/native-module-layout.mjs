import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, resolve, sep } from "node:path";

/**
 * Shared structural rules for native RusToK modules. The Blog reference runner
 * applies additional Blog-specific semantic checks after this generic check.
 * This is intentionally static: it never claims runtime FBA/FFA readiness.
 */

const BACKEND_LIMIT = 32 * 1024;
const UI_LIMIT = 40 * 1024;
const FORBIDDEN_DOMAIN = [
  ["SeaORM", /\bsea_orm(?:::|\s)/],
  ["Axum", /\baxum(?:::|\s)/],
  ["GraphQL", /\basync_graphql(?:::|\s)/],
  ["Leptos", /\bleptos(?:::|\s)/],
  ["rustok-web", /\brustok_web(?:::|\s)/],
];
const FORBIDDEN_SERVICE = FORBIDDEN_DOMAIN.filter(([name]) => name !== "SeaORM");
const FORBIDDEN_CORE = [
  ["Leptos", /\bleptos(?:::|\s)/],
  ["Dioxus", /\bdioxus(?:::|\s)/],
  ["Axum", /\baxum(?:::|\s)/],
  ["GraphQL transport", /\b(?:async_graphql|rustok_graphql)(?:::|\s)/],
];
const allowedRoot = new Set(["lib.rs", "module.rs"]);

function rustFiles(directory) {
  if (!existsSync(directory)) return [];
  const result = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const file = join(directory, entry.name);
    if (entry.isDirectory()) result.push(...rustFiles(file));
    else if (entry.isFile() && entry.name.endsWith(".rs")) result.push(file);
  }
  return result;
}

function checkImports(root, directory, patterns, diagnostics) {
  for (const file of rustFiles(directory)) {
    const text = readFileSync(file, "utf8");
    for (const [label, regex] of patterns) {
      if (regex.test(text)) {
        diagnostics.push(relative(root, file) + ": forbidden " + label + " dependency in " +
          relative(root, directory) + " (static source match)");
      }
    }
  }
}

function checkSizes(root, directory, maximum, diagnostics) {
  for (const file of rustFiles(directory)) {
    const bytes = statSync(file).size;
    if (bytes > maximum) {
      diagnostics.push(relative(root, file) + ": " + bytes +
        " bytes exceeds canonical source limit of " + maximum);
    }
  }
}

function hasSource(path) {
  return existsSync(path) && statSync(path).isFile();
}

function readRequired(root, path, diagnostics) {
  if (!hasSource(path)) {
    diagnostics.push(relative(root, path) + ": required file missing");
    return "";
  }
  return readFileSync(path, "utf8");
}

function keyInSection(source, section, key) {
  let active = false;
  for (const line of source.split(/\r?\n/)) {
    if (/^\s*\[/.test(line)) {
      active = line.trim() === "[" + section + "]";
      continue;
    }
    if (!active) continue;
    const match = line.match(new RegExp("^\\s*" + key + "\\s*=\\s*\"([^\"]+)\""));
    if (match) return match[1];
  }
  return null;
}

function checkUi(root, moduleRoot, side, manifest, diagnostics) {
  const section = "provides." + (side === "admin" ? "admin_ui" : "storefront_ui");
  const requested = new RegExp("^\\[" + section.replace(".", "\\.") + "\\]$", "m").test(manifest);
  const uiRoot = join(moduleRoot, side);
  if (!requested) {
    if (existsSync(uiRoot)) {
      diagnostics.push(relative(root, uiRoot) + ": UI package exists but " + section + " is not declared");
    }
    return;
  }
  const configured = keyInSection(manifest, section, "leptos_crate");
  if (!configured) diagnostics.push(section + ": leptos_crate must identify the real package");
  const src = join(uiRoot, "src");
  const cargo = readRequired(root, join(uiRoot, "Cargo.toml"), diagnostics);
  if (configured && !cargo.includes('name = "' + configured + '"')) {
    diagnostics.push(relative(root, uiRoot) + ": Cargo package name differs from " + section + ".leptos_crate");
  }
  const lib = readRequired(root, join(src, "lib.rs"), diagnostics);
  readRequired(root, join(src, "model.rs"), diagnostics);
  readRequired(root, join(src, "i18n.rs"), diagnostics);
  const core = hasSource(join(src, "core.rs")) ? join(src, "core.rs") : join(src, "core", "mod.rs");
  const transport = hasSource(join(src, "transport.rs")) ? join(src, "transport.rs") :
    join(src, "transport", "mod.rs");
  readRequired(root, core, diagnostics);
  const transportSource = readRequired(root, transport, diagnostics);
  if (transportSource &&
      !/\b(?:async\s+)?fn\s+(?:fetch|load|create|update|delete|query|execute|mutate|request|publish|submit|save|read)(?:_[a-z0-9_]+)?\s*\(|\b(?:pub\s+)?mod\s+(?:native_server_adapter|graphql_adapter)\b/.test(transportSource)) {
    diagnostics.push(relative(root, transport) +
      ": empty transport facade without an owner operation or a real adapter module");
  }
  readRequired(root, join(src, "ui", "mod.rs"), diagnostics);
  if (!hasSource(join(src, "ui", "leptos.rs")) &&
      !hasSource(join(src, "ui", "leptos", "mod.rs"))) {
    diagnostics.push(relative(root, join(src, "ui")) +
      ": declared Leptos UI has no dedicated render adapter");
  }
  if (!existsSync(join(uiRoot, "locales"))) {
    diagnostics.push(relative(root, join(uiRoot, "locales")) + ": declared UI has no localization bundles");
  }
  if (!hasSource(join(src, "tests", "contract.rs"))) {
    diagnostics.push(relative(root, join(src, "tests", "contract.rs")) +
      ": UI contract tests must use the dedicated tests/ slot");
  }
  if (hasSource(join(src, "contract_tests.rs"))) {
    diagnostics.push(relative(root, join(src, "contract_tests.rs")) +
      ": UI contract tests belong in src/tests/contract.rs");
  }
  if (!/#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\s+tests\s*;/.test(lib)) {
    diagnostics.push(relative(root, join(src, "lib.rs")) +
      ": missing cfg(test) tests module declaration");
  }
  checkImports(root, join(src, "core"), FORBIDDEN_CORE, diagnostics);
  if (hasSource(join(src, "core.rs"))) {
    const coreSource = readFileSync(core, "utf8");
    for (const [name, regex] of FORBIDDEN_CORE) {
      if (regex.test(coreSource)) {
        diagnostics.push(relative(root, core) + ": forbidden " + name + " dependency in FFA core");
      }
    }
  }
  checkSizes(root, src, UI_LIMIT, diagnostics);
  for (const file of rustFiles(join(src, "ui"))) {
    const source = readFileSync(file, "utf8");
    if (/\bsample_rows\b|(?:create|edit|delete) action triggered|record initiated/i.test(source)) {
      diagnostics.push(relative(root, file) + ": fake CRUD/sample content is not a functional UI");
    }
  }
  if (/\#\s*\[\s*component\s*\]/.test(lib)) {
    diagnostics.push(relative(root, join(src, "lib.rs")) + ": UI components belong in ui/, not lib.rs");
  }
  // Classic Next.js UI packages are independently owned by Next hosts.
  // Their presence is neither required nor proven by this Leptos FFA lint.
}

export function resolveNativeModule(root, slug) {
  const manifest = readFileSync(join(root, "modules.toml"), "utf8");
  const moduleHeader = manifest.match(/^\[modules\]\s*$/m);
  if (!moduleHeader) throw new Error("Missing [modules] section in modules.toml");
  const afterHeader = manifest.slice(moduleHeader.index + moduleHeader[0].length);
  const block = afterHeader.split(/\n\s*\[/)[0];
  for (const line of block.split("\n")) {
    const match = line.match(/^\s*([a-z][a-z0-9_]*)\s*=\s*\{(.+)\}\s*$/);
    if (match?.[1] !== slug) continue;
    const spec = match[2];
    const source = spec.match(/\bsource\s*=\s*"([^"]+)"/)?.[1];
    const path = spec.match(/\bpath\s*=\s*"([^"]+)"/)?.[1];
    if (source !== "path" || !path) {
      throw new Error("Module '" + slug + "' is not a native local path module");
    }
    return path;
  }
  throw new Error("Unknown native path module '" + slug + "' in modules.toml");
}

export function lintNativeModule(root, modulePath, slug) {
  const diagnostics = [];
  const repo = resolve(root);
  const moduleRoot = resolve(repo, modulePath);
  if (!moduleRoot.startsWith(repo + sep)) {
    return ["Module '" + slug + "': path escapes the repository root"];
  }
  const manifest = readRequired(repo, join(moduleRoot, "rustok-module.toml"), diagnostics);
  const cargo = readRequired(repo, join(moduleRoot, "Cargo.toml"), diagnostics);
  if (!cargo.includes("[package]")) {
    diagnostics.push(relative(repo, moduleRoot) + ": missing Cargo package contract");
  }
  if (manifest && keyInSection(manifest, "module", "slug") !== slug) {
    diagnostics.push(relative(repo, moduleRoot) + ": module.slug does not match '" + slug + "'");
  }
  const entryType = keyInSection(manifest, "crate", "entry_type");
  if (!entryType || !/^[A-Za-z][A-Za-z0-9_]*$/.test(entryType)) {
    diagnostics.push(relative(repo, moduleRoot) + ": missing valid native [crate].entry_type");
  }
  const src = join(moduleRoot, "src");
  if (!existsSync(src)) {
    diagnostics.push(relative(repo, src) + ": native source root is missing");
    return diagnostics;
  }
  const lib = readRequired(repo, join(src, "lib.rs"), diagnostics);
  const module = readRequired(repo, join(src, "module.rs"), diagnostics);
  if (lib && !/\b(?:pub(?:\([^)]*\))?\s+)?mod\s+module\s*;/.test(lib)) {
    diagnostics.push(relative(repo, join(src, "lib.rs")) + ": module.rs is not declared");
  }
  if (entryType && lib && !new RegExp("\\bpub\\s+use\\s+module\\s*::\\s*(?:\\{[^}]*\\b)?"+
    entryType+"\\b").test(lib)) {
    diagnostics.push(relative(repo, join(src, "lib.rs")) + ": native entry type is not re-exported");
  }
  if (entryType && module && !new RegExp("\\bimpl\\s+RusToKModule\\s+for\\s+"+entryType+"\\b").test(module)) {
    diagnostics.push(relative(repo, join(src, "module.rs")) + ": missing native RusToKModule implementation");
  }
  if (/\bimpl\s+(?:RusToKModule|MigrationSource)\b|register_runtime_extensions\s*\(|register_event_listeners\s*\(/.test(lib)) {
    diagnostics.push(relative(repo, join(src, "lib.rs")) + ": runtime registration belongs in module.rs");
  }
  if (/\bpub\s+(?:\([^)]*\)\s*)?(?:mod|use)\s+entities\b|\bpub\s+use\s+crate\s*::\s*entities\b/.test(lib)) {
    diagnostics.push(relative(repo, join(src, "lib.rs")) + ": entities must remain private");
  }
  const entities = join(src, "entities");
  if (existsSync(entities) && !/\b(?:pub\s*\(\s*crate\s*\)\s+)?mod\s+entities\s*;/.test(lib)) {
    diagnostics.push(relative(repo, entities) + ": persistence entities must be declared crate-private");
  }
  if (existsSync(join(src, "migrations")) && !/\bimpl\s+MigrationSource\s+for\b/.test(module)) {
    diagnostics.push(relative(repo, join(src, "module.rs")) + ": migration source must be wired in module.rs");
  }
  const contractPath = join(src, "tests", "contract.rs");
  if (!hasSource(contractPath)) {
    diagnostics.push(relative(repo, contractPath) +
      ": native contract tests belong in src/tests/contract.rs");
  } else if (!/#\s*\[\s*(?:tokio::)?test\s*\]/.test(readFileSync(contractPath, "utf8"))) {
    diagnostics.push(relative(repo, contractPath) + ": contract test file has no tests");
  }
  if (!/#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\s+tests\s*;/.test(lib)) {
    diagnostics.push(relative(repo, join(src, "lib.rs")) + ": missing cfg(test) tests module");
  }
  for (const file of readdirSync(src, { withFileTypes: true })) {
    if (file.isFile() && file.name.endsWith(".rs") && !allowedRoot.has(file.name)) {
      diagnostics.push(relative(repo, join(src, file.name)) +
        ": noncanonical src/ root Rust file; move into its responsibility slot");
    }
  }
  // A generated directory with comment-only code is not an implemented layer.
  for (const slot of [
    "domain", "dto", "entities", "services", "ports", "integrations",
    "graphql", "controllers", "rest", "migrations", "events", "error"
  ]) {
    const dir = join(src, slot);
    if (!existsSync(dir)) continue;
    const files = rustFiles(dir);
    const payload = files.map((file) => readFileSync(file, "utf8")
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/^\s*\/\/.*$/gm, "")
      .trim()).join("");
    if (!files.length || !payload) {
      diagnostics.push(relative(repo, dir) +
        ": empty responsibility slot is forbidden; remove or implement it");
    }
  }
  checkImports(repo, join(src, "domain"), FORBIDDEN_DOMAIN, diagnostics);
  checkImports(repo, join(src, "services"), FORBIDDEN_SERVICE, diagnostics);
  checkSizes(repo, src, BACKEND_LIMIT, diagnostics);
  checkUi(repo, moduleRoot, "admin", manifest, diagnostics);
  checkUi(repo, moduleRoot, "storefront", manifest, diagnostics);
  return diagnostics.sort();
}
