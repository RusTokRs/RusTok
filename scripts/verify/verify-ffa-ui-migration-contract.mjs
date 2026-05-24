#!/usr/bin/env node

import { readFileSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

function parseCliArgs(argv) {
  let cliRoot;
  let showHelp = false;
  const unknownArgs = [];

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === "--help" || arg === "-h") {
      showHelp = true;
      continue;
    }

    if (arg.startsWith("--root=")) {
      cliRoot = arg.slice("--root=".length);
      continue;
    }

    if (arg === "--root") {
      cliRoot = argv[index + 1];
      index += 1;
      continue;
    }

    unknownArgs.push(arg);
  }

  return { cliRoot, showHelp, unknownArgs };
}

function printUsage() {
  console.log("Usage: node scripts/verify/verify-ffa-ui-migration-contract.mjs [--root <path>|--root=<path>] [-h|--help]");
}

function resolveRepoRoot(cliRoot, env) {
  if (typeof cliRoot === "string" && cliRoot.trim().length > 0) {
    return path.resolve(cliRoot);
  }

  if (env.RUSTOK_VERIFY_ROOT) {
    return path.resolve(env.RUSTOK_VERIFY_ROOT);
  }

  return path.resolve(__dirname, "../..");
}

const cli = parseCliArgs(process.argv.slice(2));
if (cli.showHelp) {
  printUsage();
  process.exit(0);
}
if (cli.unknownArgs.length > 0) {
  console.error("[verify-ffa-ui-migration-contract] FAIL");
  console.error(`Неизвестные аргументы: ${cli.unknownArgs.join(" ")}`);
  printUsage();
  process.exit(1);
}

const repoRoot = resolveRepoRoot(cli.cliRoot, process.env);

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

const requiredNpmScriptCommands = {
  "verify:ffa:ui:migration": null,
  "verify:ffa:ui:migration:contract": [
    "node scripts/verify/verify-ffa-ui-migration-contract.mjs",
  ],
  "verify:ffa:ui:migration:docs": [
    "bash scripts/verify/verify-ffa-ui-doc-patterns.sh",
    "sh scripts/verify/verify-ffa-ui-doc-patterns.sh",
  ],
};

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
  const referenceDefPattern = /^\[([^\]]+)\]:\s*(<[^>]+>|\S+)(?:\s+[""][^"]+[""])?$/gm;

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

function normalizeCommand(command) {
  return command.replace(/\s+/g, " ").trim();
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
  Object.entries(requiredNpmScriptCommands).forEach(([scriptName, expectedCommand]) => {
    const scriptValue = scripts[scriptName];
    if (typeof scriptValue !== "string" || scriptValue.trim().length === 0) {
      errors.push(`Не найден обязательный npm script в package.json: ${scriptName}`);
      return;
    }

    if (expectedCommand !== null) {
      const expectedVariants = Array.isArray(expectedCommand)
        ? expectedCommand
        : [expectedCommand];
      const actualNormalized = normalizeCommand(scriptValue);
      const matched = expectedVariants.some(
        (variant) => actualNormalized === normalizeCommand(variant),
      );

      if (!matched) {
        errors.push(
          `Скрипт ${scriptName} должен быть одним из: ${expectedVariants.join(" | ")}; фактически: ${scriptValue.trim()}`,
        );
      }
    }
  });

  const migrationPipeline = scripts["verify:ffa:ui:migration"];
  if (typeof migrationPipeline === "string") {
    const normalizedPipeline = normalizeCommand(migrationPipeline);
    requiredMigrationPipelineCommands.forEach((command) => {
      if (!normalizedPipeline.includes(normalizeCommand(command))) {
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
