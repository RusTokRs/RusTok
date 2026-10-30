import { existsSync, lstatSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";

/**
 * One shared static structural contract for registered native RusToK modules.
 * Blog's reference runner adds Blog-specific physical and semantic assertions.
 * This lint never certifies compilation, Next.js parity, or FBA/FFA runtime.
 */

const BACKEND_LIMIT = 32 * 1024;
const UI_LIMIT = 40 * 1024;
const BACKEND_SLOTS = new Set([
  "domain", "dto", "entities", "services", "ports", "integrations",
  "graphql", "controllers", "rest", "migrations", "tests", "events", "error",
]);
const ALLOWED_ROOT_RUST_FILES = new Set(["lib.rs", "module.rs", "runtime.rs", "error.rs"]);
const FORBIDDEN_DOMAIN = [
  ["SeaORM", /\bsea_orm(?:::|\s)/],
  ["SeaORM migrations", /\bsea_orm_migration(?:::|\s)/],
  ["Axum", /\baxum(?:_[a-z0-9]+)*(?:::|\s)/],
  ["GraphQL", /\b(?:async_graphql|rustok_graphql)(?:_[a-z0-9]+)*(?:::|\s)/],
  ["Leptos", /\bleptos(?:_[a-z0-9]+)*(?:::|\s)/],
  ["Dioxus", /\bdioxus(?:_[a-z0-9]+)*(?:::|\s)/],
  ["rustok-web", /\brustok_web(?:_[a-z0-9]+)*(?:::|\s)/],
];
const FORBIDDEN_SERVICE = FORBIDDEN_DOMAIN.filter(([name]) => name !== "SeaORM");
const FORBIDDEN_ENTITY = FORBIDDEN_DOMAIN.filter(([name]) => name !== "SeaORM");
const FORBIDDEN_CORE = [
  ["SeaORM", /\bsea_orm(?:::|\s)/],
  ["SeaORM migrations", /\bsea_orm_migration(?:::|\s)/],
  ["Leptos", /\bleptos(?:_[a-z0-9]+)*(?:::|\s)/],
  ["Dioxus", /\bdioxus(?:_[a-z0-9]+)*(?:::|\s)/],
  ["web-sys", /\bweb_sys(?:::|\s)/],
  ["wasm-bindgen", /\bwasm_bindgen(?:::|\s)/],
  ["Axum", /\baxum(?:_[a-z0-9]+)*(?:::|\s)/],
  ["GraphQL transport", /\b(?:async_graphql|rustok_graphql)(?:_[a-z0-9]+)*(?:::|\s)/],
  ["rustok-web", /\brustok_web(?:_[a-z0-9]+)*(?:::|\s)/],
];

function maskRange(chars, start, end) {
  for (let index = start; index < end; index += 1) {
    if (chars[index] !== "\n" && chars[index] !== "\r") chars[index] = " ";
  }
}

/** Mask Rust comments and string literals before source-pattern checks. */
function maskRustNonCode(source) {
  const chars = source.split("");
  let index = 0;
  while (index < source.length) {
    const raw = /^(?:br|r)(#{0,255})"/.exec(source.slice(index));
    if (raw && (index === 0 || !/[A-Za-z0-9_]/.test(source[index - 1]))) {
      const terminator = '"' + raw[1];
      const close = source.indexOf(terminator, index + raw[0].length);
      const end = close < 0 ? source.length : close + terminator.length;
      maskRange(chars, index, end);
      index = end;
      continue;
    }
    if (source.startsWith("//", index)) {
      let end = source.indexOf("\n", index + 2);
      if (end < 0) end = source.length;
      maskRange(chars, index, end);
      index = end;
      continue;
    }
    if (source.startsWith("/*", index)) {
      let depth = 1;
      let end = index + 2;
      while (end < source.length && depth > 0) {
        if (source.startsWith("/*", end)) {
          depth += 1;
          end += 2;
        } else if (source.startsWith("*/", end)) {
          depth -= 1;
          end += 2;
        } else {
          end += 1;
        }
      }
      maskRange(chars, index, end);
      index = end;
      continue;
    }
    let quote = -1;
    if (source[index] === '"') quote = index;
    else if ((source[index] === "b" || source[index] === "c") &&
             source[index + 1] === '"') quote = index + 1;
    if (quote >= 0) {
      let end = quote + 1;
      let escaped = false;
      while (end < source.length) {
        const char = source[end];
        end += 1;
        if (escaped) escaped = false;
        else if (char === "\\") escaped = true;
        else if (char === '"') break;
      }
      maskRange(chars, index, end);
      index = end;
      continue;
    }
    index += 1;
  }
  return chars.join("");
}

function maskJavaScriptNonCode(source) {
  const chars = source.split("");
  let index = 0;
  while (index < source.length) {
    if (source.startsWith("//", index)) {
      let end = source.indexOf("\n", index + 2);
      if (end < 0) end = source.length;
      maskRange(chars, index, end);
      index = end;
      continue;
    }
    if (source.startsWith("/*", index)) {
      const close = source.indexOf("*/", index + 2);
      const end = close < 0 ? source.length : close + 2;
      maskRange(chars, index, end);
      index = end;
      continue;
    }
    const quote = source[index];
    if (quote === "'" || quote === '"') {
      let end = index + 1;
      let escaped = false;
      while (end < source.length) {
        const char = source[end];
        end += 1;
        if (escaped) escaped = false;
        else if (char === "\\") escaped = true;
        else if (char === quote) break;
      }
      index = end;
      continue;
    }
    if (quote === String.fromCharCode(96)) {
      const start = index;
      let end = index + 1;
      let escaped = false;
      while (end < source.length) {
        const char = source[end];
        end += 1;
        if (escaped) escaped = false;
        else if (char === "\\") escaped = true;
        else if (char === String.fromCharCode(96)) break;
      }
      maskRange(chars, start, end);
      index = end;
      continue;
    }
    index += 1;
  }
  return chars.join("");
}

function escapeRegExp(value) {
  const special = new Set([".", "+", "*", "?", "^", "$", "(", ")", "[", "]", "{", "}", "|", "\\"]);
  return Array.from(value, (character) =>
    special.has(character) ? "\\" + character : character,
  ).join("");
}

function hasStaticImport(source, specifier) {
  const code = maskJavaScriptNonCode(source);
  const escaped = escapeRegExp(specifier);
  const quote = "[\"']";
  const sideEffectImport = new RegExp(
    "(?:^|[;\\n])\\s*import\\s*" + quote + escaped + quote + "\\s*;?",
    "m",
  );
  const fromImport = new RegExp(
    "(?:^|[;\\n])\\s*import\\s+(?:type\\s+)?[\\s\\S]*?\\s+from\\s*" +
      quote + escaped + quote + "\\s*;?",
    "m",
  );
  return sideEffectImport.test(code) || fromImport.test(code);
}

function isDirectory(path) {
  try {
    const stat = lstatSync(path);
    return stat.isDirectory() && !stat.isSymbolicLink();
  } catch {
    return false;
  }
}

function checkModulePathNoSymlinks(root, moduleRoot, diagnostics) {
  let current = root;
  for (const segment of relative(root, moduleRoot).split(sep)) {
    if (!segment || segment === ".") continue;
    current = join(current, segment);
    try {
      if (lstatSync(current).isSymbolicLink()) {
        diagnostics.push(relative(root, current) + ": native module path cannot traverse symlinks");
        return false;
      }
    } catch {
      return true;
    }
  }
  return true;
}

function checkNoSymlinks(root, directory, diagnostics) {
  let stat;
  try {
    stat = lstatSync(directory);
  } catch {
    return;
  }
  if (stat.isSymbolicLink()) {
    diagnostics.push(relative(root, directory) + ": symlinks are forbidden in native source/package trees");
    return;
  }
  if (!stat.isDirectory()) return;
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isSymbolicLink()) {
      diagnostics.push(relative(root, path) + ": symlinks are forbidden in native source/package trees");
    } else if (entry.isDirectory()) {
      checkNoSymlinks(root, path, diagnostics);
    }
  }
}

