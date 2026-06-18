import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const fixturePath = join(here, "..", "contracts", "seo", "runtime-parity-fixtures.json");
const repoRoot = resolve(here, "..", "..", "..");
const fixtures = JSON.parse(readFileSync(fixturePath, "utf8"));

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

assert(fixtures.version === 1, "Expected SEO runtime fixture contract version 1");
assert(typeof fixtures.updatedAt === "string", "Expected updatedAt timestamp");

const requiredFallbackCases = new Map([
  ["module_disabled", "NOT_FOUND"],
  ["not_found", "NOT_FOUND"],
  ["permission_denied", "PERMISSION_DENIED"],
  ["transport_failure", "TRANSPORT_ERROR"],
]);
const fallbackRows = fixtures.fallbackBehavior ?? [];
const fallbackCases = new Map(fallbackRows.map((item) => [item.case, item]));
for (const [requiredCase, transportCode] of requiredFallbackCases) {
  const row = fallbackCases.get(requiredCase);
  assert(row, `Missing fallback fixture case: ${requiredCase}`);
  assert(
    row.transportCode === transportCode,
    `Fallback case ${requiredCase} expected transportCode ${transportCode}`,
  );
  assert(
    row.expectedSource === "fallback_static",
    `Fallback case ${requiredCase} must preserve static fallback source`,
  );
  assert(
    row.expectedReason === requiredCase,
    `Fallback case ${requiredCase} must map to matching expectedReason`,
  );
}

const routeRows = fixtures.routeOwnership ?? [];
const requiredRouteOwners = new Map([
  ["page", "rustok-pages"],
  ["product", "rustok-product"],
  ["blog_post", "rustok-blog"],
  ["forum_topic", "rustok-forum"],
]);
for (const [targetKind, ownerModule] of requiredRouteOwners) {
  const row = routeRows.find((item) => item.targetKind === targetKind);
  assert(row, `Missing route ownership target kind: ${targetKind}`);
  assert(row.ownerModule === ownerModule, `Unexpected owner for ${targetKind}: ${row.ownerModule}`);
  assert(row.nextSmokeRoute?.locale, `Missing Next locale smoke route for ${targetKind}`);
  assert(row.nextSmokeRoute?.routeSegment, `Missing Next route segment for ${targetKind}`);
  assert(row.rustStorefrontRoute?.startsWith("/"), `Missing Rust storefront route for ${targetKind}`);
  assert(
    Array.isArray(row.routePatterns) && row.routePatterns.length >= 1,
    `Missing route patterns for ${targetKind}`,
  );
}

const smokeRows = fixtures.smokeEvidence ?? [];
const smokeRoutes = new Map(smokeRows.map((item) => [item.route, item]));
for (const [route, requiredAssertions] of [
  ["/modules/product?slug=demo-product", ["canonical", "robots", "openGraph", "twitter", "structuredDataBlocks"]],
  ["/modules/blog?slug=release-notes", ["canonical", "hreflang", "robots", "openGraph", "structuredDataBlocks"]],
]) {
  const row = smokeRoutes.get(route);
  assert(row, `Missing non-home metadata smoke route: ${route}`);
  for (const requiredAssertion of requiredAssertions) {
    assert(row.assertions?.includes(requiredAssertion), `Smoke route ${route} misses ${requiredAssertion}`);
  }
}

const allowlistFields = new Set((fixtures.longTailDiffAllowlist ?? []).map((item) => item.field));
for (const field of ["metadataBase", "scriptNonce", "jsonLdWhitespace"]) {
  assert(allowlistFields.has(field), `Missing long-tail metadata diff allowlist field: ${field}`);
}

const matrix = fixtures.verificationMatrix ?? [];
assert(matrix.length >= 5, "Expected D8 compile-free verification matrix entries");
for (const row of matrix) {
  assert(row.compileFree === true, `D8 lightweight gate must be compile-free: ${row.gate}`);
  assert(row.command, `D8 verification gate misses command: ${row.gate}`);
  assert(row.scope, `D8 verification gate misses scope: ${row.gate}`);
}
assert(
  fixtures.d8EvidencePacket?.compilationPolicy === "not_run_by_request",
  "D8 evidence packet must record no-compilation policy",
);

