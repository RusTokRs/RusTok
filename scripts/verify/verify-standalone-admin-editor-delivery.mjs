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
  {
    file: "crates/modules/rustok-pages/admin/src/inline_edit_launch.rs",
    component: "PagesInlineEditLaunch",
    resource: "Resource::new(",
    forbidden: "LocalResource::new(",
    description: "the inline edit launch resolves the page it links to during the render",
  },
  {
    file: "crates/modules/rustok-pages/admin/src/rollback_control.rs",
    component: "PagesRollbackControl",
    resource: "Resource::new(",
    forbidden: "LocalResource::new(",
    description: "the rollback control resolves the published page during the render",
  },
  {
    file: "crates/modules/rustok-pages/admin/src/standalone_metadata.rs",
    component: "PagesPublishedMetadataSurface",
    resource: "Resource::new(",
    forbidden: "LocalResource::new(",
    description: "published metadata is admitted on a page status the render resolves",
  },
  {
    file: "crates/modules/rustok-pages/admin/src/templates_admin.rs",
    component: "TemplatesAdmin",
    resource: "Resource::new(",
    forbidden: "LocalResource::new(",
    description: "both layout template catalogs load during the render",
  },
  {
    file: "crates/modules/rustok-page-builder/admin/src/editor/consumer_properties.rs",
    component: "ConsumerPropertiesPanel",
    resource: "Resource::new(",
    forbidden: "LocalResource::new(",
    description: "the consumer properties snapshot is resolved by the request",
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

// Module availability is the decision every module surface branches on. A `LocalResource` fed into
// an `Effect` never runs before the response, so the decision it produces is "no modules", which is
// the failure that hid every standalone editor. The request has to resolve it.
const providerFile = "apps/admin/src/shared/context/enabled_modules.rs";
const providerSource = read(providerFile);
if (!providerSource.includes("Resource::new(")) {
  failures.push(
    `${providerFile}: module availability is not resolved by a \`Resource\`, so the decision is ` +
      "not part of the response and every module surface is gated on an empty set.",
  );
}
if (/LocalResource::new\(/.test(providerSource)) {
  failures.push(
    `${providerFile}: module availability is loaded through a \`LocalResource\`, which is ` +
      "`pending()` on the server; the decision must come from the request.",
  );
}
// The decision is derived, not written. A cell (`RwSignal`) needs a writer, and the only writer the
// component has is an `Effect`, which is a task for a later tick that the server does not run.
for (const field of ["modules", "is_loading", "error"]) {
  if (!new RegExp(`pub ${field}: Memo<`).test(providerSource)) {
    failures.push(
      `${providerFile}: \`${field}\` is not a \`Memo\` over the resolved decision, so something ` +
        "has to write it, and a write from an `Effect` never reaches the response.",
    );
  }
}
if (/Effect::new\(/.test(providerSource)) {
  failures.push(
    `${providerFile}: the decision still depends on an \`Effect\`, which is not driven before the ` +
      "response is produced; derive it from the resource instead.",
  );
}

// A `Resource` alone is not enough: under the default out-of-order streaming mode a suspended
// fragment is written as the `Suspense` fallback plus a `<template>` that a script swaps in, and the
// admin shell serves no script. `SsrMode::Async` is the mode that waits for the resources of the
// route and renders one complete document, so every route that renders a moved surface has to
// declare it.
// The route that resolves module availability is the layout, not the module leaves: the provider
// lives in `AppLayout`, which wraps every admin surface, and the sidebar renders the module
// navigation from it on all of them. Declaring the mode on a leaf would leave the sidebar streaming
// a fallback on every other route, so the assertion is on the route that owns the resource.
//
// Leptos derives a listing's mode from the route definition and promotes it to the most restrictive
// mode among its ancestors, so a child cannot lower what the layout declares.
const routerSource = read("apps/admin/src/app/router.rs");
const layoutRoutes = [
  ...routerSource.matchAll(/<ParentRoute[\s\S]*?view=AppLayout[\s\S]*?>/g),
];
if (layoutRoutes.length !== 1) {
  failures.push(
    `apps/admin/src/app/router.rs: expected exactly one route rendering \`AppLayout\`, found ` +
      `${layoutRoutes.length}; the module-availability provider is mounted there, so its SSR mode ` +
      "has to be checkable at exactly one place.",
  );
}
for (const match of layoutRoutes) {
  if (!match[0].includes("ssr=SsrMode::Async")) {
    failures.push(
      "apps/admin/src/app/router.rs: the route rendering `AppLayout` does not declare " +
        "`ssr=SsrMode::Async`. `AppLayout` mounts the module-availability provider and renders the " +
        "sidebar from it, so under the default mode every admin page streams that `Suspense` as a " +
        "fallback plus a `<template>` this shell never runs.",
    );
  }
}

// The layout route sets `Async`, which covers every page it wraps. Inside such a render there is no
// streaming left to fill a `Suspense` later, and a `LocalResource` is `pending()` on the server, so
// any surface that still uses one arrives empty even though the route was made to wait. The
// installer is the only page outside the layout, and it declares no mode of its own.
const adminSources = [
  "apps/admin/src/pages/dashboard.rs",
  "apps/admin/src/pages/modules.rs",
  "apps/admin/src/pages/email_settings.rs",
  "apps/admin/src/app/modules/search_composition.rs",
  "apps/admin/src/app/page_builder_contributions.rs",
];
for (const file of adminSources) {
  if (/LocalResource::new\(/.test(read(file))) {
    failures.push(
      `${file}: creates a \`LocalResource\` inside the \`Async\` admin layout, where nothing fills ` +
        "the fallback on the server. Load this state through `Resource` so it is in the response.",
    );
  }
}

// The rest of the workspace is not yet moved. Report it, do not fail it: the ADR moves surfaces one
// at a time, and a gate that failed on the whole backlog would be turned off instead of worked
// through.
// Every surface the ADR names has moved. The list stays so a surface that regresses is reported
// rather than silently re-listed: an entry here must still create a `LocalResource`.
const notYetMoved = [];
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
// The rest of the composition file belongs to panels rendered inside the moved surface. They are
// not counted as separate surfaces here, but a `LocalResource` left in that file must still be
// reported, because a panel that a host shows is a surface it shows.
const compositionFile = "crates/modules/rustok-pages/admin/src/composition.rs";
const compositionLocalResources = [...read(compositionFile).matchAll(
  /let\s+(\w+)\s*=\s*LocalResource::new\(/g,
)];
for (const match of compositionLocalResources) {
  pending.push(`${compositionFile} (${match[1]})`);
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
