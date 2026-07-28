#!/usr/bin/env node

import { createHash } from 'node:crypto';
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import { dirname, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = fileURLToPath(new URL('../../', import.meta.url));
const contractPath =
  'crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-transport-parity-execution-contract.json';
const expectedRunnerPath =
  'scripts/evidence/capture-fulfillment-lifecycle-transport-parity.mjs';
const expectedVerifierPath =
  'scripts/verify/verify-fulfillment-lifecycle-transport-parity-capture.mjs';
const expectedEvidencePath =
  'crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-transport-parity-execution.json';
const expectedSourceFiles = [
  'apps/server/src/controllers/graphql.rs',
  'crates/rustok-commerce/src/graphql/query.rs',
  'crates/rustok-commerce/src/graphql/safe_query.rs',
  'crates/rustok-commerce/src/graphql_runtime.rs',
  'crates/rustok-commerce/src/controllers/admin/fulfillments.rs',
  'crates/rustok-fulfillment/src/fulfillment_read.rs',
  'crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-read-port-source.json',
];
const expectedScenarioIds = [
  'lookup_rest_detail_projection_parity',
  'filtered_list_projection_parity',
  'latest_by_order_projection_parity',
  'optional_not_found_transport_policy',
];
const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), 'utf8'));
const outputPath = resolve(repoRoot, contract.evidence_path);
const maximumResponseBytes = contract.request_policy.maximum_response_bytes;

const projectionSelection = `
  id
  tenantId
  orderId
  shippingOptionId
  customerId
  status
  carrier
  trackingNumber
  deliveredNote
  cancellationReason
  createdAt
  updatedAt
  shippedAt
  deliveredAt
  cancelledAt
  items {
    id
    fulfillmentId
    orderLineItemId
    quantity
    shippedQuantity
    deliveredQuantity
    createdAt
    updatedAt
  }
`;

function fail(message) {
  throw new Error(message);
}

function sameRecord(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function sha256(value) {
  return createHash('sha256').update(value).digest('hex');
}

function repositoryPath(relativePath) {
  const root = resolve(repoRoot) + sep;
  const candidate = resolve(repoRoot, relativePath);
  if (!candidate.startsWith(root)) fail(`repository path escapes capture root: ${relativePath}`);
  return candidate;
}

function fileSha256(relativePath) {
  return sha256(readFileSync(repositoryPath(relativePath)));
}

function sourceHashes() {
  return Object.fromEntries(
    expectedSourceFiles.map((relativePath) => [relativePath, fileSha256(relativePath)]),
  );
}

function oneLine(value, field, maximumLength = 4096) {
  if (typeof value !== 'string') fail(`${field} must be a string`);
  const line = value.trim();
  if (
    line.length === 0 ||
    line.length > maximumLength ||
    /[\u0000-\u001f\u007f]/u.test(line)
  ) {
    fail(`${field} is missing or outside the capture boundary`);
  }
  return line;
}

function requiredEnvironment(name, maximumLength = 4096) {
  return oneLine(process.env[name] ?? '', name, maximumLength);
}

function optionalEnvironment(name, maximumLength = 4096) {
  return oneLine(
    process.env[name] ?? contract.optional_environment[name],
    name,
    maximumLength,
  );
}

function positiveInteger(value, field, maximum) {
  if (!/^\d+$/u.test(value)) fail(`${field} must be a positive integer`);
  const parsed = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(parsed) || parsed < 1 || parsed > maximum) {
    fail(`${field} must be between 1 and ${maximum}`);
  }
  return parsed;
}

function uuid(value, field) {
  const parsed = oneLine(value, field, 36).toLowerCase();
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u.test(parsed)) {
    fail(`${field} must be a canonical UUID`);
  }
  return parsed;
}

function gitRevision(value, field) {
  const parsed = oneLine(value, field, 40).toLowerCase();
  if (!/^[0-9a-f]{40}$/u.test(parsed)) {
    fail(`${field} must be a 40-character Git revision`);
  }
  return parsed;
}

function headerName(value, field) {
  const parsed = oneLine(value, field, 128);
  if (!/^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/u.test(parsed)) {
    fail(`${field} must be a valid HTTP header name`);
  }
  return parsed;
}

