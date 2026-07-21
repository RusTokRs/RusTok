import { readFile } from "node:fs/promises";
import process from "node:process";

const root = process.cwd();
const read = (path) => readFile(`${root}/${path}`, "utf8");

const [cargo, adapters, service, serviceContract, browserHost, browserRuntime] =
  await Promise.all([
    read("crates/rustok-page-builder/Cargo.toml"),
    read("crates/rustok-page-builder/src/adapters.rs"),
    read("crates/rustok-page-builder/src/adapters/fly_service.rs"),
    read(
      "crates/rustok-page-builder/contracts/page-builder-service-boundary.json",
    ),
    read("crates/rustok-page-builder/src/browser_host.rs"),
    read("crates/fly-browser/assets/fly-browser.js"),
  ]);

const contract = JSON.parse(serviceContract);
const required = [
  [
    cargo,
    'fly = { path = "../fly" }',
    "rustok-page-builder must depend on Fly",
  ],
  [
    adapters,
    "pub struct FlyProjectInspection",
    "Fly project inspection is missing",
  ],
  [adapters, "GrapesJsCodec::decode_value", "Fly codec is not used"],
  [
    adapters,
    ".project\n            .pages",
    "tree traversal must start from Fly pages",
  ],
  [adapters, "component_properties", "component property lookup is missing"],
  [
    adapters,
    "pub use fly_service::FlyAdapterBackedPageBuilderService",
    "current Fly service export is missing",
  ],
  [
    service,
    "pub struct FlyAdapterBackedPageBuilderService",
    "Fly-backed service is missing",
  ],
  [
    service,
    "FlyProjectInspection::decode_with",
    "service does not decode through Fly",
  ],
  [
    service,
    "inspection.require_valid()",
    "service does not require Fly validation",
  ],
  [service, ".tree_nodes()", "service does not expose Fly tree traversal"],
  [
    service,
    ".component_properties(&input.node_id)",
    "service does not validate component lookup",
  ],
  [
    service,
    "PageBuilderRuntimeCallEvidence::render_preview",
    "preview telemetry is missing",
  ],
  [
    service,
    "PageBuilderRuntimeCallEvidence::load_project",
    "load telemetry is missing",
  ],
  [
    service,
    "PageBuilderRuntimeCallEvidence::save_project",
    "save telemetry is missing",
  ],
  [
    browserHost,
    "pub struct PageBuilderBrowserModuleOptions",
    "shared browser module options are missing",
  ],
  [
    browserHost,
    "pub struct PageBuilderBrowserModuleDescriptor",
    "shared browser module descriptor is missing",
  ],
  [
    browserHost,
    "pub nonce: Option<String>",
    "shared browser CSP nonce is missing",
  ],
  [
    browserHost,
    "pub fn page_builder_browser_module(",
    "shared browser module constructor is missing",
  ],
  [
    browserHost,
    "PAGE_BUILDER_BROWSER_SCRIPT_TYPE",
    "shared browser script type is missing",
  ],
  [
    browserHost,
    'Symbol.for("fly.browser.ssr.controls")',
    "SSR host state is not shared",
  ],
  [browserHost, '"fly:browser-ready"', "late browser mounts are not bound"],
  [
    browserRuntime,
    "export class FlyBrowserAdapter",
    "Fly browser runtime is missing",
  ],
  [
    browserRuntime,
    "event.source !== this.iframe.contentWindow",
    "iframe source validation is missing",
  ],
  [
    browserRuntime,
    "event.origin !== this.expectedOrigin",
    "iframe origin validation is missing",
  ],
];

const failures = required
  .filter(([source, marker]) => !source.includes(marker))
  .map(([, , message]) => message);

const currentSources = [adapters, service, browserHost, browserRuntime];
for (const forbidden of contract.forbidden_symbols ?? []) {
  if (currentSources.some((source) => source.includes(forbidden))) {
    failures.push(`obsolete runtime symbol '${forbidden}' is present`);
  }
}

if (service.includes('project_data.get("nodes")')) {
  failures.push(
    "Fly-backed service must not traverse the obsolete root nodes key",
  );
}
if (browserHost.includes("autoMount === false")) {
  failures.push(
    "Page Builder host must delegate auto-mount policy to FlyBrowser.bootstrap",
  );
}
if (browserHost.includes("pub fn page_builder_browser_module_source")) {
  failures.push(
    "Page Builder browser host must expose only the renderer-neutral module descriptor",
  );
}

if (failures.length > 0) {
  console.error("Fly runtime verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Fly runtime wiring verified.");
