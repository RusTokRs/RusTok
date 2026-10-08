#!/usr/bin/env node
/**
 * Source gate for the shared Next admin table hosts.
 *
 * The admin has dozens of tables. Every one of them used to decide for itself how to read the page
 * size from the route, how to compute the page count, how to render a header row, an empty state and
 * a "Previous / Next" bar — and the copies drifted (one of them divided by a page size without a
 * floor, so an empty list reported zero pages). This verifier locks the outcome of the consolidation:
 *
 *   1. exactly one module owns route state for a paginated table (`DataTableShell`) and exactly one
 *      owns the static markup, the empty state and the pagination bar (`DataTableStatic`),
 *   2. every known paginated table is a thin wrapper over the shell, with the page size and debounce
 *      it had before the change (so a refactor cannot silently resize a table),
 *   3. the host primitives are the only raw `<Table>` builders, and the list of files that still own
 *      their markup is a ratchet: a new file has to be either converted or added here on purpose,
 *   4. the Rust side keeps the same contract for the grid crate: every admin crate consumes
 *      `rustok-grid-leptos` without default features and forwards `hydrate`/`ssr`/`csr`, otherwise
 *      feature unification drags the server renderer into a hydrated build.
 *
 * Run with an optional repository root argument: `node verify-...mjs [repoRoot]`.
 */

import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
export const defaultRepoRoot = path.resolve(scriptDir, "../..");

export const HOST_DIR = "apps/next-admin/src/widgets/data-table";
export const SHELL = `${HOST_DIR}/data-table-shell.tsx`;
export const STATIC_HOST = `${HOST_DIR}/data-table-static.tsx`;
export const HOST_INDEX = `${HOST_DIR}/index.ts`;
export const TABLE_SHIM = "apps/next-admin/src/components/ui/table/data-table.ts";

/** Host primitives may build raw `<Table>` markup; nothing else may. */
export const HOST_PRIMITIVE_DIRS = [
  "apps/next-admin/src/widgets/data-table/",
  "apps/next-admin/src/components/ui/table/",
];

/**
 * Paginated tables and the configuration they must keep.
 *
 * `pageSize`/`debounceMs` are `undefined` when the value matches the shell default, so the wrapper
 * stays a one-liner; a non-default value must be passed explicitly in the wrapper.
 */
export const PAGINATED_TABLES = [
  {
    file: "apps/next-admin/packages/blog/src/components/post-table/index.tsx",
    pageSize: undefined,
    debounceMs: undefined,
  },
  {
    file: "apps/next-admin/packages/commerce/src/components/orders-table/orders-table.tsx",
    pageSize: undefined,
    debounceMs: undefined,
  },
  {
    file: "apps/next-admin/packages/rustok-product/src/components/products/product-table/product-table.tsx",
    pageSize: undefined,
    debounceMs: undefined,
  },
  {
    file: "apps/next-admin/src/features/users/components/users-table/users-table.tsx",
    pageSize: 12,
    debounceMs: undefined,
  },
  {
    file: "apps/next-admin/src/widgets/oauth-apps-table/oauth-apps-table.tsx",
    pageSize: 10,
    debounceMs: 300,
  },
  {
    file: "apps/next-admin/packages/workflow/src/components/workflows-table/workflows-table.tsx",
    pageSize: 10,
    debounceMs: 300,
  },
  {
    file: "apps/next-admin/packages/workflow/src/components/execution-history.tsx",
    pageSize: 10,
    debounceMs: 300,
  },
];

/** Tables that render through the static host instead of the stateful pair. */
export const STATIC_TABLES = [
  "apps/next-admin/packages/rustok-product/src/components/categories/categories-table.tsx",
  "apps/next-admin/packages/rustok-product/src/components/attributes/attributes-table.tsx",
  "apps/next-admin/packages/rustok-product/src/components/bundles/bundles-table.tsx",
  "apps/next-admin/packages/rbac/src/components/permissions-table.tsx",
];