function tenantHeaderName(value, field) {
  const parsed = headerName(value, field);
  const reserved = new Set([
    'accept',
    'accept-language',
    'authorization',
    'connection',
    'content-length',
    'content-type',
    'cookie',
    'host',
    'proxy-authorization',
    'transfer-encoding',
  ]);
  if (reserved.has(parsed.toLowerCase())) fail(`${field} must not override a reserved HTTP header`);
  return parsed;
}

function isLocalCaptureHost(hostname) {
  const normalized = hostname.toLowerCase();
  return ['localhost', '127.0.0.1', '[::1]', '::1'].includes(normalized);
}

function endpoint(value, field) {
  const parsed = new URL(oneLine(value, field));
  if (!['http:', 'https:'].includes(parsed.protocol)) {
    fail(`${field} must use http or https`);
  }
  if (parsed.username || parsed.password || parsed.search || parsed.hash) {
    fail(`${field} must not contain credentials, query, or fragment`);
  }
  if (parsed.protocol === 'http:' && !isLocalCaptureHost(parsed.hostname)) {
    fail(`${field} must use https unless the mounted endpoint is localhost or loopback`);
  }
  return parsed;
}

function sanitizedEndpoint(value) {
  return `${value.origin}${value.pathname}`;
}

function authorizationHeader(value) {
  const token = oneLine(value, 'RUSTOK_FULFILLMENT_PARITY_AUTH_TOKEN', 8192);
  return /^Bearer\s+/iu.test(token) ? token : `Bearer ${token}`;
}

function assertObject(value, field) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    fail(`${field} must be a JSON object`);
  }
  return value;
}

function assertArray(value, field) {
  if (!Array.isArray(value)) fail(`${field} must be a JSON array`);
  return value;
}

function requiredString(value, field) {
  return oneLine(value, field, 4096);
}

function optionalString(value, field) {
  if (value === null || value === undefined) return null;
  if (typeof value !== 'string') fail(`${field} must be a string or null`);
  const line = value.trim();
  if (line.length > 4096 || /[\u0000-\u001f\u007f]/u.test(line)) {
    fail(`${field} is outside the capture boundary`);
  }
  return line;
}

function timestamp(value, field) {
  const raw = requiredString(value, field);
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/u.test(raw)) {
    fail(`${field} must be an RFC3339 timestamp`);
  }
  const milliseconds = Date.parse(raw);
  if (!Number.isFinite(milliseconds)) fail(`${field} must be an RFC3339 timestamp`);
  return new Date(milliseconds).toISOString();
}

function optionalTimestamp(value, field) {
  if (value === null || value === undefined) return null;
  return timestamp(value, field);
}

function requiredInteger(value, field) {
  if (!Number.isSafeInteger(value)) fail(`${field} must be an integer`);
  return value;
}

function requiredBoolean(value, field) {
  if (typeof value !== 'boolean') fail(`${field} must be a boolean`);
  return value;
}

function sortedObject(value) {
  if (Array.isArray(value)) return value.map(sortedObject);
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, sortedObject(value[key])]),
    );
  }
  return value;
}

function stableJson(value) {
  return JSON.stringify(sortedObject(value));
}

function projectionHash(value) {
  return sha256(stableJson(value));
}

function normalizeItems(items, flavor, field) {
  return assertArray(items, field)
    .map((item, index) => {
      const source = assertObject(item, `${field}[${index}]`);
      const camel = flavor === 'graphql';
      return {
        id: uuid(source.id, `${field}[${index}].id`),
        fulfillment_id: uuid(
          source[camel ? 'fulfillmentId' : 'fulfillment_id'],
          `${field}[${index}].fulfillment_id`,
        ),
        order_line_item_id: uuid(
          source[camel ? 'orderLineItemId' : 'order_line_item_id'],
          `${field}[${index}].order_line_item_id`,
        ),
        quantity: requiredInteger(source.quantity, `${field}[${index}].quantity`),
        shipped_quantity: requiredInteger(
          source[camel ? 'shippedQuantity' : 'shipped_quantity'],
          `${field}[${index}].shipped_quantity`,
        ),
        delivered_quantity: requiredInteger(
          source[camel ? 'deliveredQuantity' : 'delivered_quantity'],
          `${field}[${index}].delivered_quantity`,
        ),
        created_at: timestamp(
          source[camel ? 'createdAt' : 'created_at'],
          `${field}[${index}].created_at`,
        ),
        updated_at: timestamp(
          source[camel ? 'updatedAt' : 'updated_at'],
          `${field}[${index}].updated_at`,
        ),
      };
    })
    .sort((left, right) => left.id.localeCompare(right.id));
}