const docsRows = fixtures.docsSyncMatrix ?? [];
const requiredDocs = [
  "crates/rustok-seo/docs/README.md",
  "apps/next-frontend/docs/README.md",
  "apps/next-admin/docs/README.md",
  "apps/storefront/docs/README.md",
];
for (const requiredPath of requiredDocs) {
  const row = docsRows.find((item) => item.path === requiredPath);
  assert(row, `Missing D9 docs sync row: ${requiredPath}`);
  assert(
    row.status?.includes("compile_free"),
    `Docs sync row must be compile-free: ${requiredPath}`,
  );
  assert(
    Array.isArray(row.covers) && row.covers.length >= 2,
    `Docs sync row misses coverage notes: ${requiredPath}`,
  );
  assert(existsSync(join(repoRoot, requiredPath)), `Docs sync path does not exist: ${requiredPath}`);
}

const signoffRows = fixtures.ownerSignoffChecklist ?? [];
for (const owner of ["Platform foundation", "Frontends", "Domain modules"]) {
  const row = signoffRows.find((item) => item.owner === owner);
  assert(row, `Missing D9 owner sign-off row: ${owner}`);
  assert(row.scope, `Owner sign-off row misses scope: ${owner}`);
  assert(
    Array.isArray(row.requiredEvidence) && row.requiredEvidence.length >= 2,
    `Owner sign-off row misses required evidence: ${owner}`,
  );
}

assert(
  fixtures.liveEvidencePlan?.status === "deferred_no_backend_hosts_started_by_request",
  "Live evidence plan must record no backend/host startup for this no-compilation iteration",
);
assert(
  (fixtures.liveEvidencePlan?.minimumBeforeClose ?? []).length >= 4,
  "Live evidence plan must define closeout minimums",
);

const staticAssertions = fixtures.staticEvidenceAssertions ?? [];
assert(
  staticAssertions.length >= 6,
  "Expected static evidence assertions for Next, Rust renderer, admin, and Leptos storefront",
);
for (const row of staticAssertions) {
  assert(row.name, "Static evidence assertion misses name");
  assert(row.path, `Static evidence assertion misses path: ${row.name}`);
  const absolutePath = join(repoRoot, row.path);
  assert(existsSync(absolutePath), `Static evidence assertion path does not exist: ${row.path}`);
  const source = readFileSync(absolutePath, "utf8");
  for (const token of row.mustContain ?? []) {
    assert(source.includes(token), `Static evidence ${row.name} misses token ${token}`);
  }
}

function assertStaticTokenMatrix(sectionName, rows, minimumRows) {
  assert(
    Array.isArray(rows) && rows.length >= minimumRows,
    `${sectionName} must contain at least ${minimumRows} rows`,
  );
  for (const row of rows) {
    const label = row.surface ?? row.invariant ?? row.host ?? row.name ?? sectionName;
    assert(row.path, `${sectionName} row misses path: ${label}`);
    assert(
      Array.isArray(row.mustContain) && row.mustContain.length >= 1,
      `${sectionName} row misses token assertions: ${label}`,
    );
    const absolutePath = join(repoRoot, row.path);
    assert(existsSync(absolutePath), `${sectionName} path does not exist: ${row.path}`);
    const source = readFileSync(absolutePath, "utf8");
    for (const token of row.mustContain) {
      assert(source.includes(token), `${sectionName} ${label} misses token ${token}`);
    }
  }
}

assertStaticTokenMatrix(
  "RBAC/module gating static matrix",
  fixtures.rbacModuleGatingMatrix,
  4,
);
assertStaticTokenMatrix(
  "Replay/index invariant static matrix",
  fixtures.replayIndexInvariantMatrix,
  4,
);
assertStaticTokenMatrix(
  "Host runtime entrypoint static matrix",
  fixtures.hostRuntimeEntrypointMatrix,
  4,
);

console.log(
  `SEO runtime fixture evidence OK: ${fallbackRows.length} fallback cases, `
    + `${routeRows.length} route rows, ${smokeRows.length} smoke routes, `
    + `${matrix.length} D8 gates, ${docsRows.length} docs rows, `
    + `${signoffRows.length} sign-off rows, `
    + `${staticAssertions.length} static assertions, `
    + `${fixtures.rbacModuleGatingMatrix.length} RBAC rows, `
    + `${fixtures.replayIndexInvariantMatrix.length} replay/index rows, `
    + `${fixtures.hostRuntimeEntrypointMatrix.length} host entrypoint rows`,
);
