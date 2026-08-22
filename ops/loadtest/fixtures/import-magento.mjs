#!/usr/bin/env node
import { appendFileSync, createReadStream, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { createInterface } from 'node:readline';
import { dirname, resolve } from 'node:path';

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const item = argv[i];
    if (!item.startsWith('--')) throw new Error(`Unexpected argument '${item}'`);
    const eq = item.indexOf('=');
    if (eq >= 0) out[item.slice(2, eq)] = item.slice(eq + 1);
    else {
      const key = item.slice(2);
      const next = argv[i + 1];
      if (next && !next.startsWith('--')) { out[key] = next; i += 1; }
      else out[key] = true;
    }
  }
  return out;
}

function positiveInt(value, fallback, name) {
  if (value == null) return fallback;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new Error(`${name} must be a positive integer`);
  return parsed;
}

function nonNegativeInt(value, fallback, name) {
  if (value == null) return fallback;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0) throw new Error(`${name} must be a non-negative integer`);
  return parsed;
}

function envRequired(name) {
  const value = String(process.env[name] || '').trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
}

function loadCompleted(receiptPath) {
  if (!existsSync(receiptPath)) return new Set();
  const completed = new Set();
  for (const line of readFileSync(receiptPath, 'utf8').split('\n')) {
    if (!line.trim()) continue;
    const item = JSON.parse(line);
    if (item.status === 'created' && Number.isSafeInteger(item.ordinal)) completed.add(item.ordinal);
  }
  return completed;
}

async function requestJson(url, method, payload, headers, retries) {
  let lastError;
  for (let attempt = 0; attempt <= retries; attempt += 1) {
    try {
      const response = await fetch(url, { method, headers, body: payload == null ? undefined : JSON.stringify(payload) });
      const body = await response.text();
      if (response.ok) return body ? JSON.parse(body) : null;
      if (response.status !== 429 && response.status < 500) throw new Error(`HTTP ${response.status}: ${body.slice(0, 1000)}`);
      lastError = new Error(`HTTP ${response.status}: ${body.slice(0, 1000)}`);
    } catch (error) {
      lastError = error;
    }
    if (attempt < retries) await new Promise((resolveDelay) => setTimeout(resolveDelay, Math.min(5000, 250 * (2 ** attempt))));
  }
  throw lastError;
}

function childUrlKey(product, variant) {
  return `${product.handle}-${variant.sku.toLowerCase()}`;
}

function childCustomAttributes(product, variant, configValue, attributeCode) {
  return [
    { attribute_code: 'description', value: product.description },
    { attribute_code: 'url_key', value: childUrlKey(product, variant) },
    { attribute_code: attributeCode, value: String(configValue) },
  ];
}

function parentPayload(product, attributeSetId) {
  return {
    product: {
      sku: product.sku,
      name: product.name,
      attribute_set_id: attributeSetId,
      status: 1,
      visibility: 4,
      type_id: 'configurable',
      weight: 1,
      extension_attributes: {
        stock_item: { qty: 10000, is_in_stock: true },
      },
      product_links: [],
      options: [],
      media_gallery_entries: [],
      tier_prices: [],
      custom_attributes: [
        { attribute_code: 'description', value: product.description },
        { attribute_code: 'url_key', value: product.handle },
      ],
    },
    saveOptions: true,
  };
}

function childPayload(product, variant, attributeSetId, configValue, attributeCode) {
  return {
    product: {
      sku: variant.sku,
      name: `${product.name} ${variant.title}`,
      attribute_set_id: attributeSetId,
      price: Number((variant.price_cents / 100).toFixed(2)),
      status: 1,
      visibility: 1,
      type_id: 'simple',
      weight: 1,
      extension_attributes: {
        stock_item: { qty: 10000, is_in_stock: true },
      },
      product_links: [],
      options: [],
      media_gallery_entries: [],
      tier_prices: [],
      custom_attributes: childCustomAttributes(product, variant, configValue, attributeCode),
    },
    saveOptions: true,
  };
}