function normalizeProjection(value, flavor, field) {
  const source = assertObject(value, field);
  const camel = flavor === 'graphql';
  return {
    id: uuid(source.id, `${field}.id`),
    tenant_id: uuid(source[camel ? 'tenantId' : 'tenant_id'], `${field}.tenant_id`),
    order_id: uuid(source[camel ? 'orderId' : 'order_id'], `${field}.order_id`),
    shipping_option_id:
      source[camel ? 'shippingOptionId' : 'shipping_option_id'] === null
        ? null
        : uuid(
            source[camel ? 'shippingOptionId' : 'shipping_option_id'],
            `${field}.shipping_option_id`,
          ),
    customer_id:
      source[camel ? 'customerId' : 'customer_id'] === null
        ? null
        : uuid(source[camel ? 'customerId' : 'customer_id'], `${field}.customer_id`),
    status: requiredString(source.status, `${field}.status`),
    carrier: optionalString(source.carrier, `${field}.carrier`),
    tracking_number: optionalString(
      source[camel ? 'trackingNumber' : 'tracking_number'],
      `${field}.tracking_number`,
    ),
    delivered_note: optionalString(
      source[camel ? 'deliveredNote' : 'delivered_note'],
      `${field}.delivered_note`,
    ),
    cancellation_reason: optionalString(
      source[camel ? 'cancellationReason' : 'cancellation_reason'],
      `${field}.cancellation_reason`,
    ),
    items: normalizeItems(source.items, flavor, `${field}.items`),
    created_at: timestamp(
      source[camel ? 'createdAt' : 'created_at'],
      `${field}.created_at`,
    ),
    updated_at: timestamp(
      source[camel ? 'updatedAt' : 'updated_at'],
      `${field}.updated_at`,
    ),
    shipped_at: optionalTimestamp(
      source[camel ? 'shippedAt' : 'shipped_at'],
      `${field}.shipped_at`,
    ),
    delivered_at: optionalTimestamp(
      source[camel ? 'deliveredAt' : 'delivered_at'],
      `${field}.delivered_at`,
    ),
    cancelled_at: optionalTimestamp(
      source[camel ? 'cancelledAt' : 'cancelled_at'],
      `${field}.cancelled_at`,
    ),
  };
}

function requireEqual(left, right, label) {
  if (stableJson(left) !== stableJson(right)) fail(`${label} mismatch`);
}

function validateContract() {
  if (
    contract.schema_version !== 1 ||
    contract.module !== 'fulfillment' ||
    contract.packet !== 'fulfillment-lifecycle-transport-parity-execution-contract' ||
    contract.status !== 'runtime_execution_contract_locked'
  ) {
    fail('fulfillment lifecycle parity contract identity drift');
  }
  if (
    contract.runner !== expectedRunnerPath ||
    contract.verifier !== expectedVerifierPath ||
    contract.evidence_path !== expectedEvidencePath ||
    contract.evidence_status !== 'runtime_execution_pending'
  ) {
    fail('fulfillment lifecycle parity tooling boundary drift');
  }
  if (!sameRecord(contract.source_files, expectedSourceFiles)) {
    fail('fulfillment lifecycle parity source allowlist drift');
  }
  if (!sameRecord(contract.scenarios.map((scenario) => scenario.id), expectedScenarioIds)) {
    fail('fulfillment lifecycle parity scenario allowlist drift');
  }
  if (
    maximumResponseBytes !== 1048576 ||
    contract.request_policy.graphql_method !== 'POST' ||
    contract.request_policy.rest_method !== 'GET' ||
    contract.request_policy.allow_http_for_local_capture !== true
  ) {
    fail('fulfillment lifecycle parity request policy drift');
  }
  if (
    contract.retained_boundary.bearer_token_retained !== false ||
    contract.retained_boundary.raw_response_bodies_retained !== false ||
    contract.retained_boundary.fulfillment_metadata_retained !== false ||
    contract.retained_boundary.transport_projection_parity_requires_successful_capture !== true ||
    contract.retained_boundary.restart_deadline_failure_and_remote_adapter_evidence_separate !== true
  ) {
    fail('fulfillment lifecycle parity retained boundary drift');
  }
}

function ensureOutputBoundary() {
  const root = resolve(repoRoot) + sep;
  if (!outputPath.startsWith(root)) {
    fail('parity evidence path must stay inside the repository');
  }
  if (existsSync(outputPath)) {
    fail('parity evidence already exists; remove it explicitly before a new capture');
  }
}

