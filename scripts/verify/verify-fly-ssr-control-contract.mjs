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
//
// The adapter is not the only consumer. The Page Builder host bootstrap
// (`PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS`) binds the SSR forms: `form[data-fly-intent-form]`
// and the two selection controls. Part 6 derives that contract the same way and follows it to
// the server: every form names a protocol intent that some handler resolves, and the fields a
// form sends are the fields its request type reads, with the JSON types the bootstrap produces.
//
// Part 7 covers the reach of a contract attribute: `data-fly-component-id` is on canvas nodes and
// on layer rows of one document, so a canvas style rule keyed on it must carry the canvas scope.
//
// Part 8 covers the selection: the host bootstrap retargets the SSR forms on `fly:select`, so the
// adapter may change its selection in one method only, and that method announces the change.

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
const hostBootstrapPath = 'crates/modules/rustok-page-builder/src/browser_host.rs';
const hostBootstrapConstant = 'PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS';
const hostSpecPath = 'apps/next-admin/tests/e2e/fly-browser-host-bootstrap.spec.ts';
const moduleCratesDir = 'crates/modules';
const intentFormAttribute = 'data-fly-intent-form';
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

//    A route value that a control writes can also be left through that control: the view renders
//    an option with the empty value, and the adapter turns the empty value into a route without
//    the key. Without either half the only way back is to edit the address by hand.
for (const file of emittedAttributes.get(routeQueryAttribute) ?? []) {
  if (!/<option\s+value=""/.test(productionSource(read(file)))) {
    fail(`${file}: the control that writes a route value renders no \`<option value="">\` to leave it`);
  }
}
const routeNavigation = adapter.match(/\n  navigateRouteQuery\(key, value\) \{\n([\s\S]*?)\n  \}\n/);
if (!routeNavigation) {
  fail(`${adapterPath}: the adapter has no navigateRouteQuery(key, value) method`);
} else {
  for (const marker of ['url.searchParams.set(key, value)', 'url.searchParams.delete(key)']) {
    if (!routeNavigation[1].includes(marker)) {
      fail(`${adapterPath}: navigateRouteQuery() is missing \`${marker}\``);
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

// ---------------------------------------------------------------------------------------------
// 6. The host bootstrap: the consumer of the SSR forms.
// ---------------------------------------------------------------------------------------------

const hostSource = read(hostBootstrapPath);
const hostBootstrap = new RegExp(
  `pub const ${hostBootstrapConstant}: &str = r#"([\\s\\S]*?)"#;`,
).exec(hostSource)?.[1];
if (hostBootstrap === undefined) {
  fail(`${hostBootstrapPath}: ${hostBootstrapConstant} was not found as a raw string constant`);
}

// 6a. Everything the bootstrap binds is emitted by a view.
const hostBoundAttributes = new Set();
for (const match of (hostBootstrap ?? '').matchAll(
  /(?:querySelectorAll|closest)\(\s*["'][a-z]*\[(data-fly-[a-z-]+)\]["']/g,
)) {
  hostBoundAttributes.add(match[1]);
}
for (const match of (hostBootstrap ?? '').matchAll(/\b\w+\.dataset\.(fly[A-Za-z]+)/g)) {
  hostBoundAttributes.add(kebab(match[1]));
}
if (hostBootstrap !== undefined && !hostBoundAttributes.has(intentFormAttribute)) {
  fail(`${hostBootstrapPath}: the bootstrap no longer binds [${intentFormAttribute}]; update this check with the contract`);
}
for (const attribute of [...hostBoundAttributes].sort()) {
  if (!emittedAttributes.has(attribute)) {
    fail(
      `no Rust view emits \`${attribute}\`, which ${hostBootstrapConstant} binds: ` +
        'the control it belongs to is dead in the default ssr + browser-js runtime',
    );
  }
}

/**
 * Arms of a Rust `match` on an intent name: name -> its arms in source order, each with the
 * text of the arm and the guard of its head (`"a" | "b" if guard => …`), if it has one.
 *
 * Order and guards are part of the answer to "which code handles this form": Rust takes the
 * first arm whose pattern and guard both hold, so an arm that comes later is never reached for
 * a payload an earlier one accepts.
 */
function intentArms(body) {
  const arms = new Map();
  const head = /^[ \t]*((?:"[a-z_]+"\s*\|\s*)*"[a-z_]+")\s*(?:if\s+([^=]+?)\s*)?=>/gm;
  const heads = [...body.matchAll(head)];
  const ends = [
    ...heads.map((match) => match.index),
    ...[...body.matchAll(/^[ \t]*(?:_|[a-z_]+)\s*=>/gm)].map((match) => match.index),
  ].sort((left, right) => left - right);
  for (const match of heads) {
    const end = ends.find((index) => index > match.index) ?? body.length;
    const arm = { text: body.slice(match.index, end), guard: match[2]?.trim() ?? null };
    for (const name of match[1].matchAll(/"([a-z_]+)"/g)) {
      arms.set(name[1], [...(arms.get(name[1]) ?? []), arm]);
    }
  }
  return arms;
}

function rustFiles(directory) {
  const files = [];
  for (const entry of readdirSync(path.join(repoRoot, directory), { withFileTypes: true })) {
    const relative = `${directory}/${entry.name}`;
    if (entry.isDirectory()) files.push(...rustFiles(relative));
    else if (entry.name.endsWith('.rs')) files.push(relative);
  }
  return files.sort();
}

// Handlers. The dispatcher resolves an intent in an arm of its own, or hands the name to the
// form resolver: through the helper it calls for every name it has no arm for, and from an arm
// that calls the same helper for a payload its guard recognizes as a form. A host that serves
// the endpoint may resolve an intent before it reaches the dispatcher.
const dispatcherArms = intentArms(dispatchBody ?? '');
const formHelperName = /\bother\s*=>\s*(\w+)\(\s*controller,\s*other,\s*payload\s*\)/.exec(dispatchBody ?? '')?.[1];
const formResolverName = formHelperName
  ? /\bcontroller\s*\.(\w+)\(\s*intent,\s*payload\s*\)/.exec(blockBody(dispatchSource, `fn ${formHelperName}(`) ?? '')?.[1]
  : undefined;
const delegatesToFormResolver = (arm) =>
  new RegExp(`\\b${formHelperName}\\(\\s*controller,\\s*intent,\\s*payload\\s*\\)`).test(arm.text);
const adminProducers = producerPaths.filter((file) => file !== workspacePath);
const adminSources = new Map(adminProducers.map((file) => [file, productionSource(read(file))]));
let formResolverArms = new Map();
if (!formResolverName) {
  fail(`${dispatchPath}: dispatch_named_intent no longer hands an unknown intent to a form resolver`);
} else {
  const owner = [...adminSources].find(([, source]) => source.includes(`fn ${formResolverName}(`));
  if (!owner) fail(`${adminEditorDir}: \`fn ${formResolverName}\`, which the dispatcher calls, was not found`);
  else formResolverArms = intentArms(blockBody(owner[1], `fn ${formResolverName}(`) ?? '');
  for (const [name, arms] of formResolverArms) {
    if (arms.length !== 1 || arms[0].guard !== null) {
      fail(`${owner?.[0]}: \`${formResolverName}\` resolves \`${name}\` in ${arms.length} arms or behind a guard; this check reads one unguarded arm per form`);
    }
    // A name the dispatcher takes in an arm of its own never reaches the resolver, unless one
    // of those arms hands the payload over.
    const own = dispatcherArms.get(name) ?? [];
    if (own.length > 0 && !own.some(delegatesToFormResolver)) {
      fail(
        `${owner?.[0]}: the \`${name}\` arm of \`${formResolverName}\` is unreachable: ${dispatchPath} resolves ` +
          `\`${name}\` in an arm of its own and never hands it to \`${formHelperName}\``,
      );
    }
  }
}

/**
 * Evaluates the guard of a dispatcher arm for the payload of one form.
 *
 * A guard this check can read is one call `predicate(payload)` of a function whose body is a
 * conjunction of `payload.get("key").is_some()` / `.is_none()` terms. The bootstrap sends a key
 * for every named control except an empty number input, so a key is present exactly when the
 * form has a control of that name. Returns `undefined` when the guard cannot be decided.
 */
function guardHolds(guard, controls) {
  const predicate = /^(\w+)\(\s*payload\s*\)$/.exec(guard)?.[1];
  const body = predicate ? blockBody(dispatchSource, `fn ${predicate}(`) : undefined;
  if (!body) return undefined;
  let holds = true;
  for (const term of body.split('&&')) {
    const match = /^\s*payload\s*\.get\("(\w+)"\)\s*\.is_(some|none)\(\)\s*$/.exec(term);
    if (!match || controls.get(match[1]) === 'number') return undefined;
    if (controls.has(match[1]) !== (match[2] === 'some')) holds = false;
  }
  return holds;
}

/** The control a component renders for its `name` prop: `<select name=name …>` -> `select`. */
function componentControl(component) {
  for (const source of adminSources.values()) {
    const body = blockBody(source, `fn ${component}(`);
    if (body == null) continue;
    const controls = [...body.matchAll(/<(input|select|textarea)\b([^<]*?)\/?>/g)].filter((tag) =>
      /(?:^|\s)name=/.test(tag[2]),
    );
    if (controls.length !== 1 || !/(?:^|\s)name=name(?:\s|$)/.test(controls[0][2])) return undefined;
    return { tag: controls[0][1], attributes: controls[0][2] };
  }
  return undefined;
}
const dispatchEntry = /pub fn (dispatch_\w+)\s*\(/.exec(dispatchSource)?.[1];
const hostEndpoints = [];
if (!dispatchEntry) {
  fail(`${dispatchPath}: the public dispatch entry point was not found`);
} else {
  for (const entry of readdirSync(path.join(repoRoot, moduleCratesDir), { withFileTypes: true })) {
    const hostDir = `${moduleCratesDir}/${entry.name}/admin/src`;
    if (!entry.isDirectory() || hostDir === adminSourceDir) continue;
    let files;
    try {
      files = rustFiles(hostDir);
    } catch {
      continue;
    }
    for (const file of files) {
      const source = productionSource(read(file));
      if (source.includes(`${dispatchEntry}(`)) hostEndpoints.push({ file, source, arms: intentArms(source) });
    }
  }
  if (hostEndpoints.length === 0) {
    fail(`no host under ${moduleCratesDir}/*/admin/src calls ${dispatchEntry}; the SSR forms have no endpoint`);
  }
}

// Request types: `pub struct Ssr…Request { … }` of the admin editor.
const requestTypes = new Map();
for (const [file, source] of adminSources) {
  for (const match of source.matchAll(/((?:#\[[^\]]*\]\s*)*)pub struct (\w+)\s*\{([^}]*)\}/g)) {
    if (!/\bDeserialize\b/.test(match[1])) continue;
    const containerDefault = /#\[serde\([^)]*\bdefault\b/.test(match[1]);
    const fields = new Map();
    for (const field of match[3].matchAll(/((?:[ \t]*#\[[^\]]*\]\s*)*)[ \t]*pub (\w+):\s*([^\n]+?),?[ \t]*\n/g)) {
      const type = field[3].trim();
      fields.set(/rename\s*=\s*"([^"]+)"/.exec(field[1])?.[1] ?? field[2], {
        type,
        optional: containerDefault || /#\[serde\([^)]*\bdefault\b/.test(field[1]) || type.startsWith('Option<'),
      });
    }
    requestTypes.set(match[2], { file, fields });
  }
}

/** The request type an arm deserializes the payload into, or null when it reads keys itself. */
function requestTypeOf({ text: arm }) {
  if (!/\bfrom_value\b/.test(arm)) return null;
  const explicit = /from_value::<(\w+)>/.exec(arm)?.[1];
  if (explicit) return explicit;
  // `self.ssr_x_intent(serde_json::from_value(payload.clone()))`: the parameter type decides.
  const callee = /\.(\w+)\(\s*serde_json::from_value\(/.exec(arm)?.[1];
  if (!callee) return undefined;
  for (const source of adminSources.values()) {
    const parameter = new RegExp(`fn ${callee}\\(\\s*&(?:mut )?self,\\s*\\w+:\\s*(\\w+)`).exec(source)?.[1];
    if (parameter) return parameter;
  }
  return undefined;
}

// Readers that fail when a key is absent: `fn reader(payload: &Value, field: &str)` whose body
// builds the missing-field error.
const requiredReaders = [...dispatchSource.matchAll(/fn (\w+)\(\s*payload: &Value,\s*field: &str\s*\)/g)]
  .map((match) => match[1])
  .filter((name) => /\bMissingField\b/.test(blockBody(dispatchSource, `fn ${name}(`) ?? ''));

/**
 * Keys an arm that reads the payload itself cannot do without: read through a required reader,
 * through `.get("key")….ok_or_else(…)`, or by a helper the arm passes the whole payload to.
 */
function requiredKeysOf(text, seen = new Set()) {
  const keys = new Set();
  for (const reader of requiredReaders) {
    for (const match of text.matchAll(new RegExp(`\\b${reader}\\(\\s*payload,\\s*"(\\w+)"\\s*\\)`, 'g'))) {
      keys.add(match[1]);
    }
  }
  const call = '\\s*\\.\\w+(?:::<[^>]*>)?\\((?:[^()]|\\((?:[^()]|\\([^()]*\\))*\\))*\\)';
  for (const match of text.matchAll(new RegExp(`\\.get\\("(\\w+)"\\)(?:${call})*?\\s*\\.ok_or_else\\(`, 'g'))) {
    keys.add(match[1]);
  }
  for (const match of text.matchAll(/\b(\w+)\(\s*payload\s*\)/g)) {
    if (seen.has(match[1])) continue;
    seen.add(match[1]);
    const body = blockBody(dispatchSource, `fn ${match[1]}(`);
    if (body) for (const key of requiredKeysOf(body, seen)) keys.add(key);
  }
  return keys;
}

// Keys the adapter removes from every payload before it posts it.
const reservedPayloadKeys = new Set(
  [...(blockBody(adapter, 'function normalizedIntent(') ?? '').matchAll(/delete payload\.(\w+)/g)].map(
    (match) => match[1],
  ),
);
// JSON types the bootstrap produces for a named control.
const producesBoolean = /querySelectorAll\(\s*'input\[type="checkbox"\]\[name\]'\s*\)/.test(hostBootstrap ?? '');
const producesNumber = /querySelectorAll\(\s*'input\[type="number"\]\[name\]'\s*\)/.test(hostBootstrap ?? '');
if (hostBootstrap !== undefined && (!producesBoolean || !producesNumber)) {
  fail(`${hostBootstrapPath}: the bootstrap no longer types checkbox and number inputs; update this check with the contract`);
}
const isBooleanType = (type) => type === 'bool';
const isNumberType = (type) => /^(?:Option<)?(?:[ui](?:8|16|32|64|size)|f(?:32|64))>?$/.test(type);

// 6b, 6c. Every form names an intent the server resolves, and sends what the handler reads.
let verifiedForms = 0;
for (const [file, source] of adminSources) {
  const literalForms = [...source.matchAll(
    new RegExp(`<form\\b[^>]*?${intentFormAttribute}="([a-z_]+)"([\\s\\S]*?)</form>`, 'g'),
  )];
  const emitted = [...source.matchAll(new RegExp(`(?<![[\\w-])${intentFormAttribute}=`, 'g'))].length;
  if (literalForms.length !== emitted) {
    fail(
      `${file}: ${emitted} \`${intentFormAttribute}\` attributes, ${literalForms.length} of them on a \`<form>\` with a ` +
        'literal intent name; the others cannot be verified',
    );
  }
  for (const [, intent, body] of literalForms) {
    verifiedForms += 1;
    const label = `${file}: form \`${intent}\``;
    if (!intentKinds.has(intent)) {
      fail(`${label} names no protocol intent of ${intentKindsPath}`);
      continue;
    }

    // Named controls of the form, with the JSON type the bootstrap gives each of them.
    const controls = new Map();
    const tags = [...body.matchAll(/<(input|select|textarea)\b([^<]*?)\/?>/g)].filter((tag) =>
      /(?:^|\s)name=/.test(tag[2]),
    );
    // A component that renders one control for its `name` prop counts as that control.
    for (const [, component, attributes] of body.matchAll(/<([A-Z]\w*)\b([^<]*?)\/>/g)) {
      if (!/(?:^|\s)name=/.test(attributes)) continue;
      const control = componentControl(component);
      if (!control) {
        fail(`${label}: <${component}> takes a \`name\`, and the control it renders for it cannot be resolved`);
        continue;
      }
      const name = /(?:^|\s)(name="[^"]+")/.exec(attributes)?.[1] ?? 'name=';
      tags.push([null, control.tag, control.attributes.replace(/(^|\s)name=name(?=\s|$)/, `$1${name}`)]);
    }
    const named = [...body.matchAll(/(?<![\w-])name=/g)].length;
    if (tags.length !== named) {
      fail(`${label}: ${named} \`name=\` attributes, ${tags.length} of them on a form control; the others cannot be verified`);
    }
    for (const [, tag, attributes] of tags) {
      const name = /(?:^|\s)name="([^"]+)"/.exec(attributes)?.[1];
      if (!name) {
        fail(`${label}: a <${tag}> has a \`name\` that is not a string literal; the field it sends cannot be verified`);
        continue;
      }
      const type = tag === 'input' ? (/(?:^|\s)type="([a-z]+)"/.exec(attributes)?.[1] ?? 'text') : tag;
      const kind = type === 'checkbox' ? 'boolean' : type === 'number' ? 'number' : 'string';
      if (controls.has(name) && controls.get(name) !== kind) {
        fail(`${label}: field \`${name}\` is sent by controls of different types`);
      }
      controls.set(name, kind);
      if (reservedPayloadKeys.has(name)) {
        fail(`${label}: field \`${name}\` is removed from every payload by ${path.basename(adapterPath)}`);
      }
    }

    // The arm Rust takes for this form: the first dispatcher arm whose guard holds for the
    // payload the form sends, then — through that arm or through the fallback — the resolver.
    let arm;
    let undecided = false;
    for (const candidate of dispatcherArms.get(intent) ?? []) {
      const holds = candidate.guard === null ? true : guardHolds(candidate.guard, controls);
      if (holds === undefined) {
        fail(`${label}: the guard \`${candidate.guard}\` of its dispatcher arm cannot be evaluated for the fields the form sends`);
        undecided = true;
        break;
      }
      if (holds) {
        arm = candidate;
        break;
      }
    }
    if (undecided) continue;
    if (arm === undefined || delegatesToFormResolver(arm)) arm = formResolverArms.get(intent)?.[0];
    if (arm === undefined) {
      // Not resolved by the Page Builder: every host that serves the endpoint has to.
      for (const host of hostEndpoints) {
        if (!host.arms.has(intent)) {
          fail(`${label} has no handler: neither dispatch_named_intent, \`${formResolverName}\` nor ${host.file} resolves it`);
          continue;
        }
        for (const name of controls.keys()) {
          if (!host.source.includes(`.get("${name}")`)) {
            fail(`${label}: field \`${name}\` is never read by ${host.file}`);
          }
        }
      }
      continue;
    }

    const typeName = requestTypeOf(arm);
    if (typeName === undefined) {
      fail(`${label}: the request type its handler deserializes cannot be resolved`);
      continue;
    }
    if (typeName === null) {
      for (const name of controls.keys()) {
        if (!arm.text.includes(`.get("${name}")`)) fail(`${label}: field \`${name}\` is never read by its handler`);
      }
      for (const key of requiredKeysOf(arm.text)) {
        if (!controls.has(key)) {
          fail(`${label}: its handler requires \`${key}\` and the form has no control named \`${key}\``);
        }
      }
      continue;
    }
    const request = requestTypes.get(typeName);
    if (!request) {
      fail(`${label}: request type \`${typeName}\` was not found in ${adminEditorDir}`);
      continue;
    }
    for (const [name, kind] of controls) {
      const field = request.fields.get(name);
      if (!field) {
        fail(`${label}: field \`${name}\` is not a field of \`${typeName}\`; the server drops what the control sends`);
        continue;
      }
      const expected = isBooleanType(field.type) ? 'boolean' : isNumberType(field.type) ? 'number' : 'string';
      if (expected !== kind) {
        fail(
          `${label}: field \`${name}\` is sent as a JSON ${kind}, and \`${typeName}.${name}\` is \`${field.type}\`: ` +
            'the request fails to deserialize',
        );
      }
    }
    for (const [name, field] of request.fields) {
      if (!field.optional && !controls.has(name)) {
        fail(`${label}: \`${typeName}.${name}\` is required and the form has no control named \`${name}\``);
      }
    }
  }
}
if (verifiedForms === 0) fail(`${adminEditorDir}: no \`<form ${intentFormAttribute}>\` was found`);

// 6d. Both sides are exercised: the rendered forms by the admin render tests, the bootstrap in
//     a browser that loads the module the way `page_builder_browser_module` composes it.
const hostSpec = read(hostSpecPath);
for (const attribute of [...hostBoundAttributes].sort()) {
  if (!emittedAttributes.has(attribute)) continue;
  if (!adminRenderTests.includes(attribute)) {
    fail(`${adminRenderTestsLabel}: no render test asserts \`${attribute}\``);
  }
  if (!hostSpec.includes(attribute)) {
    fail(`${hostSpecPath}: no browser test exercises \`${attribute}\``);
  }
}
for (const [what, needle] of [
  [`the shipped ${adapterPath}`, adapterPath.split('/').slice(-4).join('/')],
  [`the shipped ${hostBootstrapPath}`, hostBootstrapPath.split('/').slice(-4).join('/')],
  [hostBootstrapConstant, hostBootstrapConstant],
]) {
  if (!hostSpec.includes(needle)) fail(`${hostSpecPath} must load ${what}`);
}

// ---------------------------------------------------------------------------------------------
// 7. A contract attribute on a control must not pull the control under a canvas style.
// ---------------------------------------------------------------------------------------------
//
// The workspace renders the canvas preview and the controls into one document, and the canvas
// styles are a plain `<style>` element. `data-fly-component-id` is on the canvas nodes and, for
// this contract, on the layer rows too: a canvas rule keyed on a contract attribute and not
// scoped by a class also styles the control that names the same component.
const contractAttributes = new Set([...boundAttributes, ...hostBoundAttributes]);
const workspaceProduction = productionSource(read(workspacePath));
let scopedCanvasRules = 0;
for (const match of workspaceProduction.matchAll(
  /\[(data-fly-[a-z-]+)(?:=[^\]\n]*)?\](?::[a-z-]+)*\s*\{\{?/g,
)) {
  if (!contractAttributes.has(match[1])) continue;
  const before = workspaceProduction.slice(0, match.index);
  const selectorStart = Math.max(...['"', '}', ';', '\n'].map((stop) => before.lastIndexOf(stop))) + 1;
  const scope = before.slice(selectorStart).trim();
  if (!/^\.[\w-]+(?:\s|$)/.test(scope)) {
    const line = before.split('\n').length;
    fail(
      `${workspacePath}:${line}: a style rule keyed on \`${match[1]}\` is not scoped by a class; ` +
        'it also styles every control that carries the attribute',
    );
    continue;
  }
  scopedCanvasRules += 1;
}
if (scopedCanvasRules === 0 && !failures.some((message) => message.includes('is not scoped by a class'))) {
  fail(`${workspacePath}: no canvas style rule keyed on a contract attribute was found; update this check with the source`);
}

// 8. A selection change is announced. `this.selectedId` is written by `select()`, by the listener
//    of the announcement itself, and by the resets that are not selections (constructor, unmount,
//    canvas teardown). A write of anything else, anywhere else, is a change that the forms miss.
const selectionWrites = [...adapter.matchAll(/this\.selectedId = ([^;]+);/g)].map((match) =>
  match[1].replace(/\s+/g, ' ').trim(),
);
const silentSelectionWrites = new Set(['null', 'next', 'event.detail?.componentId || null']);
for (const value of selectionWrites) {
  if (!silentSelectionWrites.has(value)) {
    fail(
      `${adapterPath}: \`this.selectedId = ${value}\` changes the selection outside select(); ` +
        'the host bootstrap is not told and the SSR forms keep the previous component',
    );
  }
}
const selectMethod = adapter.match(/\n  select\(componentId, source\) \{\n([\s\S]*?)\n  \}\n/);
if (!selectMethod) {
  fail(`${adapterPath}: the adapter has no select(componentId, source) method`);
} else {
  for (const marker of ['this.selectedId = next;', 'new CustomEvent("fly:select"', 'this.drawSelection()']) {
    if (!selectMethod[1].includes(marker)) {
      fail(`${adapterPath}: select() is missing ${marker}`);
    }
  }
}
if (selectionWrites.filter((value) => value === 'next').length !== 1) {
  fail(`${adapterPath}: exactly one write of the selection belongs to select()`);
}
const selectionCalls = [...adapter.matchAll(/this\.select\(/g)].length;
if (selectionCalls < 4) {
  fail(
    `${adapterPath}: expected the four causes of a selection (canvas focus, row click, row drag, ` +
      `delegated select-component) to call select(), found ${selectionCalls}`,
  );
}

if (failures.length > 0) {
  console.error('Fly SSR control contract verification failed:');
  for (const message of failures) console.error(`- ${message}`);
  process.exit(1);
}

console.log(
  `Fly SSR control contract verified: ${boundAttributes.size} bound attributes, ` +
    `${handledActions.size} adapter actions, ${emittedActions.size} emitted actions ` +
    `(${recordedUnhandledActions.size} recorded as unhandled); ${hostBoundAttributes.size} host-bound ` +
    `attributes, ${verifiedForms} SSR forms; ${scopedCanvasRules} scoped canvas rules; ` +
    `${selectionCalls} announced selection causes.`,
);
