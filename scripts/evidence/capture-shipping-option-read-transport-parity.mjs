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
  'crates/modules/rustok-fulfillment/contracts/evidence/shipping-option-read-transport-parity-execution-contract.json';
const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), 'utf8'));
const outputPath = resolve(repoRoot, contract.evidence_path);
const maximumResponseBytes = contract.request_policy.maximum_response_bytes;

const projectionSelection = `
  id
  tenantId
  name
  currencyCode
  amount
  providerId
  active
  allowedShippingProfileSlugs
  createdAt
  updatedAt
  requestedLocale
  effectiveLocale
  availableLocales
  translationRevision
  translations {
    locale
    name
  }
`;

function fail(message) {
  throw new Error(message);
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
    contract.source_files.map((relativePath) => [relativePath, fileSha256(relativePath)]),
  );
}

function oneLine(value, field, maximumLength = 4096) {
  if (typeof value !== 'string') fail(`${field} must be a string`);
  const line = value.trim();
  if (
    line.length === 0 ||
    line.length > maximumLength ||
    /[\\u0000-\\u001f\\u007f]/u.test(line)
  ) {
    fail(`${field} is missing or outside the capture boundary`);
  }
  return line;
}

function optionalEnvironment(name, maximumLength = 4096) {
  const value = process.env[name];
  if (value === undefined || value.trim() === '') {
    return contract.optional_environment[name];
  }
  return oneLine(value, name, maximumLength);
}