/** Server-paginated surfaces that use the shared bar instead of hand-rolled buttons. */
export const PAGINATION_BAR_CONSUMERS = [
  "apps/next-admin/packages/rustok-product/src/components/bundles/bundles-table.tsx",
  "apps/next-admin/packages/commerce/src/components/ShippingProfilesTemplate.tsx",
  "apps/next-admin/packages/commerce/src/components/OrderChangesTemplate.tsx",
];

/**
 * Ratchet: files that still build their own table markup.
 *
 * Each entry is a card or an aggregate template with inline editing or per-row expansion; converting
 * them is tracked work, not an accepted duplicate. A new entry means a new table copy — convert it
 * or extend this list deliberately.
 */
export const RAW_TABLE_RATCHET = [
  "apps/next-admin/packages/commerce/src/components/CartPromotionsTemplate.tsx",
  "apps/next-admin/packages/commerce/src/components/OrderChangesTemplate.tsx",
  "apps/next-admin/packages/commerce/src/components/ShippingProfilesTemplate.tsx",
  "apps/next-admin/packages/rbac/src/components/roles-table.tsx",
  "apps/next-admin/packages/rustok-product/src/components/products/product-bundle-card.tsx",
  "apps/next-admin/packages/rustok-product/src/components/products/product-relations-card.tsx",
  "apps/next-admin/packages/rustok-product/src/components/products/product-variants-card.tsx",
  "apps/next-admin/packages/search/src/index.tsx",
  "apps/next-admin/packages/translation/src/index.tsx",
];

const NEXT_SOURCE_ROOT = "apps/next-admin";
const CRATE_ROOT = "crates";

function walk(directory, relative, sources, extensions) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (entry.name === "node_modules" || entry.name.startsWith(".")) continue;
    const absolute = path.join(directory, entry.name);
    const childRelative = `${relative}/${entry.name}`;
    if (entry.isDirectory()) {
      walk(absolute, childRelative, sources, extensions);
      continue;
    }
    if (extensions.test(entry.name)) sources.set(childRelative, readFileSync(absolute, "utf8"));
  }
}

/** Reads every Next admin source and every crate manifest under `root`. */
export function collectSources(root) {
  const files = new Map();
  const manifests = new Map();

  walk(path.join(root, NEXT_SOURCE_ROOT), NEXT_SOURCE_ROOT, files, /\.(ts|tsx)$/);
  walk(
    path.join(root, CRATE_ROOT),
    CRATE_ROOT,
    manifests,
    /^Cargo\.toml$/,
  );
  const workspaceManifest = path.join(root, "Cargo.toml");
  if (existsSync(workspaceManifest)) manifests.set("Cargo.toml", readFileSync(workspaceManifest, "utf8"));

  return { files, manifests };
}

const isHostPrimitive = (relative) =>
  HOST_PRIMITIVE_DIRS.some((prefix) => relative.startsWith(prefix));

