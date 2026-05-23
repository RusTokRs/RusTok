#!/usr/bin/env node

import { readFileSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "../..");

const requiredDocs = [
  "docs/research/dioxus-ffa-ui-migration-plan.md",
  "docs/research/dioxus-ffa-pilot-connectivity-map.md",
  "docs/verification/ffa-ui-parity-checklist.md",
];

const requiredPlanSections = [
  "## Фазы реализации",
  "## Принцип исполнения backlog (одна задача за итерацию)",
  "## Политика актуализации verification scripts",
  "## Phase-gate критерии (обязательные переходы между фазами)",
  "## KPI parity (измеримые пороги)",
];

const requiredChecklistItems = [
  "- [ ] Native path (Leptos SSR/hydrate) работает для целевого сценария.",
  "- [ ] GraphQL fallback работает для того же сценария.",
  "- [ ] UI слой не владеет transport/business логикой.",
  "- [ ] Доступ к transport идёт через core ports.",
  "- [ ] Core слой не зависит от `leptos*`.",
];

function assertFileExists(relPath) {
  const fullPath = path.join(repoRoot, relPath);
  if (!existsSync(fullPath)) {
    throw new Error(`Отсутствует обязательный документ: ${relPath}`);
  }
  return fullPath;
}

function assertContains(content, value, label) {
  if (!content.includes(value)) {
    throw new Error(`Не найден обязательный фрагмент (${label}): ${value}`);
  }
}

try {
  const planPath = assertFileExists(requiredDocs[0]);
  const connectivityPath = assertFileExists(requiredDocs[1]);
  const checklistPath = assertFileExists(requiredDocs[2]);

  const plan = readFileSync(planPath, "utf8");
  const checklist = readFileSync(checklistPath, "utf8");
  const connectivity = readFileSync(connectivityPath, "utf8");

  requiredPlanSections.forEach((section) => {
    assertContains(plan, section, "plan section");
  });

  requiredChecklistItems.forEach((item) => {
    assertContains(checklist, item, "checklist item");
  });

  assertContains(
    connectivity,
    "rustok-pages",
    "pilot connectivity map should include rustok-pages",
  );
  assertContains(
    connectivity,
    "rustok-search",
    "pilot connectivity map should include rustok-search",
  );

  console.log("[verify-ffa-ui-migration-contract] PASS");
  console.log("Проверены обязательные документы и baseline-контракты FFA migration.");
} catch (error) {
  console.error("[verify-ffa-ui-migration-contract] FAIL");
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