function positiveInteger(value, field, maximum) {
  if (!/^\\d+$/u.test(value)) fail(`${field} must be a positive integer`);
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

function tenantHeaderName(value) {
  const parsed = headerName(value, 'RUSTOK_SHIPPING_PARITY_TENANT_HEADER');
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
  if (reserved.has(parsed.toLowerCase())) {
    fail('RUSTOK_SHIPPING_PARITY_TENANT_HEADER must not override a reserved HTTP header');
  }
  return parsed;
}

function isLocalCaptureHost(hostname) {
  return ['localhost', '127.0.0.1', '[::1]', '::1'].includes(hostname.toLowerCase());
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
  const token = oneLine(value, 'RUSTOK_SHIPPING_PARITY_AUTH_TOKEN', 8192);
  return /^Bearer\\s+/iu.test(token) ? token : `Bearer ${token}`;
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
  return oneLine(value, field);
}

function optionalString(value, field) {
  if (value === null || value === undefined) return null;
  if (typeof value !== 'string') fail(`${field} must be a string or null`);
  const line = value.trim();
  if (line.length > 4096 || /[\\u0000-\\u001f\\u007f]/u.test(line)) {
    fail(`${field} is outside the capture boundary`);
  }
  return line;
}

function decimalString(value, field) {
  if (typeof value === 'string') return requiredString(value, field);
  if (typeof value === 'number' && Number.isFinite(value)) return String(value);
  fail(`${field} must be a finite decimal string or number`);
}

function requiredBoolean(value, field) {
  if (typeof value !== 'boolean') fail(`${field} must be a boolean`);
  return value;
}

function uuidOrNull(value, field) {
  if (value === null || value === undefined) return null;
  return uuid(value, field);
}

function localeList(value, field) {
  return assertArray(value, field)
    .map((item, index) => requiredString(item, `${field}[${index}]`))
    .sort((left, right) => left.localeCompare(right));
}

function normalizeTranslations(value, flavor, field) {
  return assertArray(value, field)
    .map((item, index) => {
      const source = assertObject(item, `${field}[${index}]`);
      return {
        locale: requiredString(source.locale, `${field}[${index}].locale`),
        name: requiredString(source.name, `${field}[${index}].name`),
      };
    })
    .sort(
      (left, right) =>
        left.locale.localeCompare(right.locale) || left.name.localeCompare(right.name),
    );
}

function normalizeOption(value, flavor, field) {
  const source = assertObject(value, field);
  const camel = flavor === 'graphql';
  return {
    id: uuid(source.id, `${field}.id`),
    tenant_id: uuid(source[camel ? 'tenantId' : 'tenant_id'], `${field}.tenant_id`),
    name: requiredString(source.name, `${field}.name`),
    currency_code: requiredString(
      source[camel ? 'currencyCode' : 'currency_code'],
      `${field}.currency_code`,
    ),
    amount: decimalString(source.amount, `${field}.amount`),
    provider_id: requiredString(
      source[camel ? 'providerId' : 'provider_id'],
      `${field}.provider_id`,
    ),
    active: requiredBoolean(source.active, `${field}.active`),
    allowed_shipping_profile_slugs:
      source[camel ? 'allowedShippingProfileSlugs' : 'allowed_shipping_profile_slugs'] === null ||
      source[camel ? 'allowedShippingProfileSlugs' : 'allowed_shipping_profile_slugs'] === undefined
        ? null
        : assertArray(
            source[camel ? 'allowedShippingProfileSlugs' : 'allowed_shipping_profile_slugs'],
            `${field}.allowed_shipping_profile_slugs`,
          )
            .map((item, index) =>
              requiredString(item, `${field}.allowed_shipping_profile_slugs[${index}]`),
            )
            .sort(),
    created_at: requiredString(
      source[camel ? 'createdAt' : 'created_at'],
      `${field}.created_at`,
    ),
    updated_at: requiredString(
      source[camel ? 'updatedAt' : 'updated_at'],
      `${field}.updated_at`,
    ),
    requested_locale: optionalString(
      source[camel ? 'requestedLocale' : 'requested_locale'],
      `${field}.requested_locale`,
    ),
    effective_locale: optionalString(
      source[camel ? 'effectiveLocale' : 'effective_locale'],
      `${field}.effective_locale`,
    ),
    available_locales: localeList(
      source[camel ? 'availableLocales' : 'available_locales'],
      `${field}.available_locales`,
    ),
    translation_revision: requiredString(
      source[camel ? 'translationRevision' : 'translation_revision'],
      `${field}.translation_revision`,
    ),
    translations: normalizeTranslations(
      source.translations,
      flavor,
      `${field}.translations`,
    ),
  };
}

function normalizeTimestampFields(option, field) {
  for (const key of ['created_at', 'updated_at']) {
    const raw = option[key];
    const milliseconds = Date.parse(raw);
    if (!Number.isFinite(milliseconds)) fail(`${field}.${key} must be an RFC3339 timestamp`);
    option[key] = new Date(milliseconds).toISOString();
  }
  return option;
}

function normalizeProjectionList(value, flavor, field) {
  return assertArray(value, field).map((item, index) =>
    normalizeTimestampFields(normalizeOption(item, flavor, `${field}[${index}]`), `${field}[${index}]`),
  );
}

function stableJson(value) {
  return JSON.stringify(value);
}

function projectionHash(value) {
  return sha256(stableJson(value));
}

function requireEqual(left, right, label) {
  if (stableJson(left) !== stableJson(right)) fail(`${label} mismatch`);
}

function optionalCsv(value) {
  if (value === undefined || value === null || value === '') return null;
  return oneLine(value, 'optional csv value', 256);
}

function buildHeaders(tenantId, tenantHeader, locale, token) {
  return {
    accept: 'application/json',
    'accept-language': locale,
    authorization: authorizationHeader(token),
    [tenantHeader]: tenantId,
  };
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
    if (error?.name === 'AbortError') fail(`${operation} exceeded the client capture timeout`);
    throw error;
  } finally {
    clearTimeout(timeout);
  }
}

function graphQLErrorCodes(body) {
  const errors = body?.errors;
  if (errors === undefined) return [];
  return assertArray(errors, 'GraphQL errors').map((error, index) => ({
    code: optionalString(error?.extensions?.code, `GraphQL errors[${index}].code`),
  }));
}

