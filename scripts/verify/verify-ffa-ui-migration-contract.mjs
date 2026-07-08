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
  console.error(`Unknown arguments: ${cli.unknownArgs.join(" ")}`);
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
  "Implementation Phases",
  "Backlog Execution Principle (One Task Per Iteration)",
  "Standard for minimal FFA slice and anti-over-extraction",
  "Verification Script Update Policy",
  "Phase-Gate Criteria (mandatory transitions between phases)",
  "KPI Parity (measurable thresholds)",
  "RACI (who approves phase-gates)",
];

const requiredChecklistChecks = [
  {
    label: "native path checklist item",
    pattern: /- \[[ xX]\] Native path \(Leptos SSR\/hydrate\) works for the target scenario\./,
  },
  {
    label: "graphql selected path checklist item",
    pattern: /- \[[ xX]\] GraphQL selected path works for the same scenario\./,
  },
  {
    label: "headless host path checklist item",
    pattern: /- \[[ xX]\] Headless host path \(Next\/mobile\/external\) is not broken\./,
  },
  {
    label: "graphql-rest contract guard checklist item",
    pattern: /- \[[ xX]\] GraphQL\/REST surface is not removed or weakened\./,
  },
  {
    label: "ui/business ownership checklist item",
    pattern: /- \[[ xX]\] UI layer does not own transport\/business logic\./,
  },
  {
    label: "transport-facade-core-ownership checklist item",
    pattern: /- \[[ xX]\] UI adapter accesses transport only through module-owned facade; request\/command\/state construction and business\/policy remain in core ports\/helpers\./,
  },
  {
    label: "core-leptos-independence checklist item",
    pattern: /- \[[ xX]\] Core layer does not depend on `leptos\*`\./,
  },
  {
    label: "transport adapter roles checklist item",
    pattern: /- \[[ xX]\] Transport adapters are separated by role: native and GraphQL selected path, or a temporary single-adapter state with a next-step parity plan is explicitly documented\./,
  },
  {
    label: "host-visible error-status checklist item",
    pattern: /- \[[ xX]\] Host-visible UI status\/error contracts have stable machine-readable codes and documented locale keys\./,
  },
  {
    label: "ffa verify evidence checklist item",
    pattern: /- \[[ xX]\] `npm run verify:ffa:ui:migration` executed\./,
  },
  {
    label: "error-status evidence checklist item",
    pattern: /- \[[ xX]\] For changed error\/status contracts, a list of stable codes and locale keys is attached\./,
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


const requiredAntiOverExtractionPlanMarkers = [
  "An FFA slice should reduce coupling",
  "request/command construction, normalization and validation",
  "simple i18n label bindings",
  "reset/refresh side effects after mutation",
  "mechanical wrappers over a single formatting line",
  "If a change adds more boilerplate than it removes coupling",
  "if over-extraction is detected, revert it in the same iteration",
];

const requiredKpiMentions = [
  "Functional parity",
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
    stableCode: "graphql_unavailable",
    localeKey: "region.error.status.graphqlUnavailable",
    enumVariant: "GraphqlUnavailable",
  },
];

const requiredRegionRouteDomAttributes = [
  "data-region-route-query-key",
  "data-region-route-query-value",
];

const pagesStorefrontRootPath = "crates/rustok-pages/storefront/src/lib.rs";
const pagesStorefrontLeptosUiPath = "crates/rustok-pages/storefront/src/ui/leptos.rs";
const pagesStorefrontReadmePath = "crates/rustok-pages/storefront/README.md";

const regionStorefrontCorePath = "crates/rustok-region/storefront/src/core.rs";
const regionStorefrontLeptosUiPath = "crates/rustok-region/storefront/src/ui/leptos.rs";
const regionStorefrontReadmePath = "crates/rustok-region/storefront/README.md";
const regionStorefrontLocalePaths = [
  "crates/rustok-region/storefront/locales/en.json",
  "crates/rustok-region/storefront/locales/ru.json",
];


const productAdminCorePath = "crates/rustok-product/admin/src/core.rs";
const productAdminLeptosUiPath = "crates/rustok-product/admin/src/ui/leptos.rs";
const customerAdminRootPath = "crates/rustok-customer/admin/src/lib.rs";
const customerAdminCorePath = "crates/rustok-customer/admin/src/core.rs";
const customerAdminLegacyApiPath = "crates/rustok-customer/admin/src/api.rs";
const customerAdminTransportPath = "crates/rustok-customer/admin/src/transport/mod.rs";
const customerAdminNativeAdapterPath = "crates/rustok-customer/admin/src/transport/native_server_adapter.rs";
const customerAdminLeptosUiPath = "crates/rustok-customer/admin/src/ui/leptos.rs";
const customerAdminReadmePath = "crates/rustok-customer/admin/README.md";

const productAdminReadmePath = "crates/rustok-product/admin/README.md";
const productStorefrontCorePath = "crates/rustok-product/storefront/src/core.rs";
const productStorefrontTransportPath = "crates/rustok-product/storefront/src/transport/mod.rs";
const productStorefrontLeptosUiPath = "crates/rustok-product/storefront/src/ui/leptos.rs";
const productStorefrontReadmePath = "crates/rustok-product/storefront/README.md";
const requiredProductTransportDomAttributes = [
  "data-product-transport-failed-path",
  "data-product-transport-fallback-attempted",
  "data-product-transport-native-error",
  "data-product-transport-graphql-error",
];

const packageJsonPath = "package.json";

const requiredNpmScriptCommands = {
  "verify:ffa:ui:migration": null,
  "verify:ffa:ui:migration:contract": [
    "node scripts/verify/verify-ffa-ui-migration-contract.mjs",
  ],
  "verify:ffa:ui:migration:docs": [
    "node scripts/verify/verify-ffa-ui-doc-patterns.mjs",
    "bash scripts/verify/verify-ffa-ui-doc-patterns.sh",
    "sh scripts/verify/verify-ffa-ui-doc-patterns.sh",
  ],
  "verify:ffa:ui:migration:boundary-sweep": [
    "node scripts/verify/verify-ffa-ui-boundary-sweep.mjs",
  ],
  "verify:ffa:ui:migration:transport-profile": [
    "node scripts/verify/verify-ffa-ui-transport-profile-sweep.mjs",
  ],
  "verify:channel:admin-boundary": [
    "node scripts/verify/verify-channel-admin-boundary.mjs",
  ],
  "verify:ai:admin-boundary": [
    "node scripts/verify/verify-ai-admin-boundary.mjs",
  ],
  "verify:tenant:admin-boundary": [
    "node scripts/verify/verify-tenant-admin-boundary.mjs",
  ],
};

const requiredMigrationPipelineCommands = [
  "npm run verify:ffa:ui:migration:contract",
  "npm run verify:ffa:ui:migration:docs",
  "npm run verify:ffa:ui:migration:boundary-sweep",
  "npm run verify:ffa:ui:migration:transport-profile",
  "npm run verify:channel:admin-boundary",
  "npm run verify:ai:admin-boundary",
  "npm run verify:tenant:admin-boundary",
];

function assertFileExists(relPath) {
  const fullPath = path.join(repoRoot, relPath);
  if (!existsSync(fullPath)) {
    throw new Error(`Missing required document: ${relPath}`);
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
  const referenceDefPattern = /^\[([^\]]+)\]:\s*(<[^>]+>|\S+)(?:\s+"[^"]+")?$/gm;

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
    throw new Error(`Missing required file: ${packageJsonPath}`);
  }

  const raw = readFileSync(fullPath, "utf8");
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(`Failed to parse ${packageJsonPath}: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function normalizeCommand(command) {
  return command.replace(/\s+/g, " ").trim();
}

function collectPagesStorefrontUiSplitErrors() {
  const errors = [];
  const root = readText(pagesStorefrontRootPath);
  const leptosUi = readText(pagesStorefrontLeptosUiPath);
  const storefrontReadme = readText(pagesStorefrontReadmePath);

  ["mod core;", "mod transport;", "mod ui;", "pub use ui::leptos::PagesView;"].forEach((requiredSnippet) => {
    if (!root.includes(requiredSnippet)) {
      errors.push(`Pages storefront crate root must wire/re-export FFA module snippet: ${requiredSnippet}`);
    }
  });

  ["#[component]", "Resource::new_blocking", "transport::fetch_pages"].forEach((requiredSnippet) => {
    if (!leptosUi.includes(requiredSnippet)) {
      errors.push(`Pages storefront Leptos adapter must contain render/bind snippet: ${requiredSnippet}`);
    }
  });

  ["src/ui/leptos.rs", "core.rs", "transport.rs"].forEach((requiredSnippet) => {
    if (!storefrontReadme.includes(requiredSnippet)) {
      errors.push(`Pages storefront README must document FFA split snippet: ${requiredSnippet}`);
    }
  });

  if (root.includes("use leptos") || root.includes("#[component]") || root.includes("Resource::new_blocking")) {
    errors.push("Pages storefront crate root must not contain Leptos render/runtime code after ui/leptos split");
  }

  return errors;
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
    errors.push("Region storefront core must contain RegionErrorStatusDescriptor for the host-visible status contract");
  }

  if (!core.includes("REGION_ERROR_STATUS_DESCRIPTORS")) {
    errors.push("Region storefront core must contain REGION_ERROR_STATUS_DESCRIPTORS");
  }

  ["data-region-error-status", "data-region-error-locale-key"].forEach((attributeName) => {
    if (!leptosUi.includes(attributeName)) {
      errors.push(`Region storefront Leptos error adapter must publish DOM attribute: ${attributeName}`);
    }
    if (!storefrontReadme.includes(attributeName)) {
      errors.push(`Region storefront README must document DOM attribute: ${attributeName}`);
    }
  });

  [
    "RegionRouteState",
    "RegionRouteSelectionUpdate",
    "SELECTED_REGION_QUERY_KEY",
  ].forEach((contractName) => {
    if (!core.includes(contractName)) {
      errors.push(`Region storefront core must contain route/query contract: ${contractName}`);
    }
    if (!storefrontReadme.includes(contractName)) {
      errors.push(`Region storefront README must document route/query contract: ${contractName}`);
    }
  });

  requiredRegionRouteDomAttributes.forEach((attributeName) => {
    if (!leptosUi.includes(attributeName)) {
      errors.push(`Region storefront Leptos route adapter must publish DOM attribute: ${attributeName}`);
    }
    if (!storefrontReadme.includes(attributeName)) {
      errors.push(`Region storefront README must document route DOM attribute: ${attributeName}`);
    }
  });

  requiredRegionErrorStatusContracts.forEach(({ stableCode, localeKey, enumVariant }) => {
    if (!core.includes(`RegionErrorStatusCode::${enumVariant}`)) {
      errors.push(`Region storefront core is missing status enum variant: ${enumVariant}`);
    }
    if (!core.includes(`stable_code: "${stableCode}"`)) {
      errors.push(`Region storefront core is missing stable status code: ${stableCode}`);
    }
    if (!core.includes(`locale_key: "${localeKey}"`)) {
      errors.push(`Region storefront core is missing locale key mapping for ${stableCode}: ${localeKey}`);
    }
    if (!storefrontReadme.includes(stableCode) || !storefrontReadme.includes(localeKey)) {
      errors.push(`Region storefront README must document status contract: ${stableCode} -> ${localeKey}`);
    }

    locales.forEach(({ path: localePath, content }) => {
      if (!content.includes(`"${localeKey}"`)) {
        errors.push(`${localePath} must contain locale key for ${stableCode}: ${localeKey}`);
      }
    });
  });

  return errors;
}

function collectProductTransportEvidenceContractErrors() {
  const errors = [];
  const core = readText(productStorefrontCorePath);
  const transport = readText(productStorefrontTransportPath);
  const leptosUi = readText(productStorefrontLeptosUiPath);
  const storefrontReadme = readText(productStorefrontReadmePath);

  [
    "ProductTransportError",
    "UiTransportPath",
    "fallback_attempted",
    "native_error",
    "graphql_error",
  ].forEach((contractName) => {
    if (!transport.includes(contractName)) {
      errors.push(`Product storefront transport must contain fallback evidence contract: ${contractName}`);
    }
  });

  [
    "NativeServer",
    "Graphql",
    "native_server",
    "graphql",
  ].forEach((contractName) => {
    if (!transport.includes(contractName)) {
      errors.push(`Product storefront transport must contain stable transport path marker: ${contractName}`);
    }
  });

  requiredProductTransportDomAttributes.forEach((attributeName) => {
    if (!leptosUi.includes(attributeName)) {
      errors.push(`Product storefront Leptos error adapter must publish DOM attribute: ${attributeName}`);
    }
  });

  [
    "ProductTransportErrorDomEvidence",
    "build_transport_error_dom_evidence",
  ].forEach((contractName) => {
    if (!core.includes(contractName)) {
      errors.push(`Product storefront core must contain DOM evidence builder contract: ${contractName}`);
    }
  });

  if (!leptosUi.includes("build_transport_error_dom_evidence")) {
    errors.push("Product storefront Leptos error adapter must use the core-owned transport DOM evidence builder");
  }

  ["ProductTransportError", "ProductTransportErrorDomEvidence", "data-product-transport-*"].forEach((requiredSnippet) => {
    if (!storefrontReadme.includes(requiredSnippet)) {
      errors.push(`Product storefront README must document transport evidence snippet: ${requiredSnippet}`);
    }
  });

  return errors;
}

function collectCustomerAdminNativeAdapterSplitErrors() {
  const errors = [];
  const root = readText(customerAdminRootPath);
  const core = readText(customerAdminCorePath);
  const transport = readText(customerAdminTransportPath);
  const nativeAdapter = readText(customerAdminNativeAdapterPath);
  const leptosUi = readText(customerAdminLeptosUiPath);
  const adminReadme = readText(customerAdminReadmePath);

  if (existsSync(path.join(repoRoot, customerAdminLegacyApiPath))) {
    errors.push("Customer admin legacy api.rs must stay removed after native_server_adapter split");
  }

  if (root.includes("mod api;")) {
    errors.push("Customer admin crate root must not wire the legacy api module after transport/native split");
  }

  [
    "CustomerAdminDraftInput",
    "CustomerAdminSubmitCommand",
    "CustomerAdminSubmitCommandError",
    "build_customer_admin_submit_command",
    "EmailRequired",
    "LocaleUnavailable",
  ].forEach((requiredSnippet) => {
    if (!core.includes(requiredSnippet)) {
      errors.push(`Customer admin core must contain submit-command policy snippet: ${requiredSnippet}`);
    }
  });

  [
    "HostRuntimeContext",
    "runtime_ctx.db_clone()",
  ].forEach((requiredSnippet) => {
    if (!nativeAdapter.includes(requiredSnippet)) {
      errors.push(`Customer admin native adapter must contain Loco-free runtime snippet: ${requiredSnippet}`);
    }
  });

  if (nativeAdapter.includes("loco_rs")) {
    errors.push("Customer admin native adapter must not depend on Loco runtime context");
  }

  const customerAdminCargo = readText("crates/rustok-customer/admin/Cargo.toml");
  if (customerAdminCargo.includes("loco-rs")) {
    errors.push("Customer admin Cargo.toml must not depend on Loco");
  }
  [
    "mod native_server_adapter;",
    "pub use native_server_adapter::ApiError;",
    "use native_server_adapter as native;",
    "native::fetch_bootstrap().await",
    "native::fetch_customers(search, page, per_page).await",
    "native::fetch_customer_detail(customer_id).await",
    "native::create_customer(payload).await",
    "native::update_customer(customer_id, payload).await",
  ].forEach((requiredSnippet) => {
    if (!transport.includes(requiredSnippet)) {
      errors.push(`Customer admin transport facade must contain native adapter split snippet: ${requiredSnippet}`);
    }
  });

  [
    "#[server(prefix = \"/api/fn\", endpoint = \"customer/bootstrap\")]",
    "#[server(prefix = \"/api/fn\", endpoint = \"customer/list\")]",
    "#[server(prefix = \"/api/fn\", endpoint = \"customer/detail\")]",
    "#[server(prefix = \"/api/fn\", endpoint = \"customer/create\")]",
    "#[server(prefix = \"/api/fn\", endpoint = \"customer/update\")]",
  ].forEach((requiredSnippet) => {
    if (!nativeAdapter.includes(requiredSnippet)) {
      errors.push(`Customer admin native adapter must own server function endpoint: ${requiredSnippet}`);
    }
  });

  if (/(?:crate|super|self)::api\b|\bapi::(?:fetch|create|update|delete|customer_)/.test(leptosUi)) {
    errors.push("Customer admin Leptos adapter must not call legacy api::* directly");
  }

  [
    "build_customer_admin_submit_command",
    "CustomerAdminDraftInput",
    "CustomerAdminSubmitCommandError::EmailRequired",
    "CustomerAdminSubmitCommandError::LocaleUnavailable",
  ].forEach((requiredSnippet) => {
    if (!leptosUi.includes(requiredSnippet)) {
      errors.push(`Customer admin Leptos adapter must use core submit-command policy snippet: ${requiredSnippet}`);
    }
  });

  [
    "admin/src/core.rs",
    "submit-command",
    "admin/src/transport/mod.rs",
    "admin/src/transport/native_server_adapter.rs",
    "admin/src/ui/leptos.rs",
  ].forEach((requiredSnippet) => {
    if (!adminReadme.includes(requiredSnippet)) {
      errors.push(`Customer admin README must document native adapter split snippet: ${requiredSnippet}`);
    }
  });

  return errors;
}

function collectProductAdminShellProfileContractErrors() {
  const errors = [];
  const core = readText(productAdminCorePath);
  const leptosUi = readText(productAdminLeptosUiPath);
  const adminReadme = readText(productAdminReadmePath);

  [
    "ProductAdminShellViewModel",
    "build_product_admin_shell_view_model",
    "ProductAdminProfilePanelViewModel",
    "build_product_admin_profile_panel_loading_view_model",
    "build_product_admin_profile_panel_error_view_model",
    "build_product_admin_profile_panel_ready_view_model",
    "ShippingProfilesLoadViewModel",
    "shipping_profiles_load_view_from_result",
  ].forEach((contractName) => {
    if (!core.includes(contractName)) {
      errors.push(`Product admin core must contain shell/profile view-model contract: ${contractName}`);
    }
  });

  [
    "build_product_admin_shell_view_model",
    "shipping_profiles_load_view_from_result",
  ].forEach((contractName) => {
    if (!leptosUi.includes(contractName)) {
      errors.push(`Product admin Leptos adapter must use core-owned shell/profile helper: ${contractName}`);
    }
  });

  [
    "admin shell copy",
    "profile-panel state",
  ].forEach((requiredSnippet) => {
    if (!adminReadme.includes(requiredSnippet)) {
      errors.push(`Product admin README must document shell/profile FFA snippet: ${requiredSnippet}`);
    }
  });

  return errors;
}

function collectStructuralShapeErrors(registry) {
  const errors = [];

  if (!registry.includes("Structural shape captures")) {
    errors.push("docs/modules/registry.md must describe Structural shape for the FFA/FBA board");
  }

  if (!registry.includes("| Module slug | UI surfaces | FFA status | FBA status | Structural shape | Source plan |")) {
    errors.push("docs/modules/registry.md FFA/FBA board must contain the Structural shape column");
  }

  requiredStructuralShapes.forEach((shape) => {
    if (!registry.includes(`\`${shape}\``)) {
      errors.push(`docs/modules/registry.md must document Structural shape: ${shape}`);
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
      errors.push(`FFA/FBA board contains unknown Structural shape for ${moduleSlug}: ${structuralShape}`);
      return;
    }

    if (!sourcePlanPath) {
      errors.push(`FFA/FBA board does not contain source implementation plan path for ${moduleSlug}`);
      return;
    }

    const sourcePlan = readText(sourcePlanPath);
    const expectedShapeLine = `- Structural shape: \`${structuralShape}\``;
    if (!sourcePlan.includes(expectedShapeLine)) {
      errors.push(`${sourcePlanPath} must contain local status line: ${expectedShapeLine}`);
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
    const transportPaths = surfaces.flatMap((surface) =>
      moduleSurfacePaths(moduleRoot, surface, ["transport.rs", "transport", "native.rs"]),
    );
    const uiPaths = surfaces.flatMap((surface) => moduleSurfacePaths(moduleRoot, surface, ["ui", path.join("ui", "leptos.rs"), path.join("ui", "leptos")]));

    const hasCore = hasAnyPath(corePaths);
    const hasTransport = hasAnyPath(transportPaths);
    const hasUi = hasAnyPath(uiPaths);

    if (["core_only", "core_transport", "core_transport_ui"].includes(structuralShape) && !hasCore) {
      errors.push(`${moduleSlug}: Structural shape ${structuralShape} requires core.rs or core/ in admin/storefront src`);
    }
    if (["core_transport", "core_transport_ui"].includes(structuralShape) && !hasTransport) {
      errors.push(
        `${moduleSlug}: Structural shape ${structuralShape} requires transport.rs, transport/, or documented single-adapter native.rs in admin/storefront src`,
      );
    }
    if (structuralShape === "core_transport_ui" && !hasUi) {
      errors.push(`${moduleSlug}: Structural shape ${structuralShape} requires ui/leptos.rs or ui/leptos/ adapter in admin/storefront src`);
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
      errors.push(`Missing required heading in migration plan: ${heading}`);
    }
  });

  requiredChecklistChecks.forEach(({ label, pattern }) => {
    if (!pattern.test(checklist)) {
      errors.push(`Missing required checklist pattern (${label}) in docs/verification/ffa-ui-parity-checklist.md`);
    }
  });

  requiredKpiMentions.forEach((kpi) => {
    if (!plan.includes(kpi)) {
      errors.push(`Missing required KPI marker in migration plan: ${kpi}`);
    }
  });

  requiredAntiOverExtractionPlanMarkers.forEach((marker) => {
    if (!plan.includes(marker)) {
      errors.push(`Missing anti-over-extraction marker in migration plan: ${marker}`);
    }
  });

  const connectivityText = stripCodeFences(connectivity);
  requiredConnectivityMentions.forEach((mention) => {
    if (!connectivityText.includes(mention)) {
      errors.push(`Missing required pilot in docs/research/dioxus-ffa-pilot-connectivity-map.md: ${mention}`);
    }
  });

  const scripts = packageJson?.scripts ?? {};
  Object.entries(requiredNpmScriptCommands).forEach(([scriptName, expectedCommand]) => {
    const scriptValue = scripts[scriptName];
    if (typeof scriptValue !== "string" || scriptValue.trim().length === 0) {
      errors.push(`Missing required npm script in package.json: ${scriptName}`);
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
          `Script ${scriptName} must be one of: ${expectedVariants.join(" | ")}; actual: ${scriptValue.trim()}`,
        );
      }
    }
  });

  const migrationPipeline = scripts["verify:ffa:ui:migration"];
  if (typeof migrationPipeline === "string") {
    const normalizedPipeline = normalizeCommand(migrationPipeline);
    requiredMigrationPipelineCommands.forEach((command) => {
      if (!normalizedPipeline.includes(normalizeCommand(command))) {
        errors.push(`Script verify:ffa:ui:migration must include command: ${command}`);
      }
    });
  }

  requiredIndexRefs.forEach((refPath) => {
    if (!hasMarkdownLink(docsIndex, refPath)) {
      errors.push(`Missing required markdown link in docs/index.md: ${refPath}`);
    }
  });

  errors.push(...collectStructuralShapeErrors(registry));
  errors.push(...collectRegistryLocalShapeErrors(registry));
  errors.push(...collectStructuralShapeFilesystemErrors(registry));
  errors.push(...collectPagesStorefrontUiSplitErrors());
  errors.push(...collectRegionErrorStatusContractErrors());
  errors.push(...collectProductTransportEvidenceContractErrors());
  errors.push(...collectProductAdminShellProfileContractErrors());
  errors.push(...collectCustomerAdminNativeAdapterSplitErrors());

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
  console.log("Required FFA migration documents and baseline contracts checked.");
} catch (error) {
  console.error("[verify-ffa-ui-migration-contract] FAIL");
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
