import { ApolloClient, CombinedGraphQLErrors, gql, HttpLink, InMemoryCache } from "@apollo/client/core";
import type { OperationVariables } from "@apollo/client/core";

export const GRAPHQL_ENDPOINT = "/api/graphql";
export const TENANT_HEADER = "X-Tenant-Slug";
export const AUTH_HEADER = "Authorization";

export type GraphqlRequest<V = Record<string, unknown>> = {
  query: string;
  variables?: V;
};

export type GraphqlError = {
  message: string;
};

export type GraphqlResponse<T> = {
  data?: T;
  errors?: GraphqlError[];
};

type ApolloExecutionResult<T> = {
  data?: T | null;
  error?: { message: string };
};

export type FrontendGraphqlOptions<V> = {
  query: string;
  variables?: V;
  token?: string;
  tenant?: string;
  baseUrl?: string;
};

const apolloClients = new Map<string, ApolloClient>();

export function defaultGraphqlBaseUrl(): string {
  const configured =
    process.env.NEXT_PUBLIC_API_URL ??
    (typeof window === "undefined" ? process.env.RUSTOK_API_URL : undefined);
  if (configured) return configured.replace(/\/$/, "");
  return typeof window === "undefined" ? "http://localhost:5150" : "";
}

function apolloClientFor(baseUrl = defaultGraphqlBaseUrl()): ApolloClient {
  const endpoint = `${baseUrl}${GRAPHQL_ENDPOINT}`;
  const existing = apolloClients.get(endpoint);
  if (existing) return existing;

  const client = new ApolloClient({
    cache: new InMemoryCache(),
    link: new HttpLink({
      uri: endpoint,
      fetch,
    }),
    defaultOptions: {
      query: {
        fetchPolicy: "no-cache",
      },
      mutate: {
        fetchPolicy: "no-cache",
      },
    },
  });
  apolloClients.set(endpoint, client);
  return client;
}

export async function storefrontGraphql<T, V = Record<string, unknown>>(
  options: FrontendGraphqlOptions<V>,
): Promise<GraphqlResponse<T>> {
  const { query, variables, token, tenant, baseUrl } = options;
  const headers: Record<string, string> = {};

  if (tenant) {
    headers[TENANT_HEADER] = tenant;
  }

  if (token) {
    headers[AUTH_HEADER] = `Bearer ${token}`;
  }

  const document = gql(query);
  // Browser and Next server requests must not retain a viewer's response in an
  // HTTP/data cache. Apollo's `no-cache` option alone only covers Apollo storage.
  const context = { headers, fetchOptions: token ? { cache: "no-store" as const } : undefined };
  const client = apolloClientFor(baseUrl);
  const isMutation = query.trimStart().toLowerCase().startsWith("mutation");
  const apolloVariables =
    variables === undefined ? undefined : (variables as OperationVariables);
  const result = (isMutation
    ? await client.mutate<T>({
        mutation: document,
        variables: apolloVariables,
        context,
        errorPolicy: "all",
      })
    : await client.query<T>({
        query: document,
        variables: apolloVariables,
        context,
        errorPolicy: "all",
      })) as ApolloExecutionResult<T>;

  return {
    data: result.data ?? undefined,
    // Apollo Client 4 returns `CombinedGraphQLErrors` in `result.error` under
    // `errorPolicy: "all"`; the old Apollo 3 `graphQLErrors` property is absent.
    // Preserve failures even when Apollo also returned partial data, otherwise
    // a permission denial looks like a legitimately empty Forum page.
    errors: result.error
      ? CombinedGraphQLErrors.is(result.error)
        ? result.error.errors.map((error) => ({ message: error.message }))
        : [{ message: result.error.message }]
      : undefined,
  };
}
