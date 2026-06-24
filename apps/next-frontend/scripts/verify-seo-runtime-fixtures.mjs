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
  assert(
    Array.isArray(row.mayMoveToSignedWhen) && row.mayMoveToSignedWhen.length >= 3,
    `Owner sign-off row must define signed-state preconditions: ${owner}`,
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
assertStaticTokenMatrix(
  "Semantic error parity static matrix",
  fixtures.semanticErrorParityMatrix,
  5,
);

const liveEvidenceTemplate = fixtures.liveEvidenceCaptureTemplate ?? {};
assert(
  liveEvidenceTemplate.status === "template_only_no_runtime_started",
  "Live evidence capture template must remain marked as template-only for no-runtime iterations",
);
assert(
  Array.isArray(liveEvidenceTemplate.commands) && liveEvidenceTemplate.commands.length >= 5,
  "Live evidence capture template must list backend, pipeline, Next, Leptos and admin commands",
);
for (const row of liveEvidenceTemplate.commands) {
  assert(row.surface, "Live evidence command misses surface");
  assert(row.command, `Live evidence command misses command: ${row.surface}`);
  assert(
    Array.isArray(row.requiredArtifacts) && row.requiredArtifacts.length >= 2,
    `Live evidence command misses artifact requirements: ${row.surface}`,
  );
}
assert(
  (liveEvidenceTemplate.redactionRules ?? []).some((rule) => rule.includes("auth tokens")),
  "Live evidence template must include credential redaction guidance",
);

const incidentTemplates = fixtures.incidentEvidenceTemplates ?? [];
assert(
  incidentTemplates.length >= 3,
  "Expected D9 incident evidence templates for backlog, indexing failures and replay/reindex",
);
for (const row of incidentTemplates) {
  assert(row.scenario, "Incident evidence template misses scenario");
  assert(row.runbook, `Incident evidence template misses runbook: ${row.scenario}`);
  assert(existsSync(join(repoRoot, row.runbook)), `Incident runbook path does not exist: ${row.runbook}`);
  assert(
    Array.isArray(row.requiredEvidence) && row.requiredEvidence.length >= 3,
    `Incident evidence template misses required evidence: ${row.scenario}`,
  );
  assert(
    row.status === "pending_live_incident_or_drill",
    `Incident template must stay pending until live evidence is attached: ${row.scenario}`,
  );
}

const ownerCloseoutCriteria = fixtures.ownerCloseoutCriteria ?? [];
assert(
  ownerCloseoutCriteria.length >= 3,
  "Expected owner closeout criteria for platform, frontend and domain owners",
);
for (const row of ownerCloseoutCriteria) {
  assert(row.owner, "Owner closeout row misses owner");
  assert(
    Array.isArray(row.acceptance) && row.acceptance.length >= 3,
    `Owner closeout row misses acceptance criteria: ${row.owner}`,
  );
  assert(
    Array.isArray(row.blocksCloseoutIf) && row.blocksCloseoutIf.length >= 3,
    `Owner closeout row misses blockers: ${row.owner}`,
  );
}

const unitCoverageInventory = fixtures.unitCoverageInventory ?? [];
assert(
  unitCoverageInventory.length >= 4,
  "Expected D8 unit coverage inventory for normalization, replay, GraphQL and storefront locale checks",
);
for (const row of unitCoverageInventory) {
  assert(row.batch, "D8 unit inventory row misses batch");
  assert(row.path, `D8 unit inventory row misses path: ${row.batch}`);
  assert(
    row.commandWhenCompilationAllowed?.startsWith("cargo test"),
    `D8 unit inventory must record the future cargo test command: ${row.batch}`,
  );
  assert(
    row.status === "source_locked_pending_execution",
    `D8 unit inventory must stay pending execution until compilation is allowed: ${row.batch}`,
  );
  const source = readFileSync(join(repoRoot, row.path), "utf8");
  for (const token of row.mustContain ?? []) {
    assert(source.includes(token), `D8 unit inventory ${row.batch} misses token ${token}`);
  }
}

const integrationMatrixPlan = fixtures.integrationMatrixPlan ?? [];
for (const surface of [
  "backend_graphql_rest_parity",
  "outbox_index_pipeline",
  "next_frontend_runtime",
  "leptos_storefront_runtime",
  "media_descriptor_fallback_smoke",
]) {
  const row = integrationMatrixPlan.find((item) => item.surface === surface);
  assert(row, `Missing D8 integration matrix plan surface: ${surface}`);
  assert(row.pendingCommand, `D8 integration matrix surface misses pending command: ${surface}`);
  assert(
    Array.isArray(row.requiredArtifacts) && row.requiredArtifacts.length >= 3,
    `D8 integration matrix surface misses artifact list: ${surface}`,
  );
  assert(
    Array.isArray(row.blocksCloseoutIf) && row.blocksCloseoutIf.length >= 2,
    `D8 integration matrix surface misses closeout blockers: ${surface}`,
  );
}

const liveArtifactManifestTemplate = fixtures.liveArtifactManifestTemplate ?? {};
assert(
  liveArtifactManifestTemplate.status === "template_only_pending_d8_runtime",
  "Live artifact manifest template must remain pending until D8 runtime evidence is captured",
);
for (const requiredFile of [
  "backend-graphql-rest-parity.json",
  "outbox-index-before-after-counters.json",
  "next-runtime-robots-sitemap-metadata.json",
  "leptos-storefront-page-context-smoke.json",
  "media-descriptor-fallback-smoke.json",
  "owner-signoff.md",
]) {
  assert(
    liveArtifactManifestTemplate.requiredFiles?.includes(requiredFile),
    `Live artifact manifest template misses ${requiredFile}`,
  );
}
for (const counterField of ["pending", "sent", "retry", "failed", "dead_letter", "replay_mode"]) {
  assert(
    liveArtifactManifestTemplate.counterFields?.includes(counterField),
    `Live artifact manifest template misses counter field ${counterField}`,
  );
}
assert(
  (liveArtifactManifestTemplate.redactionPolicy ?? []).some((rule) => rule.includes("auth tokens")),
  "Live artifact manifest template must include auth token redaction",
);

const liveEvidenceArtifactTemplates = fixtures.liveEvidenceArtifactTemplates ?? [];
assert(
  liveEvidenceArtifactTemplates.length >= 6,
  "Expected concrete live evidence artifact templates for D8 closeout files",
);
const liveTemplateFiles = new Set(liveEvidenceArtifactTemplates.map((row) => row.file));
for (const requiredFile of liveArtifactManifestTemplate.requiredFiles ?? []) {
  const row = liveEvidenceArtifactTemplates.find((item) => item.file === requiredFile);
  assert(row, `Missing concrete live evidence artifact template: ${requiredFile}`);
  assert(
    row.status === "template_only_pending_d8_runtime",
    `Live evidence artifact template must stay pending until runtime capture: ${requiredFile}`,
  );
  assert(row.surface, `Live evidence artifact template misses surface: ${requiredFile}`);
  assert(
    Array.isArray(row.mustCapture) && row.mustCapture.length >= 3,
    `Live evidence artifact template misses capture checklist: ${requiredFile}`,
  );
  assert(
    Array.isArray(row.blocksCloseoutIf) && row.blocksCloseoutIf.length >= 3,
    `Live evidence artifact template misses closeout blockers: ${requiredFile}`,
  );
}
assert(
  liveTemplateFiles.has("media-descriptor-fallback-smoke.json"),
  "Live evidence templates must include SEO media descriptor fallback smoke",
);

const liveArtifactSchemaTemplate = fixtures.liveArtifactSchemaTemplate ?? {};
assert(
  liveArtifactSchemaTemplate.status === "template_only_pending_d8_runtime",
  "Live artifact schema template must remain pending until D8 runtime evidence is captured",
);
for (const requiredField of [
  "captured_at",
  "environment",
  "surface",
  "command_or_ci_job",
  "before",
  "after",
  "samples",
  "redactions_applied",
  "result",
]) {
  assert(
    liveArtifactSchemaTemplate.requiredTopLevelFields?.includes(requiredField),
    `Live artifact schema template misses top-level field ${requiredField}`,
  );
}
for (const counterField of ["pending", "sent", "retry", "failed", "dead_letter", "replay_mode"]) {
  assert(
    liveArtifactSchemaTemplate.counterSnapshots?.before?.includes(counterField)
      && liveArtifactSchemaTemplate.counterSnapshots?.after?.includes(counterField),
    `Live artifact schema template misses before/after counter ${counterField}`,
  );
}
assert(
  (liveArtifactSchemaTemplate.redactionMustRemove ?? []).some((rule) => rule.includes("cookies")),
  "Live artifact schema template must include cookie redaction",
);

console.log(
  `SEO runtime fixture evidence OK: ${fallbackRows.length} fallback cases, `
    + `${routeRows.length} route rows, ${smokeRows.length} smoke routes, `
    + `${matrix.length} D8 gates, ${docsRows.length} docs rows, `
    + `${signoffRows.length} sign-off rows, `
    + `${staticAssertions.length} static assertions, `
    + `${fixtures.rbacModuleGatingMatrix.length} RBAC rows, `
    + `${fixtures.replayIndexInvariantMatrix.length} replay/index rows, `
    + `${fixtures.hostRuntimeEntrypointMatrix.length} host entrypoint rows, `
    + `${fixtures.semanticErrorParityMatrix.length} semantic-error rows, `
    + `${fixtures.liveEvidenceCaptureTemplate.commands.length} live evidence commands, `
    + `${fixtures.incidentEvidenceTemplates.length} incident templates, `
    + `${fixtures.ownerCloseoutCriteria.length} owner closeout rows, `
    + `${unitCoverageInventory.length} unit inventory rows, `
    + `${integrationMatrixPlan.length} integration plan rows, `
    + `${liveEvidenceArtifactTemplates.length} live artifact templates`,
);