async function requestJson(url, options, timeoutMs, operation) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  const startedAt = performance.now();
  try {
    const response = await fetch(url, {
      ...options,
      redirect: 'error',
      signal: controller.signal,
    });
    const declaredLength = response.headers.get('content-length');
    if (declaredLength && Number.parseInt(declaredLength, 10) > maximumResponseBytes) {
      fail(`${operation} response exceeds the retained capture boundary`);
    }
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (bytes.byteLength > maximumResponseBytes) {
      fail(`${operation} response exceeds the retained capture boundary`);
    }
    let body;
    try {
      body = JSON.parse(new TextDecoder().decode(bytes));
    } catch {
      fail(`${operation} did not return JSON`);
    }
    return {
      status: response.status,
      duration_ms: Math.max(0, Math.round(performance.now() - startedAt)),
      body,
    };
  } catch (error) {
    if (error?.name === 'AbortError') {
      fail(`${operation} exceeded the client capture timeout`);
    }
    throw error;
  } finally {
    clearTimeout(timeout);
  }
}

function graphqlErrors(body) {
  const errors = body?.errors;
  if (errors === undefined) return [];
  return assertArray(errors, 'GraphQL errors').map((error, index) => ({
    code: optionalString(error?.extensions?.code, `GraphQL errors[${index}].code`),
  }));
}

async function graphqlRequest(graphqlUrl, headers, timeoutMs, operation, query, variables) {
  const response = await requestJson(
    graphqlUrl,
    {
      method: 'POST',
      headers: { ...headers, 'content-type': 'application/json' },
      body: JSON.stringify({ query, variables }),
    },
    timeoutMs,
    operation,
  );
  if (response.status !== 200) fail(`${operation} returned HTTP ${response.status}`);
  const body = assertObject(response.body, `${operation} response`);
  const errors = graphqlErrors(body);
  if (errors.length > 0) {
    const codes = errors.map((error) => error.code ?? 'unclassified').join(',');
    fail(`${operation} returned GraphQL errors: ${codes}`);
  }
  return { ...response, body };
}

function restUrl(restBaseUrl, path) {
  const base = restBaseUrl.href.endsWith('/')
    ? restBaseUrl.href.slice(0, -1)
    : restBaseUrl.href;
  return new URL(`${base}${path}`);
}

function publicErrorCode(body) {
  if (typeof body?.code === 'string') return body.code;
  if (typeof body?.error?.code === 'string') return body.error.code;
  return null;
}

function writeEvidence(packet) {
  mkdirSync(dirname(outputPath), { recursive: true });
  const temporaryPath = `${outputPath}.tmp-${process.pid}`;
  try {
    writeFileSync(temporaryPath, `${JSON.stringify(packet, null, 2)}\n`, { flag: 'wx' });
    renameSync(temporaryPath, outputPath);
  } catch (error) {
    if (existsSync(temporaryPath)) unlinkSync(temporaryPath);
    throw error;
  }
}

