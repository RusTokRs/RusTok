#!/usr/bin/env node

// SEO redirects must behave identically on both storefront hosts (audit P2-13).
//
// The backend puts a `route.redirect` decision into every SEO page context. The Leptos
// storefront answers with `redirect_response(target, status)` as soon as a resolved context
// carries a decision. The Next storefront used to apply the same decision on only three
// pages while the remaining route families rendered the redirect context as a noindex page.
// The fix moves the application into the single `resolveSeoPageContextForRoute` choke point,
// so this verifier pins:
//   1. the choke point applies 301/308 permanently and everything else temporarily,
//   2. no page re-implements the decision locally (it would drift again),
//   3. all 16 storefront route families resolve their SEO context through the choke point,
//   4. the Leptos host still answers redirect decisions from its resolved context.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const runtime = read('apps/next-frontend/src/shared/seo/runtime.ts');
const leptosStorefront = read('apps/storefront/src/lib.rs');

// 1. Single choke point inside resolveSeoPageContextForRoute.
for (const [value, label] of [
  ['import { permanentRedirect, redirect } from "next/navigation";', 'next navigation import'],
  [
    'function applySeoRouteRedirect(resolution: SeoPageContextResolution): void {',
    'redirect application helper',
  ],
  ['resolution.context?.route.redirect', 'decision read from the resolved context'],
  ['decision.statusCode === 301 || decision.statusCode === 308', 'permanent status mapping'],
  ['permanentRedirect(decision.targetUrl);', 'permanent redirect call'],
  ['  applySeoRouteRedirect(resolution);', 'choke point invocation'],
]) {
  requireText(runtime, value, `runtime ${label}`);
}
const resolveStart = runtime.indexOf('export async function resolveSeoPageContextForRoute(');
const helperStart = runtime.indexOf('function applySeoRouteRedirect(');
if (resolveStart < 0) {
  failures.push('runtime: resolveSeoPageContextForRoute must exist');
} else if (helperStart > resolveStart) {
  failures.push('runtime: applySeoRouteRedirect helper must precede resolveSeoPageContextForRoute');
}
if (resolveStart >= 0 && runtime.indexOf('applySeoRouteRedirect(resolution);') < resolveStart) {
  failures.push('runtime: choke point must be invoked inside resolveSeoPageContextForRoute');
}

// 2. No page outside the shared runtime reads the SEO redirect decision itself.
const appDir = fileURLToPath(new URL('apps/next-frontend/src/app/', root));
const walk = (dir) =>
  readdirSync(dir).flatMap((entry) => {
    const fullPath = path.join(dir, entry);
    return statSync(fullPath).isDirectory() ? walk(fullPath) : [fullPath];
  });
for (const filePath of walk(appDir)) {
  if (!/\.(ts|tsx)$/.test(filePath)) continue;
  const relativePath = path.relative(appDir, filePath);
  const source = readFileSync(filePath, 'utf8');
  forbidText(source, 'route.redirect', `${relativePath} SEO decision read`);
  forbidText(source, 'redirectDecision', `${relativePath} local redirect decision`);
}

// 3. All 16 route families resolve their SEO context through the choke point.
const routeFamilies = [
  '[locale]/page.tsx',
  '[locale]/blog/page.tsx',
  '[locale]/blog/[slug]/page.tsx',
  '[locale]/pages/[slug]/page.tsx',
  '[locale]/products/page.tsx',
  '[locale]/products/[slug]/page.tsx',
  '[locale]/profiles/page.tsx',
  '[locale]/profiles/[handle]/page.tsx',
  '[locale]/orders/page.tsx',
  '[locale]/orders/[id]/page.tsx',
  '[locale]/account/page.tsx',
  '[locale]/account/orders/page.tsx',
  '[locale]/checkout/page.tsx',
  '[locale]/notifications/page.tsx',
  '[locale]/pricing/page.tsx',
  '[locale]/search/page.tsx',
];
for (const family of routeFamilies) {
  const source = read(`apps/next-frontend/src/app/${family}`);
  requireText(
    source,
    'resolveSeoPageContextForRoute(',
    `route family ${family} SEO resolution`,
  );
}

// 4. Leptos parity: the same decision still answers with an HTTP redirect.
for (const [value, label] of [
  ['resolved.route.redirect.is_some()', 'Leptos decision guard'],
  ['redirect_response(', 'Leptos redirect responder'],
  ['fn redirect_response(', 'Leptos responder definition'],
]) {
  requireText(leptosStorefront, value, `storefront ${label}`);
}

if (failures.length > 0) {
  console.error('SEO Next/Leptos redirect parity verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ SEO redirect decisions apply through one Next choke point on 16/16 route families with Leptos parity',
);
