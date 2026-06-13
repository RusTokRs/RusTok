import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const fixturePath = join(here, "..", "contracts", "seo", "runtime-parity-fixtures.json");
const fixtures = JSON.parse(readFileSync(fixturePath, "utf8"));

const requiredFallbackCases = new Set([
  "module_disabled",
  "not_found",
  "permission_denied",
  "transport_failure",
]);
const fallbackCases = new Set(fixtures.fallbackBehavior?.map((item) => item.case));
for (const requiredCase of requiredFallbackCases) {
  if (!fallbackCases.has(requiredCase)) {
    throw new Error(`Missing fallback fixture case: ${requiredCase}`);
  }
}

const routeRows = fixtures.routeOwnership ?? [];
for (const targetKind of ["page", "product", "blog_post", "forum_topic"]) {
  if (!routeRows.some((row) => row.targetKind === targetKind)) {
    throw new Error(`Missing route ownership target kind: ${targetKind}`);
  }
}

const smokeRoutes = new Set((fixtures.smokeEvidence ?? []).map((item) => item.route));
for (const route of [
  "/modules/product?slug=demo-product",
  "/modules/blog?slug=release-notes",
]) {
  if (!smokeRoutes.has(route)) {
    throw new Error(`Missing non-home metadata smoke route: ${route}`);
  }
}

if ((fixtures.longTailDiffAllowlist ?? []).length < 3) {
  throw new Error("Expected explicit long-tail metadata diff allowlist entries");
}

console.log(
  `SEO runtime fixture evidence OK: ${fixtures.fallbackBehavior.length} fallback cases, ${routeRows.length} route rows, ${fixtures.smokeEvidence.length} smoke routes`,
);
