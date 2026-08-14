import fs from "node:fs";
import path from "node:path";

const root = path.resolve(
  process.env.RUSTOK_TAXONOMY_RECOVERY_ROOT || process.cwd(),
);
const failures = [];
const sqlPath = "crates/rustok-taxonomy/docs/sql/route-registry-drift.sql";
const runbookPath = "crates/rustok-taxonomy/docs/route-registry-recovery.md";

function read(relative) {
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    failures.push(`missing recovery contract artifact: ${relative}`);
    return "";
  }
  return fs.readFileSync(absolute, "utf8");
}

function requireMarkers(label, source, markers) {
  for (const marker of markers) {
    if (!source.includes(marker)) {
      failures.push(`${label}: missing recovery marker: ${marker}`);
    }
  }
}

const sql = read(sqlPath);
const runbook = read(runbookPath);

requireMarkers(sqlPath, sql, [
  "taxonomy_term_translations",
  "taxonomy_term_aliases",
  "taxonomy_term_route_keys",
  "localized_routes",
  "localized_state",
  "stale_registry",
  "LEFT JOIN taxonomy_term_route_keys",
  "NOT EXISTS",
  ":'tenant_id'::uuid",
  "missing_reservation",
  "cross_term_collision",
  "stale_reservation",
  "consistent",
]);

const sqlWithoutComments = sql
  .replace(/\/\*[\s\S]*?\*\//g, "")
  .replace(/^\s*--.*$/gm, "")
  .trim();

if (!/^WITH\b/i.test(sqlWithoutComments)) {
  failures.push(`${sqlPath}: diagnostic must be a read-only WITH/SELECT statement`);
}

const forbiddenMutation =
  /\b(?:INSERT|UPDATE|DELETE|MERGE|CREATE|ALTER|DROP|TRUNCATE|GRANT|REVOKE|CALL|COPY)\b/i;
if (forbiddenMutation.test(sqlWithoutComments)) {
  failures.push(`${sqlPath}: operational diagnostic must remain read-only`);
}

const statementTerminators = sqlWithoutComments.match(/;/g)?.length || 0;
if (statementTerminators !== 1 || !sqlWithoutComments.endsWith(";")) {
  failures.push(`${sqlPath}: diagnostic must contain exactly one SQL statement`);
}

requireMarkers(runbookPath, runbook, [
  sqlPath,
  "stale_reservation",
  "missing_reservation",
  "cross_term_collision",
  "TaxonomyService::update_term",
  "Do **not** insert, update, or delete",
  "blog_post_tags",
  "forum_topic_tags",
  "product_tags",
  "profile_tags",
]);

if (!runbook.includes("--set=tenant_id")) {
  failures.push(`${runbookPath}: missing parameterized psql invocation`);
}

if (failures.length > 0) {
  console.error("Taxonomy route-registry recovery contract failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Taxonomy route-registry recovery contract passed: read-only tenant-scoped diagnostic covers missing, cross-term, stale, and consistent route states.",
);
