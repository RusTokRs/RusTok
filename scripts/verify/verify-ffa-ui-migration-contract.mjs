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
  "docs/modules/registry.md",
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
    label: "transport-facade-core-ownership checklist item",
    pattern: /- \[[ xX]\] UI adapter обращается к transport только через module-owned facade; request\/command\/state construction и business\/policy остаются в core ports\/helpers\./,
  },
  {
    label: "core-leptos-independence checklist item",
    pattern: /- \[[ xX]\] Core слой не зависит от `leptos\*`\./,
  },
  {
    label: "transport adapter roles checklist item",
    pattern: /- \[[ xX]\] Transport adapters разделены по ролям: native и GraphQL fallback либо явно зафиксирован temporary single-adapter state с next-step parity plan\./,
  },
  {
    label: "host-visible error-status checklist item",
    pattern: /- \[[ xX]\] Host-visible UI status\/error contracts имеют stable machine-readable codes и documented locale keys\./,
  },
  {
    label: "ffa verify evidence checklist item",
    pattern: /- \[[ xX]\] Выполнен `npm run verify:ffa:ui:migration`\./,
  },
  {
    label: "error-status evidence checklist item",
    pattern: /- \[[ xX]\] Для изменённых error\/status контрактов приложен список stable codes и locale keys\./,
  },
];

const requiredConnectivityMentions = ["rustok-pages", "rustok-search"];

const requiredStructuralShapes = [
  "none",
  "docs_boundary",
  "core_only",
  "core_transport",
  "core_transport_ui",
  "no_ui_boundary",
];

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

const requiredRegionErrorStatusContracts = [
  {
    stableCode: "native_unavailable",
    localeKey: "region.error.status.nativeUnavailable",
    enumVariant: "NativeUnavailable",
  },
  {
    stableCode: "fallback_unavailable",
    localeKey: "region.error.status.fallbackUnavailable",
    enumVariant: "FallbackUnavailable",
  },
];

const requiredRegionRouteDomAttributes = [
  "data-region-route-query-key",
  "data-region-route-query-value",
];

