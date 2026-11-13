#!/usr/bin/env node
// The Pages workspace is the first surface a standalone SSR host is expected to show, so its
// server state has to be loadable during the render.
//
// `LocalResource` cannot do that: under `ssr` it is `pending().await` by construction
// (`leptos_server/src/local_resource.rs`), so a response that reads one contains the `Suspense`
// fallback and never the workspace. This gate holds the surfaces that have been moved to an
// awaitable `Resource` — listed here as they move — and reports any that fell back.
//
// The list is the ratchet: adding a surface to it is the act of moving it, and this script reports
// which listed surfaces are not done yet rather than failing on the whole migration.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

const read = (relativePath) => readFileSync(join(root, relativePath), "utf8");

// A function body, from its `fn` line to the matching closing brace at the same indentation.
function functionBody(source, name) {
  const start = source.search(new RegExp(`fn\\s+${name}\\s*\\(`));
  if (start === -1) return null;
  const open = source.indexOf("{", start);
  if (open === -1) return null;
  const indent = source.slice(source.lastIndexOf("\n", start) + 1).match(/^\s*/)[0];
  let depth = 0;
  for (let i = open; i < source.length; i += 1) {
    if (source[i] === "{") depth += 1;
    if (source[i] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(start, i + 1);
    }
  }
  return null;
}

const failures = [];
const pending = [];

// Surfaces that must load their server state through an awaitable resource. Each entry is the
// source file, the component that renders the surface, and what it must not create any more.
const movedSurfaces = [
  {
    file: "crates/modules/rustok-pages/admin/src/composition.rs",
    component: "PagesAdmin",
    resource: "Resource::new(",
    forbidden: "LocalResource::new(",
    description: "the Pages workspace loads its page list and open page during the render",
  },
];

for (const surface of movedSurfaces) {
  const source = read(surface.file);
  const body = functionBody(source, surface.component);
  if (!body) {
    failures.push(`${surface.file}: ${surface.component} is not defined`);
    continue;
  }
  if (!body.includes(surface.resource)) {
    failures.push(
      `${surface.file}: ${surface.component} does not create a \`${surface.resource.slice(0, -1)}\`, ` +
        `so ${surface.description} cannot hold`,
    );
  }
  if (body.includes(surface.forbidden)) {
    failures.push(
      `${surface.file}: ${surface.component} still creates a \`${surface.forbidden.slice(0, -1)}\`, ` +
        "which is `pending().await` under `ssr` and renders the Suspense fallback instead of the " +
        "surface. A host that must show this surface cannot show it. See " +
        "DECISIONS/2026-11-13-standalone-admin-editor-delivery.md",
    );
  }
}

// The rest of the workspace is not yet moved. Report it, do not fail it: the ADR moves surfaces one
// at a time, and a gate that failed on the whole backlog would be turned off instead of worked
// through.
const notYetMoved = [
  "crates/modules/rustok-pages/admin/src/inline_edit_launch.rs",
  "crates/modules/rustok-pages/admin/src/rollback_control.rs",
  "crates/modules/rustok-pages/admin/src/standalone_metadata.rs",
  "crates/modules/rustok-pages/admin/src/templates_admin.rs",
  "crates/modules/rustok-page-builder/admin/src/editor/consumer_properties.rs",
];
for (const file of notYetMoved) {
  const source = read(file);
  if (source.includes("LocalResource::new(")) {
    pending.push(file);
  } else {
    failures.push(
      `${file} no longer creates a LocalResource but is still listed as not yet moved in ` +
        `${import.meta.url.split("/").slice(-1)[0]}; move it to the moved surfaces list so the ` +
        "gate keeps holding it",
    );
  }
}
// Within the moved surface, the remaining `LocalResource`s are the tabs and panels it renders.
for (const name of ["template_keys", "history", "schedule"]) {
  const source = read("crates/modules/rustok-pages/admin/src/composition.rs");
  if (source.includes(`let ${name} = LocalResource::new(`)) {
    pending.push(`crates/modules/rustok-pages/admin/src/composition.rs (${name})`);
  }
}

if (failures.length > 0) {
  console.error("Standalone admin editor delivery verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `Standalone admin editor delivery verified: ${movedSurfaces.length} surface(s) load their state ` +
    `during the render; ${pending.length} remain on LocalResource and are not shown by an SSR host.`,
);
for (const entry of pending) console.log(`  pending: ${entry}`);
