/**
 * GraphQL client for RusTok frontend (storefront).
 * Resolves tenant from env variables NEXT_PUBLIC_TENANT_ID / NEXT_PUBLIC_TENANT_SLUG.
 */

const GRAPHQL_URL = process.env.NEXT_PUBLIC_API_URL
  ? `${process.env.NEXT_PUBLIC_API_URL}/api/graphql`
  : 'http://localhost:5150/api/graphql';

const DEFAULT_TENANT_SLUG = process.env.NEXT_PUBLIC_TENANT_SLUG ?? '';
const DEFAULT_TENANT_ID = process.env.NEXT_PUBLIC_TENANT_ID ?? '';

interface GraphqlRequestBody<V> {
  query: string;
  variables?: V;
}

interface GraphqlResponse<T> {
  data?: T;
  errors?: Array<{ message: string; extensions?: { code?: string } }>;
}

export class GraphqlError extends Error {
  public readonly code?: string;
  constructor(message: string, code?: string) {
    super(message);
    this.name = 'GraphqlError';
    this.code = code;
  }
}

export async function graphqlRequest<V, T>(
  query: string,
  variables?: V,
  token?: string | null,
  tenantSlug?: string | null
): Promise<T> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json'
  };

  const slug = tenantSlug ?? DEFAULT_TENANT_SLUG;
  if (slug) {
    headers['X-Tenant-Slug'] = slug;
  }

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const body: GraphqlRequestBody<V> = { query };
  if (variables !== undefined) {
    body.variables = variables;
  }

  const response = await fetch(GRAPHQL_URL, {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
    next: { revalidate: 60 }
  });

  if (!response.ok) {
    throw new GraphqlError(`HTTP error ${response.status}`, 'HTTP_ERROR');
  }

  const json: GraphqlResponse<T> = await response.json();

  if (json.errors && json.errors.length > 0) {
    const err = json.errors[0];
    throw new GraphqlError(err.message, err.extensions?.code);
  }

  if (!json.data) {
    throw new GraphqlError('No data returned from GraphQL');
  }

  return json.data;
}

/** Returns the default tenant ID from env. */
export function getDefaultTenantId(): string {
  return DEFAULT_TENANT_ID;
}

/** Returns the default tenant slug from env. */
export function getDefaultTenantSlug(): string {
  return DEFAULT_TENANT_SLUG;
}
