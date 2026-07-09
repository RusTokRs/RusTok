import { ApolloClient, gql, HttpLink, InMemoryCache } from '@apollo/client/core';
import type { OperationVariables } from '@apollo/client/core';

const GRAPHQL_URL = process.env.NEXT_PUBLIC_API_URL
  ? `${process.env.NEXT_PUBLIC_API_URL}/api/graphql`
  : 'http://localhost:5150/api/graphql';

const DEFAULT_TENANT_SLUG = process.env.NEXT_PUBLIC_TENANT_SLUG ?? '';
const DEFAULT_TENANT_ID = process.env.NEXT_PUBLIC_TENANT_ID ?? '';

interface GraphqlResponse<T> {
  data?: T;
  errors?: Array<{ message: string; extensions?: { code?: string } }>;
}

type ApolloGraphqlError = {
  message: string;
  extensions?: { code?: string };
};

type ApolloExecutionResult<T> = {
  data?: T | null;
  error?: {
    message?: string;
    graphQLErrors?: ReadonlyArray<ApolloGraphqlError>;
  };
};

export class GraphqlError extends Error {
  public readonly code?: string;
  constructor(message: string, code?: string) {
    super(message);
    this.name = 'GraphqlError';
    this.code = code;
  }
}

const apolloClient = new ApolloClient({
  cache: new InMemoryCache(),
  link: new HttpLink({
    uri: GRAPHQL_URL,
    fetch
    }),
    defaultOptions: {
      query: {
        fetchPolicy: 'no-cache'
      },
      mutate: {
        fetchPolicy: 'no-cache'
      }
    }
});

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

  const document = gql(query);
  const context = {
    headers,
    fetchOptions: {
      next: { revalidate: 60 }
    } as RequestInit
  };
  const isMutation = query.trimStart().toLowerCase().startsWith('mutation');
  const apolloVariables =
    variables === undefined ? undefined : (variables as OperationVariables);
  const result = (isMutation
    ? await apolloClient.mutate<T>({
        mutation: document,
        variables: apolloVariables,
        context
      })
    : await apolloClient.query<T>({
        query: document,
        variables: apolloVariables,
        context
      })) as ApolloExecutionResult<T>;

  const json: GraphqlResponse<T> = {
    data: result.data ?? undefined,
    errors: result.error?.graphQLErrors?.map((error) => ({
      message: error.message,
      extensions: error.extensions
    }))
  };

  if (result.error && (!json.errors || json.errors.length === 0)) {
    throw new GraphqlError(result.error.message ?? 'GraphQL request failed');
  }

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
