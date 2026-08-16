#!/usr/bin/env node
import { createHash } from 'node:crypto';
import { createWriteStream, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { once } from 'node:events';

const TIERS = Object.freeze({
  s: 10_000,
  m: 100_000,
  l: 1_000_000,
});
const CONTRACT = 'rustok_vs_magento_fixture_v1';
const GENERATOR_VERSION = 1;
const VARIANTS_PER_PRODUCT = 2;
const ATTRIBUTE_COUNT = 8;
const SEARCH_GROUPS = 20;

function parseArgs(argv) {
  const result = {};
  for (let index = 0; index < argv.length; index += 1) {
    const item = argv[index];
    if (!item.startsWith('--')) throw new Error(`Unexpected argument '${item}'`);
    const equals = item.indexOf('=');
    if (equals >= 0) {
      result[item.slice(2, equals)] = item.slice(equals + 1);
      continue;
    }
    const key = item.slice(2);
    const next = argv[index + 1];
    if (next && !next.startsWith('--')) {
      result[key] = next;
      index += 1;
    } else {
      result[key] = true;
    }
  }
  return result;
}

function positiveInt(value, name) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new Error(`${name} must be a positive integer`);
  return parsed;
}

function stableUuid(seed, namespace, index) {
  const bytes = createHash('sha256').update(`${seed}\0${namespace}\0${index}`).digest().subarray(0, 16);
  bytes[6] = (bytes[6] & 0x0f) | 0x50;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = bytes.toString('hex');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function pad(index) {
  return String(index).padStart(8, '0');
}

function productFor(seed, index) {
  const ordinal = index + 1;
  const group = index % SEARCH_GROUPS;
  const sku = `RTBM-${pad(ordinal)}`;
  const searchToken = `bench-group-${String(group).padStart(2, '0')}`;
  const priceCents = 1_000 + ((index * 97) % 99_000);
  const attributes = {};
  for (let attr = 1; attr <= ATTRIBUTE_COUNT; attr += 1) {
    attributes[`bench_attr_${attr}`] = `v${attr}-${(index * (attr * 13 + 7)) % 997}`;
  }
  return {
    ordinal,
    shared_id: stableUuid(seed, 'product', index),
    sku,
    handle: `bench-product-${pad(ordinal)}`,
    name: `Benchmark Product ${pad(ordinal)} ${searchToken}`,
    description: `Deterministic benchmark fixture ${sku}; search token ${searchToken}.`,
    status: 'active',
    published: true,
    vendor: `benchmark-vendor-${String(index % 100).padStart(3, '0')}`,
    product_type: 'benchmark',
    currency: 'USD',
    price_cents: priceCents,
    search_token: searchToken,
    attributes,
    variants: Array.from({ length: VARIANTS_PER_PRODUCT }, (_, variantIndex) => ({
      shared_id: stableUuid(seed, `variant-${variantIndex}`, index),
      sku: `${sku}-V${variantIndex + 1}`,
      title: variantIndex === 0 ? 'Default' : 'Alternate',
      price_cents: priceCents + variantIndex * 100,
    })),
  };
}

function csvEscape(value) {
  const text = String(value ?? '');
  return /[",\n\r]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

async function writeLine(stream, line) {
  if (!stream.write(line)) await once(stream, 'drain');
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const tier = String(args.tier || 's').toLowerCase();
  if (!Object.hasOwn(TIERS, tier) && !args.count) throw new Error(`Unknown tier '${tier}'. Use s, m, l, or --count.`);
  const productCount = args.count ? positiveInt(args.count, 'count') : TIERS[tier];
  const seed = String(args.seed || 'rustok-vs-magento-v1');
  const outputDir = resolve(String(args.out || `target/loadtest-fixtures/${tier}`));
  const force = Boolean(args.force);
  const manifestPath = resolve(outputDir, 'manifest.json');
  if (existsSync(manifestPath) && !force) {
    throw new Error(`Refusing to overwrite ${manifestPath}; use a new --out or --force`);
  }
  mkdirSync(outputDir, { recursive: true });

  const productsPath = resolve(outputDir, 'products.jsonl');
  const csvPath = resolve(outputDir, 'products.csv');
  const products = createWriteStream(productsPath, { encoding: 'utf8' });
  const csv = createWriteStream(csvPath, { encoding: 'utf8' });
  const productsDigest = createHash('sha256');
  const csvDigest = createHash('sha256');
  const groupCounts = Array.from({ length: SEARCH_GROUPS }, () => 0);

  const csvHeader = [
    'shared_id', 'sku', 'handle', 'name', 'description', 'status', 'published', 'vendor',
    'product_type', 'currency', 'price_cents', 'search_token',
    ...Array.from({ length: ATTRIBUTE_COUNT }, (_, index) => `bench_attr_${index + 1}`),
    'variant_1_id', 'variant_1_sku', 'variant_1_title', 'variant_1_price_cents',
    'variant_2_id', 'variant_2_sku', 'variant_2_title', 'variant_2_price_cents',
  ].join(',') + '\n';
  csvDigest.update(csvHeader);
  await writeLine(csv, csvHeader);

  for (let index = 0; index < productCount; index += 1) {
    const product = productFor(seed, index);
    groupCounts[index % SEARCH_GROUPS] += 1;
    const jsonLine = `${JSON.stringify(product)}\n`;
    productsDigest.update(jsonLine);
    await writeLine(products, jsonLine);

    const row = [
      product.shared_id,
      product.sku,
      product.handle,
      product.name,
      product.description,
      product.status,
      product.published,
      product.vendor,
      product.product_type,
      product.currency,
      product.price_cents,
      product.search_token,
      ...Object.values(product.attributes),
      ...product.variants.flatMap((variant) => [variant.shared_id, variant.sku, variant.title, variant.price_cents]),
    ].map(csvEscape).join(',') + '\n';
    csvDigest.update(row);
    await writeLine(csv, row);
  }

  products.end();
  csv.end();
  await Promise.all([once(products, 'finish'), once(csv, 'finish')]);

  const fixtureSelection = [0, Math.floor(productCount / 2), productCount - 1]
    .map((index) => productFor(seed, index));
  const searchCases = groupCounts.map((count, group) => ({
    term: `bench-group-${String(group).padStart(2, '0')}`,
    expected_matches: count,
  }));

  const manifestCore = {
    contract: CONTRACT,
    generator_version: GENERATOR_VERSION,
    seed,
    requested_tier: Object.hasOwn(TIERS, tier) ? tier : 'custom',
    product_count: productCount,
    variants_per_product: VARIANTS_PER_PRODUCT,
    fixture_attribute_count: ATTRIBUTE_COUNT,
    search_semantics: 'shared token embedded in localized/public product title',
    search_groups: SEARCH_GROUPS,
    selection: fixtureSelection.map((product) => ({
      shared_id: product.shared_id,
      sku: product.sku,
      handle: product.handle,
      name: product.name,
      search_token: product.search_token,
    })),
    search_cases: searchCases,
    files: {
      'products.jsonl': { sha256: productsDigest.digest('hex') },
      'products.csv': { sha256: csvDigest.digest('hex') },
    },
  };
  const manifestCanonical = `${JSON.stringify(manifestCore)}\n`;
  const manifest = {
    ...manifestCore,
    manifest_core_sha256: createHash('sha256').update(manifestCanonical).digest('hex'),
  };
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');

  const firstLine = readFileSync(productsPath, 'utf8').split('\n', 1)[0];
  if (firstLine !== JSON.stringify(productFor(seed, 0))) throw new Error('Determinism self-check failed for first product');

  process.stdout.write(`${JSON.stringify({ output_dir: outputDir, product_count: productCount, manifest_core_sha256: manifest.manifest_core_sha256 })}\n`);
}

main().catch((error) => {
  console.error(error.stack || String(error));
  process.exitCode = 1;
});
