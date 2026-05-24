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
  "docs/index.md",
];

const requiredPlanHeadings = [
  "Фазы реализации",
  "Принцип исполнения backlog (одна задача за итерацию)",
  "Политика актуализации verification scripts",
  "Phase-gate критерии (обязательные переходы между фазами)",
  "KPI parity (измеримые пороги)",
  "RACI (кто принимает phase-gates)",
];

const requiredChecklistChecks = [
  {
    label: "native path checklist item",
    pattern: /- \[[ xX]\] Native path \(Leptos SSR\/hydrate\) работает для целевого сценария\./,
  },
  {
    label: "graphql fallback checklist item",
    pattern: /- \[[ xX]\] GraphQL fallback работает для того же сценария\./,
  },
  {
    label: "headless host path checklist item",
    pattern: /- \[[ xX]\] Headless host path \(Next\/mobile\/external\) не сломан\./,
  },
  {
    label: "graphql-rest contract guard checklist item",
    pattern: /- \[[ xX]\] GraphQL\/REST surface не удалён и не ослаблен\./,
  },
  {
    label: "ui/business ownership checklist item",
    pattern: /- \[[ xX]\] UI слой не владеет transport\/business логикой\./,
  },
  {
    label: "transport-through-core checklist item",
    pattern: /- \[[ xX]\] Доступ к transport идёт через core ports\./,
  },
  {
    label: "core-leptos-independence checklist item",
    pattern: /- \[[ xX]\] Core слой не зависит от `leptos\*`\./,
  },
  {
    label: "ffa verify evidence checklist item",
    pattern: /- \[[ xX]\] Выполнен `npm run verify:ffa:ui:migration`\./,
  },
];

const requiredConnectivityMentions = ["rustok-pages", "rustok-search"];

const requiredIndexRefs = [
  "dioxus-ffa-ui-migration-plan.md",
  "dioxus-ffa-pilot-connectivity-map.md",
  "ffa-ui-parity-checklist.md",
];

const requiredKpiMentions = [
  "Функциональный parity",
  "Error parity",
  "Performance guard",
  "Contract guard",
  "Docs guard",
];

const packageJsonPath = "package.json";

const requiredNpmScripts = [
  "verify:ffa:ui:migration",
  "verify:ffa:ui:migration:contract",
  "verify:ffa:ui:migration:docs",
];

const requiredMigrationPipelineCommands = [
  "npm run verify:ffa:ui:migration:contract",
  "npm run verify:ffa:ui:migration:docs",
];

function assertFileExists(relPath) {
  const fullPath = path.join(repoRoot, relPath);
  if (!existsSync(fullPath)) {
    throw new Error(`Отсутствует обязательный документ: ${relPath}`);
  }
  return fullPath;
}

function normalizeMarkdown(content) {
  return content.replace(/\r\n/g, "\n").replace(/[ \t]+$/gm, "");
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function stripCodeFences(content) {
  return content.replace(/```[\s\S]*?```/g, "");
}

function stripHtmlComments(content) {
  return content.replace(/<!--[\s\S]*?-->/g, "");
}


function getMarkdownHeadings(content) {
  return content
    .split("\n")
    .map((line, index) => {
      const match = line.match(/^#{1,6}\s+(.*)$/);
      return match ? { text: match[1].trim(), line: index + 1 } : null;
    })
    .filter(Boolean);
}

function readRequiredDocs() {
  const [planPath, connectivityPath, checklistPath, docsIndexPath] = requiredDocs.map(assertFileExists);

  return {
    plan: normalizeMarkdown(readFileSync(planPath, "utf8")),
    connectivity: normalizeMarkdown(readFileSync(connectivityPath, "utf8")),
    checklist: normalizeMarkdown(readFileSync(checklistPath, "utf8")),
    docsIndex: normalizeMarkdown(readFileSync(docsIndexPath, "utf8")),
  };
}

function hasMarkdownLink(content, target) {
  const normalizedContent = stripHtmlComments(stripCodeFences(content));
  const escapedTarget = escapeRegExp(target);

  const inlineLinkPattern = new RegExp(`\\[[^\\]]+\\]\\([^)]*${escapedTarget}[^)]*\\)`);
  if (inlineLinkPattern.test(normalizedContent)) {
    return true;
  }

  const autoLinkPattern = new RegExp(`<[^>]*${escapedTarget}[^>]*>`);
  if (autoLinkPattern.test(normalizedContent)) {
    return true;
  }

  const referenceUsePattern = /\[[^\]]+\]\[([^\]]+)\]/g;
  const referenceDefPattern = /^\[([^\]]+)\]:\s*(<[^>]+>|\S+)(?:\s+[""][^""]+[""])?$/gm;

  const usedRefs = new Set();
  let useMatch;
  while ((useMatch = referenceUsePattern.exec(normalizedContent)) !== null) {
    usedRefs.add(useMatch[1].toLowerCase());
  }

  let defMatch;
  while ((defMatch = referenceDefPattern.exec(normalizedContent)) !== null) {
    const ref = defMatch[1].toLowerCase();
    const href = defMatch[2].replace(/^<|>$/g, "");
    if (usedRefs.has(ref) && href.includes(target)) {
      return true;
    }
  }

  return false;
}


