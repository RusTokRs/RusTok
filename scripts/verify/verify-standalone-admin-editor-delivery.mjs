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
import { readFileSync, readdirSync } from "node:fs";
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
  const source = read(file);
  // An `Effect` runs on a later tick and is never driven before the response. Using one to turn a
  // resolved value into what the page renders keeps the whole page out of the HTML, which is the
  // ADR's second invariant, so the page's data path must be the render's.
  if (/Effect::new\(/.test(source)) {
    const isInstaller = file.endsWith("installer.rs");
    if (!isInstaller) {
      failures.push(
        `${file}: turns fetched data into rendered output with an \`Effect\`, which is not driven ` +
          "before the response. Derive the rendered value from the resource instead, and keep only " +
          "user edits in signals.",
      );
    }
  }
}

// The five files above were the ones carrying the defect when the ADR was written. The layout covers
// the whole `apps/admin` tree, so the tree is what is checked: a page added later sits inside the
// same `Async` render, and a list of five files would have nothing to say about it. The installer is
// the one page outside the layout — it declares no mode and keeps a browser-side loader — so it is
// the one file where a `LocalResource` is the documented answer rather than a leftover.
const adminLocalResourceSources = [];
{
  const walkAdmin = (relative) => {
    let entries;
    try {
      entries = readdirSync(join(root, relative), { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const path = `${relative}/${entry.name}`;
      if (entry.isDirectory()) {
        if (["node_modules", "target", "tests"].includes(entry.name)) continue;
        walkAdmin(path);
      } else if (entry.name.endsWith(".rs")) {
        adminLocalResourceSources.push(path);
      }
    }
  };
  walkAdmin("apps/admin/src");
}
for (const file of adminLocalResourceSources) {
  if (file.endsWith("/pages/installer.rs")) continue;
  if (!/LocalResource::new\(/.test(read(file))) continue;
  failures.push(
    `${file}: creates a \`LocalResource\` inside the \`Async\` admin layout, where nothing fills ` +
      "the fallback on the server. Load this state through `Resource` so it is in the response, or " +
      "move the page outside the layout the way the installer is.",
  );
}

// Module admin surfaces are mounted at `/modules/:module_slug/*module_path`, inside the same `Async`
// layout, so the same rule applies to them. The list is the worked set: a surface is added here when
// it moves. `crates/modules/*/admin` still holds dozens of `LocalResource`s; they are reported by
// count so the backlog is visible rather than silently accepted.
const movedModuleSurfaces = [
  {
    file: "crates/modules/rustok-product/admin/src/ui/attributes.rs",
    marker: "let attributes_resource = Resource::new(",
    description: "the attributes page loads its directory and schemas during the render",
  },
  {
    file: "crates/modules/rustok-product/admin/src/ui/product_grid.rs",
    marker: "let products_resource = Resource::new(",
    description: "the product grid loads its rows, options and facets during the render",
  },
  {
    file: "crates/modules/rustok-product/admin/src/ui/categories.rs",
    marker: "let categories_resource = Resource::new(",
    description: "the categories page loads its tree during the render",
  },
  {
    file: "crates/modules/rustok-product/admin/src/ui/catalog_admin.rs",
    marker: "let catalog_options = Resource::new(",
    description: "the catalog filter controls resolve their options during the render",
  },
  {
    file: "crates/modules/rustok-product/admin/src/ui/leptos.rs",
    marker: "let categories_resource = Resource::new(",
    description: "the schema editor resolves its categories during the render",
  },
  {
    file: "crates/modules/rustok-forum/admin/src/ui/topic_split.rs",
    marker: "let candidates = Resource::new(",
    description: "the split panel resolves its topic candidates during the render",
  },
  {
    file: "crates/modules/rustok-auth/admin/src/ui/invites.rs",
    marker: "let invites_resource = Resource::new(",
    description: "the invites page resolves its invitations during the render",
  },
  {
    file: "crates/modules/rustok-blog/admin/src/ui/moderation.rs",
    marker: "let comments_resource = Resource::new(",
    description: "the moderation queue resolves its comments during the render",
  },
  {
    file: "crates/modules/rustok-forms/admin/src/lib.rs",
    marker: "let submissions = Resource::new(",
    description: "the form inbox resolves its submissions during the render",
  },
  {
    file: "crates/modules/rustok-brand/admin/src/ui/leptos.rs",
    marker: "let directory = loaded_resource(",
    description: "the brand directory resolves its rows during the render",
  },
  {
    file: "crates/modules/rustok-cache/admin/src/ui/leptos.rs",
    marker: "let health_resource = Resource::new(",
    description: "the cache diagnostics card resolves its health during the render",
  },
  {
    file: "crates/modules/rustok-comments/admin/src/ui/leptos.rs",
    marker: "let threads = loaded_resource(",
    description: "the comments admin resolves its threads during the render",
  },
  {
    file: "crates/modules/rustok-commerce/admin/src/ui/leptos.rs",
    marker: "let shipping_profiles = Resource::new(",
    description: "the commerce admin resolves its shipping profiles during the render",
  },
  {
    file: "crates/modules/rustok-customer/admin/src/ui/leptos.rs",
    marker: "let customers = loaded_resource(",
    description: "the customer directory resolves its rows during the render",
  },
  {
    file: "crates/modules/rustok-pricing/admin/src/ui/leptos.rs",
    marker: "let bootstrap = loaded_resource(",
    description: "the pricing admin resolves its tenant and feed during the render",
  },
  {
    file: "crates/modules/rustok-product-bundles/admin/src/ui/leptos.rs",
    marker: "let directory = loaded_resource(",
    description: "the bundle directory resolves its rows during the render",
  },
  {
    file: "crates/modules/rustok-product-relations/admin/src/ui/leptos.rs",
    marker: "let relations_resource = loaded_resource(",
    description: "the product relation editor resolves its rows during the render",
  },
  {
    file: "crates/modules/rustok-marketplace-listing/admin/src/ui/leptos.rs",
    marker: "let directory = loaded_resource(",
    description: "the marketplace listing admin resolves its directory during the render",
  },
  {
    file: "crates/modules/rustok-marketplace-seller/admin/src/ui/leptos.rs",
    marker: "let directory = loaded_resource(",
    description: "the marketplace seller admin resolves its directory during the render",
  },
  {
    file: "crates/modules/rustok-marketplace-seller/admin/src/ui/event_history_panel.rs",
    marker: "let history = Resource::new(",
    description: "the seller event timeline resolves its history during the render",
  },
  {
    file: "crates/modules/rustok-translation/admin/src/ui/leptos.rs",
    marker: "let bootstrap = Resource::new(",
    description: "the translation admin resolves its glossaries and memory during the render",
  },
  {
    file: "crates/modules/rustok-workflow/admin/src/ui/leptos.rs",
    marker: "let workflows_resource = loaded_resource(",
    description: "the workflow admin resolves its workflows and templates during the render",
  },
  {
    file: "crates/modules/rustok-groups/admin/src/ui/leptos.rs",
    marker: "let directory = Resource::new(",
    description: "the groups admin resolves its directory during the render",
  },
  {
    file: "crates/modules/rustok-ai/admin/src/ui/leptos.rs",
    marker: "let bootstrap = loaded_resource(",
    description: "the AI control plane resolves its bootstrap during the render",
  },
  {
    file: "crates/modules/rustok-auth/admin/src/ui/users.rs",
    marker: "let users_resource = loaded_resource(",
    description: "the user directory resolves its rows during the render",
  },
  {
    file: "crates/modules/rustok-auth/admin/src/ui/user_details.rs",
    marker: "let user_resource = loaded_resource(",
    description: "the user details page resolves its user during the render",
  },
  {
    file: "crates/modules/rustok-blog/admin/src/ui/leptos.rs",
    marker: "let posts_resource = loaded_resource(",
    description: "the blog admin resolves its posts and categories during the render",
  },
  {
    file: "crates/modules/rustok-blog/admin/src/ui/categories.rs",
    marker: "let categories = loaded_resource(",
    description: "the blog categories page resolves its rows during the render",
  },
  {
    file: "crates/modules/rustok-forum/admin/src/ui/leptos.rs",
    marker: "let categories = loaded_resource(",
    description: "the forum admin resolves its tree, topics and replies during the render",
  },
  {
    file: "crates/modules/rustok-product/admin/src/ui/product_editor.rs",
    marker: "let categories_resource = Resource::new(",
    description: "the product editor resolves its categories and shipping profiles during the render",
  },
  {
    file: "crates/modules/rustok-events-module/admin/src/ui.rs",
    marker: "let configuration = loaded_resource(",
    description: "the events admin renders the resolved delivery profile during the render",
  },
  {
    file: "crates/modules/rustok-iggy-connector/admin/src/ui.rs",
    marker: "let configuration = loaded_resource(",
    description: "the Iggy connector form renders the resolved configuration during the render",
  },
  {
    file: "crates/modules/rustok-cache/admin/src/ui/leptos.rs",
    marker: "let settings_resource = loaded_resource(",
    description: "the cache settings form renders the resolved settings during the render",
  },
];
for (const surface of movedModuleSurfaces) {
  const source = read(surface.file);
  if (!source.includes(surface.marker)) {
    failures.push(`${surface.file}: ${surface.description}, so it must create \`${surface.marker.slice(0, -1)}\``);
  }
}

const remainingModuleLocalResources = (
  read("docs/modules/page-builder-backlog.md").match(
    /module admin surfaces still on `LocalResource`: *\n? *(\d+)/i,
  ) ?? []
)[1];
if (!remainingModuleLocalResources) {
  failures.push(
    "docs/modules/page-builder-backlog.md: the count of module admin surfaces still on " +
      "`LocalResource` is not recorded, so the remaining work is invisible",
  );
}
// The count is a tally of what is still there, so it is compared with the source rather than
// trusted: a surface that moves without the record moving makes the backlog claim work that is done
// and hides work that is not, and the number is the only thing a reader of the backlog has.
const actualLocalResources = moduleAdminSources().reduce(
  (total, file) => total + [...read(file).matchAll(/LocalResource::new\(/g)].length,
  0,
);
if (
  remainingModuleLocalResources &&
  Number(remainingModuleLocalResources) !== actualLocalResources
) {
  failures.push(
    `docs/modules/page-builder-backlog.md: records ${remainingModuleLocalResources} module admin ` +
      `surfaces on \`LocalResource\`, but the module packages create ${actualLocalResources}; the ` +
      "record and the source move together",
  );
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

// A surface that takes a resource as a prop declares its type. When the caller moves to `Resource`
// and the callee still says `LocalResource`, the two do not unify and the crate does not build — and
// the served output is wrong even before that, because the callee would read a pending local
// resource. The declaration and the call often sit in different files, so this builds one registry
// of declarations across the modules that moved, then checks every call site against it.
// Every Rust file under a module admin package is a candidate declaration site. Listing files by
// hand let a module that moved later declare its props in a file nobody added; the walk keeps the
// check proportional to the tree instead of to the list.
function moduleAdminSources() {
  const found = [];
  const modules = readdirSync(join(root, "crates/modules"), { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort();
  for (const module of modules) {
    const walk = (relative) => {
      let entries;
      try {
        entries = readdirSync(join(root, relative), { withFileTypes: true });
      } catch {
        return;
      }
      for (const entry of entries) {
        const path = `${relative}/${entry.name}`;
        if (entry.isDirectory()) {
          if (entry.name === "tests" || entry.name === "target") continue;
          walk(path);
        } else if (entry.name.endsWith(".rs")) {
          found.push(path);
        }
      }
    };
    walk(`crates/modules/${module}/admin/src`);
  }
  return found;
}
const resourcePropFiles = moduleAdminSources();

// Splits an argument list on top-level commas.
function splitArguments(list) {
  const parts = [];
  let depth = 0;
  let current = "";
  for (const character of list) {
    if ("([{<".includes(character)) depth += 1;
    if (")]}>".includes(character)) depth -= 1;
    if (character === "," && depth === 0) {
      parts.push(current.trim());
      current = "";
      continue;
    }
    current += character;
  }
  if (current.trim()) parts.push(current.trim());
  return parts;
}

// Reads a balanced parenthesised list starting at the `(` at `open`.
function readArguments(source, open) {
  let depth = 0;
  for (let i = open; i < source.length; i += 1) {
    if ("([{<".includes(source[i])) depth += 1;
    if (")]}>".includes(source[i])) {
      depth -= 1;
      if (depth === 0) return splitArguments(source.slice(open + 1, i));
    }
  }
  return null;
}

// name -> parameter names that are declared `LocalResource<…>`, across every listed file.
const localResourceProps = new Map();
const sources = new Map();
for (const file of resourcePropFiles) {
  const source = read(file);
  sources.set(file, source);
  for (const match of source.matchAll(/fn\s+(\w+)\s*\(/g)) {
    const parameters = readArguments(source, match.index + match[0].length - 1);
    if (!parameters) continue;
    const local = parameters
      .map((parameter) => parameter.split(":")[0].trim())
      .filter((_, index) => /LocalResource\s*</.test(parameters[index]))
      .filter((name) => /^\w+$/.test(name));
    if (local.length > 0) localResourceProps.set(match[1], local);
  }
}

// Reports a mismatch for one call site: `attributes` is a list of `prop=value` strings.
function checkCallSite(file, source, name, attributes, at) {
  const props = localResourceProps.get(name);
  if (!props) return;
  const resourceBindings = new Set(
    [...source.matchAll(/let\s+(\w+)\s*=\s*(?:Resource::new|loaded_resource)\(/g)].map(
      (match) => match[1],
    ),
  );
  for (const attribute of attributes) {
    const [prop, rawValue] = attribute.split("=");
    const value = (rawValue ?? "").trim();
    if (!props.includes(prop.trim())) continue;
    if (!resourceBindings.has(value)) continue;
    const line = source.slice(0, at).split("\n").length;
    failures.push(
      `${file}:${line}: \`${name}\` declares \`${prop.trim()}\` as \`LocalResource<…>\` but ` +
        `is handed \`${value}\`, a \`Resource\`; the prop and its callers move together`,
    );
  }
}

for (const [file, source] of sources) {
  for (const name of localResourceProps.keys()) {
    // Plain function calls: `name(a, b)`.
    for (const call of source.matchAll(new RegExp(`(?<!fn\\s)\\b${name}\\s*\\(`, "g"))) {
      const arguments_ = readArguments(source, call.index + call[0].length - 1);
      if (!arguments_) continue;
      checkCallSite(file, source, name, arguments_.filter((a) => a.includes("=")), call.index);
    }

    // Component calls in `view!`: `<Name prop=value prop2=value2 />`, attributes one per line.
    const component = new RegExp(`<${name}\\b([^>]*)>`, "g");
    for (const call of source.matchAll(component)) {
      const attributes = call[1]
        .split("\n")
        .map((line) => line.trim())
        .filter((line) => line.includes("=") && !line.startsWith("//"));
      checkCallSite(file, source, name, attributes, call.index);
    }
  }
}

// A `Resource` payload is serialised into the response and read back on the client, so it has to
// implement `Serialize + DeserializeOwned`; a payload that is only `Clone` fails to compile and, if
// it compiled, would arrive empty. The transport return type is where that shows up. A transport
// function name is not unique across modules — every module has its own `fetch_bootstrap` and its
// own `execute` — so the signature is resolved inside the crate that renders the surface, never
// across the workspace.
const typeDeclarations = new Map();
const signatureByFile = new Map();
function walkRustFiles(relative, into) {
  let entries;
  try {
    entries = readdirSync(join(root, relative), { withFileTypes: true });
  } catch {
    return;
  }
  for (const entry of entries) {
    const path = `${relative}/${entry.name}`;
    if (entry.isDirectory()) {
      if (["node_modules", "target", "tests"].includes(entry.name)) continue;
      walkRustFiles(path, into);
    } else if (entry.name.endsWith(".rs")) {
      into.push(path);
    }
  }
}
const packageFiles = [];
for (const base of ["crates/modules", "apps/admin/src"]) walkRustFiles(base, packageFiles);
for (const file of packageFiles) {
  const source = read(file);
  for (const match of source.matchAll(
    /((?:#\[[^\]]*\]\s*)+)\s*(?:pub(?:\([^)]*\))?\s+)?(?:struct|enum)\s+(\w+)/g,
  )) {
    const existing = typeDeclarations.get(match[2]) ?? [];
    existing.push(match[1]);
    typeDeclarations.set(match[2], existing);
  }
  const signatures = new Map();
  for (const match of source.matchAll(/pub async fn (\w+)\b/g)) {
    const tail = source.slice(match.index, match.index + 1200);
    const result = /->\s*([^;{]+)/.exec(tail);
    if (!result) continue;
    const types = [...result[1].matchAll(/\b([A-Z]\w+)\b/g)].map((type) => type[1]);
    const existing = signatures.get(match[1]) ?? new Set();
    for (const type of types) existing.add(type);
    signatures.set(match[1], existing);
  }
  if (signatures.size > 0) signatureByFile.set(file, signatures);
}
function crateOf(file) {
  const parts = file.split("/");
  if (parts[0] === "apps") return "apps/admin/src";
  const admin = parts.indexOf("admin");
  return admin > 0 ? parts.slice(0, admin + 2).join("/") : parts.slice(0, 2).join("/");
}
function declaresSerde(name) {
  const attributes = typeDeclarations.get(name);
  if (!attributes) return null;
  return attributes.some(
    (attribute) => /Serialize/.test(attribute) && /Deserialize/.test(attribute),
  );
}
const resourcePayloadFailures = [];
for (const file of moduleAdminSources()) {
  const crate = crateOf(file);
  const source = read(file);
  for (const match of source.matchAll(/(?:Resource::new|loaded_resource)\(/g)) {
    const call = source.slice(match.index, match.index + 3000);
    const transport = /(?:\w*transport)::(\w+)\(/.exec(call);
    if (!transport) continue;
    const payloadTypes = new Set();
    for (const [path, signatures] of signatureByFile) {
      if (!path.startsWith(crate)) continue;
      for (const type of signatures.get(transport[1]) ?? []) payloadTypes.add(type);
    }
    for (const type of payloadTypes) {
      if (declaresSerde(type) === false) {
        const line = source.slice(0, match.index).split("\n").length;
        resourcePayloadFailures.push(
          `${file}:${line}: the resource fetches \`${transport[1]}\`, whose payload \`${type}\` is ` +
            "declared without `Serialize`/`Deserialize`; a `Resource` serialises its payload into " +
            "the response, so the type has to be serialisable",
        );
      }
    }
  }
}
failures.push(...resourcePayloadFailures);

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
