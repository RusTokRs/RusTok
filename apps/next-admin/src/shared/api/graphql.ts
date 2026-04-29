/**
 * Simple GraphQL client for RusTok backend.
 * Uses fetch API, works in both browser and Node.js environments.
 */

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL ?? 'http://localhost:5150';
const SERVER_GRAPHQL_URL = `${API_BASE_URL}/api/graphql`;
const CLIENT_GRAPHQL_URL =
  process.env.NEXT_PUBLIC_GRAPHQL_ENDPOINT ?? SERVER_GRAPHQL_URL;

interface GraphqlRequest<V> {
  query: string;
  variables?: V;
}

interface GraphqlResponse<T> {
  data?: T;
  errors?: Array<{ message: string; extensions?: { code?: string } }>;
}

interface GraphqlRequestOptions {
  graphqlUrl?: string;
  tenantId?: string | null;
}

export class GraphqlError extends Error {
  public readonly code?: string;
  constructor(message: string, code?: string) {
    super(message);
    this.name = 'GraphqlError';
    this.code = code;
  }
}

/** Read the host-resolved admin UI locale from the rendered document. */
function getClientLocale(): string | undefined {
  if (typeof document === 'undefined') return undefined;
  return document.documentElement.lang || undefined;
}

function resolveGraphqlUrl(explicit?: string): string {
  if (explicit) return explicit;
  return typeof window === 'undefined'
    ? SERVER_GRAPHQL_URL
    : CLIENT_GRAPHQL_URL;
}

function decodeBase64UrlJson(value: string): Record<string, unknown> | null {
  try {
    const normalized = value.replace(/-/g, '+').replace(/_/g, '/');
    const padded = normalized.padEnd(
      normalized.length + ((4 - (normalized.length % 4)) % 4),
      '='
    );
    const decoded =
      typeof atob === 'function'
        ? atob(padded)
        : Buffer.from(padded, 'base64').toString('utf8');
    return JSON.parse(decoded) as Record<string, unknown>;
  } catch {
    return null;
  }
}

function resolveTenantIdFromToken(token?: string | null): string | null {
  const payload = token?.split('.')[1];
  if (!payload) return null;

  const claims = decodeBase64UrlJson(payload);
  const tenantId = claims?.tenant_id ?? claims?.tenantId ?? claims?.tid;
  return typeof tenantId === 'string' && tenantId.length > 0 ? tenantId : null;
}

function resolveTenantIdFromVariables(variables: unknown): string | null {
  if (!variables || typeof variables !== 'object') return null;

  const tenantId = (variables as { tenantId?: unknown }).tenantId;
  return typeof tenantId === 'string' && tenantId.length > 0 ? tenantId : null;
}

export async function graphqlRequest<V, T>(
  query: string,
  variables?: V,
  token?: string | null,
  tenantSlug?: string | null,
  options?: GraphqlRequestOptions
): Promise<T> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json'
  };

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  if (tenantSlug) {
    headers['X-Tenant-Slug'] = tenantSlug;
  }

  const tenantId =
    options?.tenantId ??
    resolveTenantIdFromVariables(variables) ??
    resolveTenantIdFromToken(token);
  if (tenantId) {
    headers['X-Tenant-ID'] = tenantId;
  }

  // Forward the admin UI locale so the server returns localised error messages.
  const locale = getClientLocale();
  if (locale) {
    headers['Accept-Language'] = locale;
  }

  const body: GraphqlRequest<V> = { query };
  if (variables !== undefined) {
    body.variables = variables;
  }

  const response = await fetch(resolveGraphqlUrl(options?.graphqlUrl), {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
    cache: 'no-store'
  });

  if (!response.ok) {
    if (response.status === 401) {
      throw new GraphqlError('Unauthorized', 'UNAUTHORIZED');
    }
    throw new GraphqlError(`HTTP error ${response.status}`, 'HTTP_ERROR');
  }

  const json: GraphqlResponse<T> = await response.json();

  if (json.errors && json.errors.length > 0) {
    const err = json.errors[0];
    const code = err.extensions?.code;
    if (
      code === 'UNAUTHORIZED' ||
      err.message.toLowerCase().includes('unauthorized')
    ) {
      throw new GraphqlError(err.message, 'UNAUTHORIZED');
    }
    throw new GraphqlError(err.message, code);
  }

  if (!json.data) {
    throw new GraphqlError('No data returned from GraphQL');
  }

  return json.data;
}
