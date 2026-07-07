import { ApolloClient, gql, HttpLink, InMemoryCache } from "@apollo/client/core";
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
  error?: {
    message?: string;
    graphQLErrors?: ReadonlyArray<GraphqlError>;
  };
};

export type FrontendGraphqlOptions<V> = {
  query: string;
  variables?: V;
  token?: string;
  tenant?: string;
  baseUrl?: string;
};

const apolloClients = new Map<string, ApolloClient>();

function apolloClientFor(baseUrl = ""): ApolloClient {
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
  const context = { headers };
  const client = apolloClientFor(baseUrl);
  const isMutation = query.trimStart().toLowerCase().startsWith("mutation");
  const apolloVariables =
    variables === undefined ? undefined : (variables as OperationVariables);
  const result = (isMutation
    ? await client.mutate<T>({
        mutation: document,
        variables: apolloVariables,
        context,
      })
    : await client.query<T>({
        query: document,
        variables: apolloVariables,
        context,
      })) as ApolloExecutionResult<T>;

  return {
    data: result.data ?? undefined,
    errors: result.error?.graphQLErrors?.map((error) => ({
      message: error.message,
    })),
  };
}
