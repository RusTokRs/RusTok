/**
 * Simple GraphQL client for RusTok backend.
 * Uses fetch API, works in both browser and Node.js environments.
 */

const GRAPHQL_URL = process.env.NEXT_PUBLIC_API_URL
  ? `${process.env.NEXT_PUBLIC_API_URL}/api/graphql`
  : 'http://localhost:5150/api/graphql';

interface GraphqlRequest<V> {
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

/** Read the admin UI locale from the browser cookie (client-side only). */
function getClientLocale(): string | undefined {
  if (typeof document === 'undefined') return undefined;
  const match = document.cookie.match(/rustok-admin-locale=([^;]+)/);
  return match?.[1];
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

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  if (tenantSlug) {
    headers['X-Tenant-Slug'] = tenantSlug;
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

  const response = await fetch(GRAPHQL_URL, {
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
    if (code === 'UNAUTHORIZED' || err.message.toLowerCase().includes('unauthorized')) {
      throw new GraphqlError(err.message, 'UNAUTHORIZED');
    }
    throw new GraphqlError(err.message, code);
  }

  if (!json.data) {
    throw new GraphqlError('No data returned from GraphQL');
  }

  return json.data;
}