function parsePackageJson() {
  const fullPath = path.join(repoRoot, packageJsonPath);
  if (!existsSync(fullPath)) {
    throw new Error(`Отсутствует обязательный файл: ${packageJsonPath}`);
  }

  const raw = readFileSync(fullPath, "utf8");
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(`Не удалось распарсить ${packageJsonPath}: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function collectValidationErrors({ plan, connectivity, checklist, docsIndex, packageJson }) {
  const errors = [];

  const planHeadingIndex = new Map(
    getMarkdownHeadings(plan).map((heading) => [heading.text, heading.line]),
  );

  requiredPlanHeadings.forEach((heading) => {
    if (!planHeadingIndex.has(heading)) {
      errors.push(`Не найден обязательный heading в migration plan: ${heading}`);
    }
  });

  requiredChecklistChecks.forEach(({ label, pattern }) => {
    if (!pattern.test(checklist)) {
      errors.push(`Не найден обязательный checklist-паттерн (${label}) в docs/verification/ffa-ui-parity-checklist.md`);
    }
  });

  requiredKpiMentions.forEach((kpi) => {
    if (!plan.includes(kpi)) {
      errors.push(`Не найден обязательный KPI-маркер в migration plan: ${kpi}`);
    }
  });

  const connectivityText = stripCodeFences(connectivity);
  requiredConnectivityMentions.forEach((mention) => {
    if (!connectivityText.includes(mention)) {
      errors.push(`Не найден обязательный пилот в docs/research/dioxus-ffa-pilot-connectivity-map.md: ${mention}`);
    }
  });


  const scripts = packageJson?.scripts ?? {};
  requiredNpmScripts.forEach((scriptName) => {
    if (typeof scripts[scriptName] !== "string" || scripts[scriptName].trim().length === 0) {
      errors.push(`Не найден обязательный npm script в package.json: ${scriptName}`);
    }
  });

  const migrationPipeline = scripts["verify:ffa:ui:migration"];
  if (typeof migrationPipeline === "string") {
    requiredMigrationPipelineCommands.forEach((command) => {
      if (!migrationPipeline.includes(command)) {
        errors.push(`Скрипт verify:ffa:ui:migration должен содержать команду: ${command}`);
      }
    });
  }

  requiredIndexRefs.forEach((refPath) => {
    if (!hasMarkdownLink(docsIndex, refPath)) {
      errors.push(`Не найдена обязательная markdown-ссылка в docs/index.md: ${refPath}`);
    }
  });

  return errors.sort((a, b) => a.localeCompare(b, "ru"));
}

try {
  const docs = readRequiredDocs();
  const packageJson = parsePackageJson();
  const errors = collectValidationErrors({ ...docs, packageJson });

  if (errors.length > 0) {
    console.error("[verify-ffa-ui-migration-contract] FAIL");
    errors.forEach((error) => console.error(`- ${error}`));
    process.exit(1);
  }

  console.log("[verify-ffa-ui-migration-contract] PASS");
  console.log("Проверены обязательные документы и baseline-контракты FFA migration.");
} catch (error) {
  console.error("[verify-ffa-ui-migration-contract] FAIL");
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