async function graphqlRequest(url, headers, timeoutMs, operation, query, variables) {
  const response = await requestJson(
    url,
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
  const errors = graphQLErrorCodes(body);
  if (errors.length > 0) {
    fail(`${operation} returned GraphQL errors: ${errors.map((item) => item.code ?? 'unclassified').join(',')}`);
  }
  return { ...response, body };
}

function restUrl(base, pathName, params = {}) {
  const url = new URL(base.href);
  url.pathname = `${base.pathname.replace(/\\/$/u, '')}${pathName}`;
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== null && value !== undefined && value !== '') query.set(key, String(value));
  }
  url.search = query.toString();
  return url;
}

function publicErrorCode(body) {
  if (typeof body?.code === 'string') return body.code;
  if (typeof body?.error?.code === 'string') return body.error.code;
  return null;
}

function paginationMeta(value, field) {
  const source = assertObject(value, field);
  return {
    page: Number(source.page),
    per_page: Number(source.per_page),
    total: Number(source.total),
    total_pages: Number(source.total_pages),
    has_next: Boolean(source.has_next),
    has_prev: Boolean(source.has_prev),
  };
}

function writeEvidence(packet) {
  mkdirSync(dirname(outputPath), { recursive: true });
  const temporaryPath = `${outputPath}.tmp-${process.pid}`;
  try {
    writeFileSync(temporaryPath, `${JSON.stringify(packet, null, 2)}\\n`, { flag: 'wx' });
    renameSync(temporaryPath, outputPath);
  } catch (error) {
    if (existsSync(temporaryPath)) unlinkSync(temporaryPath);
    throw error;
  }
}

function ensureOutputBoundary() {
  const root = resolve(repoRoot) + sep;
  if (!outputPath.startsWith(root)) fail('parity evidence path must stay inside the repository');
  if (existsSync(outputPath)) {
    fail('parity evidence already exists; remove it explicitly before a new capture');
  }
}

function optionalUuidEnvironment(name) {
  const value = optionalEnvironment(name, 64);
  return value ? uuid(value, name) : null;
}