const regionStorefrontCorePath = "crates/rustok-region/storefront/src/core.rs";
const regionStorefrontLeptosUiPath = "crates/rustok-region/storefront/src/ui/leptos.rs";
const regionStorefrontReadmePath = "crates/rustok-region/storefront/README.md";
const regionStorefrontLocalePaths = [
  "crates/rustok-region/storefront/locales/en.json",
  "crates/rustok-region/storefront/locales/ru.json",
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

function readText(relPath) {
  const fullPath = assertFileExists(relPath);
  return readFileSync(fullPath, "utf8");
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
  const [planPath, connectivityPath, checklistPath, registryPath, docsIndexPath] = requiredDocs.map(assertFileExists);

  return {
    plan: normalizeMarkdown(readFileSync(planPath, "utf8")),
    connectivity: normalizeMarkdown(readFileSync(connectivityPath, "utf8")),
    checklist: normalizeMarkdown(readFileSync(checklistPath, "utf8")),
    registry: normalizeMarkdown(readFileSync(registryPath, "utf8")),
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

function collectRegionErrorStatusContractErrors() {
  const errors = [];
  const core = readText(regionStorefrontCorePath);
  const leptosUi = readText(regionStorefrontLeptosUiPath);
  const storefrontReadme = readText(regionStorefrontReadmePath);
  const locales = regionStorefrontLocalePaths.map((localePath) => ({
    path: localePath,
    content: readText(localePath),
  }));

  if (!core.includes("RegionErrorStatusDescriptor")) {
    errors.push("Region storefront core должен содержать RegionErrorStatusDescriptor для host-visible status contract");
  }

  if (!core.includes("REGION_ERROR_STATUS_DESCRIPTORS")) {
    errors.push("Region storefront core должен содержать REGION_ERROR_STATUS_DESCRIPTORS");
  }

  ["data-region-error-status", "data-region-error-locale-key"].forEach((attributeName) => {
    if (!leptosUi.includes(attributeName)) {
      errors.push(`Region storefront Leptos error adapter должен публиковать DOM attribute: ${attributeName}`);
    }
    if (!storefrontReadme.includes(attributeName)) {
      errors.push(`Region storefront README должен документировать DOM attribute: ${attributeName}`);
    }
  });

  [
    "RegionRouteState",
    "RegionRouteSelectionUpdate",
    "SELECTED_REGION_QUERY_KEY",
  ].forEach((contractName) => {
    if (!core.includes(contractName)) {
      errors.push(`Region storefront core должен содержать route/query contract: ${contractName}`);
    }
    if (!storefrontReadme.includes(contractName)) {
      errors.push(`Region storefront README должен документировать route/query contract: ${contractName}`);
    }
  });

  requiredRegionRouteDomAttributes.forEach((attributeName) => {
    if (!leptosUi.includes(attributeName)) {
      errors.push(`Region storefront Leptos route adapter должен публиковать DOM attribute: ${attributeName}`);
    }
    if (!storefrontReadme.includes(attributeName)) {
      errors.push(`Region storefront README должен документировать route DOM attribute: ${attributeName}`);
    }
  });

  requiredRegionErrorStatusContracts.forEach(({ stableCode, localeKey, enumVariant }) => {
    if (!core.includes(`RegionErrorStatusCode::${enumVariant}`)) {
      errors.push(`Region storefront core не содержит status enum variant: ${enumVariant}`);
    }
    if (!core.includes(`stable_code: "${stableCode}"`)) {
      errors.push(`Region storefront core не содержит stable status code: ${stableCode}`);
    }
    if (!core.includes(`locale_key: "${localeKey}"`)) {
      errors.push(`Region storefront core не содержит locale key mapping для ${stableCode}: ${localeKey}`);
    }
    if (!storefrontReadme.includes(stableCode) || !storefrontReadme.includes(localeKey)) {
      errors.push(`Region storefront README должен документировать status contract: ${stableCode} -> ${localeKey}`);
    }

    locales.forEach(({ path: localePath, content }) => {
      if (!content.includes(`"${localeKey}"`)) {
        errors.push(`${localePath} должен содержать locale key для ${stableCode}: ${localeKey}`);
      }
    });
  });

  return errors;
}

function collectStructuralShapeErrors(registry) {
  const errors = [];

  if (!registry.includes("Structural shape фиксирует")) {
    errors.push("docs/modules/registry.md должен описывать Structural shape для FFA/FBA board");
  }

  if (!registry.includes("| Module slug | UI surfaces | FFA status | FBA status | Structural shape | Source plan |")) {
    errors.push("docs/modules/registry.md FFA/FBA board должен содержать колонку Structural shape");
  }

  requiredStructuralShapes.forEach((shape) => {
    if (!registry.includes(`\`${shape}\``)) {
      errors.push(`docs/modules/registry.md должен документировать Structural shape: ${shape}`);
    }
  });

  return errors;
}


function parseRegistryModuleRows(registry) {
  return registry
    .split("\n")
    .filter((line) => line.startsWith("| `") && line.includes("docs/implementation-plan.md"))
    .map((line) => {
      const columns = line.split("|").map((column) => column.trim());
      const sourcePlanCell = columns[6] ?? "";
      const sourcePlanMatch = sourcePlanCell.match(/(crates\/[^`) ]+\/docs\/implementation-plan\.md)/);

      return {
        moduleSlug: columns[1]?.replace(/`/g, "") ?? "<unknown>",
        structuralShape: columns[5]?.replace(/`/g, ""),
        sourcePlanPath: sourcePlanMatch?.[1],
      };
    });
}

function collectRegistryLocalShapeErrors(registry) {
  const errors = [];

  parseRegistryModuleRows(registry).forEach(({ moduleSlug, structuralShape, sourcePlanPath }) => {
    if (!requiredStructuralShapes.includes(structuralShape)) {
      errors.push(`FFA/FBA board содержит неизвестный Structural shape для ${moduleSlug}: ${structuralShape}`);
      return;
    }

    if (!sourcePlanPath) {
      errors.push(`FFA/FBA board не содержит source implementation plan path для ${moduleSlug}`);
      return;
    }

    const sourcePlan = readText(sourcePlanPath);
    const expectedShapeLine = `- Structural shape: \`${structuralShape}\``;
    if (!sourcePlan.includes(expectedShapeLine)) {
      errors.push(`${sourcePlanPath} должен содержать строку локального статуса: ${expectedShapeLine}`);
    }
  });

  return errors;
}

function moduleSurfacePaths(moduleRoot, surface, candidates) {
  return candidates.map((candidate) => path.join(moduleRoot, surface, "src", candidate));
}

function hasAnyPath(paths) {
  return paths.some((candidate) => existsSync(candidate));
}

function collectStructuralShapeFilesystemErrors(registry) {
  const errors = [];

  parseRegistryModuleRows(registry).forEach(({ moduleSlug, structuralShape, sourcePlanPath }) => {
    if (!sourcePlanPath || ["none", "docs_boundary", "no_ui_boundary"].includes(structuralShape)) {
      return;
    }

    const moduleRoot = path.join(repoRoot, path.dirname(path.dirname(sourcePlanPath)));
    const surfaces = ["admin", "storefront"];
    const corePaths = surfaces.flatMap((surface) => moduleSurfacePaths(moduleRoot, surface, ["core.rs", "core"]));
    const transportPaths = surfaces.flatMap((surface) => moduleSurfacePaths(moduleRoot, surface, ["transport.rs", "transport"]));
    const uiPaths = surfaces.flatMap((surface) => moduleSurfacePaths(moduleRoot, surface, ["ui", path.join("ui", "leptos.rs"), path.join("ui", "leptos")]));

    const hasCore = hasAnyPath(corePaths);
    const hasTransport = hasAnyPath(transportPaths);
    const hasUi = hasAnyPath(uiPaths);

    if (["core_only", "core_transport", "core_transport_ui"].includes(structuralShape) && !hasCore) {
      errors.push(`${moduleSlug}: Structural shape ${structuralShape} требует core.rs или core/ в admin/storefront src`);
    }
    if (["core_transport", "core_transport_ui"].includes(structuralShape) && !hasTransport) {
      errors.push(`${moduleSlug}: Structural shape ${structuralShape} требует transport.rs или transport/ в admin/storefront src`);
    }
    if (structuralShape === "core_transport_ui" && !hasUi) {
      errors.push(`${moduleSlug}: Structural shape ${structuralShape} требует ui/leptos.rs или ui/leptos/ adapter в admin/storefront src`);
    }
  });

  return errors;
}

function collectValidationErrors({ plan, connectivity, checklist, registry, docsIndex, packageJson }) {
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

  errors.push(...collectStructuralShapeErrors(registry));
  errors.push(...collectRegistryLocalShapeErrors(registry));
  errors.push(...collectStructuralShapeFilesystemErrors(registry));
  errors.push(...collectRegionErrorStatusContractErrors());

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
