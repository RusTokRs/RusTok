#!/usr/bin/env node
// Source and Cargo-metadata checks for the one Auth-owned client boundary.
// These are structural checks; they do not prove browser or network behavior.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const owner = "crates/modules/rustok-auth/ui";
const requiredFiles = [
  `${owner}/src/lib.rs`,
  `${owner}/src/core/mod.rs`,
  `${owner}/src/core/cookie.rs`,
  `${owner}/src/model.rs`,
  `${owner}/src/model/scope.rs`,
  `${owner}/src/transport/mod.rs`,
  `${owner}/src/transport/graphql_adapter.rs`,
  `${owner}/src/transport/native_server_adapter.rs`,
  `${owner}/src/transport/cookie_adapter.rs`,
  `${owner}/src/ui/leptos/context.rs`,
  `${owner}/src/ui/leptos/hooks.rs`,
  `${owner}/src/ui/leptos/storage.rs`,
];

export function validateMetadata(metadata) {
  const failures = [];
  const packages = metadata.packages ?? [];
  const owners = packages.filter((pkg) => pkg.name === "rustok-auth-ui");
  if (owners.length !== 1) {
    failures.push("workspace must have exactly one rustok-auth-ui package");
  } else {
    const ui = owners[0];
    if (!ui.manifest_path.replaceAll("\\", "/").endsWith(`/${owner}/Cargo.toml`)) {
      failures.push("Auth UI package must live inside the Auth module");
    }
    const forbidden = new Set(["rustok-auth", "rustok-auth-admin", "rustok-admin", "rustok-storefront"]);
    for (const dep of ui.dependencies ?? []) {
      if (forbidden.has(dep.name)) {
        failures.push(`Auth UI must not directly depend on server or host package ${dep.name}`);
      }
    }
    if (!(ui.dependencies ?? []).some((dep) => dep.name === "rustok-ui-auth")) {
      failures.push("Auth UI must use the shared framework-neutral session models");
    }
    for (const mode of ["csr", "hydrate", "ssr"]) {
      if (!(ui.features?.[mode] ?? []).includes(`leptos/${mode}`)) {
        failures.push(`Auth UI ${mode} must enable the matching Leptos feature`);
      }
    }
  }
  for (const pkg of packages) {
    if (pkg.name === "leptos-auth" || (pkg.dependencies ?? []).some((dep) => dep.name === "leptos-auth")) {
      failures.push(`${pkg.name}: retired Auth package must not be a workspace package or dependency`);
    }
    if (!(pkg.dependencies ?? []).some((dep) => dep.name === "rustok-auth-ui")) continue;
    for (const mode of ["csr", "hydrate", "ssr"]) {
      const features = pkg.features?.[mode] ?? [];
      if (features.includes(`leptos/${mode}`) && !features.includes(`rustok-auth-ui/${mode}`)) {
        failures.push(`${pkg.name}: ${mode} must propagate to rustok-auth-ui`);
      }
    }
  }
  for (const name of ["rustok-admin", "rustok-storefront", "rustok-auth-admin"]) {
    const pkg = packages.find((item) => item.name === name);
    if (!pkg || !(pkg.dependencies ?? []).some((dep) => dep.name === "rustok-auth-ui")) {
      failures.push(`${name}: missing direct dependency on the shared Auth UI boundary`);
    }
  }
  return failures;
}

