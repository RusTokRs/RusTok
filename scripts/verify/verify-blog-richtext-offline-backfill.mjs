import fs from "node:fs";

function read(path) { return fs.readFileSync(path, "utf8"); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-blog-richtext-offline-backfill] ${message}`); process.exit(1); }
function hasAll(text, markers, label) {
  for (const marker of markers) if (!text.includes(marker)) fail(`${label} missing ${marker}`);
}
function hasNone(text, markers, label) {
  for (const marker of markers) if (text.includes(marker)) fail(`${label} contains forbidden ${marker}`);
}

const sourcePath = "crates/rustok-blog/src/bin/blog_article_richtext_backfill.rs";
const evidencePath = "crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json";
const inventoryPath = "crates/rustok-blog/contracts/evidence/blog-richtext-cutover-inventory.json";
const planPath = "crates/rustok-blog/docs/implementation-plan.md";
const inventoryDocPath = "crates/rustok-blog/docs/richtext-cutover-inventory.md";

const source = read(sourcePath);
const evidence = json(evidencePath);
const inventory = json(inventoryPath);
const plan = read(planPath);
const inventoryDoc = read(inventoryDocPath);
const packageJson = json("package.json");

if (evidence.schema_version !== 1 || evidence.module !== "blog" || evidence.surface !== "article_richtext_offline_backfill") {
  fail("evidence identity drift");
}
if (evidence.status !== "executable_no_run" || evidence.compile_policy !== "not_run_by_request") {
  fail("evidence status drift");
}
if (evidence.runner !== "cargo run -p rustok-blog --bin blog_article_richtext_backfill --") {
  fail("runner drift");
}
if (evidence.source !== sourcePath || evidence.verifier !== "scripts/verify/verify-blog-richtext-offline-backfill.mjs") {
  fail("source/verifier path drift");
}
if (evidence.safety?.default_mode !== "dry_run"
  || evidence.safety?.apply_flag !== "--apply"
  || evidence.safety?.markdown_plain_text_flag !== "--allow-markdown-plain-text"
  || evidence.safety?.preflight_before_apply !== true
  || evidence.safety?.optimistic_updates !== true
  || evidence.safety?.checkpoint_mutation !== false) {
  fail("safety contract drift");
}

hasAll(source, [
  "const TARGET_FORMAT: &str = \"richtext\";",
  "async fn preflight_pass(",
  "async fn apply_pass(",
  "async fn optimistic_update(",
  "--apply",
  "--allow-markdown-plain-text",
  "article_document_from_plain_text",
  "normalize_article",
  "canonical_article_body",
  "full successful preflight",
  "optimistic update conflict",
  "post-apply verification failed",
  "ReportRecord",
  "body_format = $2",
  "body_format = ?",
], "backfill source");
hasNone(source, [
  "rustok_content::entities",
  "validate_and_sanitize_rt_json",
  "persist_checkpoint",
  "checkpoint_file",
  "CONTENT_FORMAT_MARKDOWN",
], "backfill source");

const check = inventory.checks?.find((entry) => entry.name === "offline_backfill");
if (!check || check.status !== "executable_no_run" || check.path !== sourcePath) {
  fail("inventory offline_backfill check drift");
}
if (!inventory.completion_conditions?.includes("legacy_rows_have_owner_specific_dry_run_backfill")) {
  fail("inventory completion condition missing");
}

hasAll(plan, [sourcePath, evidencePath, "--allow-markdown-plain-text", "offline backfill"], "implementation plan");
hasAll(inventoryDoc, [sourcePath, evidencePath, "Dry-run is the default", "optimistic"], "inventory documentation");

if (packageJson.scripts?.["verify:blog:richtext-offline-backfill"] !== "node scripts/verify/verify-blog-richtext-offline-backfill.mjs") {
  fail("package verifier command drift");
}
if (!packageJson.scripts?.["verify:blog:fba"]?.includes("verify:blog:richtext-offline-backfill")) {
  fail("Blog FBA aggregate does not include offline backfill verifier");
}

console.log("[verify-blog-richtext-offline-backfill] owner-specific dry-run/apply safety contract is consistent");
