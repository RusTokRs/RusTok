import test from "node:test";
import assert from "node:assert/strict";
import { validateMetadata, validateSources } from "./verify-auth-ui-boundary.mjs";

const owner = "crates/modules/rustok-auth/ui";
const modes = ["csr", "hydrate", "ssr"];
const modeFeatures = (prefix) => Object.fromEntries(modes.map((mode) => [mode, [`leptos/${mode}`, `${prefix}/${mode}`]]));

function metadataFixture() {
  return {
    packages: [
      {
        name: "rustok-auth-ui",
        manifest_path: `/repo/${owner}/Cargo.toml`,
        features: Object.fromEntries(modes.map((mode) => [mode, [`leptos/${mode}`]])),
        dependencies: [{ name: "rustok-ui-auth" }, { name: "leptos" }],
      },
      ...["rustok-admin", "rustok-storefront", "rustok-auth-admin"].map((name) => ({
        name,
        dependencies: [{ name: "rustok-auth-ui" }],
        features: modeFeatures("rustok-auth-ui"),
      })),
    ],
  };
}

function sourcesFixture() {
  const filenames = [
    "src/lib.rs", "src/core/mod.rs", "src/core/cookie.rs", "src/model.rs", "src/model/scope.rs",
    "src/transport/mod.rs", "src/transport/graphql_adapter.rs", "src/transport/native_server_adapter.rs",
    "src/transport/cookie_adapter.rs", "src/ui/leptos/context.rs", "src/ui/leptos/hooks.rs", "src/ui/leptos/storage.rs",
  ];
  const files = filenames.map((name) => ({ name: `${owner}/${name}`, content: "// present" }));
  files.find((file) => file.name.endsWith("/src/lib.rs")).content =
    "pub mod core; pub mod model; pub mod transport; pub mod ui;";
  files.find((file) => file.name.endsWith("/src/ui/leptos/context.rs")).content = "pub fn AuthProvider() {}";
  files.push({
    name: "apps/storefront/src/main.rs",
    content: '#[cfg(feature = "csr")]\nfn main() { use rustok_auth_ui::ui::leptos::AuthProvider; <AuthProvider><StorefrontModulePage /><StorefrontShell /></AuthProvider> }\n#[cfg(feature = "ssr")]\nfn other() {}',
  });
  files.find((file) => file.name.endsWith("/src/transport/native_server_adapter.rs")).content =
    ["sign-in", "sign-up", "sign-out", "refresh-token", "accept-invite", "forgot-password", "current-user"]
      .map((endpoint) => `endpoint = "auth/${endpoint}"`).join("\n");
  return files;
}

test("accepts one owner with shared model and propagated modes", () => {
  assert.deepEqual(validateMetadata(metadataFixture()), []);
  assert.deepEqual(validateSources(sourcesFixture()), []);
});

test("rejects backend or host dependencies and old package references", () => {
  const metadata = metadataFixture();
  metadata.packages[0].dependencies.push({ name: "rustok-auth" });
  metadata.packages[1].dependencies.push({ name: "leptos-auth" });
  const failures = validateMetadata(metadata).join("\n");
  assert.match(failures, /server or host package rustok-auth/);
  assert.match(failures, /retired Auth package/);
});

test("rejects missing propagation separately for CSR, hydrate and SSR", () => {
  for (const mode of modes) {
    const metadata = metadataFixture();
    metadata.packages[2].features[mode] = [`leptos/${mode}`];
    assert.match(validateMetadata(metadata).join("\n"), new RegExp(`${mode} must propagate`));
  }
});

test("rejects duplicate providers, framework leakage and old executable code", () => {
  const files = sourcesFixture();
  files.find((file) => file.name.endsWith("/src/core/mod.rs")).content = "use leptos::prelude::*;";
  files.push({ name: "apps/admin/src/old.rs", content: "use leptos_auth::AuthContext; pub fn AuthProvider() {}" });
  files.push({ name: "apps/storefront/src/flat.rs", content: "use rustok_auth_ui::AuthContext;" });
  files.push({ name: "crates/ui/leptos-auth/Cargo.toml", content: "[package]" });
  files.push({ name: "other/Cargo.lock", content: 'name = "leptos-auth"' });
  const failures = validateSources(files).join("\n");
  assert.match(failures, /framework dependency leaked/);
  assert.match(failures, /duplicate Auth provider/);
  assert.match(failures, /retired Rust namespace/);
  assert.match(failures, /explicit Auth FFA layer/);
  assert.match(failures, /retired Auth package or dependency remains/);
  assert.match(failures, /must not contain a Cargo package/);
});

test("rejects a removed native endpoint and missing FFA layer", () => {
  const files = sourcesFixture().filter((file) => !file.name.endsWith("/src/model/scope.rs"));
  files.find((file) => file.name.endsWith("/src/transport/native_server_adapter.rs")).content = "// absent";
  const failures = validateSources(files).join("\n");
  assert.match(failures, /missing or empty Auth FFA layer/);
  assert.match(failures, /Auth native endpoint sign-out is missing/);
});

test("rejects a CSR route rendered outside the Auth provider", () => {
  const files = sourcesFixture();
  const host = files.find((file) => file.name === "apps/storefront/src/main.rs");
  host.content = host.content.replace(
    "<StorefrontShell /></AuthProvider>",
    "</AuthProvider><StorefrontShell />",
  );
  assert.match(validateSources(files).join("\n"), /storefront CSR route branches/);
});
