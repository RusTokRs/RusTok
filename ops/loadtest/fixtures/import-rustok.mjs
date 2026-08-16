#!/usr/bin/env node
import { createReadStream, existsSync, mkdirSync, readFileSync, writeFileSync, appendFileSync } from 'node:fs';
import { createInterface } from 'node:readline';
import { dirname, resolve } from 'node:path';

function parseArgs(argv) {
  const result = {};
  for (let i = 0; i < argv.length; i += 1) {
    const item = argv[i];
    if (!item.startsWith('--')) throw new Error(`Unexpected argument '${item}'`);
    const eq = item.indexOf('=');
    if (eq >= 0) {
      result[item.slice(2, eq)] = item.slice(eq + 1);
    } else {
      const key = item.slice(2);
      const next = argv[i + 1];
      if (next && !next.startsWith('--')) {
        result[key] = next;
        i += 1;
      } else {
        result[key] = true;
      }
    }
  }
  return result;
}

function intArg(value, fallback, name) {
  if (value == null) return fallback;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new Error(`${name} must be a positive integer`);
  return parsed;
}

function decimalFromCents(cents) {
  return Number((Number(cents) / 100).toFixed(2));
}

function rustokPayload(product, locale) {
  return {
    translations: [{
      locale,
      title: product.name,
      handle: product.handle,
      description: product.description,
      meta_title: product.name,
      meta_description: product.description,
    }],
    options: [{
      translations: [{
        locale,
        name: 'Edition',
        values: product.variants.map((variant) => variant.title),
      }],
    }],
    variants: product.variants.map((variant) => ({
      sku: variant.sku,
      barcode: null,
      shipping_profile_slug: null,
      option1: variant.title,
      option2: null,
      option3: null,
      prices: [{
        currency_code: product.currency,
        channel_id: null,
        channel_slug: null,
        amount: decimalFromCents(variant.price_cents),
        compare_at_amount: null,
      }],
      inventory_quantity: 10_000,
      inventory_policy: 'deny',
      weight: null,
      weight_unit: null,
    })),
    seller_id: null,
    vendor: product.vendor,
    product_type: product.product_type,
    shipping_profile_slug: null,
    primary_category_id: null,
    tags: ['benchmark', product.search_token],
    metadata: {
      benchmark_contract: 'rustok_vs_magento_fixture_v1',
      benchmark_shared_id: product.shared_id,
      benchmark_sku: product.sku,
      benchmark_attributes: product.attributes,
    },
    publish: true,
  };
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

async function postWithRetry(url, payload, headers, retries) {
  let lastError;
  for (let attempt = 0; attempt <= retries; attempt += 1) {
    try {
      const response = await fetch(url, {
        method: 'POST',
        headers,
        body: JSON.stringify(payload),
      });
      const body = await response.text();
      if (response.ok) return { response, body };
      if (response.status !== 429 && response.status < 500) {
        throw new Error(`HTTP ${response.status}: ${body.slice(0, 1000)}`);
      }
      lastError = new Error(`HTTP ${response.status}: ${body.slice(0, 1000)}`);
    } catch (error) {
      lastError = error;
    }
    if (attempt < retries) {
      const delayMs = Math.min(5_000, 250 * (2 ** attempt));
      await new Promise((resolveDelay) => setTimeout(resolveDelay, delayMs));
    }
  }
  throw lastError;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const input = resolve(String(args.input || 'target/loadtest-fixtures/s/products.jsonl'));
  const receipt = resolve(String(args.receipt || `${input}.rustok-receipts.jsonl`));
  const url = String(args.url || process.env.RUSTOK_ADMIN_PRODUCTS_URL || '').trim();
  const token = String(process.env.RUSTOK_TOKEN || '').trim();
  const tenantId = String(process.env.RUSTOK_TENANT_ID || '').trim();
  const channel = String(process.env.RUSTOK_CHANNEL || '').trim();
  const locale = String(args.locale || process.env.RUSTOK_LOCALE || 'en').trim();
  const concurrency = intArg(args.concurrency, 16, 'concurrency');
  const retries = intArg(args.retries, 3, 'retries');
  const resume = Boolean(args.resume);
  const limit = args.limit ? intArg(args.limit, null, 'limit') : null;

  if (!url) throw new Error('--url or RUSTOK_ADMIN_PRODUCTS_URL is required');
  if (!token) throw new Error('RUSTOK_TOKEN is required');
  if (!tenantId) throw new Error('RUSTOK_TENANT_ID is required');
  if (!existsSync(input)) throw new Error(`Input not found: ${input}`);
  if (existsSync(receipt) && !resume) throw new Error(`Receipt exists: ${receipt}; pass --resume or choose another --receipt`);
  mkdirSync(dirname(receipt), { recursive: true });
  if (!existsSync(receipt)) writeFileSync(receipt, '', 'utf8');

  const completed = resume ? loadCompleted(receipt) : new Set();
  const headers = {
    'content-type': 'application/json',
    accept: 'application/json',
    authorization: `Bearer ${token}`,
    'x-tenant-id': tenantId,
  };
  if (channel) headers['x-channel'] = channel;

  const queue = [];
  const inFlight = new Set();
  let scheduled = 0;
  let created = 0;
  let skipped = 0;
  let failed = 0;

  async function schedule(product) {
    if (completed.has(product.ordinal)) {
      skipped += 1;
      return;
    }
    const task = (async () => {
      try {
        const { response, body } = await postWithRetry(url, rustokPayload(product, locale), headers, retries);
        const parsed = JSON.parse(body);
        appendFileSync(receipt, `${JSON.stringify({
          ordinal: product.ordinal,
          shared_id: product.shared_id,
          sku: product.sku,
          rustok_product_id: parsed.id,
          status: 'created',
          http_status: response.status,
        })}\n`, 'utf8');
        created += 1;
      } catch (error) {
        failed += 1;
        appendFileSync(receipt, `${JSON.stringify({
          ordinal: product.ordinal,
          shared_id: product.shared_id,
          sku: product.sku,
          status: 'failed',
          error: String(error.message || error).slice(0, 2000),
        })}\n`, 'utf8');
        throw error;
      }
    })();
    inFlight.add(task);
    task.finally(() => inFlight.delete(task));
    if (inFlight.size >= concurrency) await Promise.race(inFlight);
  }

  const reader = createInterface({ input: createReadStream(input, { encoding: 'utf8' }), crlfDelay: Infinity });
  try {
    for await (const line of reader) {
      if (!line.trim()) continue;
      const product = JSON.parse(line);
      if (limit != null && scheduled >= limit) break;
      scheduled += 1;
      queue.push(schedule(product));
      if (queue.length >= concurrency * 4) {
        await Promise.all(queue.splice(0, queue.length));
      }
    }
    await Promise.all(queue);
    await Promise.all(inFlight);
  } catch (error) {
    await Promise.allSettled([...queue, ...inFlight]);
    throw error;
  }

  process.stdout.write(`${JSON.stringify({ input, receipt, scheduled, created, skipped, failed })}\n`);
}

main().catch((error) => {
  console.error(error.stack || String(error));
  process.exitCode = 1;
});