/** All rules; returns a list of human readable failures (empty means the gate passes). */
export function evaluateHostRules({ files, manifests }) {
  const failures = [];
  const nextFiles = [...files.entries()];
  const source = (relative) => files.get(relative);
  const filesMatching = (pattern) =>
    nextFiles.filter(([, content]) => pattern.test(content)).map(([relative]) => relative);

  // 1. One owner of route state, one owner of the page-count maths. The framework hook may keep
  // its own query plumbing, but the table page size is read in exactly one place.
  const routeStateOwners = filesMatching(/useQueryState\(\s*['"]perPage['"]/);
  if (routeStateOwners.length !== 1 || routeStateOwners[0] !== SHELL) {
    failures.push(
      `the table page size must be read in ${SHELL} only, found: ${
        routeStateOwners.join(", ") || "none"
      }`,
    );
  }
  const dataTableOwners = filesMatching(/useDataTable\(/);
  if (dataTableOwners.length !== 1 || dataTableOwners[0] !== SHELL) {
    failures.push(
      `useDataTable must be called from ${SHELL} only, found: ${
        dataTableOwners.join(", ") || "none"
      }`,
    );
  }
  const shell = source(SHELL) ?? "";
  for (const marker of [
    "Math.max(1, Math.ceil(",
    "shallow: false",
    "parseAsInteger.withDefault(defaultPageSize)",
  ]) {
    if (!shell.includes(marker)) failures.push(`${SHELL}: missing ${marker}`);
  }
  for (const marker of ["defaultPageSize = 20", "debounceMs = 500", "export function DataTableShell"]) {
    if (!shell.includes(marker)) failures.push(`${SHELL}: missing ${marker}`);
  }
  const unflooredPageCount = nextFiles
    .filter(([relative]) => !isHostPrimitive(relative))
    .filter(([, content]) =>
      [
        ...content.matchAll(
          /Math\.ceil\(\s*[A-Za-z_$][\w$.]*\s*\/\s*(?:pageSize|perPage|page_size)\b/g,
        ),
      ].some(
        (match) => !content.slice(Math.max(0, match.index - 14), match.index).includes("Math.max(1, "),
      ),
    )
    .map(([relative]) => relative);
  for (const relative of unflooredPageCount) {
    failures.push(
      `${relative}: divides by a page size without a floor, so an empty list reports zero pages`,
    );
  }

  // 2. Every paginated table is a thin wrapper that keeps its page size and debounce.
  for (const { file, pageSize, debounceMs } of PAGINATED_TABLES) {
    const content = source(file);
    if (content === undefined) {
      failures.push(`${file}: paginated table is missing`);
      continue;
    }
    if (!content.includes("DataTableShell")) {
      failures.push(`${file}: must render through DataTableShell`);
    }
    for (const forbidden of ["useDataTable(", "useQueryState(", "Math.ceil("]) {
      if (content.includes(forbidden)) {
        failures.push(`${file}: must not keep its own ${forbidden} wiring`);
      }
    }
    if (pageSize !== undefined && !content.includes(`defaultPageSize={${pageSize}}`)) {
      failures.push(`${file}: lost its page size (expected defaultPageSize={${pageSize}})`);
    }
    if (debounceMs !== undefined && !content.includes(`debounceMs={${debounceMs}}`)) {
      failures.push(`${file}: lost its debounce (expected debounceMs={${debounceMs}})`);
    }
  }

  // 3. The static host owns the header, the empty state and the pagination bar.
  const staticHost = source(STATIC_HOST) ?? "";
  for (const marker of [
    "export function DataTableStatic",
    "export function DataTableEmptyState",
    "export function DataTablePaginationBar",
    "export function dataTableRangeLabel",
    "emptyState",
    "colSpan={columns.length}",
  ]) {
    if (!staticHost.includes(marker)) failures.push(`${STATIC_HOST}: missing ${marker}`);
  }
  const hostIndex = source(HOST_INDEX) ?? "";
  for (const exportLine of ["./data-table-shell", "./data-table-static"]) {
    if (!hostIndex.includes(exportLine)) failures.push(`${HOST_INDEX}: must re-export ${exportLine}`);
  }
  for (const file of STATIC_TABLES) {
    const content = source(file);
    if (content === undefined) {
      failures.push(`${file}: static table is missing`);
      continue;
    }
    if (!content.includes("DataTableStatic")) {
      failures.push(`${file}: must render through DataTableStatic`);
    }
    if (/<Table(?=[\s>\n])/.test(content)) {
      failures.push(`${file}: still builds its own <Table> markup`);
    }
  }
  for (const file of PAGINATION_BAR_CONSUMERS) {
    const content = source(file);
    if (content === undefined) {
      failures.push(`${file}: pagination bar consumer is missing`);
      continue;
    }
    if (!content.includes("DataTablePaginationBar")) {
      failures.push(`${file}: must use the shared DataTablePaginationBar`);
    }
    if (/>\s*Previous\s*</.test(content) || />\s*Next\s*</.test(content)) {
      failures.push(`${file}: keeps hand-rolled Previous/Next buttons`);
    }
  }

  // 4. The legacy shim must stay a pure re-export.
  const shim = source(TABLE_SHIM) ?? "";
  if (!shim) {
    failures.push(`${TABLE_SHIM}: shim is missing`);
  } else {
    for (const forbidden of ["useState(", "useDataTable(", "export function", "export const"]) {
      if (shim.includes(forbidden)) {
        failures.push(`${TABLE_SHIM}: shim must stay a re-export, found ${forbidden}`);
      }
    }
  }

  // 5. Raw table markup is a ratchet: convert or extend the list on purpose.
  const rawTableFiles = filesMatching(/<Table(?=[\s>\n])/)
    .filter((relative) => !isHostPrimitive(relative))
    .sort();
  const ratchet = [...RAW_TABLE_RATCHET].sort();
  const newOffenders = rawTableFiles.filter((relative) => !ratchet.includes(relative));
  const staleEntries = ratchet.filter((relative) => !rawTableFiles.includes(relative));
  if (newOffenders.length > 0) {
    failures.push(
      `new files build raw <Table> markup instead of the host: ${newOffenders.join(", ")}`,
    );
  }
  if (staleEntries.length > 0) {
    failures.push(
      `table-host ratchet is stale, these files no longer build markup: ${staleEntries.join(", ")}`,
    );
  }

  // 6. Rust: one grid crate, no default features, forwarded across the workspace.
  for (const [relative, content] of manifests) {
    if (!content.includes("rustok-grid-leptos")) continue;
    // The adapter's own manifest is where the `ssr` package default lives; consumers opt out.
    if (/name\s*=\s*"rustok-grid-leptos"/.test(content.slice(0, 400))) continue;
    const dependency = content.match(/^rustok-grid-leptos\s*=\s*\{([^}]*)\}/m);
    if (!dependency) {
      failures.push(
        `${relative}: rustok-grid-leptos must be a workspace dependency with default-features = false`,
      );
      continue;
    }
    if (!/default-features\s*=\s*false/.test(dependency[1])) {
      failures.push(
        `${relative}: rustok-grid-leptos must set default-features = false (the ssr default leaks into hydrate builds)`,
      );
    }
    const features = parseFeatures(content);
    for (const feature of ["hydrate", "ssr", "csr"]) {
      const members = features.get(feature);
      if (!members) continue;
      if (!members.some((member) => member === `rustok-grid-leptos/${feature}`)) {
        failures.push(
          `${relative}: feature "${feature}" must forward "rustok-grid-leptos/${feature}"`,
        );
      }
    }
  }

  return failures;
}

/** Minimal TOML feature-table reader: only `[features]` arrays of strings are interpreted. */
function parseFeatures(content) {
  const lines = content.split(/\r?\n/);
  const features = new Map();
  let inFeatures = false;
  let pending = null;
  for (const line of lines) {
    const section = line.match(/^\s*\[([^\]]+)\]\s*$/);
    if (section) {
      inFeatures = section[1].trim() === "features";
      continue;
    }
    if (!inFeatures) continue;
    const entry = line.match(/^\s*([A-Za-z0-9_-]+)\s*=\s*\[(.*)$/);
    if (entry) {
      pending = { name: entry[1], body: entry[2] };
    } else if (pending) {
      pending.body += `\n${line}`;
    } else {
      continue;
    }
    if (pending.body.includes("]")) {
      const values = [...pending.body.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
      features.set(pending.name, values);
      pending = null;
    }
  }
  if (pending) {
    const values = [...pending.body.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
    features.set(pending.name, values);
  }
  return features;
}

function main() {
  const root = process.argv[2] ? path.resolve(process.argv[2]) : defaultRepoRoot;
  if (!existsSync(path.join(root, NEXT_SOURCE_ROOT))) {
    console.error(`shared data-table host verification failed:\n- ${root}: not a RusTok checkout`);
    process.exit(2);
  }
  const failures = evaluateHostRules(collectSources(root));
  if (failures.length > 0) {
    console.error("shared data-table host verification failed:");
    for (const failure of failures) console.error(`- ${failure}`);
    process.exit(1);
  }
  console.log("shared data-table host verification passed");
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) main();
