import { existsSync, readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..", "..");

const paths = {
  canonicalAsset: join(repoRoot, "crates", "ui", "fly", "browser", "assets", "fly-actions.js"),
  vendoredAsset: join(here, "..", "packages", "rustok-pages", "src", "components", "fly-actions.js"),
  assetTypes: join(here, "..", "packages", "rustok-pages", "src", "components", "fly-actions.d.ts"),
  bridge: join(here, "..", "packages", "rustok-pages", "src", "components", "fly-action-bridge.tsx"),
  retiredRuntime: join(here, "..", "packages", "rustok-pages", "src", "components", "fly-actions-runtime.ts"),
  actionModel: join(repoRoot, "crates", "ui", "fly", "src", "action", "model.rs"),
};

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

/**
 * Action kinds the browser handles natively: Fly materializes them as
 * `href`/`form` attributes (see GENERATED_INTERACTION_ATTRIBUTES in
 * action/model.rs), so they never reach the JS runtime's switch.
 */
const nativeKinds = new Set(["navigate_page", "navigate_url", "submit_form"]);

// 1. The vendored runtime must be a byte-exact copy of the canonical asset.
const canonical = readFileSync(paths.canonicalAsset);
const vendored = readFileSync(paths.vendoredAsset);
assert(
  canonical.equals(vendored),
  "packages/rustok-pages/src/components/fly-actions.js has drifted from "
    + "crates/ui/fly/browser/assets/fly-actions.js. The Next package must "
    + "consume the canonical asset verbatim: re-copy the file (byte-exact) "
    + "instead of maintaining a TypeScript rewrite. "
    + `canonical sha256=${createHash("sha256").update(canonical).digest("hex")} `
    + `vendored sha256=${createHash("sha256").update(vendored).digest("hex")}`,
);

// 2. The runtime's supported kind set must match the Rust ComponentAction
//    enum minus the natively-handled kinds, so a new Rust action kind cannot
//    ship without runtime support (and vice versa).
const caseLabels = new Set();
for (const match of canonical.toString("utf8").matchAll(/case\s+"([a-z_]+)"\s*:/g)) {
  caseLabels.add(match[1]);
}

const modelSource = readFileSync(paths.actionModel, "utf8");
const kindArm = /pub const fn kind\(&self\)\s*->\s*&'static str\s*\{([\s\S]*?)\n\s*\}/;
const kindBody = modelSource.match(kindArm)?.[1];
assert(kindBody, "Could not locate ComponentAction::kind() in crates/ui/fly/src/action/model.rs");
const rustKinds = new Set();
for (const match of kindBody.matchAll(/Self::\w+\s*\{\s*\.\.\s*\}\s*=>\s*"([a-z_]+)"/g)) {
  rustKinds.add(match[1]);
}
assert(rustKinds.size > 0, "No kind() arms parsed from crates/ui/fly/src/action/model.rs");

const expectedRuntimeKinds = new Set([...rustKinds].filter((kind) => !nativeKinds.has(kind)));
for (const kind of expectedRuntimeKinds) {
  assert(
    caseLabels.has(kind),
    `fly-actions.js does not handle Rust action kind "${kind}" (missing case "${kind}")`,
  );
}
for (const kind of caseLabels) {
  assert(
    expectedRuntimeKinds.has(kind),
    `fly-actions.js handles "${kind}", which is not a non-native ComponentAction kind; `
      + "either the Rust enum or the runtime asset is out of sync",
  );
}

// 3. The bridge must consume the vendored asset as a side-effect import and
//    must not resurrect the retired TypeScript runtime.
const bridge = readFileSync(paths.bridge, "utf8");
assert(
  /^\s*import\s+"\.\/fly-actions\.js";/m.test(bridge),
  "fly-action-bridge.tsx must side-effect import \"./fly-actions.js\" (the canonical runtime)",
);
assert(
  !bridge.includes("initFlyActions") && !bridge.includes("fly-actions-runtime"),
  "fly-action-bridge.tsx must not reference the retired fly-actions-runtime rewrite",
);
assert(
  !existsSync(paths.retiredRuntime),
  "fly-actions-runtime.ts must not exist: the Next package consumes the canonical "
    + "fly-actions.js asset instead of a parallel TypeScript implementation",
);

// 4. tsc (allowJs: false) resolves the .js import through the sibling .d.ts.
assert(
  existsSync(paths.assetTypes),
  "packages/rustok-pages/src/components/fly-actions.d.ts is missing: tsc needs it to type "
    + "the vendored fly-actions.js module",
);

console.log(
  "fly-actions parity OK: "
    + `byte-exact asset copy (sha256 ${createHash("sha256").update(vendored).digest("hex").slice(0, 12)}…), `
    + `${caseLabels.size} runtime kinds {${[...caseLabels].sort().join(", ")}} `
    + `= ${rustKinds.size} ComponentAction kinds − ${nativeKinds.size} native`,
);