async function main() {
  ensureOutputBoundary();

  const graphqlUrl = endpoint(
    oneLine(process.env.RUSTOK_SHIPPING_PARITY_GRAPHQL_URL ?? '', 'RUSTOK_SHIPPING_PARITY_GRAPHQL_URL'),
    'RUSTOK_SHIPPING_PARITY_GRAPHQL_URL',
  );
  const restBaseUrl = endpoint(
    oneLine(process.env.RUSTOK_SHIPPING_PARITY_REST_BASE_URL ?? '', 'RUSTOK_SHIPPING_PARITY_REST_BASE_URL'),
    'RUSTOK_SHIPPING_PARITY_REST_BASE_URL',
  );
  const tenantId = uuid(
    oneLine(process.env.RUSTOK_SHIPPING_PARITY_TENANT_ID ?? '', 'RUSTOK_SHIPPING_PARITY_TENANT_ID', 36),
    'RUSTOK_SHIPPING_PARITY_TENANT_ID',
  );
  const detailId = uuid(
    oneLine(process.env.RUSTOK_SHIPPING_PARITY_DETAIL_ID ?? '', 'RUSTOK_SHIPPING_PARITY_DETAIL_ID', 36),
    'RUSTOK_SHIPPING_PARITY_DETAIL_ID',
  );
  const missingId = uuid(
    oneLine(process.env.RUSTOK_SHIPPING_PARITY_MISSING_ID ?? '', 'RUSTOK_SHIPPING_PARITY_MISSING_ID', 36),
    'RUSTOK_SHIPPING_PARITY_MISSING_ID',
  );
  if (detailId === missingId) fail('detail and missing shipping-option ids must differ');

  const token = oneLine(
    process.env.RUSTOK_SHIPPING_PARITY_AUTH_TOKEN ?? '',
    'RUSTOK_SHIPPING_PARITY_AUTH_TOKEN',
    8192,
  );
  const locale = optionalEnvironment('RUSTOK_SHIPPING_PARITY_LOCALE', 64);
  const currencyCode = optionalCsv(process.env.RUSTOK_SHIPPING_PARITY_CURRENCY_CODE);
  const regionId = optionalUuidEnvironment('RUSTOK_SHIPPING_PARITY_REGION_ID');
  const countryCode = optionalCsv(process.env.RUSTOK_SHIPPING_PARITY_COUNTRY_CODE);
  const adminActiveRaw = optionalCsv(process.env.RUSTOK_SHIPPING_PARITY_ADMIN_ACTIVE);
  if (adminActiveRaw !== null && !['true', 'false'].includes(adminActiveRaw.toLowerCase())) {
    fail('RUSTOK_SHIPPING_PARITY_ADMIN_ACTIVE must be true or false when supplied');
  }
  const adminActive = adminActiveRaw === null ? null : adminActiveRaw.toLowerCase() === 'true';
  const adminProviderId = optionalCsv(process.env.RUSTOK_SHIPPING_PARITY_ADMIN_PROVIDER_ID);
  const adminSearch = optionalCsv(process.env.RUSTOK_SHIPPING_PARITY_ADMIN_SEARCH);
  const page = positiveInteger(
    optionalEnvironment('RUSTOK_SHIPPING_PARITY_PAGE', 8),
    'page',
    1000000,
  );
  const perPage = positiveInteger(
    optionalEnvironment('RUSTOK_SHIPPING_PARITY_PER_PAGE', 8),
    'per_page',
    100,
  );
  const timeoutMs = positiveInteger(
    optionalEnvironment('RUSTOK_SHIPPING_PARITY_TIMEOUT_MS', 8),
    'timeout_ms',
    120000,
  );
  const tenantHeader = tenantHeaderName(
    optionalEnvironment('RUSTOK_SHIPPING_PARITY_TENANT_HEADER', 128),
  );
  const sourceRevision = gitRevision(
    oneLine(process.env.RUSTOK_SHIPPING_PARITY_SOURCE_REVISION ?? contract.source_base_revision, 'RUSTOK_SHIPPING_PARITY_SOURCE_REVISION'),
    'RUSTOK_SHIPPING_PARITY_SOURCE_REVISION',
  );
  const adapterProfile = optionalEnvironment('RUSTOK_SHIPPING_PARITY_ADAPTER_PROFILE', 128) || 'unreported';
  const headers = buildHeaders(tenantId, tenantHeader, locale, token);

  const storefrontFilter = {
    regionId,
    countryCode,
    locale,
    currencyCode,
  };
  const adminFilter = {
    active: adminActive,
    currencyCode,
    providerId: adminProviderId,
    search: adminSearch,
    page,
    perPage,
  };

  const storefrontGraphql = await graphqlRequest(
    graphqlUrl,
    headers,
    timeoutMs,
    'GraphQL storefront shipping-option list',
    `
      query ShippingOptionStorefrontParity($tenantId: UUID, $filter: StorefrontContextFilter) {
        storefrontShippingOptions(tenantId: $tenantId, filter: $filter) {${projectionSelection}}
      }
    `,
    { tenantId, filter: storefrontFilter },
  );
  const storefrontRest = await requestJson(
    restUrl(restBaseUrl, '/store/shipping-options', {
      region_id: regionId,
      country_code: countryCode,
      locale,
      currency_code: currencyCode,
    }),
    { method: 'GET', headers },
    timeoutMs,
    'REST storefront shipping-option list',
  );
  if (storefrontRest.status !== 200) {
    fail(`REST storefront shipping-option list returned HTTP ${storefrontRest.status}`);
  }
  const storefrontGraphqlBody = assertObject(storefrontGraphql.body, 'GraphQL storefront response');
  const storefrontGraphqlOptions = normalizeProjectionList(
    storefrontGraphqlBody.data?.storefrontShippingOptions,
    'graphql',
    'GraphQL storefront shipping options',
  );
  const storefrontRestOptions = normalizeProjectionList(
    storefrontRest.body,
    'rest',
    'REST storefront shipping options',
  );
  if (storefrontGraphqlOptions.some((option) => !option.active)) {
    fail('GraphQL storefront shipping-option list contains inactive options');
  }
  if (storefrontRestOptions.some((option) => !option.active)) {
    fail('REST storefront shipping-option list contains inactive options');
  }
  requireEqual(
    storefrontGraphqlOptions,
    storefrontRestOptions,
    'storefront shipping-option projection',
  );

  const adminLookup = await graphqlRequest(
    graphqlUrl,
    headers,
    timeoutMs,
    'GraphQL admin shipping-option lookup',
    `
      query ShippingOptionAdminLookup($tenantId: UUID!, $id: UUID!) {
        shippingOption(tenantId: $tenantId, id: $id) {${projectionSelection}}
      }
    `,
    { tenantId, id: detailId },
  );
  const adminRestLookup = await requestJson(
    restUrl(restBaseUrl, `/admin/shipping-options/${detailId}`),
    { method: 'GET', headers },
    timeoutMs,
    'REST admin shipping-option lookup',
  );
  if (adminRestLookup.status !== 200) {
    fail(`REST admin shipping-option lookup returned HTTP ${adminRestLookup.status}`);
  }
  const graphqlLookup = adminLookup.body?.data?.shippingOption;
  if (graphqlLookup === null || graphqlLookup === undefined) {
    fail('GraphQL admin shipping-option lookup unexpectedly returned null');
  }
  requireEqual(
    normalizeOption(normalizeTimestampFields(graphqlLookup, 'GraphQL admin lookup'), 'graphql', 'GraphQL admin lookup'),
    normalizeOption(normalizeTimestampFields(adminRestLookup.body, 'REST admin lookup'), 'rest', 'REST admin lookup'),
    'admin shipping-option lookup projection',
  );

  const adminGraphqlList = await graphqlRequest(
    graphqlUrl,
    headers,
    timeoutMs,
    'GraphQL admin shipping-option list',
    `
      query ShippingOptionAdminList($tenantId: UUID!, $filter: ShippingOptionsFilter) {
        shippingOptions(tenantId: $tenantId, filter: $filter) {
          items {${projectionSelection}}
          total
          page
          perPage
          hasNext
        }
      }
    `,
    { tenantId, filter: adminFilter },
  );
  const adminRestList = await requestJson(
    restUrl(restBaseUrl, '/admin/shipping-options', {
      active: adminActive,
      currency_code: currencyCode,
      provider_id: adminProviderId,
      search: adminSearch,
      page,
      per_page: perPage,
    }),
    { method: 'GET', headers },
    timeoutMs,
    'REST admin shipping-option list',
  );
  if (adminRestList.status !== 200) {
    fail(`REST admin shipping-option list returned HTTP ${adminRestList.status}`);
  }
  const gqlListBody = assertObject(adminGraphqlList.body, 'GraphQL admin list response');
  const gqlList = normalizeProjectionList(
    gqlListBody.data?.shippingOptions?.items,
    'graphql',
    'GraphQL admin shipping options',
  );
  const restListBody = assertObject(adminRestList.body, 'REST admin list response');
  const restList = normalizeProjectionList(restListBody.data, 'rest', 'REST admin shipping options');
  requireEqual(gqlList, restList, 'admin shipping-option list projection');
  requireEqual(
    {
      total: Number(gqlListBody.data.shippingOptions.total),
      page: Number(gqlListBody.data.shippingOptions.page),
      per_page: Number(gqlListBody.data.shippingOptions.perPage),
      has_next: Boolean(gqlListBody.data.shippingOptions.hasNext),
    },
    paginationMeta(restListBody.meta, 'REST admin shipping-option metadata'),
    'admin shipping-option pagination',
  );

  const graphqlMissing = await graphqlRequest(
    graphqlUrl,
    headers,
    timeoutMs,
    'GraphQL admin missing shipping-option lookup',
    `
      query ShippingOptionMissing($tenantId: UUID!, $id: UUID!) {
        shippingOption(tenantId: $tenantId, id: $id) {${projectionSelection}}
      }
    `,
    { tenantId, id: missingId },
  );
  if (graphqlMissing.body?.data?.shippingOption !== null) {
    fail('GraphQL missing shipping-option lookup must return null');
  }
  const restMissing = await requestJson(
    restUrl(restBaseUrl, `/admin/shipping-options/${missingId}`),
    { method: 'GET', headers },
    timeoutMs,
    'REST missing shipping-option lookup',
  );
  const restMissingCode = publicErrorCode(restMissing.body);
  if (restMissing.status !== 404 || restMissingCode !== 'commerce_admin_not_found') {
    fail(
      `REST missing shipping-option lookup must return 404 commerce_admin_not_found (received ${restMissing.status} ${restMissingCode ?? 'null'})`,
    );
  }

  const packet = {
    schema_version: 1,
    status: 'transport_projection_parity_captured_unreviewed',
    captured_at: new Date().toISOString(),
    claimed_source_revision: sourceRevision,
    claimed_adapter_profile: adapterProfile,
    claims_verified_by_runner: false,
    request_context: {
      graphql_endpoint: sanitizedEndpoint(graphqlUrl),
      rest_base_endpoint: sanitizedEndpoint(restBaseUrl),
      tenant_id: tenantId,
      locale,
      storefront_filter: storefrontFilter,
      admin_filter: adminFilter,
      page,
      per_page: perPage,
      client_timeout_ms: timeoutMs,
    },
    source_hashes: sourceHashes(),
    scenarios: {
      storefront_active_list_projection_parity: {
        status: 'passed',
        ordered_projection_sha256: projectionHash(storefrontGraphqlOptions),
        option_count: storefrontGraphqlOptions.length,
        graphql_http_status: storefrontGraphql.status,
        graphql_duration_ms: storefrontGraphql.duration_ms,
        rest_http_status: storefrontRest.status,
        rest_duration_ms: storefrontRest.duration_ms,
      },
      admin_lookup_projection_parity: {
        status: 'passed',
        shipping_option_id: detailId,
        projection_sha256: projectionHash(
          normalizeOption(normalizeTimestampFields(graphqlLookup, 'GraphQL admin lookup'), 'graphql', 'GraphQL admin lookup'),
        ),
        graphql_http_status: adminLookup.status,
        graphql_duration_ms: adminLookup.duration_ms,
        rest_http_status: adminRestLookup.status,
        rest_duration_ms: adminRestLookup.duration_ms,
      },
      admin_list_projection_parity: {
        status: 'passed',
        ordered_projection_sha256: projectionHash(gqlList),
        option_count: gqlList.length,
        total: Number(gqlListBody.data.shippingOptions.total),
        page: Number(gqlListBody.data.shippingOptions.page),
        per_page: Number(gqlListBody.data.shippingOptions.perPage),
        has_next: Boolean(gqlListBody.data.shippingOptions.hasNext),
        graphql_http_status: adminGraphqlList.status,
        graphql_duration_ms: adminGraphqlList.duration_ms,
        rest_http_status: adminRestList.status,
        rest_duration_ms: adminRestList.duration_ms,
      },
      optional_not_found_transport_policy: {
        status: 'passed',
        missing_shipping_option_id: missingId,
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
      shipping_option_metadata_retained: false,
      normalized_projection_hashes_retained: true,
      source_hashes_retained: true,
    },
    limitations: {
      runtime_context_failure_injection_proven: false,
      process_restart_proven: false,
      remote_adapter_identity_proven: false,
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
  console.log(`✔ retained shipping-option transport projection parity evidence at ${contract.evidence_path}`);
}

main().catch((error) => {
  console.error(`[capture-shipping-option-read-transport-parity] ${error.message}`);
  process.exitCode = 1;
});