async function importProduct(product, settings) {
  const baseProducts = `${settings.base}/V1/products`;
  await requestJson(baseProducts, 'POST', parentPayload(product, settings.attributeSetId), settings.headers, settings.retries);
  await Promise.all(product.variants.map((variant, index) => requestJson(
    baseProducts,
    'POST',
    childPayload(product, variant, settings.attributeSetId, settings.configValues[index], settings.configAttributeCode),
    settings.headers,
    settings.retries,
  )));

  await requestJson(
    `${settings.base}/V1/configurable-products/${encodeURIComponent(product.sku)}/options`,
    'POST',
    {
      option: {
        attribute_id: String(settings.configAttributeId),
        label: settings.configAttributeLabel,
        position: 0,
        is_use_default: true,
        values: settings.configValues.map((valueIndex) => ({ value_index: Number(valueIndex) })),
      },
    },
    settings.headers,
    settings.retries,
  );

  for (const variant of product.variants) {
    await requestJson(
      `${settings.base}/V1/configurable-products/${encodeURIComponent(product.sku)}/child`,
      'POST',
      { childSku: variant.sku },
      settings.headers,
      settings.retries,
    );
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const input = resolve(String(args.input || 'target/loadtest-fixtures/s/products.jsonl'));
  const receipt = resolve(String(args.receipt || `${input}.magento-receipts.jsonl`));
  const host = String(args.host || process.env.MAGENTO_BASE_URL || '').replace(/\/$/, '');
  const storeCode = String(args['store-code'] || process.env.MAGENTO_STORE_CODE || 'default');
  const token = envRequired('MAGENTO_TOKEN');
  const attributeSetId = positiveInt(process.env.MAGENTO_ATTRIBUTE_SET_ID, null, 'MAGENTO_ATTRIBUTE_SET_ID');
  const configAttributeId = positiveInt(process.env.MAGENTO_CONFIG_ATTRIBUTE_ID, null, 'MAGENTO_CONFIG_ATTRIBUTE_ID');
  const configAttributeCode = envRequired('MAGENTO_CONFIG_ATTRIBUTE_CODE');
  const configAttributeLabel = String(process.env.MAGENTO_CONFIG_ATTRIBUTE_LABEL || 'Edition');
  const configValues = [
    positiveInt(process.env.MAGENTO_CONFIG_VALUE_1, null, 'MAGENTO_CONFIG_VALUE_1'),
    positiveInt(process.env.MAGENTO_CONFIG_VALUE_2, null, 'MAGENTO_CONFIG_VALUE_2'),
  ];
  const concurrency = positiveInt(args.concurrency, 4, 'concurrency');
  const retries = nonNegativeInt(args.retries, 3, 'retries');
  const limit = args.limit ? positiveInt(args.limit, null, 'limit') : null;
  const resume = Boolean(args.resume);

  if (!host) throw new Error('--host or MAGENTO_BASE_URL is required');
  if (!existsSync(input)) throw new Error(`Input not found: ${input}`);
  if (existsSync(receipt) && !resume) throw new Error(`Receipt exists: ${receipt}; pass --resume or choose another --receipt`);
  if (new Set(configValues).size !== configValues.length) throw new Error('Magento configurable option value indexes must be distinct');
  mkdirSync(dirname(receipt), { recursive: true });
  if (!existsSync(receipt)) writeFileSync(receipt, '', 'utf8');

  const completed = resume ? loadCompleted(receipt) : new Set();
  const base = `${host}/rest/${encodeURIComponent(storeCode)}`;
  const settings = {
    base,
    attributeSetId,
    configAttributeId,
    configAttributeCode,
    configAttributeLabel,
    configValues,
    retries,
    headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json', accept: 'application/json' },
  };

  let scheduled = 0;
  let created = 0;
  let skipped = 0;
  let failed = 0;

  async function importOne(product) {
    if (completed.has(product.ordinal)) {
      skipped += 1;
      return;
    }
    try {
      await importProduct(product, settings);
      appendFileSync(receipt, `${JSON.stringify({ ordinal: product.ordinal, shared_id: product.shared_id, sku: product.sku, status: 'created' })}\n`, 'utf8');
      created += 1;
    } catch (error) {
      failed += 1;
      appendFileSync(receipt, `${JSON.stringify({ ordinal: product.ordinal, shared_id: product.shared_id, sku: product.sku, status: 'failed', error: String(error.message || error).slice(0, 2000) })}\n`, 'utf8');
      throw error;
    }
  }

  const reader = createInterface({ input: createReadStream(input, { encoding: 'utf8' }), crlfDelay: Infinity });
  let batch = [];
  for await (const line of reader) {
    if (!line.trim()) continue;
    if (limit != null && scheduled >= limit) break;
    const product = JSON.parse(line);
    if (!Array.isArray(product.variants) || product.variants.length !== configValues.length) {
      throw new Error(`Fixture ${product.sku} has ${product.variants?.length} variants; importer is configured for ${configValues.length}`);
    }
    scheduled += 1;
    batch.push(product);
    if (batch.length >= concurrency) {
      const current = batch;
      batch = [];
      await Promise.all(current.map(importOne));
    }
  }
  if (batch.length) await Promise.all(batch.map(importOne));

  process.stdout.write(`${JSON.stringify({ input, receipt, scheduled, created, skipped, failed, store_code: storeCode })}\n`);
}

main().catch((error) => { console.error(error.stack || String(error)); process.exitCode = 1; });