async function main() {
  validateContract();
  ensureOutputBoundary();

  const graphqlUrl = endpoint(
    requiredEnvironment('RUSTOK_FULFILLMENT_PARITY_GRAPHQL_URL'),
    'RUSTOK_FULFILLMENT_PARITY_GRAPHQL_URL',
  );
  const restBaseUrl = endpoint(
    requiredEnvironment('RUSTOK_FULFILLMENT_PARITY_REST_BASE_URL'),
    'RUSTOK_FULFILLMENT_PARITY_REST_BASE_URL',
  );
  const tenantId = uuid(
    requiredEnvironment('RUSTOK_FULFILLMENT_PARITY_TENANT_ID', 36),
    'RUSTOK_FULFILLMENT_PARITY_TENANT_ID',
  );
  const detailId = uuid(
    requiredEnvironment('RUSTOK_FULFILLMENT_PARITY_DETAIL_ID', 36),
    'RUSTOK_FULFILLMENT_PARITY_DETAIL_ID',
  );
  const orderId = uuid(
    requiredEnvironment('RUSTOK_FULFILLMENT_PARITY_ORDER_ID', 36),
    'RUSTOK_FULFILLMENT_PARITY_ORDER_ID',
  );
  const latestId = uuid(
    requiredEnvironment('RUSTOK_FULFILLMENT_PARITY_LATEST_ID', 36),
    'RUSTOK_FULFILLMENT_PARITY_LATEST_ID',
  );
  const missingId = uuid(
    requiredEnvironment('RUSTOK_FULFILLMENT_PARITY_MISSING_ID', 36),
    'RUSTOK_FULFILLMENT_PARITY_MISSING_ID',
  );
  if ([detailId, latestId].includes(missingId)) {
    fail('RUSTOK_FULFILLMENT_PARITY_MISSING_ID must differ from retained fulfillment ids');
  }
  const status = requiredEnvironment('RUSTOK_FULFILLMENT_PARITY_STATUS', 64);
  const sourceRevision = gitRevision(
    requiredEnvironment('RUSTOK_FULFILLMENT_PARITY_SOURCE_REVISION', 40),
    'RUSTOK_FULFILLMENT_PARITY_SOURCE_REVISION',
  );
  const adapterProfile = requiredEnvironment(
    'RUSTOK_FULFILLMENT_PARITY_ADAPTER_PROFILE',
    128,
  );
  const tenantHeader = tenantHeaderName(
    optionalEnvironment('RUSTOK_FULFILLMENT_PARITY_TENANT_HEADER', 128),
    'RUSTOK_FULFILLMENT_PARITY_TENANT_HEADER',
  );
  const locale = optionalEnvironment('RUSTOK_FULFILLMENT_PARITY_LOCALE', 64);
  const page = positiveInteger(
    optionalEnvironment('RUSTOK_FULFILLMENT_PARITY_PAGE', 8),
    'page',
    1000000,
  );
  const perPage = positiveInteger(
    optionalEnvironment('RUSTOK_FULFILLMENT_PARITY_PER_PAGE', 8),
    'per_page',
    100,
  );
  const timeoutMs = positiveInteger(
    optionalEnvironment('RUSTOK_FULFILLMENT_PARITY_TIMEOUT_MS', 8),
    'timeout_ms',
    120000,
  );
  const runtimeInstance = optionalEnvironment(
    'RUSTOK_FULFILLMENT_PARITY_RUNTIME_INSTANCE',
    256,
  );
  const headers = {
    accept: 'application/json',
    'accept-language': locale,
    authorization: authorizationHeader(
      requiredEnvironment('RUSTOK_FULFILLMENT_PARITY_AUTH_TOKEN', 8192),
    ),
    [tenantHeader]: tenantId,
  };

  const lookupQuery = `
    query FulfillmentLifecycleParityLookup($tenantId: UUID!, $id: UUID!) {
      lookup: fulfillment(tenantId: $tenantId, id: $id) {${projectionSelection}}
    }
  `;
  const listQuery = `
    query FulfillmentLifecycleParityList($tenantId: UUID!, $filter: FulfillmentsFilter) {
      list: fulfillments(tenantId: $tenantId, filter: $filter) {
        items {${projectionSelection}}
        total
        page
        perPage
        hasNext
      }
    }
  `;
  const latestQuery = `
    query FulfillmentLifecycleParityLatest($tenantId: UUID!, $id: UUID!) {
      order(tenantId: $tenantId, id: $id) {
        fulfillment {${projectionSelection}}
      }
    }
  `;

  const graphqlLookup = await graphqlRequest(
    graphqlUrl,
    headers,
    timeoutMs,
    'GraphQL fulfillment lookup',
    lookupQuery,
    { tenantId, id: detailId },
  );
  const graphqlLookupProjection = normalizeProjection(
    graphqlLookup.body.data?.lookup,
    'graphql',
    'GraphQL fulfillment lookup',
  );
  const restDetail = await requestJson(
    restUrl(restBaseUrl, `/admin/fulfillments/${detailId}`),
    { method: 'GET', headers },
    timeoutMs,
    'REST fulfillment detail',
  );
  if (restDetail.status !== 200) {
    fail(`REST fulfillment detail returned HTTP ${restDetail.status}`);
  }
  const restDetailProjection = normalizeProjection(
    restDetail.body,
    'rest',
    'REST fulfillment detail',
  );
  requireEqual(
    graphqlLookupProjection,
    restDetailProjection,
    'lookup/detail projection parity',
  );
  if (
    graphqlLookupProjection.id !== detailId ||
    graphqlLookupProjection.tenant_id !== tenantId ||
    graphqlLookupProjection.order_id !== orderId
  ) {
    fail('lookup/detail projection does not match the configured tenant, order, and fulfillment');
  }

  const graphqlList = await graphqlRequest(
    graphqlUrl,
    headers,
    timeoutMs,
    'GraphQL fulfillment list',
    listQuery,
    { tenantId, filter: { status, orderId, page, perPage } },
  );
  const graphqlListBody = assertObject(
    graphqlList.body.data?.list,
    'GraphQL fulfillment list',
  );
  const graphqlListProjections = assertArray(
    graphqlListBody.items,
    'GraphQL fulfillment list items',
  ).map((item, index) =>
    normalizeProjection(item, 'graphql', `GraphQL list[${index}]`),
  );

  const restListUrl = restUrl(restBaseUrl, '/admin/fulfillments');
  restListUrl.searchParams.set('status', status);
  restListUrl.searchParams.set('order_id', orderId);
  restListUrl.searchParams.set('page', String(page));
  restListUrl.searchParams.set('per_page', String(perPage));
  const restList = await requestJson(
    restListUrl,
    { method: 'GET', headers },
    timeoutMs,
    'REST fulfillment list',
  );
  if (restList.status !== 200) {
    fail(`REST fulfillment list returned HTTP ${restList.status}`);
  }
  const restListBody = assertObject(restList.body, 'REST fulfillment list');
  const restListProjections = assertArray(
    restListBody.data,
    'REST fulfillment list data',
  ).map((item, index) => normalizeProjection(item, 'rest', `REST list[${index}]`));
  requireEqual(
    graphqlListProjections,
    restListProjections,
    'filtered list projection parity',
  );

  const restMeta = assertObject(restListBody.meta, 'REST fulfillment list meta');
  const graphqlTotal = requiredInteger(graphqlListBody.total, 'GraphQL list total');
  const graphqlPage = requiredInteger(graphqlListBody.page, 'GraphQL list page');
  const graphqlPerPage = requiredInteger(
    graphqlListBody.perPage,
    'GraphQL list perPage',
  );
  const graphqlHasNext = requiredBoolean(
    graphqlListBody.hasNext,
    'GraphQL list hasNext',
  );
  const restHasNext = requiredBoolean(restMeta.has_next, 'REST list has_next');
  requireEqual(
    graphqlTotal,
    requiredInteger(restMeta.total, 'REST list total'),
    'list total parity',
  );
  requireEqual(
    graphqlPage,
    requiredInteger(restMeta.page, 'REST list page'),
    'list page parity',
  );
  requireEqual(
    graphqlPerPage,
    requiredInteger(restMeta.per_page, 'REST list per_page'),
    'list per-page parity',
  );
  requireEqual(graphqlHasNext, restHasNext, 'list has-next parity');
  if (graphqlPage !== page || graphqlPerPage !== perPage) {
    fail('configured list pagination was not retained');
  }
  if (!graphqlListProjections.some((projection) => projection.id === detailId)) {
    fail('configured detail fulfillment is absent from the filtered parity list');
  }

  const graphqlLatest = await graphqlRequest(
    graphqlUrl,
    headers,
    timeoutMs,
    'GraphQL latest fulfillment by order',
    latestQuery,
    { tenantId, id: orderId },
  );
  const orderDetail = assertObject(
    graphqlLatest.body.data?.order,
    'GraphQL order detail',
  );
  const graphqlLatestProjection = normalizeProjection(
    orderDetail.fulfillment,
    'graphql',
    'GraphQL latest fulfillment',
  );
  if (graphqlLatestProjection.id !== latestId) {
    fail('GraphQL latest fulfillment id does not match RUSTOK_FULFILLMENT_PARITY_LATEST_ID');
  }
  const restLatest = await requestJson(
    restUrl(restBaseUrl, `/admin/fulfillments/${latestId}`),
    { method: 'GET', headers },
    timeoutMs,
    'REST latest fulfillment detail',
  );
  if (restLatest.status !== 200) {
    fail(`REST latest fulfillment detail returned HTTP ${restLatest.status}`);
  }
  const restLatestProjection = normalizeProjection(
    restLatest.body,
    'rest',
    'REST latest fulfillment detail',
  );
  requireEqual(
    graphqlLatestProjection,
    restLatestProjection,
    'latest-by-order projection parity',
  );

  const graphqlMissing = await graphqlRequest(
    graphqlUrl,
    headers,
    timeoutMs,
    'GraphQL missing fulfillment lookup',
    lookupQuery,
    { tenantId, id: missingId },
  );
  if (graphqlMissing.body.data?.lookup !== null) {
    fail('GraphQL missing fulfillment lookup must return null');
  }
  const restMissing = await requestJson(
    restUrl(restBaseUrl, `/admin/fulfillments/${missingId}`),
    { method: 'GET', headers },
    timeoutMs,
    'REST missing fulfillment detail',
  );
  const restMissingCode = publicErrorCode(restMissing.body);
  if (restMissing.status !== 404 || restMissingCode !== 'commerce_admin_not_found') {
    fail('REST missing fulfillment detail must return 404 commerce_admin_not_found');
  }

  const listIds = graphqlListProjections.map((projection) => projection.id);
  const packet = {
    schema_version: 1,
    module: 'fulfillment',
    packet: 'fulfillment-lifecycle-transport-parity-execution',
    status: 'transport_projection_parity_captured_unreviewed',
    captured_at: new Date().toISOString(),
    contract: {
      path: contractPath,
      sha256: fileSha256(contractPath),
      source_base_revision: contract.source_base_revision,
    },
    runtime_claims: {
      claimed_source_revision: sourceRevision,
      claimed_adapter_profile: adapterProfile,
      claimed_runtime_instance: runtimeInstance,
      claims_verified_by_runner: false,
      graphql_endpoint: sanitizedEndpoint(graphqlUrl),
      rest_base_endpoint: sanitizedEndpoint(restBaseUrl),
      tenant_id: tenantId,
      locale,
      page,
      per_page: perPage,
      client_timeout_ms: timeoutMs,
    },
    source_hashes: sourceHashes(),
    scenarios: {
      lookup_rest_detail_projection_parity: {
        status: 'passed',
        fulfillment_id: detailId,
        projection_sha256: projectionHash(graphqlLookupProjection),
        graphql_http_status: graphqlLookup.status,
        graphql_duration_ms: graphqlLookup.duration_ms,
        rest_http_status: restDetail.status,
        rest_duration_ms: restDetail.duration_ms,
      },
      filtered_list_projection_parity: {
        status: 'passed',
        order_id: orderId,
        filter_status: status,
        ordered_ids_sha256: sha256(JSON.stringify(listIds)),
        projections_sha256: projectionHash(graphqlListProjections),
        total: graphqlTotal,
        page: graphqlPage,
        per_page: graphqlPerPage,
        has_next: graphqlHasNext,
        graphql_http_status: graphqlList.status,
        graphql_duration_ms: graphqlList.duration_ms,
        rest_http_status: restList.status,
        rest_duration_ms: restList.duration_ms,
      },
      latest_by_order_projection_parity: {
        status: 'passed',
        order_id: orderId,
        fulfillment_id: latestId,
        projection_sha256: projectionHash(graphqlLatestProjection),
        graphql_http_status: graphqlLatest.status,
        graphql_duration_ms: graphqlLatest.duration_ms,
        rest_http_status: restLatest.status,
        rest_duration_ms: restLatest.duration_ms,
      },
      optional_not_found_transport_policy: {
        status: 'passed',
        missing_fulfillment_id: missingId,
        graphql_result: 'null_without_errors',
        graphql_http_status: graphqlMissing.status,
        graphql_duration_ms: graphqlMissing.duration_ms,
        rest_http_status: restMissing.status,
        rest_public_code: restMissingCode,
        rest_duration_ms: restMissing.duration_ms,
      },
    },
    retained_boundary: {
      bearer_token_retained: false,
      raw_response_bodies_retained: false,
      fulfillment_metadata_retained: false,
      normalized_projection_hashes_retained: true,
      source_hashes_retained: true,
    },
    limitations: {
      owner_deadline_failure_injection_proven: false,
      process_restart_proven: false,
      external_adapter_identity_proven: false,
      remote_adapter_behavior_proven: false,
    },
    transport_projection_parity_proven: true,
    runtime_parity_proven: false,
    review: {
      maintainer_reviewed: false,
      production_promotion_authorized: false,
    },
  };

  writeEvidence(packet);
  console.log(
    `✔ retained fulfillment lifecycle transport projection parity evidence at ${contract.evidence_path}`,
  );
}

main().catch((error) => {
  console.error(`[capture-fulfillment-lifecycle-transport-parity] ${error.message}`);
  process.exitCode = 1;
});
