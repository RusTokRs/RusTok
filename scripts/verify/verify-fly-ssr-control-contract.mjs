#!/usr/bin/env node
// Verifies that the classic-SSR control contract of the Fly editor has both of its sides.
//
// `DECISIONS/2026-07-16-fly-ssr-first-browser-runtime.md` makes `ssr + browser-js` the default
// Page Builder admin runtime. A server-rendered page has no Leptos event handlers, so the only
// thing that makes a palette, a layer row or a toolbar button work is `fly-browser.js` binding a
// `data-fly-*` attribute that the Rust view emitted. Neither side fails on its own when the other
// one changes: the adapter binds zero elements, the view renders an inert control, every build
// stays green. This check derives the contract from the adapter source and compares it with what
// the Rust views emit, in both directions.
//
// It reads sources only. Rendered markup is asserted by the render tests of the crate that owns
// the view (`fly-leptos`, `rustok-page-builder-admin`) and the adapter behaviour by
// `apps/next-admin/tests/e2e/fly-browser-ssr-controls.spec.ts`; this check also requires that
// both of those exist and cover every part of the contract, and that the `fly-dioxus` twin of the
// workspace emits the same contract markup.

import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (relativePath) => {
  try {
    return readFileSync(path.join(repoRoot, relativePath), 'utf8');
  } catch (error) {
    // A moved or deleted source is a contract failure to report, not a stack trace.
    console.error(`Fly SSR control contract verification failed:\n- cannot read ${relativePath}: ${error.code ?? error.message}`);
    process.exit(1);
  }
};

const adapterPath = 'crates/ui/fly/browser/assets/fly-browser.js';
const intentKindsPath = 'crates/ui/fly/browser/src/lib.rs';
const dispatchPath = 'crates/modules/rustok-page-builder/admin/src/browser_intent.rs';
const workspacePath = 'crates/ui/fly/leptos/src/editor.rs';
const adminSourceDir = 'crates/modules/rustok-page-builder/admin/src';
const adminEditorDir = `${adminSourceDir}/editor`;
const twinWorkspacePath = 'crates/ui/fly/dioxus/src/editor.rs';
const browserSpecPath = 'apps/next-admin/tests/e2e/fly-browser-ssr-controls.spec.ts';
const backlogPath = 'docs/modules/page-builder-backlog.md';

// Actions a Rust view emits that the adapter does not handle yet. Every entry is a control that
// renders and does nothing in the default runtime, so the list is a debt ledger, not an allow
// list: an entry must name its record in the backlog, and it fails the check as soon as the
// adapter handles the action (delete the entry then). A new unhandled action always fails.
const recordedUnhandledActions = new Map([]);

const failures = [];
const fail = (message) => failures.push(message);

/** Source of a Rust file without its trailing `#[cfg(test)] mod tests`: test strings are not producers. */
function productionSource(source) {
  const match = /^#\[cfg\(test\)\]\s*\nmod tests\b/m.exec(source);
  return match ? source.slice(0, match.index) : source;
}

function testSource(source) {
  const match = /^#\[cfg\(test\)\]\s*\nmod tests\b/m.exec(source);
  return match ? source.slice(match.index) : '';
}

/** Body of a JavaScript block that starts at `header`, found by brace matching outside strings and comments. */
function blockBody(source, header, from = 0) {
  const start = source.indexOf(header, from);
  if (start < 0) return null;
  let index = source.indexOf('{', start + header.length - 1);
  if (index < 0) return null;
  const open = index;
  let depth = 0;
  while (index < source.length) {
    const char = source[index];
    const next = source[index + 1];
    if (char === '/' && next === '/') {
      index = source.indexOf('\n', index);
      if (index < 0) return null;
      continue;
    }
    if (char === '/' && next === '*') {
      index = source.indexOf('*/', index + 2);
      if (index < 0) return null;
      index += 2;
      continue;
    }
    if (char === '"' || char === "'" || char === '`') {
      index += 1;
      while (index < source.length && source[index] !== char) {
        index += source[index] === '\\' ? 2 : 1;
      }
      index += 1;
      continue;
    }
    if (char === '{') depth += 1;
    if (char === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(open + 1, index);
    }
    index += 1;
  }
  return null;
}

const kebab = (datasetKey) =>
  `data-${datasetKey.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`;

// ---------------------------------------------------------------------------------------------
// Consumer side: what `bindSsrControls` binds.
// ---------------------------------------------------------------------------------------------