export function validateSources(files) {
  const failures = [];
  const text = new Map(files.map(({ name, content }) => [name, content]));
  for (const name of requiredFiles) {
    if (!text.get(name)?.trim()) failures.push(`${name}: missing or empty Auth FFA layer`);
  }
  if (text.has("crates/ui/leptos-auth/Cargo.toml") ||
      [...text.keys()].some((name) => name.startsWith("crates/ui/leptos-auth/src/"))) {
    failures.push("retired Auth directory must not contain a Cargo package or implementation");
  }
  for (const { name, content } of files) {
    if (name.endsWith(".rs") && /\bleptos_auth\b/.test(content)) {
      failures.push(`${name}: retired Rust namespace remains`);
    }
    if (name.endsWith(".rs") && /\brustok_auth_ui::(?!(?:core|model|transport|ui)::)/.test(content)) {
      failures.push(`${name}: use the explicit Auth FFA layer, not a flat compatibility facade`);
    }
    if (name.endsWith("Cargo.toml") && /(?:^|\n)\s*leptos-auth\s*[.=]|["']leptos-auth\//.test(content)) {
      failures.push(`${name}: retired dependency or feature remains`);
    }
    if (name.endsWith("Cargo.lock") && /(?:name = )?["']leptos-auth["']/.test(content)) {
      failures.push(`${name}: retired Auth package or dependency remains`);
    }
    if ((name.startsWith(`${owner}/src/core/`) || name === `${owner}/src/model.rs` ||
         name.startsWith(`${owner}/src/model/`)) && /\bleptos::|\buse\s+leptos\b/.test(content)) {
      failures.push(`${name}: framework dependency leaked into Auth core/model`);
    }
    if (name.endsWith(".rs") && /\bpub\s+fn\s+AuthProvider\s*\(/.test(content) &&
        name !== `${owner}/src/ui/leptos/context.rs`) {
      failures.push(`${name}: duplicate Auth provider outside the module owner`);
    }
  }
  const lib = text.get(`${owner}/src/lib.rs`) ?? "";
  for (const layer of ["core", "model", "transport", "ui"]) {
    if (!lib.includes(`pub mod ${layer};`)) failures.push(`Auth UI must expose its ${layer} layer`);
  }
  const context = text.get(`${owner}/src/ui/leptos/context.rs`) ?? "";
  if (!/\bpub\s+fn\s+AuthProvider\s*\(/.test(context)) failures.push("Auth-owned provider is missing");
  // The standalone CSR host must provide the same owner context to both route branches.
  // This does not assert anything about the separate SSR render path or mounting behavior.
  const storefront = text.get("apps/storefront/src/main.rs") ?? "";
  const csrStart = storefront.indexOf('#[cfg(feature = "csr")]\nfn main()');
  const csrEnd = storefront.indexOf('\n#[cfg(', csrStart + 1);
  const csr = csrStart < 0 ? "" : storefront.slice(csrStart, csrEnd < 0 ? undefined : csrEnd);
  const open = csr.indexOf("<AuthProvider>");
  const close = csr.indexOf("</AuthProvider>");
  const page = csr.indexOf("<StorefrontModulePage");
  const shell = csr.indexOf("<StorefrontShell");
  if (!csr.includes("use rustok_auth_ui::ui::leptos::AuthProvider;") ||
      open < 0 || page < open || shell < open || close < page || close < shell) {
    failures.push("storefront CSR route branches must be composed under the Auth-owned provider");
  }
  const adapter = text.get(`${owner}/src/transport/native_server_adapter.rs`) ?? "";
  for (const endpoint of ["sign-in", "sign-up", "sign-out", "refresh-token", "accept-invite", "forgot-password", "current-user"]) {
    if (!adapter.includes(`endpoint = "auth/${endpoint}"`)) failures.push(`Auth native endpoint ${endpoint} is missing`);
  }
  return failures;
}

function main() {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
  const metadata = JSON.parse(execFileSync("cargo", ["metadata", "--offline", "--locked", "--no-deps", "--format-version", "1"], { cwd: root, encoding: "utf8", maxBuffer: 128 * 1024 * 1024 }));
  const names = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"], { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 }).split("\0").filter(Boolean);
  const files = [...new Set(names)]
    .filter((name) => name.endsWith(".rs") || name.endsWith("Cargo.toml") || name.endsWith("Cargo.lock"))
    .map((name) => {
      try { return { name, content: readFileSync(path.join(root, name), "utf8") }; }
      catch (error) { if (error.code === "ENOENT") return null; throw error; }
    }).filter(Boolean);
  const failures = [...validateMetadata(metadata), ...validateSources(files)];
  if (failures.length) {
    for (const failure of failures) console.error(`FAIL: ${failure}`);
    process.exitCode = 1;
  } else {
    console.log("Auth UI owner, FFA layers, shared provider, features and legacy removal: OK (source/metadata only)");
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main();