function rustFiles(directory) {
  if (!isDirectory(directory)) return [];
  const result = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const file = join(directory, entry.name);
    if (entry.isDirectory()) result.push(...rustFiles(file));
    else if (entry.isFile() && entry.name.endsWith(".rs")) result.push(file);
  }
  return result;
}

function isTestSource(file) {
  const parts = file.split(/[\\/]/);
  const name = parts.at(-1);
  const isTestGated = (parentModule, moduleName) => {
    if (!hasSource(parentModule)) return false;
    const parentCode = maskRustNonCode(readFileSync(parentModule, "utf8"));
    return new RegExp("#\\s*\\[\\s*cfg\\s*\\(\\s*test\\s*\\)\\s*\\]\\s*(?:pub(?:\\([^)]*\\))?\\s+)?mod\\s+" +
      moduleName + "\\s*;").test(parentCode);
  };

  const moduleName = name.replace(/\.rs$/, "");
  if (/(?:^|_)tests?$/.test(moduleName) &&
      isTestGated(join(dirname(file), "mod.rs"), moduleName)) {
    return true;
  }

  const testDirectoryIndex = parts.findIndex((part) => part === "test" || part === "tests");
  if (testDirectoryIndex >= 0) {
    const parentDirectory = parts.slice(0, testDirectoryIndex).join(sep);
    const parentModule = parentDirectory.endsWith(sep + "src")
      ? join(parentDirectory, "lib.rs")
      : join(parentDirectory, "mod.rs");
    if (isTestGated(parentModule, parts[testDirectoryIndex])) return true;
  }
  return false;
}
function checkImports(root, directory, patterns, diagnostics) {
  for (const file of rustFiles(directory)) {
    // Test fixtures may need migration tooling or framework adapters to assemble
    // isolated databases; dependency direction applies to production source.
    if (isTestSource(file)) continue;
    const source = maskRustNonCode(readFileSync(file, "utf8"));
    for (const [label, regex] of patterns) {
      if (regex.test(source)) {
        diagnostics.push(relative(root, file) + ": forbidden " + label + " dependency in " +
          relative(root, directory) + " (static source match)");
      }
    }
  }
}

