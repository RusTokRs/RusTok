import http from 'k6/http';
import { check } from 'k6';
import { Rate } from 'k6/metrics';

const CONFIG_PATH = __ENV.CONFIG;
if (!CONFIG_PATH) {
  throw new Error('CONFIG is required');
}

const config = JSON.parse(open(`../${CONFIG_PATH}`));
const baseUrl = String(__ENV.BASE_URL || config.baseUrl || '').replace(/\/$/, '');
if (!baseUrl) {
  throw new Error('BASE_URL or config.baseUrl is required');
}

const operation = (__ENV.OPERATION || 'mixed').toLowerCase();
const supportedOperations = new Set(['catalog', 'product', 'search', 'mixed']);
if (!supportedOperations.has(operation)) {
  throw new Error(`Unsupported OPERATION '${operation}'`);
}

const measuredRate = Math.max(1, Number(__ENV.RATE || 100));
const warmupRate = Math.max(1, Number(__ENV.WARMUP_RATE || Math.ceil(measuredRate / 4)));
const warmupDuration = __ENV.WARMUP || '30s';
const measuredDuration = __ENV.DURATION || '3m';
const preAllocatedVUs = Math.max(1, Number(__ENV.PRE_ALLOCATED_VUS || 64));
const maxVUs = Math.max(preAllocatedVUs, Number(__ENV.MAX_VUS || 2048));

export const responseValidationFailures = new Rate('response_validation_failures');

const workloadTags = { benchmark: 'rustok_vs_magento', platform: config.name || 'unknown' };

export const options = {
  discardResponseBodies: false,
  scenarios: {
    warmup: {
      executor: 'constant-arrival-rate',
      rate: warmupRate,
      timeUnit: '1s',
      duration: warmupDuration,
      preAllocatedVUs,
      maxVUs,
      exec: 'warmup',
      tags: { ...workloadTags, phase: 'warmup' },
    },
    measure: {
      executor: 'constant-arrival-rate',
      rate: measuredRate,
      timeUnit: '1s',
      duration: measuredDuration,
      startTime: warmupDuration,
      preAllocatedVUs,
      maxVUs,
      exec: 'measure',
      tags: { ...workloadTags, phase: 'measure' },
    },
  },
  thresholds: {
    'http_req_failed{phase:measure}': ['rate<0.001'],
    'http_req_duration{phase:measure}': ['p(95)<250', 'p(99)<500'],
    'response_validation_failures{phase:measure}': ['rate<0.001'],
  },
};

const placeholders = {
  PRODUCT_ID: __ENV.PRODUCT_ID || '',
  PRODUCT_SKU: __ENV.PRODUCT_SKU || '',
  SEARCH_TERM: __ENV.SEARCH_TERM || 'shirt',
  TENANT_ID: __ENV.TENANT_ID || '',
  CHANNEL: __ENV.CHANNEL || '',
};

function interpolate(value) {
  if (typeof value !== 'string') return value;
  return Object.entries(placeholders).reduce(
    (result, [key, replacement]) => result.split(`{{${key}}}`).join(replacement),
    value,
  );
}

function headersFor(descriptor) {
  const headers = {};
  for (const [key, value] of Object.entries(config.headers || {})) {
    const resolved = interpolate(value);
    if (resolved !== '') headers[key] = resolved;
  }
  for (const [key, value] of Object.entries(descriptor.headers || {})) {
    const resolved = interpolate(value);
    if (resolved !== '') headers[key] = resolved;
  }
  return headers;
}

function requestDescriptor(name) {
  const descriptor = config.operations?.[name];
  if (!descriptor) throw new Error(`Operation '${name}' is missing from ${CONFIG_PATH}`);
  return descriptor;
}

function execute(name, phase) {
  const descriptor = requestDescriptor(name);
  const method = String(descriptor.method || 'GET').toUpperCase();
  const url = `${baseUrl}${interpolate(descriptor.path || '')}`;
  const body = descriptor.body == null ? null : interpolate(JSON.stringify(descriptor.body));
  const params = {
    headers: headersFor(descriptor),
    tags: { operation: name, phase, platform: config.name || 'unknown' },
    timeout: descriptor.timeout || '10s',
  };

  const response = http.request(method, url, body, params);
  const expectedStatus = Number(descriptor.expectedStatus || 200);
  const requiredBodyFragments = descriptor.requiredBodyFragments || [];

  let valid = check(response, {
    [`${name}: status ${expectedStatus}`]: (r) => r.status === expectedStatus,
    [`${name}: non-empty body`]: (r) => typeof r.body === 'string' && r.body.length > 0,
    [`${name}: no GraphQL errors`]: (r) => !String(r.body || '').includes('"errors":'),
  });

  for (const fragment of requiredBodyFragments) {
    const resolved = interpolate(fragment);
    const fragmentValid = check(response, {
      [`${name}: body contains ${resolved}`]: (r) => String(r.body || '').includes(resolved),
    });
    valid = valid && fragmentValid;
  }

  responseValidationFailures.add(!valid, { phase, operation: name, platform: config.name || 'unknown' });
}

function chooseOperation() {
  if (operation !== 'mixed') return operation;

  // Deterministic mix by VU/iteration avoids random-run drift between platforms.
  const bucket = ((__VU * 131 + __ITER * 17) % 100);
  if (bucket < 50) return 'catalog';
  if (bucket < 85) return 'product';
  return 'search';
}

export function warmup() {
  execute(chooseOperation(), 'warmup');
}

export function measure() {
  execute(chooseOperation(), 'measure');
}

export function handleSummary(data) {
  const metadata = {
    contract: 'rustok_vs_magento_read_v1',
    platform: config.name || 'unknown',
    config: CONFIG_PATH,
    base_url: baseUrl,
    operation,
    requested_rps: measuredRate,
    warmup_rps: warmupRate,
    warmup_duration: warmupDuration,
    measured_duration: measuredDuration,
  };

  return {
    stdout: `${JSON.stringify(metadata)}\n`,
    'summary.json': JSON.stringify({ metadata, k6: data }, null, 2),
  };
}