const adapter = read(adapterPath);
const controls = blockBody(adapter, 'bindSsrControls(signal) {');
if (!controls) {
  console.error(`${adapterPath}: bindSsrControls(signal) was not found`);
  process.exit(1);
}

const boundAttributes = new Set();
for (const match of controls.matchAll(/(?:querySelectorAll|closest)\(\s*"\[(data-fly-[a-z-]+)\]"/g)) {
  boundAttributes.add(match[1]);
}
// Attributes read from a control. `this.root.dataset.*` is adapter state, not a control attribute.
for (const match of controls.matchAll(/([\w.?)\]]*)\.dataset\.(fly[A-Za-z]+)/g)) {
  if (match[1].endsWith('this.root')) continue;
  boundAttributes.add(kebab(match[2]));
}

// `bindSsrControls` dispatches on the action once per DOM event it listens to (`click` for
// buttons, `change` for a `<select>`); the contract is the union of those switches.
const actionSwitchHeader = 'switch (action) {';
const actionSwitches = [];
for (
  let from = controls.indexOf(actionSwitchHeader);
  from >= 0;
  from = controls.indexOf(actionSwitchHeader, from + actionSwitchHeader.length)
) {
  const body = blockBody(controls, actionSwitchHeader, from);
  if (body === null) {
    fail(`${adapterPath}: a \`switch (action)\` in bindSsrControls has no matching closing brace`);
    break;
  }
  actionSwitches.push(body);
}
if (actionSwitches.length === 0) fail(`${adapterPath}: bindSsrControls has no \`switch (action)\``);
const actionSwitch = actionSwitches.join('\n');
const handledActions = new Set(
  [...(actionSwitch ?? '').matchAll(/case\s+"([a-z-]+)"\s*:/g)].map((match) => match[1]),
);
const intentPrefix = 'intent:';
const handlesIntentPrefix = (actionSwitch ?? '').includes(`action.startsWith("${intentPrefix}")`);
if (!handlesIntentPrefix) {
  fail(`${adapterPath}: bindSsrControls no longer forwards \`${intentPrefix}<name>\` actions`);
}
// Intents the adapter resolves on the client instead of posting them.
const localIntents = new Set(
  [...(actionSwitch ?? '').matchAll(/intent\s*===\s*"([a-z_]+)"/g)].map((match) => match[1]),
);
// Intents the adapter posts under a fixed name from an SSR control.
const postedIntents = new Set(
  [...controls.matchAll(/emitIntent\(\s*"([a-z_]+)"/g)].map((match) => match[1]),
);

for (const required of ['data-fly-block-id', 'data-fly-component-id', 'data-fly-action']) {
  if (!boundAttributes.has(required)) {
    fail(`${adapterPath}: bindSsrControls no longer binds [${required}]; update this check with the contract`);
  }
}

// ---------------------------------------------------------------------------------------------
// Producer side: what the Rust views of the default admin surface emit.
// ---------------------------------------------------------------------------------------------

const producerPaths = [
  workspacePath,
  ...readdirSync(path.join(repoRoot, adminEditorDir))
    .filter((name) => name.endsWith('.rs'))
    .sort()
    .map((name) => `${adminEditorDir}/${name}`),
];

/** attribute -> files that emit it as a view attribute (not as a CSS selector such as `[data-fly-x=…]`). */
const emittedAttributes = new Map();
/** action -> files that emit it. */
const emittedActions = new Map();
const note = (map, key, file) => {
  if (!map.has(key)) map.set(key, new Set());
  map.get(key).add(file);
};

for (const file of producerPaths) {
  const source = productionSource(read(file));
  for (const match of source.matchAll(/(?<![[\w-])(data-fly-[a-z-]+)=/g)) {
    note(emittedAttributes, match[1], file);
  }
  for (const match of source.matchAll(/(?<![[\w-])data-fly-action=([^\n]*)/g)) {
    const literal = /"([^"]*)"/.exec(match[1]);
    if (!literal) {
      fail(`${file}: \`data-fly-action=${match[1].trim()}\` has no string literal; its value cannot be verified`);
      continue;
    }
    const value = literal[1];
    const placeholder = /^intent:\{(\w+)\}$/.exec(value);
    if (!placeholder) {
      if (value.includes('{')) {
        fail(`${file}: \`data-fly-action\` value "${value}" is built dynamically; only \`intent:{name}\` is supported`);
        continue;
      }
      note(emittedActions, value, file);
      continue;
    }
    // `format!("intent:{ssr_intent}")`: the names are the literal values passed for that prop.
    const names = [
      ...source.matchAll(new RegExp(`\\b${placeholder[1]}="([a-z_]+)"`, 'g')),
    ].map((prop) => prop[1]);
    if (names.length === 0) {
      fail(`${file}: \`data-fly-action="${value}"\` has no literal \`${placeholder[1]}="…"\` value to verify`);
    }
    for (const name of names) note(emittedActions, `${intentPrefix}${name}`, file);
  }
}

const where = (files) => [...files].map((file) => path.basename(file)).join(', ');

// 1. Everything the adapter binds is emitted by a view.
for (const attribute of [...boundAttributes].sort()) {
  if (!emittedAttributes.has(attribute)) {
    fail(
      `no Rust view emits \`${attribute}\`, which ${path.basename(adapterPath)} binds: ` +
        'the control it belongs to is dead in the default ssr + browser-js runtime',
    );
  }
}
for (const action of [...handledActions].sort()) {
  if (!emittedActions.has(action)) {
    fail(
      `no Rust view emits \`data-fly-action="${action}"\`, which ${path.basename(adapterPath)} handles: ` +
        'the control it belongs to is dead in the default ssr + browser-js runtime',
    );
  }
}

// 2. Every action a view emits is handled: by the adapter itself, or by the server through an
//    `intent:<name>` action whose name is a protocol intent with a dispatch arm.
const intentKinds = new Set(
  [...read(intentKindsPath).matchAll(/Self::\w+\s*=>\s*"([a-z_]+)"\s*,/g)].map((match) => match[1]),
);
const dispatchSource = productionSource(read(dispatchPath));
const dispatchBody = blockBody(dispatchSource, 'fn dispatch_named_intent(');
// `blockBody` starts at the first `{` after the header, which for a Rust fn is its body.
const dispatchArms = new Set(
  [...(dispatchBody ?? '').matchAll(/^\s*((?:"[a-z_]+"\s*\|\s*)*"[a-z_]+")\s*=>/gm)].flatMap((match) =>
    [...match[1].matchAll(/"([a-z_]+)"/g)].map((name) => name[1]),
  ),
);
if (dispatchArms.size === 0) fail(`${dispatchPath}: no intent arms were found in dispatch_named_intent`);

const serverHandles = (intent) => intentKinds.has(intent) && dispatchArms.has(intent);

const unhandled = new Set();
for (const [action, files] of [...emittedActions].sort()) {
  if (handledActions.has(action)) continue;
  if (action.startsWith(intentPrefix) && handlesIntentPrefix) {
    const intent = action.slice(intentPrefix.length);
    if (localIntents.has(intent) || serverHandles(intent)) continue;
    fail(
      `${where(files)} emits \`data-fly-action="${action}"\`, but \`${intent}\` is neither resolved by the adapter ` +
        `nor a protocol intent with an arm in dispatch_named_intent`,
    );
    continue;
  }
  unhandled.add(action);
  if (!recordedUnhandledActions.has(action)) {
    fail(
      `${where(files)} emits \`data-fly-action="${action}"\`, which ${path.basename(adapterPath)} does not handle: ` +
        'the control renders and does nothing in the default ssr + browser-js runtime',
    );
  }
}
for (const intent of [...postedIntents].sort()) {
  if (!serverHandles(intent)) {
    fail(`${adapterPath} posts \`${intent}\` from an SSR control, but the server has no dispatch arm for it`);
  }
}

const backlog = read(backlogPath);
for (const [action, record] of recordedUnhandledActions) {
  if (!unhandled.has(action)) {
    fail(`recorded unhandled action \`${action}\` is handled or no longer emitted: delete its entry from this check`);
  }
  if (!backlog.includes(record) || !backlog.includes(action)) {
    fail(`recorded unhandled action \`${action}\` has no "${record}" record in ${backlogPath}`);
  }
}

// 3. A control whose value travels in the route has a server half: the view names the query key
//    through a constant, and the same constant is what the server resolves before it renders.
//    Without that half the adapter navigates and the page comes back unchanged.
const routeQueryAttribute = 'data-fly-route-query';
for (const file of emittedAttributes.get(routeQueryAttribute) ?? []) {
  const source = productionSource(read(file));
  for (const match of source.matchAll(new RegExp(`(?<![[\\w-])${routeQueryAttribute}=([^\\n]*)`, 'g'))) {
    const constant = /^([A-Z][A-Z0-9_]*)\b/.exec(match[1].trim());
    if (!constant) {
      fail(`${file}: \`${routeQueryAttribute}=${match[1].trim()}\` must be a named constant shared with the server resolver`);
      continue;
    }
    const name = constant[1];
    if (!new RegExp(`const\\s+${name}\\s*:\\s*&str\\s*=\\s*"[a-z_]+"\\s*;`).test(source)) {
      fail(`${file}: route query constant \`${name}\` is not defined in the file that emits it`);
    }
    const resolver = new RegExp(`fn\\s+(\\w+)\\s*\\([^)]*\\)[^{]*\\{[^}]*query\\.get\\(${name}\\)`).exec(source);
    if (!resolver) {
      fail(`${file}: no function resolves the route value with \`query.get(${name})\``);
      continue;
    }
    const applied = producerPaths.some((producer) =>
      productionSource(read(producer))
        .replace(new RegExp(`fn\\s+${resolver[1]}\\s*\\(`), '')
        .includes(`${resolver[1]}(`),
    );
    if (!applied) {
      fail(`${file}: \`${resolver[1]}\` is never called, so the route value named by \`${name}\` is not applied before render`);
    }
  }
}

// 4. Both sides are exercised: rendered markup by render tests, behaviour in a browser. Every
//    part is asserted on the composed admin surface, because that is what the default runtime
//    serves and what CI executes (`cargo test -p rustok-page-builder-admin --lib`); a part that
//    `fly-leptos` emits is also asserted by its own tests, which CI only compiles.
const adminRenderTests = readdirSync(path.join(repoRoot, adminSourceDir))
  .filter((name) => /^ssr_\w+_tests\.rs$/.test(name))
  .sort()
  .map((name) => read(`${adminSourceDir}/${name}`))
  .join('\n');
const adminRenderTestsLabel = `${adminSourceDir}/ssr_*_tests.rs`;
const renderTestsFor = (file) =>
  file === workspacePath
    ? { label: workspacePath, source: testSource(read(workspacePath)) }
    : { label: adminRenderTestsLabel, source: adminRenderTests };
const browserSpec = read(browserSpecPath);
const contractParts = [
  ...[...boundAttributes]
    .filter((attribute) => attribute !== 'data-fly-action')
    .map((attribute) => ({ part: attribute, files: emittedAttributes.get(attribute) })),
  ...[...handledActions].map((action) => ({
    part: `data-fly-action=\\"${action}\\"`,
    files: emittedActions.get(action),
  })),
];
for (const { part, files } of contractParts.sort((left, right) => left.part.localeCompare(right.part))) {
  const readable = part.replaceAll('\\', '');
  // A part with no producer was already reported above; there is nothing to render-test.
  const owners = new Map([...(files ?? [])].map((file) => {
    const tests = renderTestsFor(file);
    return [tests.label, tests.source];
  }));
  if (owners.size > 0) owners.set(adminRenderTestsLabel, adminRenderTests);
  for (const [label, source] of owners) {
    if (!source.includes(part)) fail(`${label}: no render test asserts \`${readable}\``);
  }
  if (!browserSpec.includes(readable)) {
    fail(`${browserSpecPath}: no browser test exercises \`${readable}\``);
  }
}
if (!browserSpec.includes(adapterPath.split('/').slice(-4).join('/'))) {
  fail(`${browserSpecPath} must load the shipped ${adapterPath}`);
}

// 5. `fly-dioxus` is the twin of the `fly-leptos` workspace: whichever adapter renders the
//    palette and the layer tree, the contract markup is the same. `rsx!` writes an attribute as
//    `"data-fly-x": value`.
const twin = productionSource(read(twinWorkspacePath));
for (const attribute of [...boundAttributes].sort()) {
  if (attribute === 'data-fly-action') continue;
  if (!emittedAttributes.get(attribute)?.has(workspacePath)) continue;
  if (!twin.includes(`"${attribute}":`)) {
    fail(`${twinWorkspacePath} does not emit \`${attribute}\`, which its twin ${workspacePath} emits`);
  }
}
for (const action of [...handledActions].sort()) {
  if (!emittedActions.get(action)?.has(workspacePath)) continue;
  if (!twin.includes('"data-fly-action":') || !twin.includes(`"${action}"`)) {
    fail(
      `${twinWorkspacePath} does not emit \`data-fly-action="${action}"\`, which its twin ${workspacePath} emits`,
    );
  }
}

if (failures.length > 0) {
  console.error('Fly SSR control contract verification failed:');
  for (const message of failures) console.error(`- ${message}`);
  process.exit(1);
}

console.log(
  `Fly SSR control contract verified: ${boundAttributes.size} bound attributes, ` +
    `${handledActions.size} adapter actions, ${emittedActions.size} emitted actions ` +
    `(${recordedUnhandledActions.size} recorded as unhandled).`,
);