function checkSizes(root, directory, maximum, diagnostics) {
  for (const file of rustFiles(directory)) {
    const bytes = lstatSync(file).size;
    if (bytes > maximum) {
      diagnostics.push(relative(root, file) + ": " + bytes +
        " bytes exceeds canonical source limit of " + maximum);
    }
  }
}

function hasSource(path) {
  try {
    const stat = lstatSync(path);
    return stat.isFile() && !stat.isSymbolicLink();
  } catch {
    return false;
  }
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

function hasSection(source, section) {
  return source.split(/\r?\n/).some((line) => line.trim() === "[" + section + "]");
}

function findNextPackageByName(packagesRoot, expectedName) {
  const matches = [];
  if (!isDirectory(packagesRoot)) return matches;

  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (entry.isSymbolicLink()) continue;
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        if (["node_modules", ".next", "dist", "build"].includes(entry.name)) continue;
        visit(path);
      } else if (entry.isFile() && entry.name === "package.json") {
        try {
          const manifest = JSON.parse(readFileSync(path, "utf8"));
          if (manifest.name === expectedName) {
            matches.push({ root: directory, manifest });
          }
        } catch {
          // An unrelated malformed package is not evidence for this module.
        }
      }
    }
  };

  visit(packagesRoot);
  return matches;
}

function checkNextUiSurface(root, side, manifest, diagnostics) {
  const section = "provides." + side + "_ui";
  const packageName = keyInSection(manifest, section, "next_package")?.trim();
  if (!packageName) {
    diagnostics.push(section + ": next_package is required for live Next.js parity");
    return;
  }

  const routeSegment = keyInSection(manifest, section, "route_segment")?.trim();
  if (!routeSegment || !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(routeSegment)) {
    diagnostics.push(section + ": route_segment must be a safe kebab-case Next.js route");
    return;
  }

  const host = side === "admin" ? "next-admin" : "next-frontend";
  const packagesRoot = join(root, "apps", host, "packages");
  const matches = findNextPackageByName(packagesRoot, packageName);
  if (matches.length === 0) {
    diagnostics.push(relative(root, packagesRoot) + ": Next.js package '" + packageName + "' is missing");
    return;
  }
  if (matches.length > 1) {
    diagnostics.push(relative(root, packagesRoot) + ": Next.js package '" + packageName + "' is declared more than once");
    return;
  }

  const packageRoot = matches[0].root;
  const packageManifest = matches[0].manifest;
  if (!checkModulePathNoSymlinks(root, packageRoot, diagnostics)) return;
  checkNoSymlinks(root, packageRoot, diagnostics);

  const main = typeof packageManifest.main === "string" ? packageManifest.main.trim() : "";
  if (!main) {
    diagnostics.push(relative(root, join(packageRoot, "package.json")) + ": Next.js package must declare a main entrypoint");
  } else {
    const entry = resolve(packageRoot, main);
    if (entry !== packageRoot && !entry.startsWith(packageRoot + sep)) {
      diagnostics.push(relative(root, join(packageRoot, "package.json")) + ": Next.js package main entry escapes its package root");
    } else if (!hasSource(entry)) {
      diagnostics.push(relative(root, join(packageRoot, "package.json")) + ": Next.js package main entry is missing");
    }
  }

  const hostRegistryPath = join(root, "apps", host, "src", "modules", "index.ts");
  const hostRegistry = readRequired(root, hostRegistryPath, diagnostics);
  const relativePackageSource = relative(dirname(hostRegistryPath), join(packageRoot, "src"))
    .split(sep).join("/");
  if (!hasStaticImport(hostRegistry, packageName) &&
      !hasStaticImport(hostRegistry, relativePackageSource)) {
    diagnostics.push(relative(root, hostRegistryPath) +
      ": Next.js host module registry does not import declared package '" + packageName + "'");
  }

  const routePath = side === "admin"
    ? join(root, "apps", host, "src", "app", "dashboard", routeSegment, "page.tsx")
    : join(root, "apps", host, "src", "app", "[locale]", routeSegment, "page.tsx");
  const routeSource = readRequired(root, routePath, diagnostics);
  if (routeSource && !hasStaticImport(routeSource, packageName)) {
    diagnostics.push(relative(root, routePath) +
      ": Next.js route must import declared package '" + packageName + "'");
  }
}

function checkUi(root, moduleRoot, side, manifest, diagnostics) {
  const section = "provides." + (side === "admin" ? "admin_ui" : "storefront_ui");
  const requested = hasSection(manifest, section);
  const supportSection = "provides." + side + "_support";
  const supportPackage = hasSection(manifest, supportSection);
  const uiRoot = join(moduleRoot, side);
  const futureUi = keyInSection(manifest, "module", "ui_classification") === "future_ui";
  const draft = !requested && !supportPackage && futureUi && existsSync(uiRoot);
  if (!requested && !draft) {
    if (existsSync(uiRoot) && !supportPackage) {
      diagnostics.push(relative(root, uiRoot) +
        ": undeclared UI package requires explicit module.ui_classification = \"future_ui\" or a support declaration");
    }
    return;
  }
  checkNoSymlinks(root, uiRoot, diagnostics);
  if (!isDirectory(uiRoot)) {
    diagnostics.push(relative(root, uiRoot) + ": declared UI package directory is missing");
    return;
  }
  const name = moduleRoot.split(/[\\/]/).at(-1);
  const configured = requested
    ? keyInSection(manifest, section, "leptos_crate")
    : name + "-" + side;
  if (!configured) diagnostics.push(section + ": leptos_crate must identify the real package");
  const src = join(uiRoot, "src");
  const cargo = readRequired(root, join(uiRoot, "Cargo.toml"), diagnostics);
  const packageName = keyInSection(cargo, "package", "name");
  if (configured && packageName !== configured) {
    diagnostics.push(relative(root, uiRoot) + ": Cargo package name differs from expected Leptos package " + configured);
  }
  const lib = readRequired(root, join(src, "lib.rs"), diagnostics);
  const libCode = maskRustNonCode(lib);
  for (const requiredModule of ["core", "model", "i18n", "transport", "ui"]) {
    if (!new RegExp("\\b(?:pub(?:\\([^)]*\\))?\\s+)?mod\\s+" + requiredModule + "\\s*;").test(libCode)) {
      diagnostics.push(relative(root, join(src, "lib.rs")) +
        ": FFA lib.rs must declare required module " + requiredModule);
    }
  }
  if (!/\bpub\s+use\s+(?:crate::)?ui::/.test(libCode)) {
    diagnostics.push(relative(root, join(src, "lib.rs")) +
      ": FFA lib.rs must re-export the root UI component from ui/");
  }

  const modelPath = join(src, "model.rs");
  const model = readRequired(root, modelPath, diagnostics);
  const modelCode = maskRustNonCode(model);
  if (!modelCode.trim()) {
    diagnostics.push(relative(root, modelPath) +
      ": model.rs must contain a real transport-neutral model or owner-boundary type");
  }
  for (const [name, regex] of FORBIDDEN_CORE) {
    if (regex.test(modelCode)) {
      diagnostics.push(relative(root, modelPath) + ": forbidden " + name + " dependency in FFA model");
    }
  }
  const i18nPath = hasSource(join(src, "i18n.rs")) ? join(src, "i18n.rs") : join(src, "i18n", "mod.rs");
  const i18nSource = readRequired(root, i18nPath, diagnostics);
  if (!maskRustNonCode(i18nSource).trim()) {
    diagnostics.push(relative(root, i18nPath) + ": module-owned i18n entrypoint cannot be empty");
  }

  const core = hasSource(join(src, "core.rs")) ? join(src, "core.rs") : join(src, "core", "mod.rs");
  const coreSource = readRequired(root, core, diagnostics);
  if (!maskRustNonCode(coreSource).trim()) {
    diagnostics.push(relative(root, core) + ": FFA core cannot be an empty facade");
  }

  const transport = hasSource(join(src, "transport.rs")) ? join(src, "transport.rs") :
    join(src, "transport", "mod.rs");
  const transportSource = readRequired(root, transport, diagnostics);
  const transportCode = maskRustNonCode(transportSource);
  const adapterNames = ["native_server_adapter", "graphql_adapter"].filter((adapter) =>
    new RegExp("\\b(?:pub(?:\\([^)]*\\))?\\s+)?mod\\s+" + adapter + "\\s*;").test(transportCode),
  );
  if (draft && adapterNames.length === 0) {
    diagnostics.push(relative(root, transport) +
      ": an unmounted future_ui FFA draft must declare at least one native/GraphQL adapter boundary");
  }
  if (adapterNames.length === 1) {
    const planPath = join(moduleRoot, "docs", "implementation-plan.md");
    const plan = hasSource(planPath) ? readFileSync(planPath, "utf8") : "";
    const normalizedPlan = plan.replace(/\s+/g, " ");
    const documentedException =
      normalizedPlan.includes("single-adapter state, GraphQL, native parity plan pending") ||
      normalizedPlan.includes("single-adapter state, native-only internal operator/bootstrap surface, no GraphQL/REST contract yet, parity or exemption plan pending");
    if (!documentedException) {
      diagnostics.push(relative(root, planPath) +
        ": an FFA package with a single adapter must document the canonical single-adapter exception");
    }
  }
  const referencedAdapters = adapterNames.filter((adapter) =>
    new RegExp("\\b" + adapter + "\\s*::").test(transportCode),
  );
  const hasOwnerOperation = /\bpub(?:\([^)]*\))?\s+(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+[A-Za-z_][A-Za-z0-9_]*\s*\(/.test(transportCode) &&
    referencedAdapters.length > 0;
  if (transportSource && (!adapterNames.length || !hasOwnerOperation)) {
    diagnostics.push(relative(root, transport) +
      ": transport facade must expose an owner operation delegated to a declared native/GraphQL adapter");
  }
  if (transportSource && adapterNames.some((adapter) => !referencedAdapters.includes(adapter))) {
    diagnostics.push(relative(root, transport) +
      ": every declared native/GraphQL adapter must be reached by an owner operation in the transport facade");
  }

  const transportDir = join(src, "transport");
  const adapterRoot = hasSource(join(src, "transport.rs")) ? src : transportDir;
  for (const adapter of adapterNames) {
    const candidates = [
      join(adapterRoot, adapter + ".rs"),
      join(adapterRoot, adapter, "mod.rs"),
    ];
    const adapterPath = candidates.find(hasSource) || candidates[0];
    const adapterSource = readRequired(root, adapterPath, diagnostics);
    const adapterCode = maskRustNonCode(adapterSource);
    if (!/\b(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+[A-Za-z_][A-Za-z0-9_]*\s*\(/.test(adapterCode)) {
      diagnostics.push(relative(root, adapterPath) +
        ": declared transport adapter has no callable owner operation");
    }
    if (/\b(?:todo|unimplemented)!\s*\(/.test(adapterCode)) {
      diagnostics.push(relative(root, adapterPath) + ": transport adapter contains an unfinished todo!/unimplemented! stub");
    }
    if (requested && /\bErr\s*\(\s*(?:[A-Za-z_][A-Za-z0-9_]*::)*MissingOwnerContract\s*\)/.test(adapterCode)) {
      diagnostics.push(relative(root, adapterPath) +
        ": live UI transport adapter still returns the MissingOwnerContract scaffold placeholder");
    }
  }

  const uiModulePath = join(src, "ui", "mod.rs");
  const uiModule = readRequired(root, uiModulePath, diagnostics);
  const uiModuleCode = maskRustNonCode(uiModule);
  if (!/\b(?:pub(?:\([^)]*\))?\s+)?mod\s+leptos\s*;/.test(uiModuleCode)) {
    diagnostics.push(relative(root, uiModulePath) + ": ui/mod.rs must declare its Leptos render adapter");
  }
  const uiLeptos = hasSource(join(src, "ui", "leptos.rs"))
    ? join(src, "ui", "leptos.rs")
    : join(src, "ui", "leptos", "mod.rs");
  const uiLeptosSource = readRequired(root, uiLeptos, diagnostics);
  if (!/#\s*\[\s*component\s*\]/.test(maskRustNonCode(uiLeptosSource))) {
    diagnostics.push(relative(root, uiLeptos) + ": Leptos render adapter must define a component");
  }

  const localesRoot = join(uiRoot, "locales");
  if (!isDirectory(localesRoot)) {
    diagnostics.push(relative(root, localesRoot) + ": declared UI has no localization bundles");
  } else if (!readdirSync(localesRoot, { withFileTypes: true }).some((entry) =>
    entry.isFile() && /\.(?:ftl|json)$/i.test(entry.name),
  )) {
    diagnostics.push(relative(root, localesRoot) + ": localization directory has no FTL/JSON catalog");
  }
  const testsModulePath = join(src, "tests", "mod.rs");
  const testsModule = readRequired(root, testsModulePath, diagnostics);
  if (!/\bmod\s+contract\s*;/.test(maskRustNonCode(testsModule))) {
    diagnostics.push(relative(root, testsModulePath) + ": tests/mod.rs must declare contract.rs");
  }
  const uiContractPath = join(src, "tests", "contract.rs");
  if (!hasSource(uiContractPath)) {
    diagnostics.push(relative(root, uiContractPath) +
      ": UI contract tests must use the dedicated tests/ slot");
  } else if (!/#\s*\[\s*(?:tokio::)?test\s*\]/.test(maskRustNonCode(readFileSync(uiContractPath, "utf8")))) {
    diagnostics.push(relative(root, uiContractPath) + ": UI contract test file has no tests");
  }
  // Older UI crates may retain an auxiliary root contract_tests.rs while
  // migrating. The canonical tests/contract.rs above remains mandatory.
  if (!/#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\s+tests\s*;/.test(libCode)) {
    diagnostics.push(relative(root, join(src, "lib.rs")) +
      ": missing cfg(test) tests module declaration");
  }
  checkImports(root, join(src, "core"), FORBIDDEN_CORE, diagnostics);
  if (hasSource(join(src, "core.rs"))) {
    const coreSource = maskRustNonCode(readFileSync(core, "utf8"));
    for (const [name, regex] of FORBIDDEN_CORE) {
      if (regex.test(coreSource)) {
        diagnostics.push(relative(root, core) + ": forbidden " + name + " dependency in FFA core");
      }
    }
  }
  checkSizes(root, src, UI_LIMIT, diagnostics);
  for (const file of rustFiles(join(src, "ui"))) {
    const source = maskRustNonCode(readFileSync(file, "utf8"));
    if (/\b(?:graphql_adapter|native_server_adapter)\s*::|\brustok_graphql\s*::|#\s*\[\s*server\b/.test(source)) {
      diagnostics.push(relative(root, file) + ": UI must use the module transport facade, not raw adapters");
    }
    if (/\bsample_rows\b|(?:create|edit|delete) action triggered|record initiated/i.test(source)) {
      diagnostics.push(relative(root, file) + ": fake CRUD/sample content is not a functional UI");
    }
  }
  if (/#\s*\[\s*component\s*\]/.test(libCode)) {
    diagnostics.push(relative(root, join(src, "lib.rs")) + ": UI components belong in ui/, not lib.rs");
  }
  if (/\b(?:async\s+)?fn\s+[A-Za-z_][A-Za-z0-9_]*\s*\(|\b(?:pub\s+)?(?:struct|enum|trait|type|const|static)\b|\bimpl\b/.test(libCode)) {
    diagnostics.push(relative(root, join(src, "lib.rs")) + ": FFA lib.rs must remain a module/re-export facade");
  }
  if (requested) checkNextUiSurface(root, side, manifest, diagnostics);
}

function checkSupportPackage(root, moduleRoot, side, manifest, classification, diagnostics) {
  const section = "provides." + side + "_support";
  const uiSection = "provides." + side + "_ui";
  const supportRoot = join(moduleRoot, side);
  const allowed = classification === "future_ui" ||
    (classification === "admin_only" && side === "storefront") ||
    (classification === "storefront_only" && side === "admin");
  if (!allowed) {
    diagnostics.push(relative(root, supportRoot) +
      ": " + section + " is incompatible with module.ui_classification = " + classification);
  }
  if (hasSection(manifest, uiSection)) {
    diagnostics.push(relative(root, supportRoot) +
      ": a sub-crate cannot be both " + uiSection + " and " + section);
  }
  checkNoSymlinks(root, supportRoot, diagnostics);
  if (!isDirectory(supportRoot)) {
    diagnostics.push(relative(root, supportRoot) + ": declared support package directory is missing");
    return;
  }
  const configured = keyInSection(manifest, section, "leptos_crate");
  if (!configured) diagnostics.push(section + ": leptos_crate must identify the real support package");
  const cargo = readRequired(root, join(supportRoot, "Cargo.toml"), diagnostics);
  const packageName = keyInSection(cargo, "package", "name");
  if (configured && packageName !== configured) {
    diagnostics.push(relative(root, supportRoot) +
      ": Cargo package name differs from declared support package " + configured);
  }
  const lib = readRequired(root, join(supportRoot, "src", "lib.rs"), diagnostics);
  if (lib && !maskRustNonCode(lib).trim()) {
    diagnostics.push(relative(root, supportRoot) + ": support package facade has no implementation");
  }
  checkSizes(root, join(supportRoot, "src"), UI_LIMIT, diagnostics);
}

function nativeModuleRegistryEntry(root, slug) {
  const manifest = readFileSync(join(root, "modules.toml"), "utf8");
  const moduleHeader = manifest.match(/^\[modules\]\s*$/m);
  if (!moduleHeader) throw new Error("Missing [modules] section in modules.toml");
  const afterHeader = manifest.slice(moduleHeader.index + moduleHeader[0].length);
  const block = afterHeader.split(/\n\s*\[/)[0];
  for (const line of block.split("\n")) {
    const match = line.match(/^\s*([a-z][a-z0-9_]*)\s*=\s*\{(.+)\}\s*$/);
    if (match?.[1] !== slug) continue;
    const spec = match[2];
    return {
      crate: spec.match(/\bcrate\s*=\s*"([^"]+)"/)?.[1],
      source: spec.match(/\bsource\s*=\s*"([^"]+)"/)?.[1],
      path: spec.match(/\bpath\s*=\s*"([^"]+)"/)?.[1],
    };
  }
  throw new Error("Unknown native path module '" + slug + "' in modules.toml");
}

export function resolveNativeModule(root, slug) {
  const entry = nativeModuleRegistryEntry(root, slug);
  if (entry.source !== "path" || !entry.path) {
    throw new Error("Module '" + slug + "' is not a native local path module");
  }
  return entry.path;
}

export function lintNativeModule(root, modulePath, slug) {
  const diagnostics = [];
  const repo = resolve(root);
  const moduleRoot = resolve(repo, modulePath);
  if (!moduleRoot.startsWith(repo + sep)) {
    return ["Module '" + slug + "': path escapes the repository root"];
  }
  if (!checkModulePathNoSymlinks(repo, moduleRoot, diagnostics)) return diagnostics;
  const manifest = readRequired(repo, join(moduleRoot, "rustok-module.toml"), diagnostics);
  const cargo = readRequired(repo, join(moduleRoot, "Cargo.toml"), diagnostics);
  if (!cargo.includes("[package]")) {
    diagnostics.push(relative(repo, moduleRoot) + ": missing Cargo package contract");
  }
  const manifestSlug = keyInSection(manifest, "module", "slug");
  if (manifest && manifestSlug !== slug) {
    diagnostics.push(relative(repo, moduleRoot) + ": module.slug does not match '" + slug + "'");
  }
  if (!/^[a-z][a-z0-9]*(?:_[a-z0-9]+)*$/.test(slug)) {
    diagnostics.push(relative(repo, moduleRoot) + ": module slug must use canonical snake_case");
  }
  const packageName = keyInSection(cargo, "package", "name");
  let registryEntry;
  try {
    registryEntry = nativeModuleRegistryEntry(repo, slug);
  } catch (error) {
    diagnostics.push(relative(repo, join(repo, "modules.toml")) + ": " + error.message);
  }
  if (registryEntry) {
    if (registryEntry.source !== "path" || !registryEntry.path) {
      diagnostics.push(relative(repo, moduleRoot) + ": module registry entry is not a native local path module");
    } else if (resolve(repo, registryEntry.path) !== moduleRoot) {
      diagnostics.push(relative(repo, moduleRoot) + ": module path does not match modules.toml");
    }
    if (!registryEntry.crate || packageName !== registryEntry.crate) {
      diagnostics.push(relative(repo, moduleRoot) +
        ": Cargo package.name must match modules.toml crate identity");
    }
  }
  const entryType = keyInSection(manifest, "crate", "entry_type");
  if (!entryType || !/^[A-Za-z][A-Za-z0-9_]*$/.test(entryType)) {
    diagnostics.push(relative(repo, moduleRoot) + ": missing valid native [crate].entry_type");
  }
  const src = join(moduleRoot, "src");
  checkNoSymlinks(repo, src, diagnostics);
  if (!isDirectory(src)) {
    diagnostics.push(relative(repo, src) + ": native source root is missing or is not a directory");
    return diagnostics;
  }

  const lib = readRequired(repo, join(src, "lib.rs"), diagnostics);
  const module = readRequired(repo, join(src, "module.rs"), diagnostics);
  const libCode = maskRustNonCode(lib);
  const moduleCode = maskRustNonCode(module);
  if (/\b(?:async\s+)?fn\s+[A-Za-z_][A-Za-z0-9_]*\s*\(|\b(?:pub\s+)?(?:struct|enum|trait|type|const|static)\b|\bimpl\b/.test(libCode)) {
    diagnostics.push(relative(repo, join(src, "lib.rs")) + ": lib.rs must remain a thin module/re-export facade");
  }
  if (lib && !/\b(?:pub(?:\([^)]*\))?\s+)?mod\s+module\s*;/.test(libCode)) {
    diagnostics.push(relative(repo, join(src, "lib.rs")) + ": module.rs is not declared");
  }
  if (entryType && lib && !new RegExp("\\bpub\\s+use\\s+module\\s*::\\s*(?:\\{[^}]*\\b)?" +
    entryType + "\\b").test(libCode)) {
    diagnostics.push(relative(repo, join(src, "lib.rs")) + ": native entry type is not re-exported");
  }
  if (entryType && module && !new RegExp("\\bimpl\\s+RusToKModule\\s+for\\s+" + entryType + "\\b").test(moduleCode)) {
    diagnostics.push(relative(repo, join(src, "module.rs")) + ": missing native RusToKModule implementation");
  }
  if (/\bimpl\s+(?:RusToKModule|MigrationSource)\b|register_runtime_extensions\s*\(|register_event_listeners\s*\(/.test(libCode)) {
    diagnostics.push(relative(repo, join(src, "lib.rs")) + ": runtime registration belongs in module.rs");
  }
  if (/\bpub\s+(?!\(\s*(?:crate|super)\s*\)\s*)(?:mod|use)\s+entities\b|\bpub\s+use\s+crate\s*::\s*entities\b/.test(libCode)) {
    diagnostics.push(relative(repo, join(src, "lib.rs")) + ": entities must remain private");
  }
  const entities = join(src, "entities");
  if (isDirectory(entities) && !/\b(?:pub\s*\(\s*crate\s*\)\s+)?mod\s+entities\s*;/.test(libCode)) {
    diagnostics.push(relative(repo, entities) + ": persistence entities must be declared crate-private");
  }
  if (isDirectory(join(src, "migrations")) && !/\bimpl\s+MigrationSource\s+for\b/.test(moduleCode)) {
    diagnostics.push(relative(repo, join(src, "module.rs")) + ": migration source must be wired in module.rs");
  }
  if (hasSource(join(src, "runtime.rs")) && !/\bmod\s+runtime\s*;/.test(libCode)) {
    diagnostics.push(relative(repo, join(src, "runtime.rs")) + ": optional runtime.rs must be declared by lib.rs");
  }
  if (hasSource(join(src, "error.rs")) && isDirectory(join(src, "error"))) {
    diagnostics.push(relative(repo, join(src, "error.rs")) + ": use either error.rs or error/, not both");
  }
  for (const rootModule of ["runtime", "error"]) {
    if (hasSource(join(src, rootModule + ".rs")) &&
        !new RegExp("\\bmod\\s+" + rootModule + "\\s*;").test(libCode)) {
      diagnostics.push(relative(repo, join(src, rootModule + ".rs")) + ": " + rootModule + ".rs must be declared by lib.rs");
    }
  }
  const testsModulePath = join(src, "tests", "mod.rs");
  const testsModule = readRequired(repo, testsModulePath, diagnostics);
  if (!/\bmod\s+contract\s*;/.test(maskRustNonCode(testsModule))) {
    diagnostics.push(relative(repo, testsModulePath) + ": src/tests/mod.rs must declare contract.rs");
  }
  const contractPath = join(src, "tests", "contract.rs");
  if (!hasSource(contractPath)) {
    diagnostics.push(relative(repo, contractPath) +
      ": native contract tests belong in src/tests/contract.rs");
  } else if (!/#\s*\[\s*(?:tokio::)?test\s*\]/.test(maskRustNonCode(readFileSync(contractPath, "utf8")))) {
    diagnostics.push(relative(repo, contractPath) + ": contract test file has no tests");
  }
  if (!/#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\s+tests\s*;/.test(libCode)) {
    diagnostics.push(relative(repo, join(src, "lib.rs")) + ": missing cfg(test) tests module");
  }

  for (const entry of readdirSync(src, { withFileTypes: true })) {
    if (entry.isSymbolicLink()) continue;
    if (entry.isFile() && entry.name.endsWith(".rs") && !ALLOWED_ROOT_RUST_FILES.has(entry.name)) {
      diagnostics.push(relative(repo, join(src, entry.name)) +
        ": noncanonical src/ root Rust file; move into its responsibility slot");
    } else if (entry.isDirectory() && !BACKEND_SLOTS.has(entry.name)) {
      diagnostics.push(relative(repo, join(src, entry.name)) +
        ": unrecognized native responsibility directory");
    }
  }

  for (const slot of BACKEND_SLOTS) {
    const dir = join(src, slot);
    if (!isDirectory(dir)) continue;
    if (!hasSource(join(dir, "mod.rs"))) {
      diagnostics.push(relative(repo, dir) + ": responsibility directory requires mod.rs");
    }
    if (!new RegExp("\\b(?:pub(?:\\([^)]*\\))?\\s+)?mod\\s+" + slot + "\\s*;").test(libCode)) {
      diagnostics.push(relative(repo, dir) + ": responsibility directory is not declared by lib.rs");
    }
    const files = rustFiles(dir);
    const payload = files.map((file) => maskRustNonCode(readFileSync(file, "utf8")).trim()).join("");
    if (!files.length || !payload) {
      diagnostics.push(relative(repo, dir) +
        ": empty responsibility slot is forbidden; remove or implement it");
    }
  }

  checkImports(repo, join(src, "domain"), FORBIDDEN_DOMAIN, diagnostics);
  checkImports(repo, join(src, "dto"), FORBIDDEN_DOMAIN, diagnostics);
  checkImports(repo, join(src, "ports"), FORBIDDEN_DOMAIN, diagnostics);
  checkImports(repo, join(src, "entities"), FORBIDDEN_ENTITY, diagnostics);
  checkImports(repo, join(src, "services"), FORBIDDEN_SERVICE, diagnostics);
  checkSizes(repo, src, BACKEND_LIMIT, diagnostics);

  const classification = keyInSection(manifest, "module", "ui_classification");
  const liveProfiles = {
    no_ui: [],
    capability_only: [],
    future_ui: [],
    admin_only: ["admin"],
    storefront_only: ["storefront"],
    dual_surface: ["admin", "storefront"],
  };
  if (!Object.hasOwn(liveProfiles, classification)) {
    diagnostics.push(relative(repo, moduleRoot) + ": unknown module.ui_classification");
  } else {
    for (const side of ["admin", "storefront"]) {
      const uiSection = "provides." + side + "_ui";
      const supportSection = "provides." + side + "_support";
      const uiDeclared = hasSection(manifest, uiSection);
      const supportDeclared = hasSection(manifest, supportSection);
      const packageExists = existsSync(join(moduleRoot, side));
      const live = liveProfiles[classification].includes(side);
      if (live && !packageExists) {
        diagnostics.push(relative(repo, moduleRoot) + ": declared " + side + " UI package is missing");
      }
      if (live && !uiDeclared) {
        diagnostics.push(relative(repo, moduleRoot) + ": live " + side + " UI requires [" + uiSection + "]");
      }
      if (!live && uiDeclared) {
        diagnostics.push(relative(repo, moduleRoot) + ": [" + uiSection + "] contradicts " + classification);
      }
      if (packageExists && !live && classification !== "future_ui" && !supportDeclared) {
        diagnostics.push(relative(repo, moduleRoot) + ": " + classification + " forbids unadvertised " + side + " UI package");
      }
      if (supportDeclared) {
        checkSupportPackage(repo, moduleRoot, side, manifest, classification, diagnostics);
      } else {
        checkUi(repo, moduleRoot, side, manifest, diagnostics);
      }
    }
  }
  return diagnostics.sort();
}
